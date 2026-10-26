use crate::model::{MediaKind, OutputFormat};
use serde::{Deserialize, Serialize};
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
    pub fn missing_for(&self, kind: MediaKind, format: OutputFormat) -> Option<String> {
        let missing = match kind {
            MediaKind::Audio | MediaKind::Video if !tool_exists(&self.ffmpeg) => Some(
                "FFmpeg: aggiungi ffmpeg.exe nella cartella tools o selezionalo nelle impostazioni",
            ),
            MediaKind::Pdf if format == OutputFormat::Txt && !tool_exists(&self.pdftotext) => {
                Some("Poppler: manca pdftotext.exe (estrazione del testo, senza OCR)")
            }
            MediaKind::Pdf
                if matches!(format, OutputFormat::Png | OutputFormat::Jpg)
                    && !tool_exists(&self.pdftoppm) =>
            {
                Some("Poppler: manca pdftoppm.exe per esportare le pagine PDF")
            }
            MediaKind::Document if !tool_exists(&self.soffice) => {
                Some("LibreOffice: manca soffice.exe per convertire i documenti in PDF")
            }
            _ => None,
        };
        missing.map(str::to_owned)
    }
}

fn tool_exists(path: &Option<PathBuf>) -> bool {
    path.as_ref().is_some_and(|path| path.is_file())
}

fn find_tool(name: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    let mut roots = Vec::new();
    if let Some(root) = std::env::var_os("THE_CONVERTER_TOOLS") {
        roots.push(PathBuf::from(root));
    }
    if let Ok(app) = std::env::current_exe()
        && let Some(parent) = app.parent()
    {
        for ancestor in parent.ancestors().take(4) {
            roots.push(ancestor.join("tools"));
        }
        roots.push(parent.to_owned());
    }
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd.join("tools"));
    }
    for root in roots {
        for suffix in [
            Path::new(""),
            Path::new("bin"),
            Path::new("ffmpeg/bin"),
            Path::new("poppler/Library/bin"),
            Path::new("poppler/bin"),
            Path::new("LibreOffice/program"),
        ] {
            let path = root.join(suffix).join(&exe);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    if let Ok(path) = which::which(&exe) {
        return Some(path);
    }
    if name == "soffice" {
        for env in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(root) = std::env::var_os(env) {
                let path = PathBuf::from(root).join("LibreOffice/program").join(&exe);
                if path.is_file() {
                    return Some(path);
                }
            }
        }
    }
    None
}
