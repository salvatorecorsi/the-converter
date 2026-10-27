use crate::model::{MediaKind, OutputFormat};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolPaths {
    pub ffmpeg: Option<PathBuf>,
    pub ffprobe: Option<PathBuf>,
    pub pdftoppm: Option<PathBuf>,
    pub pdftotext: Option<PathBuf>,
    pub soffice: Option<PathBuf>,
}

impl ToolPaths {
    pub fn discover() -> Self {
        Self {
            ffmpeg: find_tool("ffmpeg"),
            ffprobe: find_tool("ffprobe"),
            pdftoppm: find_tool("pdftoppm"),
            pdftotext: find_tool("pdftotext"),
            soffice: find_tool("soffice"),
        }
    }
    /// Apply explicit user selections after automatic discovery. Companion tools
    /// are taken from the same installation unless the user selected them separately.
    pub fn configured(overrides: &BTreeMap<String, PathBuf>) -> Self {
        let mut tools = Self::discover();
        tools.apply_overrides(overrides);
        tools
    }

    fn apply_overrides(&mut self, overrides: &BTreeMap<String, PathBuf>) {
        for (key, path) in overrides {
            self.set(key, path.clone());
        }
        for (key, path) in overrides {
            let companion = match key.as_str() {
                "ffmpeg" => "ffprobe",
                "ffprobe" => "ffmpeg",
                "pdftoppm" => "pdftotext",
                "pdftotext" => "pdftoppm",
                _ => continue,
            };
            if overrides.contains_key(companion) {
                continue;
            }
            if let Some(parent) = path.parent() {
                let executable = parent.join(executable_name(companion));
                if executable_available(&executable) {
                    self.set(companion, executable);
                }
            }
        }
    }

    fn set(&mut self, key: &str, path: PathBuf) {
        let path = executable_available(&path).then_some(path);
        match key {
            "ffmpeg" => self.ffmpeg = path,
            "ffprobe" => self.ffprobe = path,
            "pdftoppm" => self.pdftoppm = path,
            "pdftotext" => self.pdftotext = path,
            "soffice" => self.soffice = path,
            _ => {}
        }
    }
    pub fn missing_for(&self, kind: MediaKind, format: OutputFormat) -> Option<String> {
        let missing = match kind {
            MediaKind::Audio if format == OutputFormat::Txt => {
                return (!crate::transcription::has_api_key()).then(|| "Per trascrivere con OpenAI, aggiungi la tua chiave API OpenAI in Strumenti. L'audio verrà inviato a OpenAI.".to_owned());
            }
            MediaKind::Audio | MediaKind::Video if !tool_exists(&self.ffmpeg) => Some(
                "Per convertire audio e video serve FFmpeg. Apri Strumenti e seleziona ffmpeg.exe già installato sul PC.",
            ),
            MediaKind::Pdf if format == OutputFormat::Txt && !tool_exists(&self.pdftotext) => Some(
                "Per estrarre il testo dai PDF serve Poppler. Apri Strumenti e seleziona pdftotext.exe. Il riconoscimento OCR non è incluso.",
            ),
            MediaKind::Pdf
                if matches!(format, OutputFormat::Png | OutputFormat::Jpg)
                    && !tool_exists(&self.pdftoppm) =>
            {
                Some(
                    "Per esportare le pagine PDF serve Poppler. Apri Strumenti e seleziona pdftoppm.exe già installato sul PC.",
                )
            }
            MediaKind::Document if !tool_exists(&self.soffice) => Some(
                "Per convertire i documenti Office in PDF serve LibreOffice. Apri Strumenti e seleziona soffice.exe nella sua cartella program.",
            ),
            _ => None,
        };
        missing.map(str::to_owned)
    }
}

fn tool_exists(path: &Option<PathBuf>) -> bool {
    path.as_ref().is_some_and(|path| executable_available(path))
}

/// Windows batch files require a command processor; conversion tools must be
/// existing EXEs instead. Custom filenames and case-insensitive EXE suffixes work.
pub fn executable_available(path: &Path) -> bool {
    path.is_file()
        && (!cfg!(windows)
            || path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("exe")))
}

fn find_tool(name: &str) -> Option<PathBuf> {
    let exe = executable_name(name);
    let mut roots = Vec::new();
    if let Some(root) = std::env::var_os("THE_CONVERTER_TOOLS").map(PathBuf::from)
        && root.is_absolute()
    {
        roots.push(root);
    }
    roots.extend(user_tools_directories());
    if let Ok(app) = std::env::current_exe() {
        roots.extend(application_roots(&app));
    }
    if let Some(base) = directories::BaseDirs::new() {
        roots.push(base.data_local_dir().join("Microsoft/WinGet/Links"));
    }
    if let Some(path) = find_in_roots(&exe, &roots) {
        return Some(path);
    }
    // Explorer and MSIX activations have different working directories. Only use
    // absolute PATH entries, never an engine found beside an arbitrary input file.
    if let Ok(path) = which::which_global(&exe)
        && executable_available(&path)
    {
        return Some(path);
    }
    if name == "soffice" {
        for env in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(root) = std::env::var_os(env) {
                let path = PathBuf::from(root).join("LibreOffice/program").join(&exe);
                if executable_available(&path) {
                    return Some(path);
                }
            }
        }
        if let Some(base) = directories::BaseDirs::new() {
            let path = base
                .data_local_dir()
                .join("Programs/LibreOffice/program")
                .join(&exe);
            if executable_available(&path) {
                return Some(path);
            }
        }
    }
    None
}

fn executable_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

/// User-managed engine location shared by the portable and packaged app. Discovery
/// only reads this directory: installing the app never downloads or copies engines.
pub fn user_tools_dir() -> Option<PathBuf> {
    directories::BaseDirs::new()
        .map(|dirs| dirs.data_local_dir().join("Corsivo/TheConverter/tools"))
}

fn user_tools_directories() -> Vec<PathBuf> {
    let Some(base) = directories::BaseDirs::new() else {
        return Vec::new();
    };
    vec![
        base.data_local_dir().join("Corsivo/TheConverter/tools"),
        base.data_local_dir()
            .join("SalvatoreCorsi/TheConverter/tools"),
        base.data_local_dir().join("Programs/The Converter/tools"),
    ]
}

fn application_roots(executable: &Path) -> Vec<PathBuf> {
    let Some(parent) = executable.parent() else {
        return Vec::new();
    };
    // A portable build may carry engines alongside the EXE. The same read-only
    // probes are harmless inside WindowsApps; never search its parent directories.
    let mut roots = vec![parent.join("tools"), parent.to_owned()];
    let configuration = if parent.file_name().is_some_and(|name| name == "deps") {
        parent.parent()
    } else {
        Some(parent)
    };
    if let Some(configuration) = configuration
        && configuration
            .file_name()
            .is_some_and(|name| name == "debug" || name == "release")
        && let Some(target) = configuration.parent()
        && target.file_name().is_some_and(|name| name == "target")
        && let Some(project) = target.parent()
        && project.join("Cargo.toml").is_file()
    {
        roots.push(project.join("tools"));
    }
    roots
}

fn find_in_roots(executable: &str, roots: &[PathBuf]) -> Option<PathBuf> {
    for root in roots {
        for suffix in [
            "",
            "bin",
            "ffmpeg/bin",
            "poppler/Library/bin",
            "poppler/bin",
            "LibreOffice/program",
        ] {
            let path = root.join(suffix).join(executable);
            if executable_available(&path) {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packaged_discovery_does_not_escape_the_application_directory() {
        let package = PathBuf::from("WindowsApps/Corsivo.TheConverter_1.0.0.0_x64/example");
        let roots = application_roots(&package.join("the-converter.exe"));
        assert_eq!(roots, vec![package.join("tools"), package]);
    }

    #[test]
    fn development_tools_are_found_without_a_working_directory_dependency() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("Cargo.toml"),
            "[package]\nname='fixture'\n",
        )
        .unwrap();
        let executable = root.path().join("target/debug/deps/test.exe");
        assert!(application_roots(&executable).contains(&root.path().join("tools")));
    }

    #[test]
    fn engine_discovery_reads_nested_installations_without_creating_files() {
        let root = tempfile::tempdir().unwrap();
        let bin = root.path().join("poppler/Library/bin");
        std::fs::create_dir_all(&bin).unwrap();
        let name = executable_name("pdftoppm");
        let executable = bin.join(&name);
        std::fs::write(&executable, []).unwrap();
        assert_eq!(
            find_in_roots(&name, &[root.path().to_owned()]),
            Some(executable)
        );
        assert!(find_in_roots(&executable_name("ffmpeg"), &[root.path().to_owned()]).is_none());
        assert!(!root.path().join("ffmpeg").exists());
    }

    #[test]
    fn native_image_conversion_does_not_require_optional_engines() {
        let tools = ToolPaths::default();
        assert!(
            tools
                .missing_for(MediaKind::Image, OutputFormat::Jpg)
                .is_none()
        );
        assert!(
            tools
                .missing_for(MediaKind::Image, OutputFormat::Pdf)
                .is_none()
        );
        assert!(
            tools
                .missing_for(MediaKind::Audio, OutputFormat::Mp3)
                .unwrap()
                .contains("Strumenti")
        );
        assert!(
            tools
                .missing_for(MediaKind::Pdf, OutputFormat::Txt)
                .unwrap()
                .contains("pdftotext.exe")
        );
    }

    #[test]
    fn selecting_an_installation_picks_its_companion_and_respects_manual_choices() {
        let root = tempfile::tempdir().unwrap();
        let ffmpeg = root.path().join(executable_name("ffmpeg"));
        let ffprobe = root.path().join(executable_name("ffprobe"));
        std::fs::write(&ffmpeg, []).unwrap();
        std::fs::write(&ffprobe, []).unwrap();
        let mut overrides = BTreeMap::from([("ffmpeg".to_owned(), ffmpeg.clone())]);
        let mut tools = ToolPaths::default();
        tools.apply_overrides(&overrides);
        assert_eq!(tools.ffmpeg, Some(ffmpeg));
        assert_eq!(tools.ffprobe, Some(ffprobe));
        let manual = root.path().join("manual-ffprobe.exe");
        std::fs::write(&manual, []).unwrap();
        overrides.insert("ffprobe".into(), manual.clone());
        tools.apply_overrides(&overrides);
        assert_eq!(tools.ffprobe, Some(manual));
    }

    #[cfg(windows)]
    #[test]
    fn windows_tools_accept_custom_exes_but_never_batch_or_script_files() {
        let directory = tempfile::tempdir().unwrap();
        let custom = directory.path().join("FFmpeg custom build.EXE");
        std::fs::write(&custom, []).unwrap();
        assert!(executable_available(&custom));
        assert!(!executable_available(&directory.path().join("missing.exe")));
        let folder = directory.path().join("folder.exe");
        std::fs::create_dir_all(&folder).unwrap();
        assert!(!executable_available(&folder));
        for extension in ["cmd", "bat", "ps1", "com"] {
            let path = directory.path().join(format!("not-a-tool.{extension}"));
            std::fs::write(&path, b"This fixture is never executed").unwrap();
            assert!(!executable_available(&path));
            let mut tools = ToolPaths::default();
            tools.apply_overrides(&BTreeMap::from([
                ("ffmpeg".into(), path.clone()),
                ("ffprobe".into(), path),
            ]));
            assert!(tools.ffmpeg.is_none());
            assert!(tools.ffprobe.is_none());
            assert!(
                tools
                    .missing_for(MediaKind::Audio, OutputFormat::Mp3)
                    .is_some()
            );
        }
        let mut tools = ToolPaths::default();
        tools.apply_overrides(&BTreeMap::from([("ffmpeg".into(), custom.clone())]));
        assert_eq!(tools.ffmpeg, Some(custom));
        assert!(
            tools
                .missing_for(MediaKind::Audio, OutputFormat::Mp3)
                .is_none()
        );
    }
}
