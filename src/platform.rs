//! Open user-facing folders and websites through native OS handlers.
//! Windows never invokes a command processor or PowerShell for these actions.
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

pub fn open_folder(path: &Path) -> Result<()> {
    let folder = validated_folder(path)?;
    #[cfg(windows)]
    {
        windows::open_folder(&folder)
    }
    #[cfg(not(windows))]
    {
        open::that(folder).context("Impossibile aprire la cartella")
    }
}

pub fn open_website(address: &str) -> Result<()> {
    let address = validated_website(address)?;
    #[cfg(windows)]
    {
        windows::open_website(address.as_str())
    }
    #[cfg(not(windows))]
    {
        open::that(address.as_str()).context("Impossibile aprire il sito nel browser")
    }
}

fn validated_folder(path: &Path) -> Result<PathBuf> {
    if !path.is_dir() {
        bail!("La cartella non è disponibile: {}", path.display());
    }
    path.canonicalize()
        .context("Impossibile leggere il percorso della cartella")
}

fn validated_website(address: &str) -> Result<url::Url> {
    let address = url::Url::parse(address).context("Indirizzo del sito non valido")?;
    if !matches!(address.scheme(), "http" | "https") || address.host_str().is_none() {
        bail!("Il sito deve usare un indirizzo HTTP o HTTPS valido");
    }
    Ok(address)
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        ffi::{OsStr, c_void},
        os::windows::ffi::OsStrExt,
        ptr,
    };

    const RPC_E_CHANGED_MODE: i32 = 0x80010106u32 as i32;
    const SEE_MASK_INVOKEIDLIST: u32 = 0x0000_000c;
    const SEE_MASK_NOASYNC: u32 = 0x0000_0100;
    const SEE_MASK_FLAG_NO_UI: u32 = 0x0000_0400;

    #[repr(C)]
    struct ShellExecuteInfo {
        size: u32,
        mask: u32,
        window: *mut c_void,
        verb: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show: i32,
        instance: *mut c_void,
        id_list: *mut c_void,
        class: *const u16,
        class_key: *mut c_void,
        hot_key: u32,
        icon_or_monitor: *mut c_void,
        process: *mut c_void,
    }

    #[link(name = "ole32")]
    unsafe extern "system" {
        fn CoInitializeEx(reserved: *mut c_void, mode: u32) -> i32;
        fn CoUninitialize();
        fn CoTaskMemFree(memory: *mut c_void);
    }
    #[link(name = "shell32")]
    unsafe extern "system" {
        fn SHParseDisplayName(
            name: *const u16,
            context: *mut c_void,
            id_list: *mut *mut c_void,
            attributes: u32,
            attributes_out: *mut u32,
        ) -> i32;
        fn ShellExecuteExW(info: *mut ShellExecuteInfo) -> i32;
    }

    struct Apartment(bool);
    impl Apartment {
        fn enter() -> Result<Self> {
            let status = unsafe { CoInitializeEx(ptr::null_mut(), 2 | 4) };
            if status < 0 && status != RPC_E_CHANGED_MODE {
                bail!(
                    "Apertura tramite Windows non disponibile (0x{:08X})",
                    status as u32
                );
            }
            Ok(Self(status >= 0))
        }
    }
    impl Drop for Apartment {
        fn drop(&mut self) {
            if self.0 {
                unsafe {
                    CoUninitialize();
                }
            }
        }
    }
    struct ItemIdList(*mut c_void);
    impl Drop for ItemIdList {
        fn drop(&mut self) {
            unsafe {
                CoTaskMemFree(self.0);
            }
        }
    }

    pub(super) fn open_folder(folder: &Path) -> Result<()> {
        let _apartment = Apartment::enter()?;
        let name = shell_path_argument(folder.as_os_str())?;
        let mut raw = ptr::null_mut();
        let status = unsafe {
            SHParseDisplayName(name.as_ptr(), ptr::null_mut(), &mut raw, 0, ptr::null_mut())
        };
        let item = ItemIdList(raw);
        if status < 0 || item.0.is_null() {
            bail!(
                "Impossibile aprire la cartella in Esplora file (0x{:08X})",
                status as u32
            );
        }
        let verb = wide(OsStr::new("open"))?;
        let mut info: ShellExecuteInfo = unsafe { std::mem::zeroed() };
        info.size = std::mem::size_of::<ShellExecuteInfo>() as u32;
        info.mask = SEE_MASK_INVOKEIDLIST | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI;
        info.verb = verb.as_ptr();
        info.id_list = item.0;
        info.show = 1;
        // PIDLs keep long and UNC folder paths out of command-line parsing.
        execute(&mut info).context("Impossibile aprire la cartella in Esplora file")
    }

    pub(super) fn open_website(address: &str) -> Result<()> {
        let _apartment = Apartment::enter()?;
        let address = wide(OsStr::new(address))?;
        let verb = wide(OsStr::new("open"))?;
        let mut info: ShellExecuteInfo = unsafe { std::mem::zeroed() };
        info.size = std::mem::size_of::<ShellExecuteInfo>() as u32;
        info.mask = SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI;
        info.verb = verb.as_ptr();
        info.file = address.as_ptr();
        info.show = 1;
        execute(&mut info).context("Impossibile aprire il sito nel browser predefinito")
    }

    fn execute(info: &mut ShellExecuteInfo) -> Result<()> {
        if unsafe { ShellExecuteExW(info) } == 0 {
            return Err(std::io::Error::last_os_error())
                .context("Windows non ha aperto la destinazione");
        }
        Ok(())
    }

    fn wide(value: &OsStr) -> Result<Vec<u16>> {
        let mut units: Vec<u16> = value.encode_wide().collect();
        if units.contains(&0) {
            bail!("Il percorso contiene un carattere nullo");
        }
        units.push(0);
        Ok(units)
    }

    pub(super) fn shell_path_argument(path: &OsStr) -> Result<Vec<u16>> {
        let mut path: Vec<u16> = path
            .encode_wide()
            .map(|unit| {
                if unit == b'/' as u16 {
                    b'\\' as u16
                } else {
                    unit
                }
            })
            .collect();
        let extended_unc: Vec<u16> = r"\\?\UNC\".encode_utf16().collect();
        let extended_drive: Vec<u16> = r"\\?\".encode_utf16().collect();
        if path.starts_with(&extended_unc) {
            let mut unc = vec![b'\\' as u16, b'\\' as u16];
            unc.extend_from_slice(&path[extended_unc.len()..]);
            path = unc;
        } else if path.starts_with(&extended_drive) {
            path.drain(..extended_drive.len());
        }
        if path.contains(&0) {
            bail!("Il percorso contiene un carattere nullo");
        }
        path.push(0);
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn website_validation_rejects_non_web_handlers_without_launching_them() {
        for address in [
            "file:///C:/Windows",
            "javascript:alert(1)",
            "powershell:example",
            "cmd:example",
            "mailto:test@example.com",
            "not a URL",
        ] {
            assert!(open_website(address).is_err(), "Accepted {address:?}");
        }
        for address in [
            "https://ffmpeg.org/download.html",
            "https://poppler.freedesktop.org/",
            "http://example.com/help",
        ] {
            assert!(validated_website(address).is_ok());
        }
    }

    #[test]
    fn only_existing_folders_can_be_opened() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("example.txt");
        std::fs::write(&file, b"example").unwrap();
        assert!(open_folder(&file).is_err());
        assert!(open_folder(&directory.path().join("missing")).is_err());
        assert_eq!(
            validated_folder(directory.path()).unwrap(),
            directory.path().canonicalize().unwrap()
        );
    }

    #[cfg(windows)]
    #[test]
    fn folder_arguments_preserve_long_unc_paths_and_utf16_units() {
        use std::{
            ffi::{OsStr, OsString},
            os::windows::ffi::OsStringExt,
        };
        for (input, expected) in [
            (r"\\?\C:\Users\città & mare", r"C:\Users\città & mare"),
            (
                r"\\?\UNC\server\share\città & mare",
                r"\\server\share\città & mare",
            ),
        ] {
            let actual = windows::shell_path_argument(OsStr::new(input)).unwrap();
            let expected: Vec<u16> = expected.encode_utf16().chain(Some(0)).collect();
            assert_eq!(actual, expected);
        }
        let mut path: Vec<u16> = r"\\?\C:\".encode_utf16().collect();
        path.push(0xd800);
        let actual = windows::shell_path_argument(&OsString::from_wide(&path)).unwrap();
        assert_eq!(actual, [b'C' as u16, b':' as u16, b'\\' as u16, 0xd800, 0]);
        assert!(windows::shell_path_argument(&OsString::from_wide(&[b'C' as u16, 0])).is_err());
    }
}
