//! Persistent per-user quick-conversion queue. Explorer previews exist before any encoder starts.
use crate::{
    engine::{self, PreparedConversion},
    model::ConversionSettings,
    pending::PendingLink,
    tools::ToolPaths,
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobState {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}
impl JobState {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueJob {
    pub id: String,
    pub input: PathBuf,
    pub format: String,
    pub outputs: Vec<PathBuf>,
    pub state: JobState,
    pub progress: f32,
    pub message: String,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredJob {
    #[serde(flatten)]
    view: QueueJob,
    plan: PreparedConversion,
    tools: ToolPaths,
    previews: Vec<PendingLink>,
    batch_id: String,
    submitted_at: u64,
    updated_at: u64,
}

pub struct Submission {
    pub jobs: Vec<QueueJob>,
    pub errors: Vec<String>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn queue_root() -> Result<PathBuf> {
    let path = if let Some(value) = std::env::var_os("THE_CONVERTER_QUEUE_DIR") {
        PathBuf::from(value)
    } else {
        directories::ProjectDirs::from("com", "SalvatoreCorsi", "TheConverter")
            .context("Cartella della coda utente non disponibile")?
            .data_local_dir()
            .join("queue")
    };
    fs::create_dir_all(&path).context("Impossibile creare la coda di conversione")?;
    Ok(path)
}
pub fn last_report_path() -> Option<PathBuf> {
    if let Some(root) = std::env::var_os("THE_CONVERTER_QUEUE_DIR") {
        return Some(PathBuf::from(root).join("report-last.json"));
    }
    directories::ProjectDirs::from("com", "SalvatoreCorsi", "TheConverter")
        .map(|dirs| dirs.config_dir().join("report-last.json"))
}
fn valid_id(id: &str) -> bool {
    id.starts_with("job-")
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn job_dir(root: &Path, id: &str) -> Result<PathBuf> {
    if !valid_id(id) {
        bail!("Identificatore della conversione non valido")
    }
    Ok(root.join(id))
}
fn open_lock(root: &Path, name: &str) -> Result<File> {
    Ok(OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(name))?)
}
fn dispatch_lock(root: &Path) -> Result<File> {
    let file = open_lock(root, "dispatch.lock")?;
    fs2::FileExt::lock_exclusive(&file).context("Impossibile accedere alla coda")?;
    Ok(file)
}
// Assigned under dispatch.lock, so submissions in the same millisecond keep FIFO order.
fn next_order(root: &Path) -> Result<u64> {
    let path = root.join("order.counter");
    let previous = if path.is_file() {
        let bytes: [u8; 8] = fs::read(&path)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("Contatore della coda non valido"))?;
        u64::from_le_bytes(bytes)
    } else {
        records(root)?
            .iter()
            .map(|job| job.submitted_at)
            .max()
            .unwrap_or(0)
    };
    let order = previous.saturating_add(1).max(now());
    let mut temporary = tempfile::NamedTempFile::new_in(root)?;
    temporary.write_all(&order.to_le_bytes())?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(order)
}
fn lock_contended(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::WouldBlock
        || error
            .raw_os_error()
            .zip(fs2::lock_contended_error().raw_os_error())
            .is_some_and(|(actual, expected)| actual == expected)
}
fn store(root: &Path, record: &StoredJob) -> Result<()> {
    let dir = job_dir(root, &record.view.id)?;
    fs::create_dir_all(&dir)?;
    let mut temporary = tempfile::NamedTempFile::new_in(&dir)?;
    serde_json::to_writer_pretty(&mut temporary, record)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(dir.join("job.json"))
        .map_err(|error| error.error)
        .context("Impossibile salvare lo stato della conversione")?;
    Ok(())
}
fn load(root: &Path, id: &str) -> Result<StoredJob> {
    let path = job_dir(root, id)?.join("job.json");
    let record: StoredJob =
        serde_json::from_slice(&fs::read(&path).context("Conversione non trovata nella coda")?)
            .context("Stato della conversione non leggibile")?;
    if record.view.id != id {
        bail!("Identificatore della conversione incoerente")
    }
    Ok(record)
}
fn records(root: &Path) -> Result<Vec<StoredJob>> {
    let mut jobs = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if valid_id(&id) && entry.file_type()?.is_dir() && entry.path().join("job.json").is_file() {
            jobs.push(load(root, &id)?);
        }
    }
    jobs.sort_by(|left, right| {
        (left.submitted_at, &left.view.id).cmp(&(right.submitted_at, &right.view.id))
    });
    Ok(jobs)
}

/// Read-only snapshots do not acquire the worker/dispatch locks; GUI polling stays responsive.
pub fn list_jobs() -> Result<Vec<QueueJob>> {
    let mut items = records(&queue_root()?)?;
    items.reverse();
    Ok(items.into_iter().map(|job| job.view).collect())
}
pub fn get_job(id: &str) -> Result<QueueJob> {
    Ok(load(&queue_root()?, id)?.view)
}

pub fn enqueue(
    files: &[PathBuf],
    settings: &ConversionSettings,
    tools: &ToolPaths,
    executable: &Path,
) -> Result<Submission> {
    submit_at(&queue_root()?, files, settings, tools, executable, true)
}

fn submit_at(
    root: &Path,
    files: &[PathBuf],
    settings: &ConversionSettings,
    tools: &ToolPaths,
    executable: &Path,
    launch_worker: bool,
) -> Result<Submission> {
    if files.is_empty() {
        bail!("Nessun file da convertire")
    }
    fs::create_dir_all(root)?;
    let batch_id = format!("{}-{}", now(), std::process::id());
    let mut submission = Submission {
        jobs: Vec::new(),
        errors: Vec::new(),
    };
    for input in files {
        let result = (|| -> Result<QueueJob> {
            let directory = tempfile::Builder::new().prefix("job-").tempdir_in(root)?;
            let id = directory
                .path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            let plan = engine::prepare(input, settings, tools)?;
            let mut previews = Vec::new();
            for output in &plan.outputs {
                match PendingLink::create(
                    &output.path,
                    executable,
                    &id,
                    "In coda — in attesa di conversione",
                ) {
                    Ok(preview) => previews.push(preview),
                    Err(error) => {
                        for preview in &previews {
                            let _ = preview.remove();
                        }
                        let _ = engine::release_prepared(&plan);
                        return Err(error);
                    }
                }
            }
            let mut record = StoredJob {
                view: QueueJob {
                    id,
                    input: plan.input.clone(),
                    format: settings.format.label().into(),
                    outputs: plan
                        .outputs
                        .iter()
                        .map(|output| output.path.clone())
                        .collect(),
                    state: JobState::Queued,
                    progress: 0.0,
                    message: "In coda".into(),
                    error: None,
                },
                plan,
                tools: tools.clone(),
                previews,
                batch_id: batch_id.clone(),
                submitted_at: now(),
                updated_at: now(),
            };
            let persisted = (|| -> Result<()> {
                let _dispatch = dispatch_lock(root)?;
                record.submitted_at = next_order(root)?;
                store(root, &record)
            })();
            if let Err(error) = persisted {
                for preview in &record.previews {
                    let _ = preview.remove();
                }
                let _ = engine::release_prepared(&record.plan);
                return Err(error);
            }
            let _ = directory.keep();
            Ok(record.view)
        })();
        match result {
            Ok(job) => submission.jobs.push(job),
            Err(error) => submission
                .errors
                .push(format!("{}: {error:#}", input.display())),
        }
    }
    if launch_worker && !submission.jobs.is_empty() {
        let _dispatch = dispatch_lock(root)?;
        start_worker_locked(root, executable).context("I lavori sono salvati in coda, ma non è stato possibile avviare il convertitore. Apri The Converter per riprendere la coda")?;
    }
    Ok(submission)
}

/// Called on app/status startup as well as enqueue, so queued jobs survive an app restart.
pub fn ensure_worker() -> Result<()> {
    let root = queue_root()?;
    let _dispatch = dispatch_lock(&root)?;
    if records(&root)?
        .iter()
        .any(|job| !job.view.state.is_terminal())
    {
        start_worker_locked(&root, &std::env::current_exe()?)?;
    }
    Ok(())
}
fn start_worker_locked(root: &Path, executable: &Path) -> Result<()> {
    let probe = open_lock(root, "worker.lock")?;
    match fs2::FileExt::try_lock_exclusive(&probe) {
        Ok(()) => {
            drop(probe);
        }
        Err(error) if lock_contended(&error) => return Ok(()),
        Err(error) => return Err(error.into()),
    }
    let mut command = Command::new(executable);
    command.arg("--queue-worker");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Avvio del processo della coda")?;
    Ok(())
}

pub fn cancel_job(id: &str) -> Result<()> {
    cancel_at(&queue_root()?, id)
}
fn cancel_at(root: &Path, id: &str) -> Result<()> {
    let _dispatch = dispatch_lock(root)?;
    let mut job = load(root, id)?;
    if job.view.state.is_terminal() {
        return Ok(());
    }
    let mut cancel = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(job_dir(root, id)?.join("cancel"))?;
    cancel.write_all(b"cancel")?;
    if job.view.state == JobState::Queued {
        job.view.state = JobState::Cancelled;
        job.view.message = "Annullato".into();
        job.updated_at = now();
        clean_previews(&job);
        engine::release_prepared(&job.plan)?;
        store(root, &job)?;
    }
    Ok(())
}

pub fn run_worker() -> Result<()> {
    run_worker_at(&queue_root()?, last_report_path().as_deref())
}

fn run_worker_at(root: &Path, report: Option<&Path>) -> Result<()> {
    let worker = open_lock(root, "worker.lock")?;
    match fs2::FileExt::try_lock_exclusive(&worker) {
        Ok(()) => {}
        Err(error) if lock_contended(&error) => return Ok(()),
        Err(error) => return Err(error.into()),
    }
    // The OS releases worker.lock after a crash. A previously Running job is not silently
    // treated as complete, and no external file at its reserved path is ever removed.
    {
        let _dispatch = dispatch_lock(root)?;
        for mut job in records(root)? {
            if job.view.state == JobState::Running {
                job.view.state = JobState::Failed;
                job.view.message = "Conversione interrotta".into();
                job.view.error = Some("La conversione è stata interrotta prima del completamento. Puoi avviarla di nuovo dal file originale.".into());
                job.updated_at = now();
                for preview in &job.previews {
                    let _ = preview.fail("Conversione interrotta — apri per i dettagli");
                }
                let _ = engine::release_prepared(&job.plan);
                store(root, &job)?;
                if let Some(path) = report {
                    let _ = update_report(root, &job.batch_id, path);
                }
            }
        }
    }
    loop {
        let dispatch = dispatch_lock(root)?;
        let next = records(root)?
            .into_iter()
            .find(|job| job.view.state == JobState::Queued);
        let Some(mut job) = next else {
            // Release worker ownership while enqueue is excluded. This closes the empty-queue
            // handoff race: a submitter will either be seen here or start the next worker.
            drop(worker);
            drop(dispatch);
            return Ok(());
        };
        let cancellation = job_dir(root, &job.view.id)?.join("cancel");
        if cancellation.exists() {
            job.view.state = JobState::Cancelled;
            job.view.message = "Annullato".into();
            clean_previews(&job);
            let _ = engine::release_prepared(&job.plan);
            store(root, &job)?;
            drop(dispatch);
            continue;
        }
        job.view.state = JobState::Running;
        job.view.message = "Conversione in corso…".into();
        job.updated_at = now();
        store(root, &job)?;
        drop(dispatch);
        for preview in &job.previews {
            let _ = preview.update("Conversione in corso…");
        }
        let cancel = AtomicBool::new(false);
        let finished = AtomicBool::new(false);
        let mut last_snapshot = Instant::now();
        let mut last_preview = Instant::now();
        let plan = job.plan.clone();
        let tools = job.tools.clone();
        let conversion = thread::scope(|scope| {
            scope.spawn(|| {
                while !finished.load(Ordering::Relaxed) {
                    if cancellation.exists() {
                        cancel.store(true, Ordering::Relaxed);
                        break;
                    }
                    thread::sleep(Duration::from_millis(75));
                }
            });
            let result = engine::convert_prepared(&plan, &tools, &cancel, |progress| {
                job.view.progress = if progress.fraction.is_finite() {
                    progress.fraction.clamp(0.0, 0.99)
                } else {
                    0.0
                };
                job.view.message = progress.message;
                job.updated_at = now();
                if last_snapshot.elapsed() >= Duration::from_millis(300) {
                    let _ = store(root, &job);
                    last_snapshot = Instant::now();
                }
                if last_preview.elapsed() >= Duration::from_secs(1) {
                    let status = if job.view.progress > 0.0 {
                        format!("Conversione in corso — {:.0}%", job.view.progress * 100.0)
                    } else {
                        job.view.message.clone()
                    };
                    for preview in &job.previews {
                        let _ = preview.update(&status);
                    }
                    last_preview = Instant::now();
                }
            });
            finished.store(true, Ordering::Relaxed);
            result
        });
        match conversion {
            Ok(outputs) => {
                job.view.outputs = outputs;
                job.view.state = JobState::Completed;
                job.view.progress = 1.0;
                job.view.message = "Completato".into();
                clean_previews(&job);
                for output in &job.view.outputs {
                    crate::pending::notify_ready(output);
                }
            }
            Err(error) if cancel.load(Ordering::Relaxed) => {
                job.view.state = JobState::Cancelled;
                job.view.message = "Annullato".into();
                clean_previews(&job);
                let _ = error;
            }
            Err(error) => {
                job.view.state = JobState::Failed;
                job.view.message = "Conversione non riuscita".into();
                job.view.error = Some(format!("{error:#}"));
                for preview in &job.previews {
                    let _ = preview.fail(&format!("Errore — {error:#}"));
                }
            }
        }
        if let Err(error) = engine::release_prepared(&job.plan) {
            job.view
                .message
                .push_str(&format!(" · Pulizia temporanei: {error:#}"));
        }
        job.updated_at = now();
        store(root, &job)?;
        if let Some(path) = report {
            let _ = update_report(root, &job.batch_id, path);
        }
    }
}

fn clean_previews(job: &StoredJob) {
    for preview in &job.previews {
        let _ = preview.remove();
    }
}
fn update_report(root: &Path, batch_id: &str, report: &Path) -> Result<()> {
    let jobs: Vec<_> = records(root)?
        .into_iter()
        .filter(|job| job.batch_id == batch_id)
        .collect();
    let successes: Vec<_> = jobs
        .iter()
        .filter(|job| job.view.state == JobState::Completed)
        .flat_map(|job| job.view.outputs.clone())
        .collect();
    let errors: Vec<_> = jobs
        .iter()
        .filter_map(|job| {
            job.view
                .error
                .as_ref()
                .map(|error| format!("{}: {error}", job.view.input.display()))
        })
        .collect();
    let timestamp =
        time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339)?;
    let value = serde_json::json!({"timestamp":timestamp,"format":jobs.first().map(|job|job.view.format.as_str()).unwrap_or(""),"successes":successes,"errors":errors});
    let parent = report
        .parent()
        .context("Percorso del resoconto non valido")?;
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut temporary, &value)?;
    temporary.persist(report).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::OutputFormat;

    #[test]
    fn busy_worker_accepts_new_dispatch_without_starting_another_encoder() {
        let temp = tempfile::tempdir().unwrap();
        let worker = open_lock(temp.path(), "worker.lock").unwrap();
        fs2::FileExt::lock_exclusive(&worker).unwrap();
        let competitor = open_lock(temp.path(), "worker.lock").unwrap();
        assert!(lock_contended(
            &fs2::FileExt::try_lock_exclusive(&competitor).unwrap_err()
        ));
        start_worker_locked(temp.path(), Path::new("nonexistent.exe")).unwrap();
        run_worker_at(temp.path(), None).unwrap();
    }

    #[test]
    fn queue_previews_exist_before_worker_and_publish_exact_names() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("queue");
        let source = temp.path().join("immagine è.png");
        image::RgbaImage::from_pixel(40, 20, image::Rgba([40, 90, 130, 255]))
            .save(&source)
            .unwrap();
        let settings = ConversionSettings {
            format: OutputFormat::Jpg,
            ..Default::default()
        };
        let executable = std::env::current_exe().unwrap();
        let first = submit_at(
            &root,
            std::slice::from_ref(&source),
            &settings,
            &ToolPaths::default(),
            &executable,
            false,
        )
        .unwrap();
        let second = submit_at(
            &root,
            &[source],
            &settings,
            &ToolPaths::default(),
            &executable,
            false,
        )
        .unwrap();
        assert!(first.errors.is_empty() && second.errors.is_empty());
        let first_job = &first.jobs[0];
        let second_job = &second.jobs[0];
        assert_ne!(first_job.outputs, second_job.outputs);
        for job in [first_job, second_job] {
            assert!(!job.outputs[0].exists());
            assert!(PendingLink::path_for(&job.outputs[0]).is_file());
            assert_eq!(load(&root, &job.id).unwrap().view.state, JobState::Queued);
        }
        run_worker_at(&root, Some(&root.join("report-last.json"))).unwrap();
        for job in [first_job, second_job] {
            assert_eq!(
                load(&root, &job.id).unwrap().view.state,
                JobState::Completed
            );
            assert!(image::open(&job.outputs[0]).is_ok());
            assert!(!PendingLink::path_for(&job.outputs[0]).exists());
        }
    }

    #[test]
    fn cancel_queued_and_failed_outputs_have_clear_states() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("queue");
        let source = temp.path().join("broken.png");
        fs::write(&source, b"invalid image").unwrap();
        let settings = ConversionSettings {
            format: OutputFormat::Jpg,
            ..Default::default()
        };
        let exe = std::env::current_exe().unwrap();
        let cancelled = submit_at(
            &root,
            std::slice::from_ref(&source),
            &settings,
            &ToolPaths::default(),
            &exe,
            false,
        )
        .unwrap()
        .jobs
        .remove(0);
        cancel_at(&root, &cancelled.id).unwrap();
        assert_eq!(
            load(&root, &cancelled.id).unwrap().view.state,
            JobState::Cancelled
        );
        assert!(!PendingLink::path_for(&cancelled.outputs[0]).exists());
        let failed = submit_at(
            &root,
            &[source],
            &settings,
            &ToolPaths::default(),
            &exe,
            false,
        )
        .unwrap()
        .jobs
        .remove(0);
        run_worker_at(&root, None).unwrap();
        let state = load(&root, &failed.id).unwrap();
        assert_eq!(state.view.state, JobState::Failed);
        assert!(state.view.error.is_some());
        assert!(PendingLink::path_for(&failed.outputs[0]).exists());
        assert!(!failed.outputs[0].exists());
    }

    #[test]
    fn interrupted_running_job_is_reported_and_preserves_external_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("queue");
        let source = temp.path().join("source.png");
        image::RgbaImage::new(2, 2).save(&source).unwrap();
        let settings = ConversionSettings {
            format: OutputFormat::Jpg,
            ..Default::default()
        };
        let submitted = submit_at(
            &root,
            &[source],
            &settings,
            &ToolPaths::default(),
            &std::env::current_exe().unwrap(),
            false,
        )
        .unwrap();
        let mut record = load(&root, &submitted.jobs[0].id).unwrap();
        record.view.state = JobState::Running;
        store(&root, &record).unwrap();
        fs::write(&record.view.outputs[0], b"external file").unwrap();
        run_worker_at(&root, None).unwrap();
        assert_eq!(
            load(&root, &record.view.id).unwrap().view.state,
            JobState::Failed
        );
        assert_eq!(fs::read(&record.view.outputs[0]).unwrap(), b"external file");
        assert!(!record.plan.outputs[0].marker.exists());
        assert!(get_job("../../outside").is_err());
    }
}
