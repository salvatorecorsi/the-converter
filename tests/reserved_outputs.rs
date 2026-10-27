use std::{
    fs,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use the_converter::{
    engine::{self, PreparedConversion},
    model::{ConversionSettings, OutputFormat},
    tools::ToolPaths,
};

fn fixture(path: &Path) {
    image::RgbaImage::from_pixel(40, 20, image::Rgba([18, 100, 220, 255]))
        .save(path)
        .unwrap();
}

fn settings(format: OutputFormat) -> ConversionSettings {
    ConversionSettings {
        format,
        ..Default::default()
    }
}

#[test]
fn names_are_reserved_without_creating_invalid_media_and_survive_serialization() {
    let folder = tempfile::tempdir().unwrap();
    let input = folder.path().join("città & mare.png");
    fixture(&input);
    fs::write(folder.path().join("città & mare.jpg"), b"existing output").unwrap();
    fs::write(folder.path().join("città & mare (1).jpg.lnk"), b"user link").unwrap();
    let first =
        engine::prepare(&input, &settings(OutputFormat::Jpg), &ToolPaths::default()).unwrap();
    assert_eq!(
        first.outputs[0].path.file_name().unwrap(),
        "città & mare (2).jpg"
    );
    assert!(!first.outputs[0].path.exists());
    assert!(first.outputs[0].marker.is_file());
    let restored: PreparedConversion =
        serde_json::from_slice(&serde_json::to_vec(&first).unwrap()).unwrap();
    engine::validate_prepared(&restored).unwrap();
    let second =
        engine::prepare(&input, &settings(OutputFormat::Jpg), &ToolPaths::default()).unwrap();
    assert_eq!(
        second.outputs[0].path.file_name().unwrap(),
        "città & mare (3).jpg"
    );
    engine::release_prepared(&restored).unwrap();
    engine::release_prepared(&restored).unwrap();
    engine::release_prepared(&second).unwrap();
    assert_eq!(
        fs::read(folder.path().join("città & mare.jpg")).unwrap(),
        b"existing output"
    );
    assert_eq!(
        fs::read(folder.path().join("città & mare (1).jpg.lnk")).unwrap(),
        b"user link"
    );
    assert!(!folder.path().join(".the-converter-reservations").exists());
}

#[test]
fn simultaneous_jobs_get_distinct_durable_destinations() {
    let folder = tempfile::tempdir().unwrap();
    let input = folder.path().join("source.png");
    fixture(&input);
    let input = Arc::new(input);
    let barrier = Arc::new(std::sync::Barrier::new(8));
    let threads = (0..8)
        .map(|_| {
            let input = input.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                engine::prepare(&input, &settings(OutputFormat::Jpg), &ToolPaths::default())
                    .unwrap()
            })
        })
        .collect::<Vec<_>>();
    let plans = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect::<Vec<_>>();
    let paths = plans
        .iter()
        .map(|plan| &plan.outputs[0].path)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(paths.len(), 8);
    for plan in &plans {
        engine::release_prepared(plan).unwrap();
    }
    assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 1);
    assert!(!folder.path().join(".the-converter-reservations").exists());
}

#[test]
fn namespace_stays_until_the_last_job_releases_its_markers() {
    let folder = tempfile::tempdir().unwrap();
    let input = folder.path().join("source.png");
    fixture(&input);
    let first =
        engine::prepare(&input, &settings(OutputFormat::Jpg), &ToolPaths::default()).unwrap();
    let second =
        engine::prepare(&input, &settings(OutputFormat::Jpg), &ToolPaths::default()).unwrap();
    let namespace = folder.path().join(".the-converter-reservations");
    engine::release_prepared(&first).unwrap();
    assert!(namespace.is_dir());
    assert!(second.outputs[0].marker.is_file());
    engine::validate_prepared(&second).unwrap();
    engine::release_prepared(&first).unwrap();
    assert!(second.outputs[0].marker.is_file());
    engine::release_prepared(&second).unwrap();
    assert!(!namespace.exists());
    engine::release_prepared(&first).unwrap();
    engine::release_prepared(&second).unwrap();
    assert!(!namespace.exists());
    assert!(input.is_file());
}

#[test]
fn namespace_with_an_unrelated_file_is_never_deleted() {
    let folder = tempfile::tempdir().unwrap();
    let input = folder.path().join("source.png");
    fixture(&input);
    let plan =
        engine::prepare(&input, &settings(OutputFormat::Jpg), &ToolPaths::default()).unwrap();
    let namespace = folder.path().join(".the-converter-reservations");
    let unrelated = namespace.join("user-note.txt");
    fs::write(&unrelated, b"preserve this file").unwrap();
    engine::release_prepared(&plan).unwrap();
    assert!(namespace.is_dir());
    assert_eq!(fs::read(&unrelated).unwrap(), b"preserve this file");
    assert!(!plan.outputs[0].marker.exists());
    engine::release_prepared(&plan).unwrap();
    assert_eq!(fs::read(unrelated).unwrap(), b"preserve this file");
}

#[test]
fn malformed_pdf_does_not_leave_an_empty_reservation_namespace() {
    let folder = tempfile::tempdir().unwrap();
    let input = folder.path().join("truncated.pdf");
    fs::write(&input, b"%PDF-1.7\ntruncated document").unwrap();
    // Preparation reads PDF pages without launching a tool. This existing executable satisfies
    // tool availability so the test reaches PDF validation even when Poppler is not installed.
    let tools = ToolPaths {
        pdftoppm: Some(std::env::current_exe().unwrap()),
        ..Default::default()
    };
    let error = engine::prepare(&input, &settings(OutputFormat::Png), &tools).unwrap_err();
    assert!(error.to_string().contains("pagine del PDF"));
    assert!(!folder.path().join(".the-converter-reservations").exists());
    assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 1);
}

#[test]
fn concurrent_prepare_and_release_preserve_live_ownership_and_cleanup_when_idle() {
    use std::collections::HashSet;
    use std::sync::{Barrier, Mutex};
    let folder = tempfile::tempdir().unwrap();
    let input = folder.path().join("source.png");
    fixture(&input);
    let input = Arc::new(input);
    let start = Arc::new(Barrier::new(8));
    let live = Arc::new(Mutex::new(HashSet::new()));
    let threads = (0..8)
        .map(|_| {
            let input = input.clone();
            let start = start.clone();
            let live = live.clone();
            std::thread::spawn(move || {
                start.wait();
                for _ in 0..40 {
                    let plan = engine::prepare(
                        &input,
                        &settings(OutputFormat::Jpg),
                        &ToolPaths::default(),
                    )
                    .unwrap();
                    {
                        let mut active = live.lock().unwrap();
                        assert!(
                            active.insert(plan.outputs[0].path.clone()),
                            "two live jobs reserved the same output"
                        );
                    }
                    std::thread::yield_now();
                    {
                        // Hold the bookkeeping lock across release so a valid reuse after releasing
                        // the marker cannot be mistaken for two simultaneous reservations.
                        let mut active = live.lock().unwrap();
                        engine::validate_prepared(&plan).unwrap();
                        engine::release_prepared(&plan).unwrap();
                        assert!(active.remove(&plan.outputs[0].path));
                    }
                    std::thread::yield_now();
                }
            })
        })
        .collect::<Vec<_>>();
    for thread in threads {
        thread.join().unwrap();
    }
    assert!(live.lock().unwrap().is_empty());
    assert!(!folder.path().join(".the-converter-reservations").exists());
    assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 1);
}

#[test]
fn prepared_conversion_publishes_exact_name_and_preserves_owned_markers_until_release() {
    let folder = tempfile::tempdir().unwrap();
    let input = folder.path().join("source.png");
    fixture(&input);
    let plan =
        engine::prepare(&input, &settings(OutputFormat::Jpg), &ToolPaths::default()).unwrap();
    let outputs = engine::convert_prepared(
        &plan,
        &ToolPaths::default(),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert_eq!(outputs, vec![plan.outputs[0].path.clone()]);
    assert_eq!(image::open(&outputs[0]).unwrap().width(), 40);
    assert!(!plan.work_dir.exists());
    assert!(plan.outputs[0].marker.exists());
    engine::release_prepared(&plan).unwrap();
    assert!(outputs[0].exists());
    assert!(!folder.path().join(".the-converter-reservations").exists());
}

#[test]
fn external_files_and_foreign_reservations_are_never_removed_or_overwritten() {
    let folder = tempfile::tempdir().unwrap();
    let input = folder.path().join("source.png");
    fixture(&input);
    let plan =
        engine::prepare(&input, &settings(OutputFormat::Jpg), &ToolPaths::default()).unwrap();
    let mut wrong = plan.clone();
    wrong.outputs[0].token = "different-owner".into();
    assert!(engine::validate_prepared(&wrong).is_err());
    assert!(engine::release_prepared(&wrong).is_err());
    assert!(plan.outputs[0].marker.exists());
    fs::write(
        &plan.outputs[0].path,
        b"external file created after enqueue",
    )
    .unwrap();
    assert!(
        engine::convert_prepared(
            &plan,
            &ToolPaths::default(),
            &AtomicBool::new(false),
            |_| {}
        )
        .is_err()
    );
    engine::release_prepared(&plan).unwrap();
    assert_eq!(
        fs::read(&plan.outputs[0].path).unwrap(),
        b"external file created after enqueue"
    );
}

#[test]
fn a_destination_created_after_encoding_is_still_never_overwritten() {
    let folder = tempfile::tempdir().unwrap();
    let input = folder.path().join("source.png");
    fixture(&input);
    let plan =
        engine::prepare(&input, &settings(OutputFormat::Jpg), &ToolPaths::default()).unwrap();
    let mut inserted = false;
    let result = engine::convert_prepared(
        &plan,
        &ToolPaths::default(),
        &AtomicBool::new(false),
        |progress| {
            if progress.message == "Salvataggio del risultato…" && !inserted {
                fs::write(
                    &plan.outputs[0].path,
                    b"a different program wrote this file",
                )
                .unwrap();
                inserted = true;
            }
        },
    );
    assert!(inserted);
    assert!(result.is_err());
    assert_eq!(
        fs::read(&plan.outputs[0].path).unwrap(),
        b"a different program wrote this file"
    );
    assert!(!plan.work_dir.exists());
    engine::release_prepared(&plan).unwrap();
}

fn two_page_pdf(path: &Path) {
    use lopdf::{Document, Object, Stream, dictionary};
    let mut document = Document::with_version("1.5");
    let pages = document.new_object_id();
    let mut children = Vec::new();
    for _ in 0..2 {
        let content = document.add_object(Stream::new(
            dictionary! {},
            b"0 0 1 rg 0 0 100 100 re f".to_vec(),
        ));
        let page = document.add_object(dictionary!{"Type"=>"Page", "Parent"=>pages, "MediaBox"=>vec![0.into(),0.into(),100.into(),100.into()], "Resources"=>dictionary!{}, "Contents"=>content});
        children.push(Object::Reference(page));
    }
    document.objects.insert(
        pages,
        Object::Dictionary(dictionary! {"Type"=>"Pages", "Kids"=>children, "Count"=>2}),
    );
    let root = document.add_object(dictionary! {"Type"=>"Catalog", "Pages"=>pages});
    document.trailer.set("Root", root);
    document.save(path).unwrap();
}

#[test]
fn all_pdf_page_names_are_available_before_work_and_cancel_rolls_back_publication() {
    let tools = ToolPaths::discover();
    if tools.pdftoppm.is_none() {
        return;
    }
    let folder = tempfile::tempdir().unwrap();
    let input = folder.path().join("document.pdf");
    two_page_pdf(&input);
    let plan = engine::prepare(&input, &settings(OutputFormat::Png), &tools).unwrap();
    assert_eq!(plan.outputs.len(), 2);
    assert_eq!(
        plan.outputs[0].path.file_name().unwrap(),
        "document-pagina-001.png"
    );
    assert_eq!(
        plan.outputs[1].path.file_name().unwrap(),
        "document-pagina-002.png"
    );
    let cancelled = AtomicBool::new(false);
    let result = engine::convert_prepared(&plan, &tools, &cancelled, |progress| {
        if progress.message == "Salvato 1 di 2" {
            cancelled.store(true, Ordering::Relaxed);
        }
    });
    assert!(result.is_err());
    assert!(plan.outputs.iter().all(|output| !output.path.exists()));
    assert!(!plan.work_dir.exists());
    engine::release_prepared(&plan).unwrap();
    assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 1);
    assert!(!folder.path().join(".the-converter-reservations").exists());
}

#[test]
fn rollback_preserves_a_published_file_replaced_by_another_program() {
    let tools = ToolPaths::discover();
    if tools.pdftoppm.is_none() {
        return;
    }
    let folder = tempfile::tempdir().unwrap();
    let input = folder.path().join("document.pdf");
    two_page_pdf(&input);
    let plan = engine::prepare(&input, &settings(OutputFormat::Png), &tools).unwrap();
    let cancelled = AtomicBool::new(false);
    let result = engine::convert_prepared(&plan, &tools, &cancelled, |progress| {
        if progress.message == "Salvato 1 di 2" {
            fs::remove_file(&plan.outputs[0].path).unwrap();
            fs::write(&plan.outputs[0].path, b"external replacement").unwrap();
            cancelled.store(true, Ordering::Relaxed);
        }
    });
    assert!(result.is_err());
    assert_eq!(
        fs::read(&plan.outputs[0].path).unwrap(),
        b"external replacement"
    );
    assert!(!plan.outputs[1].path.exists());
    engine::release_prepared(&plan).unwrap();
}

#[test]
fn recovery_cleans_only_the_jobs_owned_work_directory() {
    let folder = tempfile::tempdir().unwrap();
    let input = folder.path().join("source.png");
    fixture(&input);
    let plan =
        engine::prepare(&input, &settings(OutputFormat::Jpg), &ToolPaths::default()).unwrap();
    fs::create_dir(&plan.work_dir).unwrap();
    fs::write(plan.work_dir.join(".owner"), &plan.id).unwrap();
    fs::write(plan.work_dir.join("interrupted.tmp"), b"partial").unwrap();
    let restored: PreparedConversion =
        serde_json::from_slice(&serde_json::to_vec(&plan).unwrap()).unwrap();
    let outputs = engine::convert_prepared(
        &restored,
        &ToolPaths::default(),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert!(outputs[0].exists());
    assert!(!plan.work_dir.exists());
    engine::release_prepared(&restored).unwrap();
    let another =
        engine::prepare(&input, &settings(OutputFormat::Jpg), &ToolPaths::default()).unwrap();
    fs::create_dir(&another.work_dir).unwrap();
    fs::write(another.work_dir.join(".owner"), "someone else").unwrap();
    assert!(engine::release_prepared(&another).is_err());
    assert!(another.work_dir.exists());
}
