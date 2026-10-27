use std::{path::Path, sync::atomic::AtomicBool};
use the_converter::{
    engine,
    model::{ConversionSettings, MediaKind, OutputFormat, common_formats, supported_formats},
    tools::ToolPaths,
};

#[test]
fn selection_offers_only_shared_routes_and_opt_in_transcription() {
    use OutputFormat::*;
    let cases: &[(&[&str], bool, &[OutputFormat])] = &[
        (&[], true, &[]),
        (&["note.txt"], true, &[Pdf]),
        (&["note.txt", "photo.png"], true, &[Pdf]),
        (&["note.txt", "clip.mp4"], true, &[]),
        (
            &["song.WAV", "clip.mp4"],
            true,
            &[Mp3, Wav, Flac, Aac, Ogg, Opus, Aiff],
        ),
        (&["song.wav", "report.pdf"], true, &[Txt]),
        (&["song.wav", "report.pdf"], false, &[]),
        (&["report.pdf"], false, &[Png, Jpg, Txt]),
        (&["photo.png", "report.pdf"], true, &[Png, Jpg]),
        (&["song.wav", "file.xyz"], true, &[]),
    ];
    for (inputs, enabled, expected) in cases {
        assert_eq!(
            common_formats(inputs.iter().map(Path::new), *enabled),
            *expected,
            "{inputs:?}"
        );
    }
    assert!(!common_formats([Path::new("song.wav")], false).contains(&Txt));
    assert!(common_formats([Path::new("song.wav")], true).contains(&Txt));
}

#[test]
fn unsupported_routes_are_rejected_before_tools_or_output_creation() {
    let folder = tempfile::tempdir().unwrap();
    let inputs = [
        "recording.wav",
        "clip.mp4",
        "photo.png",
        "report.pdf",
        "note.txt",
        "file.xyz",
    ];
    for name in inputs {
        let input = folder.path().join(name);
        std::fs::write(&input, b"fixture").unwrap();
        let kind = MediaKind::from_path(&input);
        for format in OutputFormat::ALL {
            if supported_formats(kind).contains(&format) {
                continue;
            }
            let output = folder.path().join("must-not-be-created");
            let settings = ConversionSettings {
                format,
                output_dir: Some(output.clone()),
                ..Default::default()
            };
            let result = engine::convert(
                &input,
                &settings,
                &ToolPaths::default(),
                &AtomicBool::new(false),
                |_| {},
            );
            let error = result.unwrap_err().to_string();
            assert!(
                error.contains("Conversione non supportata"),
                "{name} -> {format}: {error}"
            );
            assert!(
                !output.exists(),
                "{name} -> {format} created an output directory"
            );
            let queued = engine::prepare(&input, &settings, &ToolPaths::default());
            assert!(queued.is_err(), "queued {name} -> {format}");
            assert!(!output.exists());
        }
    }
}

#[test]
fn old_settings_load_without_a_key_or_transcription_configuration() {
    let settings: ConversionSettings =
        serde_json::from_str(r#"{"format":"Txt","audio":{"bitrate_kbps":192}}"#).unwrap();
    assert_eq!(settings.transcription.language, None);
    let json = serde_json::to_string(&settings).unwrap();
    assert!(!json.contains("api_key"));
}
