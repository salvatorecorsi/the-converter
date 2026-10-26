#![cfg(windows)]
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};

fn wait(child: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "queue process exited {status}");
            return;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("queue process timed out");
        }
        thread::sleep(Duration::from_millis(30));
    }
}
fn command(root: &Path) -> Command {
    use std::os::windows::process::CommandExt;
    let mut command = Command::new(env!("CARGO_BIN_EXE_the-converter"));
    command
        .env("THE_CONVERTER_QUEUE_DIR", root)
        .creation_flags(0x08000000);
    command
}
fn jobs(root: &Path) -> Vec<serde_json::Value> {
    fs::read_dir(root)
        .unwrap()
        .filter_map(|entry| {
            let file = entry.ok()?.path().join("job.json");
            file.is_file()
                .then(|| serde_json::from_slice(&fs::read(file).unwrap()).unwrap())
        })
        .collect()
}

#[test]
fn separate_invocations_enqueue_immediately_and_only_one_worker_publishes() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("queue");
    fs::create_dir(&root).unwrap();
    let source = directory.path().join("foto è bella.png");
    image::RgbaImage::from_pixel(200, 100, image::Rgba([20, 80, 160, 255]))
        .save(&source)
        .unwrap();
    let original = fs::read(&source).unwrap();
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join("worker.lock"))
        .unwrap();
    fs2::FileExt::lock_exclusive(&lock).unwrap();
    for _ in 0..3 {
        let mut submitter = command(&root)
            .args(["--quick", "--target", "jpg", "--files"])
            .arg(&source)
            .spawn()
            .unwrap();
        wait(&mut submitter);
    }
    let initial = jobs(&root);
    assert_eq!(initial.len(), 3);
    let outputs: Vec<PathBuf> = initial
        .iter()
        .map(|job| PathBuf::from(job["outputs"][0].as_str().unwrap()))
        .collect();
    let distinct: std::collections::HashSet<_> = outputs.iter().collect();
    assert_eq!(distinct.len(), 3);
    for (job, output) in initial.iter().zip(&outputs) {
        assert_eq!(job["state"], "Queued");
        assert!(!output.exists());
        assert!(the_converter::pending::PendingLink::path_for(output).is_file());
    }
    drop(lock);
    let mut first = command(&root).arg("--queue-worker").spawn().unwrap();
    let mut competing = command(&root).arg("--queue-worker").spawn().unwrap();
    wait(&mut first);
    wait(&mut competing);
    for job in jobs(&root) {
        assert_eq!(job["state"], "Completed");
    }
    for output in outputs {
        assert!(image::open(&output).is_ok());
        assert!(!the_converter::pending::PendingLink::path_for(&output).exists());
    }
    assert_eq!(fs::read(source).unwrap(), original);
}

#[test]
fn cancelling_a_running_background_encoder_removes_preview_and_partial_media() {
    let tools = the_converter::tools::ToolPaths::discover();
    let Some(ffmpeg) = tools.ffmpeg else {
        return;
    };
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("queue");
    let source = directory.path().join("video.mp4");
    let mut fixture = Command::new(ffmpeg)
        .args([
            "-hide_banner",
            "-nostdin",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=1920x1080:rate=24",
            "-t",
            "4",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-crf",
            "28",
        ])
        .arg(&source)
        .spawn()
        .unwrap();
    wait(&mut fixture);
    let mut submitter = command(&root)
        .args(["--quick", "--target", "webm", "--files"])
        .arg(&source)
        .spawn()
        .unwrap();
    wait(&mut submitter);
    let deadline = Instant::now() + Duration::from_secs(8);
    let job = loop {
        let job = jobs(&root).pop().unwrap();
        if job["state"] == "Running" {
            break job;
        }
        assert!(Instant::now() < deadline, "encoder did not start");
        thread::sleep(Duration::from_millis(20));
    };
    let id = job["id"].as_str().unwrap();
    let output = PathBuf::from(job["outputs"][0].as_str().unwrap());
    fs::write(root.join(id).join("cancel"), b"cancel").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let state = jobs(&root).pop().unwrap();
        if state["state"] == "Cancelled" {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "background cancellation timed out"
        );
        thread::sleep(Duration::from_millis(30));
    }
    assert!(source.is_file());
    assert!(!output.exists());
    assert!(!the_converter::pending::PendingLink::path_for(&output).exists());
    assert!(!Path::new(job["plan"]["work_dir"].as_str().unwrap()).exists());
}
