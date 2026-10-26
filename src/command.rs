use anyhow::{Context, Result, bail};
use clap::{Parser, ValueEnum};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use the_converter::{
    engine,
    model::{ConversionSettings, OutputFormat},
    queue,
    tools::ToolPaths,
};

#[derive(Parser, Debug)]
#[command(
    version,
    about = "The Converter — audio, video, immagini e PDF sul tuo PC"
)]
struct Args {
    /// File da aprire nella finestra. Usa -- prima di nomi che iniziano con un trattino.
    #[arg(long, num_args = 1..)]
    files: Vec<PathBuf>,
    /// Selezione di Esplora file: array JSON UTF-8 di percorsi.
    #[arg(long)]
    selection_file: Option<PathBuf>,
    /// Formato di destinazione (mp3, wav, mp4, png, pdf…).
    #[arg(long, value_parser = parse_format)]
    target: Option<OutputFormat>,
    /// Esegui senza finestra; richiede --target o --settings.
    #[arg(long)]
    convert: bool,
    /// Conversione rapida da Esplora file, con preferenze salvate e resoconto locale.
    #[arg(long, requires = "target", conflicts_with_all = ["convert", "diagnose"])]
    quick: bool,
    /// Stato di una conversione, aperto dal segnaposto in Esplora file.
    #[arg(long, conflicts_with_all = ["quick", "convert", "diagnose", "queue_worker"])]
    queue_job: Option<String>,
    #[arg(long, hide = true, conflicts_with_all = ["quick", "convert", "diagnose", "queue_job"])]
    queue_worker: bool,
    /// Cartella di destinazione. Per default accanto all'originale.
    #[arg(long, requires = "convert")]
    output_dir: Option<PathBuf>,
    /// File JSON con le impostazioni di conversione.
    #[arg(long, requires = "convert")]
    settings: Option<PathBuf>,
    #[arg(long, value_enum, requires = "convert")]
    preset: Option<Preset>,
    /// Mostra i percorsi dei motori disponibili in JSON.
    #[arg(long)]
    diagnose: bool,
    /// Percorsi diretti, usabili anche da Apri con.
    #[arg(value_name = "FILE")]
    input: Vec<PathBuf>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Preset {
    Compact,
    Balanced,
    Quality,
}

fn parse_format(value: &str) -> std::result::Result<OutputFormat, String> {
    OutputFormat::ALL
        .iter()
        .copied()
        .find(|format| {
            format
                .extension()
                .eq_ignore_ascii_case(value.trim_start_matches('.'))
        })
        .ok_or_else(|| format!("Formato non supportato: {value}"))
}

fn read_selection(path: &Path) -> Result<Vec<PathBuf>> {
    let metadata = path
        .metadata()
        .context("Impossibile leggere la selezione")?;
    if metadata.len() > 16 * 1024 * 1024 {
        bail!("Selezione troppo grande")
    }
    let bytes = std::fs::read(path)?;
    let files: Vec<PathBuf> =
        serde_json::from_slice(&bytes).context("Selezione JSON non valida")?;
    if files.len() > 10_000 {
        bail!("Troppi file nella selezione (massimo 10000)")
    }
    // Only consume disposable files produced by our Explorer extension.
    let is_our_file = path
        .file_name()
        .and_then(|v| v.to_str())
        .is_some_and(|name| {
            name.starts_with("the-converter-selection-") && name.ends_with(".json")
        });
    let in_temp = path.parent().and_then(|p| p.canonicalize().ok())
        == std::env::temp_dir().canonicalize().ok();
    if is_our_file && in_temp {
        let _ = std::fs::remove_file(path);
    }
    Ok(files)
}

fn run(args: Args) -> Result<()> {
    if args.queue_worker {
        return queue::run_worker();
    }
    if let Some(id) = args.queue_job {
        queue::ensure_worker()?;
        return crate::app::launch_queue_status(id).map_err(|error| anyhow::anyhow!("{error}"));
    }
    if args.diagnose {
        println!("{}", serde_json::to_string_pretty(&ToolPaths::discover())?);
        return Ok(());
    }
    let mut files = args.files;
    files.extend(args.input);
    if let Some(selection) = args.selection_file {
        files.extend(read_selection(&selection)?);
    }
    let mut seen = std::collections::HashSet::new();
    files.retain(|p| seen.insert(p.clone()));
    if args.quick {
        return quick_convert(files, args.target.expect("clap requires target"));
    }
    if !args.convert {
        queue::ensure_worker()?;
        return crate::app::launch(files, args.target).map_err(|err| anyhow::anyhow!("{err}"));
    }
    if files.is_empty() {
        bail!("Indica almeno un file con --files")
    }
    if args.target.is_none() && args.settings.is_none() {
        bail!("Specifica --target oppure --settings")
    }
    let mut settings: ConversionSettings = if let Some(path) = args.settings {
        serde_json::from_slice(&std::fs::read(path)?).context("Impostazioni JSON non valide")?
    } else {
        ConversionSettings::default()
    };
    if let Some(format) = args.target {
        settings.format = format;
    }
    if let Some(dir) = args.output_dir {
        settings.output_dir = Some(dir);
    }
    if let Some(preset) = args.preset {
        let (bitrate, crf, quality) = match preset {
            Preset::Compact => (128, 28, 75),
            Preset::Balanced => (192, 23, 90),
            Preset::Quality => (320, 18, 95),
        };
        settings.audio.bitrate_kbps = bitrate;
        settings.video.crf = crf;
        settings.image.quality = quality;
    }
    let tools = ToolPaths::discover();
    let cancel = Arc::new(AtomicBool::new(false));
    let signal = cancel.clone();
    ctrlc::set_handler(move || {
        signal.store(true, Ordering::Relaxed);
    })?;
    let mut failures = 0;
    for file in files {
        if cancel.load(Ordering::Relaxed) {
            bail!("Conversione annullata")
        }
        eprintln!("Conversione: {}", file.display());
        match engine::convert(&file, &settings, &tools, &cancel, |progress| {
            eprintln!("{:3.0}% {}", progress.fraction * 100.0, progress.message);
        }) {
            Ok(outputs) => {
                for output in outputs {
                    println!("{}", output.display());
                }
            }
            Err(err) => {
                eprintln!("Errore: {err:#}");
                failures += 1;
            }
        }
    }
    if failures > 0 {
        bail!("{failures} conversioni non riuscite")
    }
    Ok(())
}

fn quick_convert(files: Vec<PathBuf>, format: OutputFormat) -> Result<()> {
    if files.is_empty() {
        bail!("Nessun file da convertire")
    }
    let (mut settings, tools) = crate::app::quick_configuration();
    settings.format = format;
    let submission = queue::enqueue(&files, &settings, &tools, &std::env::current_exe()?)?;
    if !submission.errors.is_empty() {
        let details = submission
            .errors
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n");
        bail!(
            "{} file non aggiunti alla coda.\n\n{details}\n\nGli altri lavori proseguono in background.",
            submission.errors.len()
        );
    }
    Ok(())
}

pub fn main() {
    let args = Args::parse();
    let headless = args.convert || args.diagnose || args.queue_worker;
    if let Err(error) = run(args) {
        eprintln!("{error:#}");
        if !headless {
            rfd::MessageDialog::new()
                .set_title("The Converter")
                .set_description(format!("{error:#}"))
                .set_level(rfd::MessageLevel::Error)
                .show();
        }
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_preserves_unicode_and_does_not_delete_user_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("my-selection.json");
        std::fs::write(
            &path,
            r#"["C:\\Musica\\città & mare.wav","C:\\video con spazi.mp4"]"#,
        )
        .unwrap();
        let result = read_selection(&path).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result[0].to_string_lossy().contains("città & mare"));
        assert!(path.exists());
    }
    #[test]
    fn rejects_invalid_selection_and_unknown_target() {
        assert!(parse_format("exe").is_err());
        assert_eq!(parse_format(".MP3").unwrap(), OutputFormat::Mp3);
        assert!(Args::try_parse_from(["the-converter", "--convert", "--target", "exe"]).is_err());
    }
}
