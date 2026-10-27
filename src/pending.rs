//! Native Explorer previews for queued outputs, without registering a global icon handler.
//!
//! A preview is a normal `.lnk` with the eventual media filename and a loading icon.
//! Explorer hides the shortcut suffix; opening it asks the app for this job's status.
//! No incomplete media is exposed as a playable file.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingLink {
    output: PathBuf,
    path: PathBuf,
    /// Stable app execution alias for MSIX, or the concrete EXE for portable builds.
    executable: PathBuf,
    /// Real PE binary used for icons. Older records used `executable` for both.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    icon_executable: Option<PathBuf>,
    job_id: String,
}

impl PendingLink {
    /// Creates a preview without replacing an existing shortcut or media file.
    pub fn create(output: &Path, executable: &Path, job_id: &str, status: &str) -> Result<Self> {
        validate_job_id(job_id)?;
        if !executable.is_file() {
            bail!("Applicazione non disponibile per mostrare lo stato della conversione");
        }
        if output.exists() {
            bail!("La destinazione esiste già: {}", output.display());
        }
        let parent = output
            .parent()
            .context("Destinazione della conversione non valida")?;
        fs::create_dir_all(parent)?;
        let target = crate::package::preview_target(executable);
        let pending = Self {
            output: output.to_owned(),
            path: Self::path_for(output),
            icon_executable: (!same_path(&target, executable)).then(|| executable.to_owned()),
            executable: target,
            job_id: job_id.to_owned(),
        };
        let temporary = tempfile::Builder::new()
            .prefix(".the-converter-preview-")
            .suffix(".lnk")
            .tempfile_in(parent)?;
        native::save_link(
            temporary.path(),
            &pending.executable,
            executable,
            job_id,
            status,
            &pending.output,
            false,
        )?;
        temporary
            .persist_noclobber(&pending.path)
            .map_err(|error| error.error)
            .with_context(|| {
                format!(
                    "Impossibile creare l'anteprima senza sovrascrivere: {}",
                    pending.path.display()
                )
            })?;
        native::notify(&pending.path, native::CREATE);
        Ok(pending)
    }

    pub fn path_for(output: &Path) -> PathBuf {
        let mut value = output.as_os_str().to_owned();
        value.push(".lnk");
        value.into()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn output(&self) -> &Path {
        &self.output
    }

    /// Updates the native tooltip. Callers should throttle this to meaningful state changes.
    pub fn update(&self, status: &str) -> Result<()> {
        self.write_state(status, false)
    }

    /// Keeps a failed job visible at its intended destination, with a red error icon.
    pub fn fail(&self, message: &str) -> Result<()> {
        self.write_state(&format!("Errore — {message}"), true)
    }

    fn write_state(&self, status: &str, failed: bool) -> Result<()> {
        self.check_ownership()?;
        let parent = self
            .path
            .parent()
            .context("Percorso anteprima non valido")?;
        let temporary = tempfile::Builder::new()
            .prefix(".the-converter-preview-")
            .suffix(".lnk")
            .tempfile_in(parent)?;
        let icon_executable = crate::package::packaged_executable()
            .or_else(|| self.icon_executable.clone())
            .unwrap_or_else(|| self.executable.clone());
        native::save_link(
            temporary.path(),
            &self.executable,
            &icon_executable,
            &self.job_id,
            status,
            &self.output,
            failed,
        )?;
        temporary
            .persist(&self.path)
            .map_err(|error| error.error)
            .context("Impossibile aggiornare lo stato della conversione in Esplora file")?;
        native::notify(&self.path, native::UPDATE);
        Ok(())
    }

    /// Removes only a preview that still points at this exact app and queue job.
    pub fn remove(&self) -> Result<()> {
        if !self.path.exists() {
            return Ok(());
        }
        self.check_ownership()?;
        fs::remove_file(&self.path)
            .context("Impossibile rimuovere l'anteprima della conversione")?;
        native::notify(&self.path, native::DELETE);
        Ok(())
    }

    fn check_ownership(&self) -> Result<()> {
        let details = native::read_link(&self.path)?;
        if !same_path(&details.executable, &self.executable)
            || details.arguments != format!("--queue-job {}", self.job_id)
        {
            bail!(
                "L'anteprima è stata modificata da un altro programma: {}",
                self.path.display()
            );
        }
        Ok(())
    }
}

/// Ask Explorer to replace a preview icon with the completed format's normal icon.
pub fn notify_ready(output: &Path) {
    native::notify(output, native::CREATE);
}

fn validate_job_id(job_id: &str) -> Result<()> {
    if job_id.is_empty()
        || job_id.len() > 128
        || !job_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        bail!("Identificatore della conversione non valido");
    }
    Ok(())
}

fn same_path(left: &Path, right: &Path) -> bool {
    let left = display_path(left);
    let right = display_path(right);
    if cfg!(windows) {
        left.eq_ignore_ascii_case(&right)
    } else {
        left == right
    }
}

fn display_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        value.trim_start_matches(r"\\?\").to_owned()
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use std::{ffi::c_void, ptr};

    pub const CREATE: i32 = 0x0000_0002;
    pub const DELETE: i32 = 0x0000_0004;
    pub const UPDATE: i32 = 0x0000_2000;
    const RPC_E_CHANGED_MODE: i32 = 0x80010106u32 as i32;
    const SLGP_RAWPATH: u32 = 0x0000_0004;

    #[repr(C)]
    struct Guid {
        a: u32,
        b: u16,
        c: u16,
        d: [u8; 8],
    }
    const CLSID_SHELL_LINK: Guid = Guid {
        a: 0x00021401,
        b: 0,
        c: 0,
        d: [0xc0, 0, 0, 0, 0, 0, 0, 0x46],
    };
    const IID_SHELL_LINK: Guid = Guid {
        a: 0x000214f9,
        b: 0,
        c: 0,
        d: [0xc0, 0, 0, 0, 0, 0, 0, 0x46],
    };
    const IID_PERSIST_FILE: Guid = Guid {
        a: 0x0000010b,
        b: 0,
        c: 0,
        d: [0xc0, 0, 0, 0, 0, 0, 0, 0x46],
    };

    #[link(name = "ole32")]
    unsafe extern "system" {
        fn CoInitializeEx(reserved: *mut c_void, mode: u32) -> i32;
        fn CoUninitialize();
        fn CoCreateInstance(
            class: *const Guid,
            outer: *mut c_void,
            context: u32,
            iid: *const Guid,
            out: *mut *mut c_void,
        ) -> i32;
    }
    #[link(name = "shell32")]
    unsafe extern "system" {
        fn SHChangeNotify(event: i32, flags: u32, item1: *const c_void, item2: *const c_void);
    }

    #[repr(C)]
    struct Unknown {
        query: unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> i32,
        add: unsafe extern "system" fn(*mut c_void) -> u32,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
    }
    #[repr(C)]
    struct ShellLink {
        unknown: Unknown,
        get_path: unsafe extern "system" fn(*mut c_void, *mut u16, i32, *mut c_void, u32) -> i32,
        get_id_list: *const c_void,
        set_id_list: *const c_void,
        get_description: unsafe extern "system" fn(*mut c_void, *mut u16, i32) -> i32,
        set_description: unsafe extern "system" fn(*mut c_void, *const u16) -> i32,
        get_working_directory: *const c_void,
        set_working_directory: unsafe extern "system" fn(*mut c_void, *const u16) -> i32,
        get_arguments: unsafe extern "system" fn(*mut c_void, *mut u16, i32) -> i32,
        set_arguments: unsafe extern "system" fn(*mut c_void, *const u16) -> i32,
        get_hotkey: *const c_void,
        set_hotkey: *const c_void,
        get_show_command: *const c_void,
        set_show_command: unsafe extern "system" fn(*mut c_void, i32) -> i32,
        get_icon: unsafe extern "system" fn(*mut c_void, *mut u16, i32, *mut i32) -> i32,
        set_icon: unsafe extern "system" fn(*mut c_void, *const u16, i32) -> i32,
        set_relative_path: *const c_void,
        resolve: *const c_void,
        set_path: unsafe extern "system" fn(*mut c_void, *const u16) -> i32,
    }
    #[repr(C)]
    struct PersistFile {
        unknown: Unknown,
        class_id: *const c_void,
        is_dirty: *const c_void,
        load: unsafe extern "system" fn(*mut c_void, *const u16, u32) -> i32,
        save: unsafe extern "system" fn(*mut c_void, *const u16, i32) -> i32,
        completed: *const c_void,
        current_file: *const c_void,
    }

    struct Apartment(bool);
    impl Apartment {
        fn enter() -> Result<Self> {
            let result = unsafe { CoInitializeEx(ptr::null_mut(), 2) };
            if result < 0 && result != RPC_E_CHANGED_MODE {
                check(result, "Inizializzazione COM")?;
            }
            Ok(Self(result >= 0))
        }
    }
    impl Drop for Apartment {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() }
            }
        }
    }
    struct Object(*mut c_void);
    impl Object {
        fn vtable<T>(&self) -> &T {
            unsafe { &**(self.0 as *const *const T) }
        }
        fn query(&self, iid: &Guid) -> Result<Self> {
            let mut value = ptr::null_mut();
            check(
                unsafe { (self.vtable::<Unknown>().query)(self.0, iid, &mut value) },
                "Interfaccia COM collegamento",
            )?;
            if value.is_null() {
                bail!("Interfaccia COM collegamento vuota");
            }
            Ok(Self(value))
        }
    }
    impl Drop for Object {
        fn drop(&mut self) {
            unsafe {
                (self.vtable::<Unknown>().release)(self.0);
            }
        }
    }

    fn check(status: i32, operation: &str) -> Result<()> {
        if status < 0 {
            bail!("{operation}: errore Windows 0x{:08X}", status as u32);
        }
        Ok(())
    }
    fn wide(value: &str) -> Result<Vec<u16>> {
        if value.contains('\0') {
            bail!("Il testo contiene un carattere nullo");
        }
        Ok(value.encode_utf16().chain(Some(0)).collect())
    }
    fn wide_path(value: &Path) -> Result<Vec<u16>> {
        wide(&display_path(value))
    }
    fn link_object() -> Result<Object> {
        let mut raw = ptr::null_mut();
        check(
            unsafe {
                CoCreateInstance(
                    &CLSID_SHELL_LINK,
                    ptr::null_mut(),
                    1,
                    &IID_SHELL_LINK,
                    &mut raw,
                )
            },
            "Creazione collegamento Esplora file",
        )?;
        if raw.is_null() {
            bail!("Collegamento Esplora file non disponibile");
        }
        Ok(Object(raw))
    }

    pub fn save_link(
        path: &Path,
        executable: &Path,
        icon_executable: &Path,
        job_id: &str,
        status: &str,
        output: &Path,
        failed: bool,
    ) -> Result<()> {
        let _apartment = Apartment::enter()?;
        let link = link_object()?;
        let vtable = link.vtable::<ShellLink>();
        let target = wide_path(executable)?;
        let icon = wide_path(icon_executable)?;
        let args = wide(&format!("--queue-job {job_id}"))?;
        // Current Explorer truncates persisted link descriptions at MAX_PATH.
        // Keep below that boundary, preserve complete Unicode characters, and
        // tolerate control bytes in a converter's diagnostic text.
        let mut description_text = String::new();
        let mut units = 0;
        for character in format!(
            "The Converter — {status}\nDestinazione: {}",
            display_path(output)
        )
        .chars()
        .filter(|&character| character != '\0')
        {
            units += character.len_utf16();
            if units > 240 {
                break;
            }
            description_text.push(character);
        }
        let description = wide(&description_text)?;
        let file = wide_path(path)?;
        unsafe {
            check(
                (vtable.set_path)(link.0, target.as_ptr()),
                "Destinazione collegamento",
            )?;
            check(
                (vtable.set_arguments)(link.0, args.as_ptr()),
                "Argomenti collegamento",
            )?;
            check(
                (vtable.set_description)(link.0, description.as_ptr()),
                "Stato collegamento",
            )?;
            check(
                (vtable.set_icon)(link.0, icon.as_ptr(), if failed { -3 } else { -2 }),
                "Icona collegamento",
            )?;
            check(
                (vtable.set_show_command)(link.0, 1),
                "Finestra collegamento",
            )?;
            if let Some(parent) = icon_executable.parent() {
                let directory = wide_path(parent)?;
                check(
                    (vtable.set_working_directory)(link.0, directory.as_ptr()),
                    "Cartella collegamento",
                )?;
            }
            let persist = link.query(&IID_PERSIST_FILE)?;
            check(
                (persist.vtable::<PersistFile>().save)(persist.0, file.as_ptr(), 1),
                "Salvataggio collegamento",
            )?;
        }
        Ok(())
    }

    pub struct Details {
        pub executable: PathBuf,
        pub arguments: String,
        #[cfg(test)]
        pub description: String,
        #[cfg(test)]
        pub icon: PathBuf,
        #[cfg(test)]
        pub icon_index: i32,
    }
    pub fn read_link(path: &Path) -> Result<Details> {
        let _apartment = Apartment::enter()?;
        let link = link_object()?;
        let persist = link.query(&IID_PERSIST_FILE)?;
        let path = wide_path(path)?;
        check(
            unsafe { (persist.vtable::<PersistFile>().load)(persist.0, path.as_ptr(), 0) },
            "Lettura anteprima conversione",
        )?;
        let mut target = vec![0u16; 32768];
        let mut args = vec![0u16; 32768];
        let vtable = link.vtable::<ShellLink>();
        check(
            unsafe {
                (vtable.get_path)(
                    link.0,
                    target.as_mut_ptr(),
                    target.len() as i32,
                    ptr::null_mut(),
                    // Preserve an app execution alias as stored. Resolving it here
                    // would turn a stable preview back into a versioned package path.
                    SLGP_RAWPATH,
                )
            },
            "Lettura destinazione anteprima",
        )?;
        check(
            unsafe { (vtable.get_arguments)(link.0, args.as_mut_ptr(), args.len() as i32) },
            "Lettura conversione anteprima",
        )?;
        #[cfg(test)]
        let description = {
            let mut data = vec![0u16; 2048];
            check(
                unsafe { (vtable.get_description)(link.0, data.as_mut_ptr(), data.len() as i32) },
                "Lettura stato",
            )?;
            from_wide(&data)
        };
        #[cfg(test)]
        let (icon, icon_index) = {
            let mut data = vec![0u16; 32768];
            let mut index = 0;
            check(
                unsafe {
                    (vtable.get_icon)(link.0, data.as_mut_ptr(), data.len() as i32, &mut index)
                },
                "Lettura icona",
            )?;
            (PathBuf::from(from_wide(&data)), index)
        };
        Ok(Details {
            executable: PathBuf::from(from_wide(&target)),
            arguments: from_wide(&args),
            #[cfg(test)]
            description,
            #[cfg(test)]
            icon,
            #[cfg(test)]
            icon_index,
        })
    }
    fn from_wide(value: &[u16]) -> String {
        String::from_utf16_lossy(
            &value[..value.iter().position(|v| *v == 0).unwrap_or(value.len())],
        )
    }

    pub fn notify(path: &Path, event: i32) {
        if let Ok(path) = wide_path(path) {
            // PATHW | FLUSHNOWAIT: refresh this item without blocking Explorer's UI.
            unsafe {
                SHChangeNotify(event, 0x0005 | 0x2000, path.as_ptr().cast(), ptr::null());
            }
        }
    }
}

#[cfg(not(windows))]
mod native {
    use super::*;
    pub const CREATE: i32 = 2;
    pub const DELETE: i32 = 4;
    pub const UPDATE: i32 = 0x2000;
    pub struct Details {
        pub executable: PathBuf,
        pub arguments: String,
    }
    pub fn save_link(
        _: &Path,
        _: &Path,
        _: &Path,
        _: &str,
        _: &str,
        _: &Path,
        _: bool,
    ) -> Result<()> {
        bail!("Le anteprime di Esplora file richiedono Windows")
    }
    pub fn read_link(_: &Path) -> Result<Details> {
        bail!("Le anteprime di Esplora file richiedono Windows")
    }
    pub fn notify(_: &Path, _: i32) {}
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    fn shell_icon_colours(path: &Path) -> (usize, usize) {
        use std::{ffi::c_void, ptr};
        #[repr(C)]
        struct FileInfo {
            icon: *mut c_void,
            index: i32,
            attributes: u32,
            name: [u16; 260],
            kind: [u16; 80],
        }
        #[repr(C)]
        struct IconInfo {
            is_icon: i32,
            x_hotspot: u32,
            y_hotspot: u32,
            mask: *mut c_void,
            colour: *mut c_void,
        }
        #[repr(C)]
        struct BitmapInfo {
            size: u32,
            width: i32,
            height: i32,
            planes: u16,
            bits: u16,
            compression: u32,
            image_size: u32,
            x_resolution: i32,
            y_resolution: i32,
            colours: u32,
            important_colours: u32,
            palette: [u32; 1],
        }
        #[link(name = "shell32")]
        unsafe extern "system" {
            fn SHGetFileInfoW(
                path: *const u16,
                attrs: u32,
                info: *mut FileInfo,
                size: u32,
                flags: u32,
            ) -> usize;
        }
        #[link(name = "user32")]
        unsafe extern "system" {
            fn GetIconInfo(icon: *mut c_void, info: *mut IconInfo) -> i32;
            fn DestroyIcon(icon: *mut c_void) -> i32;
            fn GetDC(window: *mut c_void) -> *mut c_void;
            fn ReleaseDC(window: *mut c_void, context: *mut c_void) -> i32;
        }
        #[link(name = "gdi32")]
        unsafe extern "system" {
            fn GetDIBits(
                context: *mut c_void,
                bitmap: *mut c_void,
                start: u32,
                lines: u32,
                pixels: *mut c_void,
                info: *mut BitmapInfo,
                usage: u32,
            ) -> i32;
            fn DeleteObject(object: *mut c_void) -> i32;
        }
        #[link(name = "ole32")]
        unsafe extern "system" {
            fn CoInitializeEx(reserved: *mut c_void, flags: u32) -> i32;
            fn CoUninitialize();
        }

        let path: Vec<u16> = display_path(path).encode_utf16().chain(Some(0)).collect();
        let mut file: FileInfo = unsafe { std::mem::zeroed() };
        let mut icon: IconInfo = unsafe { std::mem::zeroed() };
        let mut bitmap: BitmapInfo = unsafe { std::mem::zeroed() };
        bitmap.size = 40;
        bitmap.width = 16;
        bitmap.height = -16;
        bitmap.planes = 1;
        bitmap.bits = 32;
        let mut pixels = [0u8; 16 * 16 * 4];
        let (lookup, icon_ok, rows) = unsafe {
            let apartment = CoInitializeEx(ptr::null_mut(), 2);
            let lookup = SHGetFileInfoW(
                path.as_ptr(),
                0,
                &mut file,
                std::mem::size_of::<FileInfo>() as u32,
                0x100 | 0x1 | 0x8000,
            );
            let icon_ok = GetIconInfo(file.icon, &mut icon);
            let context = GetDC(ptr::null_mut());
            let rows = GetDIBits(
                context,
                icon.colour,
                0,
                16,
                pixels.as_mut_ptr().cast(),
                &mut bitmap,
                0,
            );
            ReleaseDC(ptr::null_mut(), context);
            DeleteObject(icon.mask);
            DeleteObject(icon.colour);
            DestroyIcon(file.icon);
            if apartment >= 0 {
                CoUninitialize();
            }
            (lookup, icon_ok, rows)
        };
        assert_ne!(lookup, 0, "Explorer could not extract the shortcut icon");
        assert_ne!(icon_ok, 0);
        assert_eq!(rows, 16);
        let blue = pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[0] > 120 && p[1] > 40 && p[1] < 160 && p[2] < 70)
            .count();
        let red = pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[2] > 120 && p[1] < 100 && p[0] < 90)
            .count();
        (blue, red)
    }

    #[test]
    fn native_shortcut_preserves_unicode_status_icon_and_never_clobbers() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("convertitore città & mare.exe");
        fs::copy(std::env::current_exe().unwrap(), &exe).unwrap();
        let output = dir.path().join("musica città & mare.mp3");
        let pending = PendingLink::create(&output, &exe, "job-123", "In coda").unwrap();
        assert!(!output.exists());
        assert!(pending.path().exists());
        let first = fs::read(pending.path()).unwrap();
        let read = native::read_link(pending.path()).unwrap();
        assert!(same_path(&read.executable, &exe));
        assert_eq!(read.arguments, "--queue-job job-123");
        assert!(read.description.contains("In coda"));
        assert!(read.description.contains("città & mare"));
        assert!(read.icon.is_file());
        assert!(same_path(&read.icon, &exe));
        assert_eq!(read.icon_index, -2);
        let (blue, red) = shell_icon_colours(pending.path());
        assert!(
            blue > 40,
            "loading icon must remain visible at Explorer's 16px size"
        );
        assert!(red < 10);
        assert!(PendingLink::create(&output, &exe, "another-job", "In coda").is_err());
        assert_eq!(fs::read(pending.path()).unwrap(), first);
        pending.update("Conversione: 42%").unwrap();
        assert!(
            native::read_link(pending.path())
                .unwrap()
                .description
                .contains("42%")
        );
        pending.fail("File danneggiato").unwrap();
        let failed = native::read_link(pending.path()).unwrap();
        assert!(failed.description.contains("Errore — File danneggiato"));
        assert!(same_path(&failed.icon, &exe));
        assert_eq!(failed.icon_index, -3);
        let (blue, red) = shell_icon_colours(pending.path());
        assert!(red > 40, "failed job icon must be visibly red at 16px");
        assert!(blue < 30);
        pending.update("Nuovo tentativo").unwrap();
        assert_eq!(native::read_link(pending.path()).unwrap().icon, read.icon);
        assert_eq!(native::read_link(pending.path()).unwrap().icon_index, -2);
        pending
            .fail(&format!("Errore\0{}", "🎵".repeat(800)))
            .unwrap();
        let diagnostic = native::read_link(pending.path()).unwrap().description;
        assert!(!diagnostic.contains('\0'));
        assert!(!diagnostic.contains('\u{fffd}'));
        assert!(diagnostic.encode_utf16().count() <= 240);
        pending.remove().unwrap();
        assert!(!pending.path().exists());
        pending.remove().unwrap();
        fs::write(&output, b"completed data").unwrap();
        assert!(PendingLink::create(&output, &exe, "job-124", "In coda").is_err());
        assert_eq!(fs::read(output).unwrap(), b"completed data");
    }

    #[test]
    fn modified_shortcut_and_unsafe_job_ids_are_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let exe = std::env::current_exe().unwrap();
        let output = dir.path().join("file.wav");
        assert!(PendingLink::create(&output, &exe, "job --other-command", "In coda").is_err());
        let pending = PendingLink::create(&output, &exe, "job-ownership", "In coda").unwrap();
        native::save_link(
            pending.path(),
            &exe,
            &exe,
            "different-job",
            "Altro lavoro",
            &output,
            false,
        )
        .unwrap();
        let changed = fs::read(pending.path()).unwrap();
        assert!(pending.update("50%").is_err());
        assert!(pending.remove().is_err());
        assert_eq!(fs::read(pending.path()).unwrap(), changed);
    }

    #[test]
    fn legacy_queue_records_without_a_separate_icon_binary_still_update_and_remove() {
        let dir = tempfile::tempdir().unwrap();
        let exe = std::env::current_exe().unwrap();
        let output = dir.path().join("legacy-output.mp3");
        let created = PendingLink::create(&output, &exe, "job-legacy", "In coda").unwrap();
        let legacy = serde_json::json!({
            "output": output,
            "path": created.path(),
            "executable": exe,
            "job_id": "job-legacy",
        });
        let pending: PendingLink = serde_json::from_value(legacy).unwrap();
        assert!(pending.icon_executable.is_none());
        pending.update("Conversione: 25%").unwrap();
        let read = native::read_link(pending.path()).unwrap();
        assert!(same_path(&read.executable, &exe));
        assert_eq!(read.arguments, "--queue-job job-legacy");
        pending.remove().unwrap();
        assert!(!pending.path().exists());
    }

    #[test]
    fn stable_alias_raw_target_and_ownership_survive_a_change_of_icon_binary() {
        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join("utente città & mare");
        let alias = user.join("Microsoft/WindowsApps/corsivo-the-converter-preview.exe");
        fs::create_dir_all(alias.parent().unwrap()).unwrap();
        // A local fixture exercises ShellLink's raw-path storage without changing
        // a real Windows alias, package registration or per-user settings.
        fs::write(&alias, []).unwrap();
        let alias = alias.canonicalize().unwrap();
        let old_icon = dir.path().join("package-v1.exe");
        let new_icon = dir.path().join("package-v2.exe");
        fs::copy(std::env::current_exe().unwrap(), &old_icon).unwrap();
        fs::copy(std::env::current_exe().unwrap(), &new_icon).unwrap();
        let output = user.join("video città & mare.mp4");
        let pending = PendingLink {
            output: output.clone(),
            path: PendingLink::path_for(&output),
            executable: alias.clone(),
            icon_executable: Some(old_icon.clone()),
            job_id: "job-stable-alias".into(),
        };
        native::save_link(
            pending.path(),
            &alias,
            &old_icon,
            &pending.job_id,
            "In coda",
            &output,
            false,
        )
        .unwrap();
        let stored = serde_json::to_vec(&pending).unwrap();
        let restored: PendingLink = serde_json::from_slice(&stored).unwrap();
        restored.check_ownership().unwrap();
        let read = native::read_link(restored.path()).unwrap();
        assert!(same_path(&read.executable, &alias));
        assert_eq!(read.arguments, "--queue-job job-stable-alias");
        assert!(same_path(&read.icon, &old_icon));
        native::save_link(
            restored.path(),
            &alias,
            &new_icon,
            &restored.job_id,
            "Nuova versione",
            &output,
            true,
        )
        .unwrap();
        restored.check_ownership().unwrap();
        let updated = native::read_link(restored.path()).unwrap();
        assert!(same_path(&updated.executable, &alias));
        assert!(same_path(&updated.icon, &new_icon));
        assert_eq!(updated.icon_index, -3);
        restored.remove().unwrap();
        assert!(!restored.path().exists());
    }
}
