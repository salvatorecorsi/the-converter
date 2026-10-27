use eframe::egui::{self, Color32, RichText};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use the_converter::{
    engine,
    model::{ConversionSettings, MediaKind, OutputFormat, VideoCodec, supported_formats},
    platform,
    queue::{self, JobState as QuickJobState, QueueJob},
    tools::{ToolPaths, executable_available},
    transcription,
};
use zeroize::Zeroize;

const ACCENT: Color32 = Color32::from_rgb(0, 103, 192);
const GREEN: Color32 = Color32::from_rgb(28, 135, 86);

pub fn launch(files: Vec<PathBuf>, target: Option<OutputFormat>) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("The Converter")
            .with_icon(app_icon())
            .with_inner_size([1060.0, 760.0])
            .with_min_inner_size([860.0, 620.0])
            .with_drag_and_drop(true),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "The Converter",
        options,
        Box::new(move |cc| {
            configure_style(&cc.egui_ctx);
            Ok(Box::new(ConverterApp::new(files, target)))
        }),
    )
}

pub fn launch_queue_status(job_id: String) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("The Converter · Stato conversione")
            .with_icon(app_icon())
            .with_inner_size([650.0, 470.0])
            .with_min_inner_size([500.0, 360.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "The Converter · Stato conversione",
        options,
        Box::new(move |cc| {
            configure_style(&cc.egui_ctx);
            Ok(Box::new(QueueStatusApp::new(job_id)))
        }),
    )
}

fn app_icon() -> egui::IconData {
    let icon = image::load_from_memory(include_bytes!("../assets/app.png"))
        .expect("L’icona incorporata deve essere valida")
        .into_rgba8();
    let (width, height) = icon.dimensions();
    egui::IconData {
        rgba: icon.into_raw(),
        width,
        height,
    }
}

fn configure_style(ctx: &egui::Context) {
    if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\segoeui.ttf") {
        let mut fonts = egui::FontDefinitions::default();
        fonts
            .font_data
            .insert("segoe".into(), Arc::new(egui::FontData::from_owned(bytes)));
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "segoe".into());
        ctx.set_fonts(fonts);
    }
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(13.0, 8.0);
    style.spacing.interact_size.y = 32.0;
    style.visuals.selection.bg_fill = ACCENT;
    style.visuals.selection.stroke = egui::Stroke::new(1.0_f32, Color32::WHITE);
    style.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);
    style.visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);
    style.visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);
    style
        .text_styles
        .insert(egui::TextStyle::Heading, egui::FontId::proportional(25.0));
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
    ctx.set_style(style);
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct SavedState {
    settings: ConversionSettings,
    overrides: BTreeMap<String, PathBuf>,
    preset: usize,
}

#[derive(Clone, Deserialize)]
struct SavedReport {
    timestamp: String,
    format: String,
    successes: Vec<PathBuf>,
    errors: Vec<String>,
}

fn load_saved() -> SavedState {
    settings_path()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn quick_configuration() -> (ConversionSettings, ToolPaths) {
    let saved = load_saved();
    let tools = ToolPaths::configured(&saved.overrides);
    (saved.settings, tools)
}

pub fn report_path() -> Option<PathBuf> {
    queue::last_report_path()
}

fn load_report() -> Option<SavedReport> {
    report_path()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
}

impl Default for SavedState {
    fn default() -> Self {
        Self {
            settings: ConversionSettings::default(),
            overrides: BTreeMap::new(),
            preset: 1,
        }
    }
}

#[derive(Clone)]
enum JobState {
    Waiting,
    Running(f32, String),
    Complete(Vec<PathBuf>),
    Failed(String),
    Cancelled,
}

struct QueueItem {
    path: PathBuf,
    size: u64,
    state: JobState,
}

enum Event {
    Started(usize),
    Progress(usize, f32, String),
    Complete(usize, Vec<PathBuf>),
    Failed(usize, String),
    Cancelled(usize),
    End,
}

pub struct ConverterApp {
    saved: SavedState,
    tools: ToolPaths,
    items: Vec<QueueItem>,
    running: bool,
    cancel: Arc<AtomicBool>,
    receiver: Option<mpsc::Receiver<Event>>,
    show_tools: bool,
    advanced: bool,
    notice: Option<String>,
    settings_dirty: bool,
    quick_report: Option<SavedReport>,
    quick_queue: QuickQueueView,
    api_key_input: String,
    api_key_configured: bool,
    api_key_status: Option<String>,
}

impl ConverterApp {
    fn new(files: Vec<PathBuf>, target: Option<OutputFormat>) -> Self {
        let mut saved = load_saved();
        if let Some(format) = target {
            saved.settings.format = format;
        }
        let tools = ToolPaths::configured(&saved.overrides);
        let mut app = Self {
            saved,
            tools,
            items: Vec::new(),
            running: false,
            cancel: Arc::new(AtomicBool::new(false)),
            receiver: None,
            show_tools: false,
            advanced: false,
            notice: None,
            settings_dirty: false,
            quick_report: load_report(),
            quick_queue: QuickQueueView::default(),
            api_key_input: String::new(),
            api_key_configured: transcription::has_api_key(),
            api_key_status: None,
        };
        app.add_files(files);
        app
    }

    fn persist(&mut self) {
        if let Some(path) = settings_path()
            && let Some(parent) = path.parent()
        {
            let result = std::fs::create_dir_all(parent).and_then(|_| {
                use std::io::Write;
                let bytes =
                    serde_json::to_vec_pretty(&self.saved).map_err(std::io::Error::other)?;
                let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
                temporary.write_all(&bytes)?;
                temporary.flush()?;
                temporary.persist(&path).map_err(|error| error.error)?;
                Ok(())
            });
            if let Err(err) = result {
                self.notice = Some(format!("Impossibile salvare le preferenze: {err}"));
            }
        }
        self.settings_dirty = false;
    }

    fn add_files(&mut self, paths: Vec<PathBuf>) {
        let mut rejected = 0;
        for path in paths {
            if !path.is_file() || supported_formats(MediaKind::from_path(&path)).is_empty() {
                rejected += 1;
                continue;
            }
            let path = path.canonicalize().unwrap_or(path);
            if !self.items.iter().any(|item| item.path == path) {
                let size = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
                self.items.push(QueueItem {
                    path,
                    size,
                    state: JobState::Waiting,
                });
            }
        }
        if rejected > 0 {
            self.notice = Some(format!(
                "{rejected} elementi ignorati: cartelle o formati non supportati."
            ));
        }
        let common = self.common_formats();
        if !common.is_empty() && !common.contains(&self.saved.settings.format) {
            self.saved.settings.format = common[0];
            self.settings_dirty = true;
        }
    }

    fn common_formats(&self) -> Vec<OutputFormat> {
        the_converter::model::common_formats(
            self.items.iter().map(|item| item.path.as_path()),
            self.api_key_configured,
        )
    }

    fn transcribes_audio(&self) -> bool {
        self.saved.settings.format == OutputFormat::Txt
            && self
                .items
                .iter()
                .any(|item| MediaKind::from_path(&item.path) == MediaKind::Audio)
    }

    fn prerequisite(&self) -> Option<String> {
        if self.items.is_empty() {
            return None;
        }
        if self
            .saved
            .settings
            .video
            .trim_end
            .is_some_and(|end| end <= self.saved.settings.video.trim_start.unwrap_or(0.0))
        {
            return Some("La fine dell’intervallo deve essere successiva all’inizio.".into());
        }
        if self.common_formats().is_empty() {
            return Some("Questi file richiedono formati di uscita diversi. Crea una coda con file compatibili.".into());
        }
        if !self.common_formats().contains(&self.saved.settings.format) {
            return Some(format!(
                "Il formato {} non è compatibile con tutti i file. Scegli un altro formato di uscita.",
                self.saved.settings.format.label()
            ));
        }
        for item in &self.items {
            let kind = MediaKind::from_path(&item.path);
            if kind == MediaKind::Audio && self.saved.settings.format == OutputFormat::Txt {
                // Rendering can visit a large queue several times per frame.
                // Credential access belongs to setup and execution, not this loop.
                if !self.api_key_configured {
                    return Some(
                        "Aggiungi una chiave API OpenAI in Strumenti per trascrivere l’audio."
                            .into(),
                    );
                }
                continue;
            }
            if let Some(missing) = self.tools.missing_for(kind, self.saved.settings.format) {
                return Some(missing);
            }
        }
        if matches!(
            self.saved.settings.format,
            OutputFormat::Opus | OutputFormat::Webm
        ) && ![0, 8000, 12000, 16000, 24000, 48000]
            .contains(&self.saved.settings.audio.sample_rate)
        {
            return Some("L’audio Opus richiede 8000, 12000, 16000, 24000 o 48000 Hz. Scegli una frequenza compatibile nelle impostazioni.".into());
        }
        None
    }

    fn start(&mut self, ctx: &egui::Context) {
        if self.running || self.items.is_empty() {
            return;
        }
        if let Some(message) = self.prerequisite() {
            self.notice = Some(message);
            return;
        }
        self.persist();
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        let paths: Vec<_> = self.items.iter().map(|item| item.path.clone()).collect();
        for item in &mut self.items {
            item.state = JobState::Waiting;
        }
        let settings = self.saved.settings.clone();
        let tools = self.tools.clone();
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        self.running = true;
        self.notice = None;
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            for (index, path) in paths.into_iter().enumerate() {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let _ = sender.send(Event::Started(index));
                ctx.request_repaint();
                let result = engine::convert(&path, &settings, &tools, &cancel, |progress| {
                    let _ =
                        sender.send(Event::Progress(index, progress.fraction, progress.message));
                    ctx.request_repaint();
                });
                let event = match result {
                    Ok(outputs) => Event::Complete(index, outputs),
                    Err(_) if cancel.load(Ordering::Relaxed) => Event::Cancelled(index),
                    Err(error) => Event::Failed(index, format!("{error:#}")),
                };
                let _ = sender.send(event);
                ctx.request_repaint();
            }
            let _ = sender.send(Event::End);
            ctx.request_repaint();
        });
    }

    fn receive_events(&mut self) {
        let events: Vec<_> = self
            .receiver
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();
        for event in events {
            match event {
                Event::Started(index) => {
                    self.items[index].state = JobState::Running(0.0, "Preparazione…".into())
                }
                Event::Progress(index, fraction, message) => {
                    self.items[index].state = JobState::Running(fraction.clamp(0.0, 1.0), message)
                }
                Event::Complete(index, outputs) => {
                    self.items[index].state = JobState::Complete(outputs)
                }
                Event::Failed(index, error) => self.items[index].state = JobState::Failed(error),
                Event::Cancelled(index) => self.items[index].state = JobState::Cancelled,
                Event::End => {
                    self.running = false;
                    self.receiver = None;
                    if self.cancel.load(Ordering::Relaxed) {
                        for item in &mut self.items {
                            if matches!(item.state, JobState::Waiting | JobState::Running(..)) {
                                item.state = JobState::Cancelled;
                            }
                        }
                    }
                }
            }
        }
    }

    fn apply_preset(&mut self, preset: usize) {
        self.saved.preset = preset;
        let (bitrate, crf, quality) = match preset {
            0 => (128, 28, 75),
            2 => (320, 18, 95),
            _ => (192, 23, 90),
        };
        self.saved.settings.audio.bitrate_kbps = bitrate;
        self.saved.settings.video.crf = crf;
        self.saved.settings.image.quality = quality;
        self.settings_dirty = true;
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(46.0, 46.0), egui::Sense::hover());
            let painter = ui.painter();
            painter.rect_filled(rect, 12.0, ACCENT);
            let c = rect.center();
            let stroke = egui::Stroke::new(2.0_f32, Color32::WHITE);
            painter.arrow(c + egui::vec2(-12.0, -6.0), egui::vec2(24.0, 0.0), stroke);
            painter.arrow(c + egui::vec2(12.0, 6.0), egui::vec2(-24.0, 0.0), stroke);
            ui.vertical(|ui| {
                ui.add_space(1.0);
                ui.label(RichText::new("The Converter").size(24.0).strong());
                ui.label(
                    RichText::new("I tuoi file, nel formato giusto.")
                        .size(13.0)
                        .weak(),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Strumenti").clicked() {
                    self.show_tools = true;
                }
                ui.label(RichText::new("Corsivo").size(12.0).weak());
            });
        });
    }

    fn settings_panel(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("CONVERSIONE").size(11.0).strong().weak());
        ui.add_space(3.0);
        ui.label(RichText::new("Formato di uscita").strong());
        let formats = self.common_formats();
        let before = self.saved.settings.format;
        let selected = if self.items.is_empty() {
            "Aggiungi un file"
        } else if formats.is_empty() {
            "Nessun formato comune"
        } else {
            self.saved.settings.format.label()
        };
        ui.add_enabled_ui(!formats.is_empty(), |ui| {
            egui::ComboBox::from_id_salt("target_format")
                .width(ui.available_width())
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    for format in formats {
                        let label = if format == OutputFormat::Txt
                            && self
                                .items
                                .iter()
                                .any(|item| MediaKind::from_path(&item.path) == MediaKind::Audio)
                        {
                            "TXT con OpenAI"
                        } else {
                            format.label()
                        };
                        ui.selectable_value(&mut self.saved.settings.format, format, label);
                    }
                });
        });
        self.settings_dirty |= before != self.saved.settings.format;
        if before != self.saved.settings.format
            && matches!(
                self.saved.settings.format,
                OutputFormat::Opus | OutputFormat::Webm
            )
            && ![0, 8000, 12000, 16000, 24000, 48000]
                .contains(&self.saved.settings.audio.sample_rate)
        {
            self.saved.settings.audio.sample_rate = 0;
        }
        if self.transcribes_audio() {
            ui.label(RichText::new("L’audio verrà inviato a OpenAI con la tua chiave API. Si applicano i costi del tuo account OpenAI.").small());
        } else if !self.api_key_configured
            && self
                .items
                .iter()
                .any(|item| MediaKind::from_path(&item.path) == MediaKind::Audio)
            && ui.link("Abilita trascrizione in TXT…").clicked()
        {
            self.show_tools = true;
        }
        ui.add_space(7.0);
        ui.label(RichText::new("Qualità").strong());
        if quality_applicable(self.saved.settings.format) {
            ui.horizontal_wrapped(|ui| {
                for (index, name) in ["Leggero", "Bilanciato", "Alta qualità"]
                    .into_iter()
                    .enumerate()
                {
                    if ui
                        .selectable_label(self.saved.preset == index, name)
                        .clicked()
                    {
                        self.apply_preset(index);
                    }
                }
            });
            ui.label(
                RichText::new(match self.saved.preset {
                    0 => "File più piccoli, conversioni più veloci.",
                    2 => "Più dettaglio, dimensioni maggiori.",
                    3 => "Impostazioni personalizzate.",
                    _ => "Il giusto equilibrio per l’uso quotidiano.",
                })
                .small()
                .weak(),
            );
        } else {
            ui.label(RichText::new("Il formato scelto non usa un livello di qualità. Puoi regolare le altre opzioni qui sotto.").small().weak());
        }
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);
        ui.label(RichText::new("Salva in").strong());
        let mut beside = self.saved.settings.output_dir.is_none();
        if ui
            .radio_value(&mut beside, true, "Accanto al file originale")
            .changed()
        {
            self.saved.settings.output_dir = None;
            self.settings_dirty = true;
        }
        if ui
            .radio_value(&mut beside, false, "Una cartella a scelta")
            .clicked()
            && self.saved.settings.output_dir.is_none()
            && let Some(path) = rfd::FileDialog::new()
                .set_title("Cartella di destinazione")
                .pick_folder()
        {
            self.saved.settings.output_dir = Some(path);
            self.settings_dirty = true;
        }
        if let Some(path) = self.saved.settings.output_dir.clone() {
            ui.label(RichText::new(display_path(&path)).small())
                .on_hover_text(path.display().to_string());
            if ui.small_button("Cambia cartella…").clicked()
                && let Some(path) = rfd::FileDialog::new().set_directory(path).pick_folder()
            {
                self.saved.settings.output_dir = Some(path);
                self.settings_dirty = true;
            }
        }
        ui.label(
            RichText::new(
                "Gli originali restano al sicuro. I nomi già presenti ricevono un numero.",
            )
            .small()
            .weak(),
        );
        ui.add_space(8.0);
        ui.separator();
        egui::CollapsingHeader::new("Impostazioni personalizzate")
            .default_open(self.advanced)
            .show(ui, |ui| {
                let before = serde_json::to_vec(&self.saved.settings).unwrap_or_default();
                self.advanced_settings(ui);
                if serde_json::to_vec(&self.saved.settings).unwrap_or_default() != before {
                    self.saved.preset = 3;
                    self.settings_dirty = true;
                }
            });
        if self.items.iter().any(|item| {
            item.path.extension().is_some_and(|ext| {
                ext.eq_ignore_ascii_case("gif")
                    || ext.eq_ignore_ascii_case("tif")
                    || ext.eq_ignore_ascii_case("tiff")
            })
        }) {
            ui.label(
                RichText::new("GIF: primo fotogramma. TIFF: prima pagina.")
                    .small()
                    .weak(),
            );
        }
        if let Some(message) = self.prerequisite() {
            ui.add_space(8.0);
            ui.colored_label(Color32::from_rgb(180, 115, 20), &message);
            if ui.small_button("Configura strumenti…").clicked() {
                self.show_tools = true;
            }
        }
    }

    fn advanced_settings(&mut self, ui: &mut egui::Ui) {
        if self.transcribes_audio() {
            ui.label("Lingua dell’audio");
            let language = &mut self.saved.settings.transcription.language;
            egui::ComboBox::from_id_salt("transcription_language")
                .selected_text(transcription_language_label(language.as_deref()))
                .show_ui(ui, |ui| {
                    ui.selectable_value(language, None, "Rileva automaticamente");
                    for (code, name) in TRANSCRIPTION_LANGUAGES {
                        ui.selectable_value(language, Some((*code).into()), *name);
                    }
                });
            ui.label(RichText::new("Il testo mantiene la lingua parlata. Il risultato viene salvato in un file TXT UTF-8.").small().weak());
            return;
        }
        let kind = if self.saved.settings.format == OutputFormat::Gif
            && self
                .items
                .iter()
                .any(|item| MediaKind::from_path(&item.path) == MediaKind::Video)
        {
            MediaKind::Video
        } else {
            self.saved.settings.format.kind()
        };
        match kind {
            MediaKind::Audio => {
                if quality_applicable(self.saved.settings.format) {
                    ui.label("Bitrate audio");
                    ui.add(
                        egui::Slider::new(&mut self.saved.settings.audio.bitrate_kbps, 48..=320)
                            .suffix(" kbps"),
                    );
                }
                let rates: &[u32] = if self.saved.settings.format == OutputFormat::Opus {
                    &[0, 8000, 12000, 16000, 24000, 48000]
                } else {
                    &[0, 22050, 44100, 48000, 96000]
                };
                let rate = &mut self.saved.settings.audio.sample_rate;
                egui::ComboBox::from_id_salt("sample_rate")
                    .selected_text(if *rate == 0 {
                        "Frequenza originale".into()
                    } else {
                        format!("{rate} Hz")
                    })
                    .show_ui(ui, |ui| {
                        for &value in rates {
                            ui.selectable_value(
                                rate,
                                value,
                                if value == 0 {
                                    "Originale".into()
                                } else {
                                    format!("{value} Hz")
                                },
                            );
                        }
                    });
                let channels = &mut self.saved.settings.audio.channels;
                egui::ComboBox::from_id_salt("channels")
                    .selected_text(match channels {
                        1 => "Mono",
                        2 => "Stereo",
                        _ => "Canali originali",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(channels, 0, "Originali");
                        ui.selectable_value(channels, 1, "Mono");
                        ui.selectable_value(channels, 2, "Stereo");
                    });
                ui.checkbox(
                    &mut self.saved.settings.audio.normalize,
                    "Normalizza volume",
                );
                if matches!(
                    self.saved.settings.format,
                    OutputFormat::Wav | OutputFormat::Flac | OutputFormat::Aiff
                ) {
                    ui.label(
                        RichText::new("Formato senza perdita: il bitrate non viene applicato.")
                            .small()
                            .weak(),
                    );
                }
                ui.label("Ritaglia intervallo (secondi)");
                optional_seconds(ui, "Da", &mut self.saved.settings.video.trim_start);
                optional_seconds(ui, "A", &mut self.saved.settings.video.trim_end);
            }
            MediaKind::Video => {
                if matches!(
                    self.saved.settings.format,
                    OutputFormat::Mp4 | OutputFormat::Mkv | OutputFormat::Mov
                ) {
                    egui::ComboBox::from_id_salt("codec")
                        .selected_text(match self.saved.settings.video.codec {
                            VideoCodec::H264 => "H.264 · compatibilità",
                            VideoCodec::H265 => "H.265 · file più piccoli",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.saved.settings.video.codec,
                                VideoCodec::H264,
                                "H.264 · compatibilità",
                            );
                            ui.selectable_value(
                                &mut self.saved.settings.video.codec,
                                VideoCodec::H265,
                                "H.265 · file più piccoli",
                            );
                        });
                }
                if self.saved.settings.format != OutputFormat::Gif {
                    ui.label("Qualità video · valori minori = più dettaglio");
                    ui.add(
                        egui::Slider::new(&mut self.saved.settings.video.crf, 16..=35)
                            .prefix("CRF "),
                    );
                }
                width_setting(ui, "video_width", &mut self.saved.settings.video.width);
                let fps = &mut self.saved.settings.video.fps;
                egui::ComboBox::from_id_salt("fps")
                    .selected_text(
                        fps.map(|n| format!("{n} fps"))
                            .unwrap_or("Fotogrammi originali".into()),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(fps, None, "Originali");
                        for n in [24, 25, 30, 50, 60] {
                            ui.selectable_value(fps, Some(n), format!("{n} fps"));
                        }
                    });
                ui.label("Ritaglia intervallo (secondi)");
                optional_seconds(ui, "Da", &mut self.saved.settings.video.trim_start);
                optional_seconds(ui, "A", &mut self.saved.settings.video.trim_end);
                if self.saved.settings.format != OutputFormat::Gif {
                    ui.label(
                        RichText::new("La traccia audio viene conservata quando disponibile.")
                            .small()
                            .weak(),
                    );
                    egui::CollapsingHeader::new("Audio del video").show(ui, |ui| {
                        ui.label("Bitrate audio");
                        ui.add(
                            egui::Slider::new(
                                &mut self.saved.settings.audio.bitrate_kbps,
                                48..=320,
                            )
                            .suffix(" kbps"),
                        );
                        let rates: &[u32] = if self.saved.settings.format == OutputFormat::Webm {
                            &[0, 48000]
                        } else {
                            &[0, 44100, 48000]
                        };
                        let rate = &mut self.saved.settings.audio.sample_rate;
                        egui::ComboBox::from_id_salt("video_sample_rate")
                            .selected_text(if *rate == 0 {
                                "Frequenza originale".into()
                            } else {
                                format!("{rate} Hz")
                            })
                            .show_ui(ui, |ui| {
                                for &value in rates {
                                    ui.selectable_value(
                                        rate,
                                        value,
                                        if value == 0 {
                                            "Originale".into()
                                        } else {
                                            format!("{value} Hz")
                                        },
                                    );
                                }
                            });
                        let channels = &mut self.saved.settings.audio.channels;
                        egui::ComboBox::from_id_salt("video_channels")
                            .selected_text(match channels {
                                1 => "Mono",
                                2 => "Stereo",
                                _ => "Canali originali",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(channels, 0, "Originali");
                                ui.selectable_value(channels, 1, "Mono");
                                ui.selectable_value(channels, 2, "Stereo");
                            });
                        ui.checkbox(
                            &mut self.saved.settings.audio.normalize,
                            "Normalizza volume",
                        );
                    });
                } else {
                    ui.label(
                        RichText::new("Il GIF animato non contiene audio.")
                            .small()
                            .weak(),
                    );
                }
            }
            MediaKind::Image => {
                if self
                    .items
                    .iter()
                    .any(|item| MediaKind::from_path(&item.path) == MediaKind::Pdf)
                {
                    ui.label("Risoluzione delle pagine PDF");
                    ui.add(
                        egui::Slider::new(&mut self.saved.settings.pdf.dpi, 72..=600)
                            .suffix(" DPI"),
                    );
                    ui.label(
                        RichText::new("Un’immagine per pagina, nella cartella di destinazione.")
                            .small()
                            .weak(),
                    );
                } else {
                    if self.saved.settings.format == OutputFormat::Jpg {
                        ui.label("Qualità JPEG");
                        ui.add(
                            egui::Slider::new(&mut self.saved.settings.image.quality, 20..=100)
                                .suffix("%"),
                        );
                    }
                    width_setting(ui, "image_width", &mut self.saved.settings.image.width);
                    if self.saved.settings.format == OutputFormat::Webp {
                        ui.label(
                            RichText::new("WebP viene codificato senza perdita.")
                                .small()
                                .weak(),
                        );
                    }
                }
            }
            MediaKind::Pdf => {
                if self
                    .items
                    .iter()
                    .any(|item| MediaKind::from_path(&item.path) == MediaKind::Image)
                {
                    ui.label("Qualità delle immagini nel PDF");
                    ui.add(
                        egui::Slider::new(&mut self.saved.settings.image.quality, 20..=100)
                            .suffix("%"),
                    );
                    width_setting(ui, "pdf_image_width", &mut self.saved.settings.image.width);
                    ui.label(RichText::new("Una pagina A4 per immagine, con proporzioni mantenute e margine di 24 pt.").small().weak());
                } else {
                    ui.label(
                        RichText::new(
                            "I documenti Office vengono esportati in PDF con LibreOffice.",
                        )
                        .small()
                        .weak(),
                    );
                }
            }
            _ => {
                ui.label(RichText::new("Il testo viene estratto dal PDF. I PDF scansionati richiedono OCR, qui non incluso.").small().weak());
            }
        }
    }

    fn queue_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "File da convertire{}",
                    if self.items.is_empty() {
                        String::new()
                    } else {
                        format!(" · {}", self.items.len())
                    }
                ))
                .size(18.0)
                .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled_ui(!self.running, |ui| {
                    if ui.button("+ Aggiungi file").clicked()
                        && let Some(paths) = rfd::FileDialog::new()
                            .set_title("Scegli i file da convertire")
                            .pick_files()
                    {
                        self.add_files(paths);
                    }
                });
            });
        });
        ui.add_space(6.0);
        if self.items.is_empty() {
            let available = ui.available_size();
            egui::Frame::new()
                .fill(subtle_fill(ui))
                .stroke(egui::Stroke::new(
                    1.0_f32,
                    ui.visuals().widgets.noninteractive.bg_stroke.color,
                ))
                .corner_radius(12)
                .inner_margin(24)
                .show(ui, |ui| {
                    ui.set_min_size(egui::vec2(
                        available.x - 50.0,
                        (available.y - 50.0).max(230.0),
                    ));
                    ui.vertical_centered(|ui| {
                        ui.add_space((available.y * 0.22).max(32.0));
                        ui.label(
                            RichText::new("Trascina qui i tuoi file")
                                .size(23.0)
                                .strong(),
                        );
                        ui.add_space(3.0);
                        ui.label(RichText::new("Audio, video, immagini e PDF.").weak());
                        ui.add_space(12.0);
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Scegli file…").color(Color32::WHITE),
                                )
                                .fill(ACCENT),
                            )
                            .clicked()
                            && let Some(paths) = rfd::FileDialog::new().pick_files()
                        {
                            self.add_files(paths);
                        }
                        ui.add_space(20.0);
                        ui.label(
                            RichText::new("Funziona anche dal tasto destro di Esplora file.")
                                .small()
                                .weak(),
                        );
                    });
                });
            return;
        }
        let mut remove = None;
        egui::ScrollArea::vertical()
            .id_salt("queue")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (index, item) in self.items.iter().enumerate() {
                    egui::Frame::new()
                        .fill(subtle_fill(ui))
                        .corner_radius(9)
                        .inner_margin(13)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                let extension = item
                                    .path
                                    .extension()
                                    .and_then(|ext| ext.to_str())
                                    .unwrap_or("FILE")
                                    .to_ascii_uppercase();
                                egui::Frame::new()
                                    .fill(if ui.visuals().dark_mode {
                                        Color32::from_rgb(28, 48, 68)
                                    } else {
                                        Color32::from_rgb(226, 239, 250)
                                    })
                                    .corner_radius(7)
                                    .inner_margin(9)
                                    .show(ui, |ui| {
                                        ui.set_min_width(36.0);
                                        ui.label(
                                            RichText::new(extension)
                                                .size(11.0)
                                                .color(if ui.visuals().dark_mode {
                                                    Color32::from_rgb(119, 190, 255)
                                                } else {
                                                    ACCENT
                                                })
                                                .strong(),
                                        );
                                    });
                                ui.allocate_ui_with_layout(
                                    egui::vec2((ui.available_width() - 94.0).max(80.0), 42.0),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        let name = item
                                            .path
                                            .file_name()
                                            .unwrap_or_default()
                                            .to_string_lossy();
                                        ui.add(
                                            egui::Label::new(RichText::new(name).strong())
                                                .truncate(),
                                        )
                                        .on_hover_text(item.path.display().to_string());
                                        ui.label(
                                            RichText::new(format!(
                                                "{}  ·  {}",
                                                human_size(item.size),
                                                kind_label(MediaKind::from_path(&item.path))
                                            ))
                                            .small()
                                            .weak(),
                                        );
                                    },
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if !self.running
                                            && ui
                                                .small_button("×")
                                                .on_hover_text("Rimuovi dalla coda")
                                                .clicked()
                                        {
                                            remove = Some(index);
                                        }
                                        match &item.state {
                                            JobState::Waiting => {
                                                ui.label(RichText::new("Pronto").small().weak());
                                            }
                                            JobState::Running(..) => {
                                                ui.spinner();
                                            }
                                            JobState::Complete(outputs) => {
                                                if ui
                                                    .small_button("Apri")
                                                    .on_hover_text("Apri cartella di destinazione")
                                                    .clicked()
                                                    && let Some(parent) =
                                                        outputs.first().and_then(|p| p.parent())
                                                    && let Err(error) =
                                                        platform::open_folder(parent)
                                                {
                                                    self.notice = Some(format!("{error:#}"));
                                                }
                                            }
                                            JobState::Failed(_) => {
                                                ui.colored_label(
                                                    ui.visuals().error_fg_color,
                                                    "Errore",
                                                );
                                            }
                                            JobState::Cancelled => {
                                                ui.label(RichText::new("Annullato").small().weak());
                                            }
                                        }
                                    },
                                );
                            });
                            match &item.state {
                                JobState::Running(fraction, message) => {
                                    ui.add_space(6.0);
                                    ui.add(
                                        egui::ProgressBar::new(*fraction)
                                            .fill(ACCENT)
                                            .animate(true)
                                            .show_percentage(),
                                    );
                                    ui.label(RichText::new(message).small().weak());
                                }
                                JobState::Complete(outputs) => {
                                    ui.label(
                                        RichText::new(if outputs.len() == 1 {
                                            format!(
                                                "Completato · {}",
                                                outputs[0]
                                                    .file_name()
                                                    .unwrap_or_default()
                                                    .to_string_lossy()
                                            )
                                        } else {
                                            format!("Completato · {} file creati", outputs.len())
                                        })
                                        .small()
                                        .color(GREEN),
                                    );
                                }
                                JobState::Failed(message) => {
                                    egui::CollapsingHeader::new("Dettagli dell’errore")
                                        .id_salt(index)
                                        .show(ui, |ui| {
                                            ui.add(
                                                egui::Label::new(RichText::new(message).small())
                                                    .wrap(),
                                            );
                                            if ui.small_button("Copia errore").clicked() {
                                                ui.ctx().copy_text(message.clone());
                                            }
                                        });
                                }
                                _ => {}
                            }
                        });
                    ui.add_space(1.0);
                }
                if !self.running && ui.small_button("Svuota coda").clicked() {
                    remove = Some(usize::MAX);
                }
            });
        if let Some(index) = remove {
            if index == usize::MAX {
                self.items.clear();
            } else {
                self.items.remove(index);
            }
        }
    }

    fn quick_report_panel(&mut self, ui: &mut egui::Ui) {
        let Some(report) = self.quick_report.clone() else {
            return;
        };
        egui::CollapsingHeader::new(format!(
            "Ultima conversione rapida · {} file creati",
            report.successes.len()
        ))
        .id_salt("last_quick_report")
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!("{} · {}", report.timestamp, report.format))
                    .small()
                    .weak(),
            );
            egui::ScrollArea::vertical()
                .id_salt("quick_outputs")
                .max_height(145.0)
                .show(ui, |ui| {
                    for path in &report.successes {
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(
                                        path.file_name().unwrap_or_default().to_string_lossy(),
                                    )
                                    .small(),
                                )
                                .truncate(),
                            )
                            .on_hover_text(display_path(path));
                            if ui.small_button("Apri cartella").clicked()
                                && let Some(parent) = path.parent()
                                && let Err(error) = platform::open_folder(parent)
                            {
                                self.notice = Some(format!("{error:#}"));
                            }
                        });
                    }
                    for error in &report.errors {
                        ui.add(
                            egui::Label::new(
                                RichText::new(error)
                                    .small()
                                    .color(ui.visuals().error_fg_color),
                            )
                            .wrap(),
                        );
                    }
                });
            if ui.small_button("Aggiorna resoconto").clicked() {
                self.quick_report = load_report();
            }
        });
        ui.add_space(4.0);
    }

    fn footer(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                let done = self
                    .items
                    .iter()
                    .filter(|item| matches!(item.state, JobState::Complete(_)))
                    .count();
                let failed = self
                    .items
                    .iter()
                    .filter(|item| matches!(item.state, JobState::Failed(_)))
                    .count();
                if self.running {
                    ui.label(
                        RichText::new(format!(
                            "Conversione in corso · {done}/{} completati",
                            self.items.len()
                        ))
                        .strong(),
                    );
                } else if done > 0 || failed > 0 {
                    ui.label(
                        RichText::new(format!(
                            "{done} completati{}",
                            if failed > 0 {
                                format!(" · {failed} errori")
                            } else {
                                String::new()
                            }
                        ))
                        .strong(),
                    );
                } else {
                    ui.label(
                        RichText::new(if self.items.is_empty() {
                            "Pronto quando vuoi.".into()
                        } else {
                            format!(
                                "{} file · {}",
                                self.items.len(),
                                human_size(self.items.iter().map(|item| item.size).sum())
                            )
                        })
                        .strong(),
                    );
                }
                ui.label(
                    RichText::new(if self.transcribes_audio() {
                        "Trascrizione online con OpenAI. Nessuna sovrascrittura."
                    } else {
                        "Conversioni di formato locali. Nessuna sovrascrittura."
                    })
                    .small()
                    .weak(),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.running {
                    let cancelling = self.cancel.load(Ordering::Relaxed);
                    if ui
                        .add_enabled(
                            !cancelling,
                            egui::Button::new(if cancelling {
                                "Annullamento…"
                            } else {
                                "Annulla"
                            })
                            .min_size(egui::vec2(168.0, 42.0)),
                        )
                        .clicked()
                    {
                        self.cancel.store(true, Ordering::Relaxed);
                    }
                } else {
                    let enabled = !self.items.is_empty() && self.prerequisite().is_none();
                    let label = if self.items.iter().any(|item| {
                        matches!(item.state, JobState::Complete(_) | JobState::Failed(_))
                    }) {
                        "Converti di nuovo"
                    } else {
                        "Converti file"
                    };
                    if ui
                        .add_enabled(
                            enabled,
                            egui::Button::new(RichText::new(label).color(Color32::WHITE).strong())
                                .fill(ACCENT)
                                .min_size(egui::vec2(168.0, 42.0)),
                        )
                        .clicked()
                    {
                        self.start(ctx);
                    }
                }
            });
        });
    }

    fn tools_window(&mut self, ctx: &egui::Context) {
        let mut opened = self.show_tools;
        egui::Window::new("Strumenti di conversione")
            .open(&mut opened)
            .default_width(610.0)
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(RichText::new("Immagini e PDF da immagini, subito disponibili.").strong());
                ui.label("Audio, video, pagine PDF e documenti Office usano strumenti opzionali. Se sono già installati, The Converter li rileva o ti permette di selezionarli.");
                ui.label(RichText::new("Le conversioni standard avvengono sul PC. La trascrizione OpenAI facoltativa invia l’audio al servizio. I siti ufficiali si aprono nel browser.").small().weak());
                ui.add_space(5.0);
                egui::ScrollArea::vertical().id_salt("tools_scroll").max_height(470.0).show(ui, |ui| {
                    self.openai_settings(ui);
                    for (key, label, description, website) in [
                        ("ffmpeg", "FFmpeg", "Necessario per audio, video e GIF da video.", "https://ffmpeg.org/download.html"),
                        ("ffprobe", "FFprobe", "Opzionale: durata dei media e avanzamento più preciso. Di solito è accanto a ffmpeg.exe.", "https://ffmpeg.org/ffprobe.html"),
                        ("pdftoppm", "Poppler · immagini", "Necessario per esportare ogni pagina PDF in PNG o JPEG. Seleziona pdftoppm.exe.", "https://poppler.freedesktop.org/"),
                        ("pdftotext", "Poppler · testo", "Necessario per estrarre testo dai PDF. Seleziona pdftotext.exe. Il riconoscimento OCR non è incluso.", "https://poppler.freedesktop.org/"),
                        ("soffice", "LibreOffice", "Necessario per i documenti Office in PDF. Seleziona soffice.exe nella cartella program di LibreOffice.", "https://www.libreoffice.org/download/download-libreoffice/"),
                    ] {
                        egui::Frame::new().fill(subtle_fill(ui)).corner_radius(7).inner_margin(11).show(ui, |ui| {
                            let path = self.saved.overrides.get(key).cloned().or_else(|| get_tool(&self.tools, key));
                            let valid = path.as_ref().is_some_and(|path| executable_available(path));
                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new(label).strong());
                                ui.label(RichText::new(if valid { "Disponibile" } else { "Non configurato" }).small().color(if valid { GREEN } else { ui.visuals().weak_text_color() }));
                            });
                            ui.add(egui::Label::new(RichText::new(description).small().weak()).wrap());
                            if let Some(path) = path {
                                ui.add(egui::Label::new(RichText::new(display_path(&path)).small()).wrap());
                                if !valid {
                                    ui.colored_label(ui.visuals().error_fg_color, "Il percorso salvato non è un programma .exe disponibile. Seleziona di nuovo lo strumento.");
                                }
                            }
                            ui.horizontal_wrapped(|ui| {
                                if ui.small_button("Seleziona programma…").clicked()
                                    && let Some(path) = rfd::FileDialog::new()
                                        .set_title(format!("Seleziona {key}.exe già installato"))
                                        .set_file_name(format!("{key}.exe"))
                                        .add_filter("Programmi Windows", &["exe"])
                                        .pick_file()
                                {
                                    if executable_available(&path) {
                                        self.saved.overrides.insert(key.into(), path);
                                        self.tools = ToolPaths::configured(&self.saved.overrides);
                                        self.settings_dirty = true;
                                    } else {
                                        self.notice = Some("Seleziona un programma esistente con estensione .exe.".into());
                                    }
                                }
                                if self.saved.overrides.contains_key(key)
                                    && ui.small_button("Usa rilevamento automatico").clicked()
                                {
                                    self.saved.overrides.remove(key);
                                    self.tools = ToolPaths::configured(&self.saved.overrides);
                                    self.settings_dirty = true;
                                }
                                if ui.link("Sito ufficiale ↗").on_hover_text(website).clicked()
                                    && let Err(error) = platform::open_website(website)
                                {
                                    self.notice = Some(format!("{error:#}"));
                                }
                            });
                        });
                    }
                    egui::CollapsingHeader::new("Cartella personale degli strumenti").show(ui, |ui| {
                        ui.label(RichText::new("Puoi usare gli strumenti già presenti in questa cartella personale. Non occorre modificare la cartella di installazione dell’app.").small().weak());
                        if let Some(path) = the_converter::tools::user_tools_dir() {
                            ui.add(egui::Label::new(RichText::new(display_path(&path)).small()).wrap());
                            if ui.small_button("Copia percorso").clicked() { ui.ctx().copy_text(display_path(&path)); }
                        }
                        ui.label(RichText::new("Sono riconosciute anche le cartelle bin, ffmpeg/bin e poppler/Library/bin. Le DLL necessarie devono restare insieme ai rispettivi programmi.").small().weak());
                    });
                });
                ui.separator();
                if ui.button("Rileva strumenti installati").clicked() {
                    self.tools = ToolPaths::configured(&self.saved.overrides);
                    self.api_key_configured = transcription::has_api_key();
                }
            });
        self.show_tools = opened;
        if !opened {
            self.api_key_input.zeroize();
        }
    }

    fn openai_settings(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new().fill(subtle_fill(ui)).corner_radius(7).inner_margin(11).show(ui, |ui| {
            ui.label(RichText::new("OpenAI · audio in testo").strong());
            ui.label(RichText::new("Facoltativo. L’audio viene inviato a OpenAI solo quando scegli TXT per una registrazione. I costi API sono a carico del tuo account; l’abbonamento ChatGPT non li include.").small());
            ui.label(RichText::new("La chiave viene protetta da Windows per il tuo utente e non viene inclusa nelle preferenze, nella coda o nei resoconti.").small().weak());
            ui.label(if self.api_key_configured { "Chiave salvata · trascrizione disponibile" } else { "Nessuna chiave salvata" });
            ui.add_enabled_ui(!self.running, |ui| {
                ui.add(egui::TextEdit::singleline(&mut self.api_key_input).password(true).hint_text(if self.api_key_configured { "Nuova chiave API per sostituire quella salvata" } else { "Incolla la tua chiave API OpenAI" }).desired_width(f32::INFINITY));
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(!self.api_key_input.trim().is_empty(), egui::Button::new("Salva e abilita trascrizione")).clicked() {
                        match transcription::save_api_key(self.api_key_input.trim()) {
                            Ok(()) => {
                                self.api_key_configured = true;
                                self.api_key_status = Some("Chiave salvata. Nel menu degli audio è disponibile TXT con OpenAI; riapri il menu se era già aperto.".into());
                                self.api_key_input.zeroize();
                            }
                            Err(error) => self.api_key_status = Some(format!("{error:#}")),
                        }
                    }
                    if self.api_key_configured && ui.button("Rimuovi chiave").clicked() {
                        match transcription::remove_api_key() {
                            Ok(()) => {
                                self.api_key_configured = false;
                                self.api_key_input.zeroize();
                                self.api_key_status = Some("Chiave rimossa. Le nuove trascrizioni sono disabilitate.".into());
                                let formats = self.common_formats();
                                if !formats.is_empty() && !formats.contains(&self.saved.settings.format) {
                                    self.saved.settings.format = formats[0];
                                    self.settings_dirty = true;
                                }
                            }
                            Err(error) => self.api_key_status = Some(format!("{error:#}")),
                        }
                    }
                    if ui.link("Crea una chiave API ↗").clicked()
                        && let Err(error) = platform::open_website("https://platform.openai.com/api-keys") {
                        self.api_key_status = Some(format!("{error:#}"));
                    }
                });
            });
            if let Some(status) = &self.api_key_status { ui.label(RichText::new(status).small()); }
            ui.label(RichText::new("Modello GPT-Transcribe · trascrizione nella lingua parlata.").small().weak());
        });
    }
}

impl eframe::App for ConverterApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.receive_events();
        self.quick_queue.refresh();
        if !self.running {
            let dropped: Vec<_> = ctx.input(|input| {
                input
                    .raw
                    .dropped_files
                    .iter()
                    .filter_map(|file| file.path.clone())
                    .collect()
            });
            if !dropped.is_empty() {
                self.add_files(dropped);
            }
        }
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(egui::Margin::symmetric(24, 19)),
            )
            .show(ctx, |ui| self.header(ui));
        egui::TopBottomPanel::bottom("footer")
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(egui::Margin::symmetric(24, 17)),
            )
            .show(ctx, |ui| self.footer(ui, ctx));
        egui::SidePanel::right("settings")
            .exact_width(314.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(20),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("settings_scroll")
                    .show(ui, |ui| {
                        ui.add_enabled_ui(!self.running, |ui| self.settings_panel(ui));
                    });
            });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.extreme_bg_color)
                    .inner_margin(22),
            )
            .show(ctx, |ui| {
                if let Some(notice) = self.notice.clone() {
                    egui::Frame::new()
                        .fill(subtle_fill(ui))
                        .corner_radius(7)
                        .inner_margin(10)
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(&notice);
                                if ui.small_button("Chiudi").clicked() {
                                    self.notice = None;
                                }
                            });
                        });
                    ui.add_space(5.0);
                }
                self.quick_queue.show(ui);
                self.quick_report_panel(ui);
                self.queue_panel(ui);
            });
        self.tools_window(ctx);
        if self.settings_dirty && !self.running {
            self.persist();
        }
        if self.running {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        ctx.request_repaint_after(QUEUE_REFRESH);
        if ctx.input(|input| !input.raw.hovered_files.is_empty()) {
            let screen = ctx.content_rect();
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("drop_overlay"),
            ));
            painter.rect_filled(
                screen,
                0.0,
                Color32::from_rgba_unmultiplied(0, 55, 110, 220),
            );
            painter.text(
                screen.center(),
                egui::Align2::CENTER_CENTER,
                if self.running {
                    "Attendi il termine della conversione"
                } else {
                    "Rilascia per aggiungere alla coda"
                },
                egui::FontId::proportional(27.0),
                Color32::WHITE,
            );
        }
        if self.running && ctx.input(|input| input.viewport().close_requested()) {
            self.cancel.store(true, Ordering::Relaxed);
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.notice =
                Some("Annullamento della conversione. Chiudi la finestra al termine.".into());
        }
    }
}

impl Drop for ConverterApp {
    fn drop(&mut self) {
        self.api_key_input.zeroize();
        self.cancel.store(true, Ordering::Relaxed);
    }
}

const QUEUE_REFRESH: std::time::Duration = std::time::Duration::from_millis(500);

#[derive(Default)]
struct QuickQueueView {
    jobs: Vec<QueueJob>,
    last_refresh: Option<std::time::Instant>,
    error: Option<String>,
    action_error: Option<String>,
}

impl QuickQueueView {
    fn refresh(&mut self) {
        if self
            .last_refresh
            .is_some_and(|time| time.elapsed() < QUEUE_REFRESH)
        {
            return;
        }
        self.last_refresh = Some(std::time::Instant::now());
        match queue::list_jobs() {
            Ok(jobs) => {
                self.jobs = jobs;
                self.error = None;
            }
            Err(error) => self.error = Some(format!("Impossibile leggere la coda: {error:#}")),
        }
    }

    fn show(&mut self, ui: &mut egui::Ui) {
        if self.jobs.is_empty() && self.error.is_none() && self.action_error.is_none() {
            return;
        }
        let active = self.jobs.iter().filter(|job| quick_job_active(job)).count();
        let title = if active == 0 {
            "Coda conversioni rapide".to_string()
        } else {
            format!("Coda conversioni rapide · {active} in corso o in attesa")
        };
        let mut cancel = None;
        egui::CollapsingHeader::new(title)
            .id_salt("persistent_quick_queue")
            .default_open(active > 0)
            .show(ui, |ui| {
                for error in self.error.iter().chain(self.action_error.iter()) {
                    ui.add(
                        egui::Label::new(
                            RichText::new(error)
                                .small()
                                .color(ui.visuals().error_fg_color),
                        )
                        .wrap(),
                    );
                }
                ui.label(
                    RichText::new("La conversione continua anche chiudendo questa finestra.")
                        .small()
                        .weak(),
                );
                egui::ScrollArea::vertical()
                    .id_salt("persistent_quick_queue_jobs")
                    .max_height(250.0)
                    .show(ui, |ui| {
                        // list_jobs returns newest first; show pending work in processing order.
                        let active_jobs = self
                            .jobs
                            .iter()
                            .rev()
                            .filter(|job| quick_job_active(job))
                            .take(50);
                        let finished_jobs = self
                            .jobs
                            .iter()
                            .filter(|job| !quick_job_active(job))
                            .take(50);
                        for job in active_jobs.chain(finished_jobs) {
                            egui::Frame::new()
                                .fill(subtle_fill(ui))
                                .corner_radius(7)
                                .inner_margin(10)
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(
                                                    job.input
                                                        .file_name()
                                                        .unwrap_or_default()
                                                        .to_string_lossy(),
                                                )
                                                .strong(),
                                            )
                                            .truncate(),
                                        )
                                        .on_hover_text(display_path(&job.input));
                                        ui.label(
                                            RichText::new(format!("→ {}", job.format))
                                                .small()
                                                .weak(),
                                        );
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if quick_job_active(job)
                                                    && ui.small_button("Annulla").clicked()
                                                {
                                                    cancel = Some(job.id.clone());
                                                }
                                                if ui.small_button("Apri cartella").clicked() {
                                                    self.action_error = open_job_folder(job)
                                                        .err()
                                                        .map(|error| format!("{error:#}"));
                                                }
                                            },
                                        );
                                    });
                                    ui.label(
                                        RichText::new(quick_state_label(&job.state))
                                            .small()
                                            .color(quick_state_color(&job.state, ui)),
                                    );
                                    if matches!(job.state, QuickJobState::Running) {
                                        ui.add(
                                            egui::ProgressBar::new(quick_progress(job.progress))
                                                .animate(true)
                                                .show_percentage(),
                                        );
                                    }
                                    if let Some(path) = job.outputs.first() {
                                        let label = if job.outputs.len() > 1 {
                                            format!(
                                                "{} · {} file",
                                                display_path(path),
                                                job.outputs.len()
                                            )
                                        } else {
                                            display_path(path)
                                        };
                                        ui.add(
                                            egui::Label::new(RichText::new(label).small().weak())
                                                .truncate(),
                                        )
                                        .on_hover_text(
                                            job.outputs
                                                .iter()
                                                .map(|path| display_path(path))
                                                .collect::<Vec<_>>()
                                                .join("\n"),
                                        );
                                    }
                                    if let Some(error) = &job.error {
                                        egui::CollapsingHeader::new("Dettagli dell’errore")
                                            .id_salt((&job.id, "error"))
                                            .show(ui, |ui| {
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(error)
                                                            .small()
                                                            .color(ui.visuals().error_fg_color),
                                                    )
                                                    .wrap(),
                                                );
                                            });
                                    }
                                });
                        }
                    });
            });
        if let Some(id) = cancel {
            self.action_error = queue::cancel_job(&id)
                .err()
                .map(|error| format!("Impossibile annullare: {error:#}"));
            self.last_refresh = None;
        }
        ui.add_space(4.0);
    }
}

struct QueueStatusApp {
    id: String,
    job: Option<QueueJob>,
    last_refresh: Option<std::time::Instant>,
    error: Option<String>,
    action_error: Option<String>,
    cancel_sent: bool,
}

impl QueueStatusApp {
    fn new(id: String) -> Self {
        Self {
            id,
            job: None,
            last_refresh: None,
            error: None,
            action_error: None,
            cancel_sent: false,
        }
    }

    fn refresh(&mut self) {
        if self
            .last_refresh
            .is_some_and(|time| time.elapsed() < QUEUE_REFRESH)
        {
            return;
        }
        self.last_refresh = Some(std::time::Instant::now());
        match queue::get_job(&self.id) {
            Ok(job) => {
                self.job = Some(job);
                self.error = None;
            }
            Err(error) => self.error = Some(format!("Stato non disponibile: {error:#}")),
        }
    }
}

impl eframe::App for QueueStatusApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.refresh();
        let mut cancel_requested = false;
        egui::TopBottomPanel::bottom("queue_status_actions")
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(18),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Chiudi").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if let Some(job) = &self.job
                            && quick_job_active(job)
                            && ui
                                .add_enabled(
                                    !self.cancel_sent,
                                    egui::Button::new(if self.cancel_sent {
                                        "Annullamento…"
                                    } else {
                                        "Annulla"
                                    }),
                                )
                                .clicked()
                        {
                            cancel_requested = true;
                        }
                    });
                });
                ui.add(
                    egui::Label::new(
                        RichText::new("Chiudere questa finestra lascia proseguire la coda.")
                            .small()
                            .weak(),
                    )
                    .wrap(),
                );
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(ctx.style().visuals.extreme_bg_color).inner_margin(24))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().id_salt("queue_status_content").show(ui, |ui| {
                    ui.label(RichText::new("Stato della conversione").size(23.0).strong());
                    ui.add_space(6.0);
                    for error in self.error.iter().chain(self.action_error.iter()) {
                        ui.add(egui::Label::new(RichText::new(error).color(ui.visuals().error_fg_color)).wrap());
                    }
                    if let Some(job) = &self.job {
                        ui.label(RichText::new(job.input.file_name().unwrap_or_default().to_string_lossy()).size(17.0).strong());
                        ui.label(RichText::new(format!("Formato di destinazione: {}", job.format)).small().weak());
                        ui.add_space(9.0);
                        ui.horizontal(|ui| {
                            if quick_job_active(job) { ui.spinner(); }
                            ui.label(RichText::new(quick_state_label(&job.state)).strong().color(quick_state_color(&job.state, ui)));
                        });
                        if matches!(job.state, QuickJobState::Running) {
                            ui.add(egui::ProgressBar::new(quick_progress(job.progress)).animate(true).show_percentage());
                        }
                        if !job.message.is_empty() { ui.add(egui::Label::new(&job.message).wrap()); }
                        if self.cancel_sent && quick_job_active(job) {
                            ui.label(RichText::new("Annullamento richiesto. Attendi la pulizia dei file temporanei.").small().weak());
                        }
                        ui.add_space(11.0);
                        ui.separator();
                        ui.label(RichText::new("Destinazione").strong());
                        for output in &job.outputs {
                            ui.add(egui::Label::new(RichText::new(display_path(output)).small()).wrap());
                        }
                        if ui.button("Apri cartella di destinazione").clicked() { self.action_error = open_job_folder(job).err().map(|error| format!("{error:#}")); }
                        if matches!(job.state, QuickJobState::Queued | QuickJobState::Running) {
                            ui.label(RichText::new("Il segnaposto con l’icona di lavorazione viene sostituito dal file pronto al termine.").small().weak());
                        }
                        if let Some(error) = &job.error {
                            ui.add_space(7.0);
                            ui.add(egui::Label::new(RichText::new(error).color(ui.visuals().error_fg_color)).wrap());
                            if ui.small_button("Copia errore").clicked() { ctx.copy_text(error.clone()); }
                        }
                    } else if self.error.is_none() {
                        ui.spinner();
                        ui.label("Lettura della coda…");
                    }
                });
            });
        if cancel_requested {
            match queue::cancel_job(&self.id) {
                Ok(()) => {
                    self.cancel_sent = true;
                    self.action_error = None;
                }
                Err(error) => self.action_error = Some(format!("Impossibile annullare: {error:#}")),
            }
            self.last_refresh = None;
        }
        ctx.request_repaint_after(QUEUE_REFRESH);
    }
}

fn quick_job_active(job: &QueueJob) -> bool {
    matches!(job.state, QuickJobState::Queued | QuickJobState::Running)
}

fn quick_state_label(state: &QuickJobState) -> &'static str {
    match state {
        QuickJobState::Queued => "In coda",
        QuickJobState::Running => "Conversione in corso",
        QuickJobState::Completed => "Completato",
        QuickJobState::Failed => "Conversione non riuscita",
        QuickJobState::Cancelled => "Annullato",
    }
}

fn quick_state_color(state: &QuickJobState, ui: &egui::Ui) -> Color32 {
    match state {
        QuickJobState::Completed => GREEN,
        QuickJobState::Failed => ui.visuals().error_fg_color,
        QuickJobState::Running => ACCENT,
        _ => ui.visuals().weak_text_color(),
    }
}

fn quick_progress(progress: f32) -> f32 {
    if progress.is_finite() {
        progress.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn open_job_folder(job: &QueueJob) -> anyhow::Result<()> {
    let folder = job
        .outputs
        .first()
        .and_then(|path| path.parent())
        .or_else(|| job.input.parent())
        .ok_or_else(|| anyhow::anyhow!("La cartella della conversione non è disponibile"))?;
    platform::open_folder(folder)
}

fn settings_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "SalvatoreCorsi", "TheConverter")
        .map(|dirs| dirs.config_dir().join("preferences.json"))
}

fn subtle_fill(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(35, 38, 43)
    } else {
        Color32::from_rgb(248, 249, 251)
    }
}

fn human_size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

fn kind_label(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Audio => "Audio",
        MediaKind::Video => "Video",
        MediaKind::Image => "Immagine",
        MediaKind::Pdf => "Documento PDF",
        MediaKind::Document => "Documento",
        MediaKind::Unknown => "File",
    }
}

fn display_path(path: &Path) -> String {
    path.display()
        .to_string()
        .trim_start_matches(r"\\?\")
        .to_string()
}

fn width_setting(ui: &mut egui::Ui, id: &str, width: &mut Option<u32>) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(
            width
                .map(|n| format!("Larghezza {n} px"))
                .unwrap_or("Dimensioni originali".into()),
        )
        .show_ui(ui, |ui| {
            ui.selectable_value(width, None, "Originali");
            for n in [320, 640, 1280, 1920, 2560, 3840] {
                ui.selectable_value(width, Some(n), format!("{n} px di larghezza"));
            }
        });
    if let Some(width) = width {
        let minimum = if id == "video_width" { 16 } else { 1 };
        ui.horizontal(|ui| {
            ui.label("Larghezza");
            ui.add(
                egui::DragValue::new(width)
                    .range(minimum..=16384)
                    .suffix(" px"),
            );
        });
    }
    ui.label(
        RichText::new("Le proporzioni vengono mantenute.")
            .small()
            .weak(),
    );
}

fn quality_applicable(format: OutputFormat) -> bool {
    matches!(
        format,
        OutputFormat::Mp3
            | OutputFormat::Aac
            | OutputFormat::Ogg
            | OutputFormat::Opus
            | OutputFormat::Mp4
            | OutputFormat::Mkv
            | OutputFormat::Mov
            | OutputFormat::Webm
            | OutputFormat::Avi
            | OutputFormat::Jpg
            | OutputFormat::Pdf
    )
}

fn optional_seconds(ui: &mut egui::Ui, label: &str, value: &mut Option<f64>) {
    ui.horizontal(|ui| {
        let mut enabled = value.is_some();
        if ui.checkbox(&mut enabled, label).changed() {
            *value = if enabled {
                Some(if label == "A" { 1.0 } else { 0.0 })
            } else {
                None
            };
        }
        if let Some(seconds) = value {
            ui.add(
                egui::DragValue::new(seconds)
                    .speed(0.1)
                    .range(0.0..=86400.0)
                    .suffix(" s"),
            );
        }
    });
}

fn get_tool(tools: &ToolPaths, key: &str) -> Option<PathBuf> {
    match key {
        "ffmpeg" => tools.ffmpeg.clone(),
        "ffprobe" => tools.ffprobe.clone(),
        "pdftoppm" => tools.pdftoppm.clone(),
        "pdftotext" => tools.pdftotext.clone(),
        "soffice" => tools.soffice.clone(),
        _ => None,
    }
}

const TRANSCRIPTION_LANGUAGES: &[(&str, &str)] = &[
    ("it", "Italiano"),
    ("en", "Inglese"),
    ("es", "Spagnolo"),
    ("fr", "Francese"),
    ("de", "Tedesco"),
    ("pt", "Portoghese"),
    ("ja", "Giapponese"),
    ("zh", "Cinese"),
];

fn transcription_language_label(language: Option<&str>) -> &str {
    match language {
        None => "Rileva automaticamente",
        Some(code) => TRANSCRIPTION_LANGUAGES
            .iter()
            .find(|(value, _)| *value == code)
            .map_or(code, |(_, label)| *label),
    }
}
