use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaKind {
    Audio,
    Video,
    Image,
    Pdf,
    Document,
    Unknown,
}

impl MediaKind {
    pub fn from_path(path: &Path) -> Self {
        match path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "mp3" | "mpga" | "wav" | "flac" | "aac" | "m4a" | "ogg" | "opus" | "aiff" | "aif"
            | "wma" | "alac" | "ape" => Self::Audio,
            "mp4" | "m4v" | "mkv" | "mov" | "webm" | "avi" | "wmv" | "mpeg" | "mpg" | "mts"
            | "m2ts" | "ts" | "3gp" | "flv" => Self::Video,
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "tif" | "tiff" | "ico" => Self::Image,
            "pdf" => Self::Pdf,
            "doc" | "docx" | "odt" | "rtf" | "txt" | "ppt" | "pptx" | "odp" | "xls" | "xlsx"
            | "ods" => Self::Document,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OutputFormat {
    #[default]
    Mp3,
    Wav,
    Flac,
    Aac,
    Ogg,
    Opus,
    Aiff,
    Mp4,
    Mkv,
    Mov,
    Webm,
    Avi,
    Gif,
    Png,
    Jpg,
    Webp,
    Bmp,
    Tiff,
    Ico,
    Pdf,
    Txt,
}

impl OutputFormat {
    pub const ALL: [Self; 21] = [
        Self::Mp3,
        Self::Wav,
        Self::Flac,
        Self::Aac,
        Self::Ogg,
        Self::Opus,
        Self::Aiff,
        Self::Mp4,
        Self::Mkv,
        Self::Mov,
        Self::Webm,
        Self::Avi,
        Self::Gif,
        Self::Png,
        Self::Jpg,
        Self::Webp,
        Self::Bmp,
        Self::Tiff,
        Self::Ico,
        Self::Pdf,
        Self::Txt,
    ];
    pub fn extension(self) -> &'static str {
        match self {
            Self::Mp3 => "mp3",
            Self::Wav => "wav",
            Self::Flac => "flac",
            Self::Aac => "aac",
            Self::Ogg => "ogg",
            Self::Opus => "opus",
            Self::Aiff => "aiff",
            Self::Mp4 => "mp4",
            Self::Mkv => "mkv",
            Self::Mov => "mov",
            Self::Webm => "webm",
            Self::Avi => "avi",
            Self::Gif => "gif",
            Self::Png => "png",
            Self::Jpg => "jpg",
            Self::Webp => "webp",
            Self::Bmp => "bmp",
            Self::Tiff => "tiff",
            Self::Ico => "ico",
            Self::Pdf => "pdf",
            Self::Txt => "txt",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Mp3 => "MP3",
            Self::Wav => "WAV",
            Self::Flac => "FLAC",
            Self::Aac => "AAC",
            Self::Ogg => "OGG",
            Self::Opus => "Opus",
            Self::Aiff => "AIFF",
            Self::Mp4 => "MP4",
            Self::Mkv => "MKV",
            Self::Mov => "MOV",
            Self::Webm => "WebM",
            Self::Avi => "AVI",
            Self::Gif => "GIF",
            Self::Png => "PNG",
            Self::Jpg => "JPEG",
            Self::Webp => "WebP",
            Self::Bmp => "BMP",
            Self::Tiff => "TIFF",
            Self::Ico => "ICO",
            Self::Pdf => "PDF",
            Self::Txt => "TXT",
        }
    }
    pub fn kind(self) -> MediaKind {
        match self {
            Self::Mp3
            | Self::Wav
            | Self::Flac
            | Self::Aac
            | Self::Ogg
            | Self::Opus
            | Self::Aiff => MediaKind::Audio,
            Self::Mp4 | Self::Mkv | Self::Mov | Self::Webm | Self::Avi => MediaKind::Video,
            Self::Gif | Self::Png | Self::Jpg | Self::Webp | Self::Bmp | Self::Tiff | Self::Ico => {
                MediaKind::Image
            }
            Self::Pdf => MediaKind::Pdf,
            Self::Txt => MediaKind::Document,
        }
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

pub fn supported_formats(kind: MediaKind) -> Vec<OutputFormat> {
    use OutputFormat::*;
    match kind {
        MediaKind::Audio => vec![Mp3, Wav, Flac, Aac, Ogg, Opus, Aiff, Txt],
        MediaKind::Video => vec![
            Mp4, Mkv, Mov, Webm, Avi, Gif, Mp3, Wav, Flac, Aac, Ogg, Opus, Aiff,
        ],
        MediaKind::Image => vec![Png, Jpg, Webp, Bmp, Tiff, Gif, Ico, Pdf],
        MediaKind::Pdf => vec![Png, Jpg, Txt],
        MediaKind::Document => vec![Pdf],
        MediaKind::Unknown => vec![],
    }
}

/// Formats shared by every input. Cloud transcription is opt-in; PDF text
/// extraction stays available independently of an OpenAI credential.
pub fn common_formats<'a>(
    inputs: impl IntoIterator<Item = &'a Path>,
    transcription_enabled: bool,
) -> Vec<OutputFormat> {
    let mut inputs = inputs.into_iter();
    let Some(first) = inputs.next() else {
        return Vec::new();
    };
    let formats_for = |path: &Path| {
        let kind = MediaKind::from_path(path);
        let mut formats = supported_formats(kind);
        if kind == MediaKind::Audio && !transcription_enabled {
            formats.retain(|format| *format != OutputFormat::Txt);
        }
        formats
    };
    let mut formats = formats_for(first);
    for path in inputs {
        let supported = formats_for(path);
        formats.retain(|format| supported.contains(format));
    }
    formats
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ConversionSettings {
    pub format: OutputFormat,
    pub output_dir: Option<PathBuf>,
    pub audio: AudioSettings,
    pub video: VideoSettings,
    pub image: ImageSettings,
    pub pdf: PdfSettings,
    pub transcription: TranscriptionSettings,
}
impl Default for ConversionSettings {
    fn default() -> Self {
        Self {
            format: OutputFormat::Mp3,
            output_dir: None,
            audio: AudioSettings::default(),
            video: VideoSettings::default(),
            image: ImageSettings::default(),
            pdf: PdfSettings::default(),
            transcription: TranscriptionSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TranscriptionSettings {
    /// None detects the recording's language. A credential is never serialized.
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    pub bitrate_kbps: u32,
    pub sample_rate: u32,
    pub channels: u8,
    pub normalize: bool,
}
impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            bitrate_kbps: 192,
            sample_rate: 0,
            channels: 0,
            normalize: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VideoCodec {
    #[default]
    H264,
    H265,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoSettings {
    pub codec: VideoCodec,
    pub crf: u8,
    pub width: Option<u32>,
    pub fps: Option<u32>,
    pub trim_start: Option<f64>,
    pub trim_end: Option<f64>,
}
impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            codec: VideoCodec::H264,
            crf: 23,
            width: None,
            fps: None,
            trim_start: None,
            trim_end: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageSettings {
    pub quality: u8,
    pub width: Option<u32>,
}
impl Default for ImageSettings {
    fn default() -> Self {
        Self {
            quality: 90,
            width: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PdfSettings {
    pub dpi: u32,
}
impl Default for PdfSettings {
    fn default() -> Self {
        Self { dpi: 150 }
    }
}
