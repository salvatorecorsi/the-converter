//! Names are reserved before a job enters the queue; only complete media are published.
use super::{Progress, cancelled, convert, stem, validate};
use crate::{
    model::{ConversionSettings, MediaKind, OutputFormat, supported_formats},
    tools::ToolPaths,
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_OUTPUTS: usize = 10_000;
const RESERVATION_DIRECTORY: &str = ".the-converter-reservations";
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Durable queue data. Call `release_prepared` after completion, failure or abandonment.
/// Dropping/deserializing this value intentionally does not release reservations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedConversion {
    pub id: String,
    pub input: PathBuf,
    pub settings: ConversionSettings,
    pub output_dir: PathBuf,
    pub work_dir: PathBuf,
    pub outputs: Vec<ReservedOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReservedOutput {
    pub path: PathBuf,
    pub marker: PathBuf,
    pub token: String,
}

#[derive(Serialize, Deserialize)]
struct Reservation {
    version: u32,
    token: String,
    input: PathBuf,
    output: PathBuf,
}

fn next_id() -> String {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "{time:x}-{:x}-{:x}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|value| value.is_ascii_hexdigit() || value == b'-')
}

fn append(path: &Path, suffix: &str) -> PathBuf {
    let mut value: OsString = path.as_os_str().to_owned();
    value.push(suffix);
    PathBuf::from(value)
}

fn exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => {
            Err(error).with_context(|| format!("Impossibile controllare {}", path.display()))
        }
    }
}

fn marker_path(output: &Path) -> Result<PathBuf> {
    let name = output
        .file_name()
        .context("Nome di destinazione non valido")?;
    let key = if cfg!(windows) {
        name.to_string_lossy().to_lowercase()
    } else {
        name.to_string_lossy().into_owned()
    };
    let digest = Sha256::digest(key.as_bytes());
    let name = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(output
        .parent()
        .context("Cartella di destinazione non valida")?
        .join(RESERVATION_DIRECTORY)
        .join(format!("{name}.json")))
}

fn ensure_reservation_directory(output_dir: &Path) -> Result<()> {
    let directory = output_dir.join(RESERVATION_DIRECTORY);
    loop {
        let result = (|| -> Result<()> {
            fs::create_dir_all(&directory)?;
            if fs::symlink_metadata(&directory)?.file_type().is_symlink()
                || fs::canonicalize(&directory)?.parent() != Some(output_dir)
            {
                bail!(
                    "La cartella delle prenotazioni deve essere una sottocartella locale della destinazione"
                );
            }
            set_hidden(&directory)
        })();
        // Another process may remove the empty namespace before our first marker exists.
        if result.as_ref().is_err_and(|error| {
            error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound)
        }) {
            continue;
        }
        return result;
    }
}

fn create_marker(output_dir: &Path, marker: &Path) -> Result<Option<File>> {
    loop {
        ensure_reservation_directory(output_dir)?;
        match OpenOptions::new().write(true).create_new(true).open(marker) {
            Ok(file) => return Ok(Some(file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(None),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error).context("Impossibile prenotare il nome del risultato"),
        }
    }
}

fn remove_empty_reservation_directory(output_dir: &Path) -> Result<()> {
    let directory = output_dir.join(RESERVATION_DIRECTORY);
    let result = (|| -> Result<()> {
        if fs::symlink_metadata(&directory)?.file_type().is_symlink()
            || fs::canonicalize(&directory)?.parent() != Some(output_dir)
        {
            bail!("La cartella delle prenotazioni non appartiene alla destinazione");
        }
        // remove_dir is atomic and never deletes markers belonging to queued/concurrent jobs.
        fs::remove_dir(&directory).context("Impossibile pulire la cartella delle prenotazioni")
    })();
    match result {
        Err(error)
            if error.downcast_ref::<std::io::Error>().is_some_and(|error| {
                matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
                )
            }) =>
        {
            Ok(())
        }
        result => result,
    }
}

#[cfg(windows)]
fn set_hidden(path: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileAttributesW(path: *const u16) -> u32;
        fn SetFileAttributesW(path: *const u16, attributes: u32) -> i32;
    }
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let attributes = GetFileAttributesW(wide.as_ptr());
        if attributes == u32::MAX || SetFileAttributesW(wide.as_ptr(), attributes | 0x2) == 0 {
            return Err(std::io::Error::last_os_error())
                .context("Impossibile nascondere i file di lavoro");
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn set_hidden(_: &Path) -> Result<()> {
    Ok(())
}

/// Allocate all names now, including one name per PDF page, without creating invalid media files.
pub fn prepare(
    input: &Path,
    settings: &ConversionSettings,
    tools: &ToolPaths,
) -> Result<PreparedConversion> {
    let input = fs::canonicalize(input)
        .with_context(|| format!("File non accessibile: {}", input.display()))?;
    if !input.is_file() {
        bail!("La selezione deve contenere file, non cartelle");
    }
    let kind = MediaKind::from_path(&input);
    if !supported_formats(kind).contains(&settings.format) {
        bail!(
            "Conversione non supportata: {:?} → {}",
            kind,
            settings.format
        );
    }
    validate(settings)?;
    if let Some(message) = tools.missing_for(kind, settings.format) {
        bail!("{message}");
    }
    let destination = settings
        .output_dir
        .clone()
        .unwrap_or_else(|| input.parent().unwrap().to_owned());
    fs::create_dir_all(&destination).context("Impossibile creare la cartella di destinazione")?;
    let output_dir = fs::canonicalize(destination)?;
    let id = next_id();
    let mut settings = settings.clone();
    settings.output_dir = Some(output_dir.clone());
    let mut plan = PreparedConversion {
        work_dir: output_dir.join(format!(".the-converter-work-{id}")),
        id,
        input,
        settings,
        output_dir,
        outputs: Vec::new(),
    };
    let result = (|| -> Result<()> {
        let names = if kind == MediaKind::Pdf
            && matches!(plan.settings.format, OutputFormat::Png | OutputFormat::Jpg)
        {
            let document = lopdf::Document::load(&plan.input)
                .context("Impossibile leggere le pagine del PDF")?;
            if document.is_encrypted() {
                bail!("Il PDF è protetto: sbloccalo prima della conversione");
            }
            let count = document.get_pages().len();
            if count == 0 || count > MAX_OUTPUTS {
                bail!("Il PDF deve contenere da 1 a {MAX_OUTPUTS} pagine");
            }
            (1..=count)
                .map(|page| format!("{}-pagina-{page:03}", stem(&plan.input)))
                .collect::<Vec<_>>()
        } else {
            vec![stem(&plan.input)]
        };
        for name in names {
            plan.outputs.push(reserve(&plan, &name)?);
        }
        Ok(())
    })();
    if let Err(error) = result {
        let _ = release_prepared(&plan);
        return Err(error);
    }
    Ok(plan)
}

fn reserve(plan: &PreparedConversion, stem: &str) -> Result<ReservedOutput> {
    for number in 0..10_000 {
        let name = if number == 0 {
            format!("{stem}.{}", plan.settings.format.extension())
        } else {
            format!("{stem} ({number}).{}", plan.settings.format.extension())
        };
        let path = plan.output_dir.join(name);
        if exists(&path)? || exists(&append(&path, ".lnk"))? {
            continue;
        }
        let marker = marker_path(&path)?;
        let mut file = match create_marker(&plan.output_dir, &marker)? {
            Some(file) => file,
            None => continue,
        };
        let reserved = ReservedOutput {
            path,
            marker,
            token: plan.id.clone(),
        };
        let record = Reservation {
            version: 1,
            token: plan.id.clone(),
            input: plan.input.clone(),
            output: reserved.path.clone(),
        };
        let result = (|| -> Result<()> {
            file.write_all(&serde_json::to_vec(&record)?)?;
            file.sync_all()?;
            set_hidden(&reserved.marker)?;
            Ok(())
        })();
        drop(file);
        if let Err(error) = result {
            let _ = fs::remove_file(&reserved.marker);
            return Err(error);
        }
        // Close the window between checking the directory and acquiring the reservation.
        match (|| -> Result<bool> {
            Ok(exists(&reserved.path)? || exists(&append(&reserved.path, ".lnk"))?)
        })() {
            Ok(false) => return Ok(reserved),
            Ok(true) => {
                remove_marker(plan, &reserved)?;
            }
            Err(error) => {
                let _ = remove_marker(plan, &reserved);
                return Err(error);
            }
        }
    }
    bail!("Troppi file o conversioni in coda con lo stesso nome")
}

fn read_record(path: &Path) -> Result<Reservation> {
    let file = File::open(path).context("La prenotazione del risultato non è più disponibile")?;
    if file.metadata()?.len() > 32 * 1024 {
        bail!("Prenotazione non valida");
    }
    serde_json::from_reader(file.take(32 * 1024 + 1)).context("Prenotazione non valida")
}

fn verify_output(plan: &PreparedConversion, output: &ReservedOutput) -> Result<()> {
    if output.path.parent() != Some(plan.output_dir.as_path())
        || output.marker != marker_path(&output.path)?
        || output.token != plan.id
    {
        bail!("Il percorso prenotato non corrisponde alla conversione");
    }
    let record = read_record(&output.marker)?;
    if record.version != 1
        || record.token != output.token
        || record.input != plan.input
        || record.output != output.path
    {
        bail!("Il nome del risultato è prenotato da un'altra conversione");
    }
    Ok(())
}

/// Verify durable ownership after loading a queued job from disk.
pub fn validate_prepared(plan: &PreparedConversion) -> Result<()> {
    if !valid_id(&plan.id)
        || plan.outputs.is_empty()
        || plan.outputs.len() > MAX_OUTPUTS
        || plan.settings.output_dir.as_ref() != Some(&plan.output_dir)
        || plan.work_dir
            != plan
                .output_dir
                .join(format!(".the-converter-work-{}", plan.id))
    {
        bail!("Piano di conversione non valido");
    }
    validate(&plan.settings)?;
    let mut paths = std::collections::HashSet::new();
    for output in &plan.outputs {
        if output.path.extension().and_then(|value| value.to_str())
            != Some(plan.settings.format.extension())
            || !paths.insert(&output.path)
        {
            bail!("Nomi di destinazione non validi nel piano");
        }
        verify_output(plan, output)?;
    }
    Ok(())
}

fn remove_marker(plan: &PreparedConversion, output: &ReservedOutput) -> Result<()> {
    if !exists(&output.marker)? {
        return Ok(());
    }
    verify_output(plan, output)?;
    fs::remove_file(&output.marker).context("Impossibile liberare il nome prenotato")
}

fn work_owner(plan: &PreparedConversion) -> PathBuf {
    plan.work_dir.join(".owner")
}

fn remove_work(plan: &PreparedConversion) -> Result<()> {
    if !exists(&plan.work_dir)? {
        return Ok(());
    }
    if !valid_id(&plan.id)
        || plan.work_dir
            != plan
                .output_dir
                .join(format!(".the-converter-work-{}", plan.id))
        || fs::symlink_metadata(&plan.work_dir)?
            .file_type()
            .is_symlink()
        || fs::canonicalize(&plan.work_dir)?.parent() != Some(plan.output_dir.as_path())
        || fs::read_to_string(work_owner(plan))? != plan.id
    {
        bail!("La cartella di lavoro non appartiene a questa conversione");
    }
    fs::remove_dir_all(&plan.work_dir).context("Impossibile pulire la cartella di lavoro")
}

/// Never removes media outputs or unrelated links/files. Safe to call more than once.
pub fn release_prepared(plan: &PreparedConversion) -> Result<()> {
    let mut errors = Vec::new();
    if let Err(error) = remove_work(plan) {
        errors.push(error.to_string());
    }
    for output in &plan.outputs {
        if let Err(error) = remove_marker(plan, output) {
            errors.push(error.to_string());
        }
    }
    if let Err(error) = remove_empty_reservation_directory(&plan.output_dir) {
        errors.push(error.to_string());
    }
    if !errors.is_empty() {
        bail!("{}", errors.join("; "));
    }
    Ok(())
}

struct WorkGuard<'a>(&'a PreparedConversion);
impl Drop for WorkGuard<'_> {
    fn drop(&mut self) {
        let _ = remove_work(self.0);
    }
}

struct Published {
    path: PathBuf,
    file: File,
}

#[cfg(windows)]
fn identity(file: &File) -> std::io::Result<(u32, u64)> {
    use std::os::windows::io::AsRawHandle;
    #[repr(C)]
    #[derive(Default)]
    struct Information {
        attributes: u32,
        times: [u32; 6],
        volume: u32,
        size: [u32; 2],
        links: u32,
        high: u32,
        low: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileInformationByHandle(
            handle: *mut std::ffi::c_void,
            information: *mut Information,
        ) -> i32;
    }
    let mut value = Information::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut value) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok((
        value.volume,
        (u64::from(value.high) << 32) | u64::from(value.low),
    ))
}

#[cfg(not(windows))]
fn identity(file: &File) -> std::io::Result<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file.metadata()?;
    Ok((metadata.dev(), metadata.ino()))
}

fn rollback(published: &[Published]) {
    for value in published {
        if let Ok(current) = File::open(&value.path)
            && matches!((identity(&value.file), identity(&current)), (Ok(a), Ok(b)) if a == b)
        {
            let _ = fs::remove_file(&value.path);
        }
    }
}

/// Convert in an isolated hidden staging directory, then publish at the exact reserved names.
/// A destination created by another program after reservation is preserved and causes an error.
pub fn convert_prepared(
    plan: &PreparedConversion,
    tools: &ToolPaths,
    cancel: &AtomicBool,
    mut progress: impl FnMut(Progress),
) -> Result<Vec<PathBuf>> {
    cancelled(cancel)?;
    validate_prepared(plan)?;
    for output in &plan.outputs {
        if exists(&output.path)? {
            bail!(
                "La destinazione è stata creata da un altro programma: {}",
                output.path.display()
            );
        }
    }
    remove_work(plan)?;
    fs::create_dir(&plan.work_dir).context("Impossibile creare la cartella di lavoro")?;
    let owner_result = fs::write(work_owner(plan), &plan.id);
    if let Err(error) = owner_result {
        let _ = fs::remove_dir(&plan.work_dir);
        return Err(error.into());
    }
    let _guard = WorkGuard(plan);
    set_hidden(&plan.work_dir)?;
    let mut settings = plan.settings.clone();
    settings.output_dir = Some(plan.work_dir.clone());
    let generated = convert(&plan.input, &settings, tools, cancel, |mut value| {
        value.fraction *= 0.97;
        if value.fraction >= 0.97 {
            value.message = "Salvataggio del risultato…".into();
        }
        progress(value);
    })?;
    if generated.len() != plan.outputs.len() {
        bail!(
            "Il numero di risultati è cambiato dopo l'inserimento in coda; nessun file è stato pubblicato"
        );
    }
    let mut published = Vec::new();
    let result = (|| -> Result<()> {
        for (index, (source, output)) in generated.iter().zip(&plan.outputs).enumerate() {
            cancelled(cancel)?;
            verify_output(plan, output)?;
            // persist_noclobber performs a same-volume atomic move, including a final race check.
            let mut temporary = tempfile::Builder::new()
                .prefix(".publish-")
                .tempfile_in(&plan.work_dir)?;
            let mut source = File::open(source)?;
            std::io::copy(&mut source, temporary.as_file_mut())?;
            temporary.as_file_mut().sync_all()?;
            let file = temporary
                .persist_noclobber(&output.path)
                .map_err(|error| error.error)
                .with_context(|| {
                    format!(
                        "Impossibile salvare senza sovrascrivere {}",
                        output.path.display()
                    )
                })?;
            published.push(Published {
                path: output.path.clone(),
                file,
            });
            progress(Progress {
                fraction: 0.97 + 0.03 * (index + 1) as f32 / plan.outputs.len() as f32,
                message: format!("Salvato {} di {}", index + 1, plan.outputs.len()),
            });
        }
        cancelled(cancel)?;
        Ok(())
    })();
    if let Err(error) = result {
        rollback(&published);
        return Err(error);
    }
    Ok(plan
        .outputs
        .iter()
        .map(|output| output.path.clone())
        .collect())
}
