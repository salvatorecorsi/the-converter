//! Package-aware activation paths for Explorer previews. This module only reads
//! signed package metadata and the user's existing app execution alias.
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct RuntimeConfiguration {
    execution_alias: String,
}

/// An MSIX update changes the package installation path. The app execution alias
/// remains at the same user path, so new previews can still open after an update.
/// If the user disabled the alias, keep the currently installed EXE as a fallback.
pub fn preview_target(executable: &Path) -> PathBuf {
    let Some(current) = packaged_executable() else {
        return executable.to_owned();
    };
    if !same_path(executable, &current) {
        return executable.to_owned();
    }
    let Some(parent) = current.parent() else {
        return executable.to_owned();
    };
    let Some(local_app_data) =
        directories::BaseDirs::new().map(|directories| directories.data_local_dir().to_owned())
    else {
        return executable.to_owned();
    };
    configured_alias(parent, &local_app_data).unwrap_or_else(|| executable.to_owned())
}

/// The real current binary provides PE icon resources; aliases are not icon files.
pub fn packaged_executable() -> Option<PathBuf> {
    let package_root = current_package_path()?;
    let executable = std::env::current_exe().ok().filter(|path| path.is_file())?;
    // A desktop tool can inherit another app's package identity. Only our binary
    // installed at the package root, with our runtime marker, is a Store runtime.
    if !belongs_to_package(&executable, &package_root) {
        return None;
    }
    runtime_alias_name(executable.parent()?)?;
    Some(executable)
}

fn configured_alias(application_dir: &Path, local_app_data: &Path) -> Option<PathBuf> {
    let name = runtime_alias_name(application_dir)?;
    let alias = alias_path(local_app_data, &name)?;
    // App aliases are APPEXECLINK reparse points, not conventional executable files.
    // Inspect the entry itself instead of resolving it as an ordinary file/symlink.
    let metadata = std::fs::symlink_metadata(&alias).ok()?;
    if metadata.is_dir() {
        return None;
    }
    Some(alias)
}

fn runtime_alias_name(application_dir: &Path) -> Option<String> {
    let configuration_path = application_dir.join("store-runtime.json");
    if std::fs::metadata(&configuration_path).ok()?.len() > 4096 {
        return None;
    }
    let bytes = std::fs::read(configuration_path).ok()?;
    let configuration: RuntimeConfiguration = serde_json::from_slice(&bytes).ok()?;
    if !is_supported_alias_name(&configuration.execution_alias) {
        return None;
    }
    Some(configuration.execution_alias)
}

fn alias_path(local_app_data: &Path, alias: &str) -> Option<PathBuf> {
    if !local_app_data.is_absolute() || !is_supported_alias_name(alias) {
        return None;
    }
    Some(local_app_data.join("Microsoft/WindowsApps").join(alias))
}

fn is_supported_alias_name(alias: &str) -> bool {
    matches!(
        alias,
        "corsivo-the-converter.exe" | "corsivo-the-converter-preview.exe"
    ) && Path::new(alias).file_name().and_then(|name| name.to_str()) == Some(alias)
}

fn belongs_to_package(executable: &Path, package_root: &Path) -> bool {
    let Some(parent) = executable
        .parent()
        .and_then(|path| path.canonicalize().ok())
    else {
        return false;
    };
    let Ok(package_root) = package_root.canonicalize() else {
        return false;
    };
    same_path(&parent, &package_root)
}

fn same_path(left: &Path, right: &Path) -> bool {
    let left = left.to_string_lossy();
    let right = right.to_string_lossy();
    if cfg!(windows) {
        left.trim_start_matches(r"\\?\")
            .eq_ignore_ascii_case(right.trim_start_matches(r"\\?\"))
    } else {
        left == right
    }
}

#[cfg(windows)]
fn current_package_path() -> Option<PathBuf> {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentPackageFullName(length: *mut u32, full_name: *mut u16) -> i32;
        fn GetCurrentPackagePath(length: *mut u32, path: *mut u16) -> i32;
    }
    let mut length = 0;
    // A null buffer is the documented size query. Unpackaged processes return
    // APPMODEL_ERROR_NO_PACKAGE; packaged processes request a larger buffer.
    if unsafe { GetCurrentPackageFullName(&mut length, std::ptr::null_mut()) } != 122 || length == 0
    {
        return None;
    }
    length = 0;
    if unsafe { GetCurrentPackagePath(&mut length, std::ptr::null_mut()) } != 122
        || !(1..=32768).contains(&length)
    {
        return None;
    }
    let mut path = vec![0u16; length as usize];
    if unsafe { GetCurrentPackagePath(&mut length, path.as_mut_ptr()) } != 0 {
        return None;
    }
    let end = path.iter().position(|value| *value == 0)?;
    Some(PathBuf::from(OsString::from_wide(&path[..end])))
}

#[cfg(not(windows))]
fn current_package_path() -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inherited_identity_does_not_make_an_external_tool_a_packaged_binary() {
        let root = tempfile::tempdir().unwrap();
        let package = root.path().join("WindowsApps/OtherPackage_1.0.0.0");
        let external = root.path().join("workspace");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::create_dir_all(&external).unwrap();
        let external_executable = external.join("the-converter.exe");
        std::fs::write(&external_executable, []).unwrap();
        assert!(!belongs_to_package(&external_executable, &package));
        let installed = package.join("the-converter.exe");
        std::fs::write(&installed, []).unwrap();
        assert!(belongs_to_package(&installed, &package));
        assert!(belongs_to_package(
            &installed.canonicalize().unwrap(),
            &package
        ));
        assert!(!belongs_to_package(
            &installed,
            &root.path().join("missing-package")
        ));
        assert!(runtime_alias_name(&package).is_none());
        std::fs::write(
            package.join("store-runtime.json"),
            br#"{"execution_alias":"other-app.exe"}"#,
        )
        .unwrap();
        assert!(runtime_alias_name(&package).is_none());
        std::fs::write(
            package.join("store-runtime.json"),
            br#"{"execution_alias":"corsivo-the-converter-preview.exe"}"#,
        )
        .unwrap();
        assert_eq!(
            runtime_alias_name(&package).as_deref(),
            Some("corsivo-the-converter-preview.exe")
        );
    }

    #[test]
    fn runtime_configuration_only_accepts_the_declared_alias_basenames() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(
            alias_path(root.path(), "corsivo-the-converter.exe"),
            Some(
                root.path()
                    .join("Microsoft/WindowsApps/corsivo-the-converter.exe")
            )
        );
        assert!(alias_path(root.path(), "corsivo-the-converter-preview.exe").is_some());
        for alias in [
            "../corsivo-the-converter.exe",
            "..\\corsivo-the-converter.exe",
            "C:\\elsewhere.exe",
            "\\\\server\\tool.exe",
            "corsivo-the-converter.exe --queue-job other",
            "other.exe",
            "corsivo-the-converter.exe\0",
        ] {
            assert!(
                alias_path(root.path(), alias).is_none(),
                "Accepted invalid alias: {alias:?}"
            );
        }
        assert!(alias_path(Path::new("relative"), "corsivo-the-converter.exe").is_none());
    }

    #[test]
    fn alias_requires_valid_configuration_and_an_existing_enabled_entry() {
        let root = tempfile::tempdir().unwrap();
        let app = root.path().join("package-version-1");
        let user = root.path().join("user");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::write(
            app.join("store-runtime.json"),
            br#"{"execution_alias":"corsivo-the-converter-preview.exe"}"#,
        )
        .unwrap();
        assert!(configured_alias(&app, &user).is_none());
        let alias = user.join("Microsoft/WindowsApps/corsivo-the-converter-preview.exe");
        std::fs::create_dir_all(alias.parent().unwrap()).unwrap();
        std::fs::write(&alias, []).unwrap();
        assert_eq!(configured_alias(&app, &user), Some(alias.clone()));
        std::fs::write(app.join("store-runtime.json"), b"invalid json").unwrap();
        assert!(configured_alias(&app, &user).is_none());
        std::fs::write(
            app.join("store-runtime.json"),
            br#"{"execution_alias":"../tool.exe"}"#,
        )
        .unwrap();
        assert!(configured_alias(&app, &user).is_none());
        std::fs::write(app.join("store-runtime.json"), vec![b' '; 4097]).unwrap();
        assert!(configured_alias(&app, &user).is_none());
    }

    #[test]
    fn alias_is_stable_across_package_version_directories() {
        let root = tempfile::tempdir().unwrap();
        let alias = root
            .path()
            .join("Microsoft/WindowsApps/corsivo-the-converter.exe");
        std::fs::create_dir_all(alias.parent().unwrap()).unwrap();
        std::fs::write(&alias, []).unwrap();
        for version in ["package-v1", "package-v2"] {
            let application = root.path().join(version);
            std::fs::create_dir_all(&application).unwrap();
            std::fs::write(
                application.join("store-runtime.json"),
                br#"{"execution_alias":"corsivo-the-converter.exe"}"#,
            )
            .unwrap();
            assert_eq!(
                configured_alias(&application, root.path()),
                Some(alias.clone())
            );
        }
    }
}
