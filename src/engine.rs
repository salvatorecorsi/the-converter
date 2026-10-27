//! Local conversion engine. Inputs are never modified; outputs are committed without clobbering.
use crate::model::{ConversionSettings, MediaKind, OutputFormat, VideoCodec, supported_formats};
use crate::tools::ToolPaths;
use anyhow::{Context, Result, bail};
use image::{DynamicImage, GenericImageView, ImageDecoder, ImageFormat};
use lopdf::{Document, Object, Stream, dictionary};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use tempfile::NamedTempFile;

#[path = "output_plan.rs"]
mod output_plan;
pub use output_plan::{
    PreparedConversion, ReservedOutput, convert_prepared, prepare, release_prepared,
    validate_prepared,
};

#[derive(Debug, Clone)]
pub struct Progress {
    pub fraction: f32,
    pub message: String,
}

fn cancelled(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        bail!("Conversione annullata");
    }
    Ok(())
}

fn hidden(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}

/// Keep tool subprocesses in a Windows job so cancellation/crashes also stop helper processes.
#[cfg(windows)]
struct ProcessJob(std::os::windows::io::OwnedHandle);
#[cfg(windows)]
impl ProcessJob {
    fn attach(child: &Child) -> Result<Self> {
        use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
        use windows_sys::Win32::System::JobObjects::*;
        unsafe {
            let raw = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if raw.is_null() {
                return Err(std::io::Error::last_os_error().into());
            }
            let handle = OwnedHandle::from_raw_handle(raw);
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                raw,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            ) == 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
            if AssignProcessToJobObject(raw, child.as_raw_handle()) == 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            Ok(Self(handle))
        }
    }
    fn terminate(&self) {
        use std::os::windows::io::AsRawHandle;
        unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.0.as_raw_handle(), 1);
        }
    }
}

#[cfg(not(windows))]
struct ProcessJob;
#[cfg(not(windows))]
impl ProcessJob {
    fn attach(_: &Child) -> Result<Self> {
        Ok(Self)
    }
    fn terminate(&self) {}
}

pub fn convert(
    input: &Path,
    settings: &ConversionSettings,
    tools: &ToolPaths,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(Progress),
) -> Result<Vec<PathBuf>> {
    cancelled(cancel)?;
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
    if let Some(reason) = tools.missing_for(kind, settings.format) {
        bail!("{reason}");
    }
    let out_dir = settings
        .output_dir
        .clone()
        .unwrap_or_else(|| input.parent().unwrap().to_owned());
    fs::create_dir_all(&out_dir).context("Impossibile creare la cartella di destinazione")?;
    let out_dir = fs::canonicalize(out_dir)?;
    on_progress(Progress {
        fraction: 0.0,
        message: "Preparazione…".into(),
    });
    let outputs = match kind {
        MediaKind::Audio | MediaKind::Video => vec![convert_media(
            &input,
            &out_dir,
            settings,
            tools,
            cancel,
            &mut on_progress,
        )?],
        MediaKind::Image => vec![convert_image(
            &input,
            &out_dir,
            settings,
            cancel,
            &mut on_progress,
        )?],
        MediaKind::Pdf => convert_pdf(&input, &out_dir, settings, tools, cancel, &mut on_progress)?,
        MediaKind::Document => vec![convert_document(
            &input,
            &out_dir,
            tools,
            cancel,
            &mut on_progress,
        )?],
        _ => bail!("Formato non supportato"),
    };
    on_progress(Progress {
        fraction: 1.0,
        message: format!("Completato: {} file", outputs.len()),
    });
    Ok(outputs)
}

fn validate(s: &ConversionSettings) -> Result<()> {
    if !(8..=512).contains(&s.audio.bitrate_kbps) {
        bail!("Il bitrate deve essere tra 8 e 512 kbps");
    }
    if s.audio.sample_rate != 0 && !(8000..=192000).contains(&s.audio.sample_rate) {
        bail!("Frequenza audio non valida (8000–192000 Hz oppure 0 per la sorgente)");
    }
    if s.audio.channels > 2 {
        bail!("Canali audio: 0 = sorgente, 1 = mono, 2 = stereo");
    }
    if matches!(s.format, OutputFormat::Opus | OutputFormat::Webm)
        && s.audio.sample_rate != 0
        && ![8000, 12000, 16000, 24000, 48000].contains(&s.audio.sample_rate)
    {
        bail!(
            "Opus/WebM: frequenza audio supportata 8000, 12000, 16000, 24000 o 48000 Hz (0 = automatica)"
        );
    }
    if s.video.crf > 51 {
        bail!("CRF non valido (0–51)");
    }
    if s.video.width.is_some_and(|v| !(16..=16384).contains(&v))
        || s.image.width.is_some_and(|v| !(1..=32768).contains(&v))
    {
        bail!("Larghezza non valida");
    }
    if s.video.fps.is_some_and(|v| !(1..=240).contains(&v)) {
        bail!("FPS non validi (1–240)");
    }
    if !(1..=100).contains(&s.image.quality) {
        bail!("Qualità JPEG non valida (1–100)");
    }
    if !(36..=600).contains(&s.pdf.dpi) {
        bail!("DPI non validi (36–600)");
    }
    if s.video.trim_start.is_some_and(|v| !v.is_finite() || v < 0.)
        || s.video.trim_end.is_some_and(|v| !v.is_finite() || v <= 0.)
    {
        bail!("Tempi di taglio non validi");
    }
    if let Some(end) = s.video.trim_end
        && end <= s.video.trim_start.unwrap_or(0.)
    {
        bail!("La fine del taglio deve essere successiva all'inizio");
    }
    Ok(())
}

fn temporary(dir: &Path, ext: &str) -> Result<NamedTempFile> {
    Ok(tempfile::Builder::new()
        .prefix(".the-converter-")
        .suffix(&format!(".{ext}"))
        .tempfile_in(dir)?)
}

/// persist_noclobber performs the final creation without replacing an existing file, including races.
fn commit(mut temp: NamedTempFile, dir: &Path, stem: &str, ext: &str) -> Result<PathBuf> {
    if temp.as_file().metadata()?.len() == 0 {
        bail!("Il convertitore ha prodotto un file vuoto");
    }
    temp.as_file_mut().flush()?;
    for n in 0..10000u32 {
        let name = if n == 0 {
            format!("{stem}.{ext}")
        } else {
            format!("{stem} ({n}).{ext}")
        };
        let dest = dir.join(name);
        match temp.persist_noclobber(&dest) {
            Ok(_) => return Ok(dest),
            Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => temp = e.file,
            Err(e) => return Err(e.error).context("Impossibile salvare il risultato"),
        }
    }
    bail!("Troppi file con lo stesso nome nella destinazione")
}

fn stem(input: &Path) -> String {
    input
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

fn copy_commit(source: &Path, dir: &Path, name: &str, ext: &str) -> Result<PathBuf> {
    let mut temp = temporary(dir, ext)?;
    let mut source = fs::File::open(source)?;
    std::io::copy(&mut source, temp.as_file_mut())?;
    commit(temp, dir, name, ext)
}

fn flatten(image: &DynamicImage) -> DynamicImage {
    let mut rgb = image::RgbImage::new(image.width(), image.height());
    for (x, y, p) in image.to_rgba8().enumerate_pixels() {
        let a = p[3] as u32;
        rgb.put_pixel(
            x,
            y,
            image::Rgb([
                ((p[0] as u32 * a + 255 * (255 - a) + 127) / 255) as u8,
                ((p[1] as u32 * a + 255 * (255 - a) + 127) / 255) as u8,
                ((p[2] as u32 * a + 255 * (255 - a) + 127) / 255) as u8,
            ]),
        );
    }
    DynamicImage::ImageRgb8(rgb)
}

fn convert_image(
    input: &Path,
    dir: &Path,
    s: &ConversionSettings,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(Progress),
) -> Result<PathBuf> {
    let mut decoder = image::ImageReader::open(input)?
        .with_guessed_format()?
        .into_decoder()
        .context("Immagine non leggibile o codec non supportato")?;
    // into_decoder does not reserve the decoded pixel allocation as ImageReader::decode does.
    if let Some(limit) = image::Limits::default().max_alloc
        && decoder.total_bytes() > limit
    {
        bail!("L'immagine supera il limite di memoria per la decodifica");
    }
    let orientation = decoder
        .orientation()
        .context("Orientamento immagine non leggibile")?;
    let mut image = DynamicImage::from_decoder(decoder)
        .context("Immagine non leggibile o codec non supportato")?;
    // Bake camera/smartphone EXIF rotation into pixels before resizing or embedding in PDF.
    image.apply_orientation(orientation);
    cancelled(cancel)?;
    progress(Progress {
        fraction: 0.25,
        message: "Elaborazione immagine…".into(),
    });
    if let Some(width) = s.image.width {
        let height =
            ((image.height() as f64 * width as f64 / image.width() as f64).round() as u32).max(1);
        if (width as u64) * (height as u64) > 150_000_000 {
            bail!("L'immagine ridimensionata supera 150 megapixel");
        }
        image = image.resize_exact(width, height, image::imageops::FilterType::Lanczos3);
    }
    if s.format == OutputFormat::Ico {
        image = image.thumbnail(256, 256);
    }
    let mut temp = temporary(dir, s.format.extension())?;
    match s.format {
        OutputFormat::Pdf => image_pdf(&image, s.image.quality, temp.path())?,
        OutputFormat::Jpg => {
            image::codecs::jpeg::JpegEncoder::new_with_quality(temp.as_file_mut(), s.image.quality)
                .encode_image(&flatten(&image))?
        }
        _ => {
            let format = match s.format {
                OutputFormat::Png => ImageFormat::Png,
                OutputFormat::Webp => ImageFormat::WebP,
                OutputFormat::Bmp => ImageFormat::Bmp,
                OutputFormat::Tiff => ImageFormat::Tiff,
                OutputFormat::Gif => ImageFormat::Gif,
                OutputFormat::Ico => ImageFormat::Ico,
                _ => bail!("Format image non supportato"),
            };
            image.write_to(temp.as_file_mut(), format)?;
        }
    }
    cancelled(cancel)?;
    progress(Progress {
        fraction: 0.9,
        message: "Salvataggio…".into(),
    });
    commit(temp, dir, &stem(input), s.format.extension())
}

fn image_pdf(image: &DynamicImage, quality: u8, path: &Path) -> Result<()> {
    let image = flatten(image);
    let (w, h) = image.dimensions();
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, quality).encode_image(&image)?;
    let mut doc = Document::with_version("1.5");
    let pages = doc.new_object_id();
    let img=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>w as i64,"Height"=>h as i64,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8,"Filter"=>"DCTDecode"},jpeg));
    // A4 page, oriented like the source, with a 24pt margin; aspect ratio is preserved.
    let (pw, ph) = if w > h {
        (842.0f64, 595.0f64)
    } else {
        (595.0, 842.0)
    };
    let scale = ((pw - 48.) / w as f64).min((ph - 48.) / h as f64);
    let (iw, ih) = (w as f64 * scale, h as f64 * scale);
    let content = format!(
        "q {iw:.4} 0 0 {ih:.4} {:.4} {:.4} cm /Im0 Do Q",
        (pw - iw) / 2.,
        (ph - ih) / 2.
    );
    let contents = doc.add_object(Stream::new(dictionary! {}, content.into_bytes()));
    let resources = doc.add_object(dictionary! {"XObject"=>dictionary!{"Im0"=>img}});
    let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![Object::from(0),Object::from(0),Object::Real(pw as f32),Object::Real(ph as f32)],"Resources"=>resources,"Contents"=>contents});
    doc.objects.insert(
        pages,
        Object::Dictionary(
            dictionary! {"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1},
        ),
    );
    let catalog = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    doc.trailer.set("Root", catalog);
    doc.compress();
    doc.save(path)?;
    Ok(())
}

fn duration(input: &Path, tools: &ToolPaths, cancel: &AtomicBool) -> Option<f64> {
    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    let exe = tools
        .ffprobe
        .as_ref()
        .filter(|path| crate::tools::executable_available(path))?;
    let mut cmd = Command::new(exe);
    hidden(&mut cmd)
        .args([
            "-v",
            "error",
            "-protocol_whitelist",
            "file,pipe",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(input)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = cmd.spawn().ok()?;
    let job = match ProcessJob::attach(&child) {
        Ok(job) => job,
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
    };
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let mut text = String::new();
                child.stdout.take()?.read_to_string(&mut text).ok()?;
                return text
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite() && *v > 0.);
            }
            Ok(None) => {}
            Err(_) => return None,
        }
        if start.elapsed() > Duration::from_secs(10) || cancel.load(Ordering::Relaxed) {
            job.terminate();
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        thread::sleep(Duration::from_millis(30));
    }
}

fn convert_media(
    input: &Path,
    dir: &Path,
    s: &ConversionSettings,
    tools: &ToolPaths,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(Progress),
) -> Result<PathBuf> {
    let temp = temporary(dir, s.format.extension())?;
    let mut duration = duration(input, tools, cancel);
    cancelled(cancel)?;
    let start = s.video.trim_start.unwrap_or(0.);
    if let Some(end) = s.video.trim_end {
        duration = Some(end - start);
    } else {
        duration = duration.map(|d| (d - start).max(0.));
    }
    let mut cmd = Command::new(tools.ffmpeg.as_ref().context("FFmpeg non trovato")?);
    hidden(&mut cmd).args([
        "-hide_banner",
        "-nostdin",
        "-y",
        "-loglevel",
        "error",
        "-progress",
        "pipe:1",
        "-nostats",
    ]);
    if start > 0. {
        cmd.args(["-ss", &start.to_string()]);
    }
    // Local media may contain playlist references; never allow them to open network URLs.
    cmd.args(["-protocol_whitelist", "file,pipe"])
        .arg("-i")
        .arg(input);
    if let Some(end) = s.video.trim_end {
        cmd.args(["-t", &(end - start).to_string()]);
    }
    if s.format.kind() == MediaKind::Audio {
        cmd.args(["-map", "0:a:0", "-vn"]);
        let codec = match s.format {
            OutputFormat::Mp3 => "libmp3lame",
            OutputFormat::Wav => "pcm_s16le",
            OutputFormat::Aiff => "pcm_s16be",
            OutputFormat::Flac => "flac",
            OutputFormat::Aac => "aac",
            OutputFormat::Ogg => "libvorbis",
            OutputFormat::Opus => "libopus",
            _ => unreachable!(),
        };
        cmd.args(["-c:a", codec]);
        if !matches!(
            s.format,
            OutputFormat::Wav | OutputFormat::Aiff | OutputFormat::Flac
        ) {
            cmd.args(["-b:a", &format!("{}k", s.audio.bitrate_kbps)]);
        }
        if s.audio.sample_rate != 0 {
            cmd.args(["-ar", &s.audio.sample_rate.to_string()]);
        }
        if s.audio.channels != 0 {
            cmd.args(["-ac", &s.audio.channels.to_string()]);
        }
        if s.audio.normalize {
            cmd.args(["-af", "loudnorm=I=-16:TP=-1.5:LRA=11"]);
        }
    } else if s.format == OutputFormat::Gif {
        let fps = s.video.fps.unwrap_or(12);
        let width = s.video.width.unwrap_or(640);
        cmd.args(["-an","-filter_complex",&format!("[0:v:0]fps={fps},scale={width}:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse[v]"),"-map","[v]","-loop","0"]);
    } else {
        cmd.args(["-map", "0:v:0", "-map", "0:a:0?"]);
        match s.format {
            OutputFormat::Webm => {
                cmd.args([
                    "-c:v",
                    "libvpx-vp9",
                    "-b:v",
                    "0",
                    "-crf",
                    &s.video.crf.to_string(),
                    "-c:a",
                    "libopus",
                ]);
            }
            OutputFormat::Avi => {
                cmd.args([
                    "-c:v",
                    "mpeg4",
                    "-q:v",
                    &((s.video.crf as u32 / 2).clamp(2, 31)).to_string(),
                    "-c:a",
                    "libmp3lame",
                ]);
            }
            _ => {
                cmd.args([
                    "-c:v",
                    if s.video.codec == VideoCodec::H265 {
                        "libx265"
                    } else {
                        "libx264"
                    },
                    "-crf",
                    &s.video.crf.to_string(),
                    "-preset",
                    "medium",
                    "-c:a",
                    "aac",
                ]);
                if s.video.codec == VideoCodec::H265
                    && matches!(s.format, OutputFormat::Mp4 | OutputFormat::Mov)
                {
                    cmd.args(["-tag:v", "hvc1"]);
                }
            }
        }
        // Even dimensions keep H.264/H.265 4:2:0 encoders happy for odd source dimensions.
        let scale = s
            .video
            .width
            .map(|w| format!("scale={}:-2", w / 2 * 2))
            .unwrap_or_else(|| "scale=trunc(iw/2)*2:trunc(ih/2)*2".into());
        cmd.args([
            "-vf",
            &scale,
            "-pix_fmt",
            "yuv420p",
            "-b:a",
            &format!("{}k", s.audio.bitrate_kbps),
        ]);
        if let Some(fps) = s.video.fps {
            cmd.args(["-r", &fps.to_string()]);
        }
        if s.audio.sample_rate != 0 {
            cmd.args(["-ar", &s.audio.sample_rate.to_string()]);
        }
        if s.audio.channels != 0 {
            cmd.args(["-ac", &s.audio.channels.to_string()]);
        }
        if s.audio.normalize {
            cmd.args(["-af", "loudnorm=I=-16:TP=-1.5:LRA=11"]);
        }
        if matches!(s.format, OutputFormat::Mp4 | OutputFormat::Mov) {
            cmd.args(["-movflags", "+faststart"]);
        }
    }
    cmd.arg(temp.path());
    run_process(cmd, cancel, duration, progress)?;
    cancelled(cancel)?;
    commit(temp, dir, &stem(input), s.format.extension())
}

/// Bounded stderr capture and polled child handle ensure cancellation also works during silent tools.
fn run_process(
    mut cmd: Command,
    cancel: &AtomicBool,
    duration: Option<f64>,
    progress: &mut impl FnMut(Progress),
) -> Result<()> {
    hidden(&mut cmd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().with_context(|| {
        format!(
            "Impossibile avviare {}",
            cmd.get_program().to_string_lossy()
        )
    })?;
    let job = match ProcessJob::attach(&child) {
        Ok(job) => job,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e).context("Impossibile isolare il processo di conversione");
        }
    };
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let (send, recv) = mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout)
            .lines()
            .map_while(std::result::Result::ok)
        {
            if let Some(time) = line
                .strip_prefix("out_time_us=")
                .and_then(|v| v.parse::<f64>().ok())
            {
                let _ = send.send(time / 1_000_000.);
            }
        }
    });
    let errors = thread::spawn(move || {
        let mut text = Vec::new();
        let mut buf = [0u8; 4096];
        let mut reader = stderr;
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    text.extend_from_slice(&buf[..n]);
                    if text.len() > 32768 {
                        text.drain(..text.len() - 32768);
                    }
                }
            }
        }
        String::from_utf8_lossy(&text).into_owned()
    });
    let result = wait_child(&mut child, &job, cancel, &recv, duration, progress);
    // Close the job before joining readers, including handles inherited by helper subprocesses.
    drop(job);
    let _ = reader.join();
    let diagnostic = errors.join().unwrap_or_default();
    match result {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => bail!(
            "Conversione non riuscita: {}",
            if diagnostic.trim().is_empty() {
                "il convertitore si è interrotto"
            } else {
                diagnostic.trim()
            }
        ),
        Err(e) => Err(e),
    }
}

fn wait_child(
    child: &mut Child,
    job: &ProcessJob,
    cancel: &AtomicBool,
    recv: &mpsc::Receiver<f64>,
    duration: Option<f64>,
    progress: &mut impl FnMut(Progress),
) -> Result<std::process::ExitStatus> {
    loop {
        if cancel.load(Ordering::Relaxed) {
            job.terminate();
            let _ = child.kill();
            let _ = child.wait();
            bail!("Conversione annullata");
        }
        for time in recv.try_iter() {
            progress(Progress {
                fraction: duration
                    .filter(|d| *d > 0.)
                    .map(|d| (time / d).clamp(0., 0.99) as f32)
                    .unwrap_or(0.),
                message: format!("Conversione… {:.0} s", time),
            });
        }
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(e) => {
                job.terminate();
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.into());
            }
        }
        thread::sleep(Duration::from_millis(40));
    }
}

fn convert_pdf(
    input: &Path,
    dir: &Path,
    s: &ConversionSettings,
    tools: &ToolPaths,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(Progress),
) -> Result<Vec<PathBuf>> {
    if s.format == OutputFormat::Txt {
        let temp = temporary(dir, "txt")?;
        let mut cmd = Command::new(tools.pdftotext.as_ref().context("pdftotext non trovato")?);
        cmd.args(["-layout", "-enc", "UTF-8"])
            .arg(input)
            .arg(temp.path());
        run_process(cmd, cancel, None, progress)?;
        cancelled(cancel)?;
        // A scanned PDF has no embedded text; avoid presenting an empty export as success.
        if fs::read_to_string(temp.path())?.trim().is_empty() {
            bail!(
                "Il PDF non contiene testo estraibile. I PDF scansionati richiedono OCR, non incluso."
            );
        }
        return Ok(vec![commit(temp, dir, &stem(input), "txt")?]);
    }
    let stage = tempfile::Builder::new()
        .prefix(".the-converter-pages-")
        .tempdir_in(dir)?;
    let mut cmd = Command::new(tools.pdftoppm.as_ref().context("pdftoppm non trovato")?);
    cmd.arg(if s.format == OutputFormat::Jpg {
        "-jpeg"
    } else {
        "-png"
    })
    .args(["-r", &s.pdf.dpi.to_string()]);
    if s.format == OutputFormat::Jpg {
        cmd.args(["-jpegopt", &format!("quality={}", s.image.quality)]);
    }
    cmd.arg(input).arg(stage.path().join("page"));
    progress(Progress {
        fraction: 0.05,
        message: "Rendering di tutte le pagine PDF…".into(),
    });
    run_process(cmd, cancel, None, progress)?;
    let mut pages: Vec<PathBuf> = fs::read_dir(stage.path())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some(s.format.extension()))
        .collect();
    pages.sort_by_key(|p| {
        p.file_stem()
            .and_then(|n| n.to_str())
            .and_then(|n| n.rsplit('-').next())
            .and_then(|n| n.parse::<u32>().ok())
            .unwrap_or(0)
    });
    if pages.is_empty() {
        bail!("Nessuna pagina esportata dal PDF");
    }
    let mut outputs = Vec::new();
    let result = (|| -> Result<()> {
        for (i, page) in pages.iter().enumerate() {
            cancelled(cancel)?;
            let path = copy_commit(
                page,
                dir,
                &format!("{}-pagina-{:03}", stem(input), i + 1),
                s.format.extension(),
            )?;
            outputs.push(path);
            progress(Progress {
                fraction: 0.5 + (i + 1) as f32 / pages.len() as f32 * 0.49,
                message: format!("Pagina {} di {}", i + 1, pages.len()),
            });
        }
        Ok(())
    })();
    if let Err(e) = result {
        for output in outputs {
            let _ = fs::remove_file(output);
        }
        return Err(e);
    }
    Ok(outputs)
}

fn convert_document(
    input: &Path,
    dir: &Path,
    tools: &ToolPaths,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(Progress),
) -> Result<PathBuf> {
    let stage = tempfile::Builder::new()
        .prefix(".the-converter-office-")
        .tempdir_in(dir)?;
    let profile = stage.path().join("profile");
    // A private profile prevents headless conversion from being handed to an existing GUI instance.
    let profile_text = profile.to_string_lossy();
    let profile_path = if let Some(unc) = profile_text.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{unc}"))
    } else if let Some(local) = profile_text.strip_prefix(r"\\?\") {
        PathBuf::from(local)
    } else {
        profile.clone()
    };
    let profile_uri = url::Url::from_directory_path(&profile_path)
        .map_err(|_| anyhow::anyhow!("Percorso del profilo LibreOffice non valido"))?
        .to_string();
    let mut cmd = Command::new(tools.soffice.as_ref().context("LibreOffice non trovato")?);
    cmd.arg(format!("-env:UserInstallation={profile_uri}"))
        .args([
            "--headless",
            "--nologo",
            "--nodefault",
            "--nofirststartwizard",
            "--convert-to",
            "pdf",
            "--outdir",
        ])
        .arg(stage.path())
        .arg(input);
    progress(Progress {
        fraction: 0.1,
        message: "Conversione documento con LibreOffice…".into(),
    });
    run_process(cmd, cancel, None, progress)?;
    cancelled(cancel)?;
    let pdf = stage.path().join(format!("{}.pdf", stem(input)));
    if !pdf.is_file() {
        bail!(
            "LibreOffice non ha prodotto il PDF. Il documento potrebbe essere protetto o non supportato."
        );
    }
    copy_commit(&pdf, dir, &stem(input), "pdf")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn stored_probe_script_is_not_executed() {
        let directory = tempfile::tempdir().unwrap();
        let script = directory.path().join("ffprobe.cmd");
        fs::write(
            &script,
            "@echo off\r\n@echo executed>\"%~dp0executed.txt\"\r\n@echo 1.0\r\n",
        )
        .unwrap();
        let tools = ToolPaths {
            ffprobe: Some(script),
            ..Default::default()
        };
        assert!(duration(Path::new("input.wav"), &tools, &AtomicBool::new(false)).is_none());
        assert!(!directory.path().join("executed.txt").exists());
    }

    #[test]
    fn running_ffmpeg_is_cancelled_promptly() {
        let tools = ToolPaths::discover();
        let Some(ffmpeg) = tools.ffmpeg else {
            return;
        };
        let mut cmd = Command::new(ffmpeg);
        cmd.args([
            "-hide_banner",
            "-nostdin",
            "-loglevel",
            "error",
            "-progress",
            "pipe:1",
            "-re",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=30",
            "-f",
            "null",
            "-",
        ]);
        let cancel = AtomicBool::new(false);
        let start = std::time::Instant::now();
        let result = run_process(cmd, &cancel, Some(30.), &mut |_| {
            cancel.store(true, Ordering::Relaxed);
        });
        assert!(result.is_err());
        assert!(cancel.load(Ordering::Relaxed));
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "Cancellation took {:?}",
            start.elapsed()
        );
    }
}
