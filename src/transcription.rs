//! Explicit, opt-in audio transcription. Credentials never enter serialized job/settings data.
use crate::{
    engine::{self, Progress},
    model::ConversionSettings,
    tools::ToolPaths,
};
use anyhow::{Context, Result, bail};
use reqwest::{
    Client, StatusCode,
    header::{AUTHORIZATION, HeaderValue},
    multipart::{Form, Part},
};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::AtomicBool,
    time::Duration,
};
use zeroize::Zeroizing;

pub const CREDENTIAL_TARGET: &str = "Corsivo.TheConverter.OpenAI";
const ENDPOINT: &str = "https://api.openai.com/v1/audio/transcriptions";
const MODEL: &str = "gpt-transcribe";
// Leave room below the API's 25MB limit, including multipart framing.
const MAX_UPLOAD_BYTES: u64 = 24_000_000;
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_ERROR_BYTES: usize = 64 * 1024;
const CHUNK_SECONDS: u32 = 1200;

pub fn has_api_key() -> bool {
    credentials::has(CREDENTIAL_TARGET)
}

pub fn save_api_key(key: &str) -> Result<()> {
    credentials::save(CREDENTIAL_TARGET, validated_key(key)?)
}

pub fn remove_api_key() -> Result<()> {
    credentials::remove(CREDENTIAL_TARGET)
}

fn validated_key(key: &str) -> Result<&str> {
    let key = key.trim();
    if !(20..=2048).contains(&key.len())
        || !key.starts_with("sk-")
        || !key.bytes().all(|value| (0x21..=0x7e).contains(&value))
    {
        bail!(
            "Formato della chiave API OpenAI non valido. Incolla una chiave API completa, senza spazi interni."
        );
    }
    Ok(key)
}

pub fn validate_language(language: &str) -> Result<()> {
    let language = language.trim();
    if !language.is_empty()
        && (language.len() != 2 || !language.bytes().all(|c| c.is_ascii_alphabetic()))
    {
        bail!(
            "Indica la lingua con un codice ISO 639-1 di due lettere, per esempio it o en, oppure usa il rilevamento automatico."
        );
    }
    Ok(())
}

#[cfg(windows)]
mod credentials {
    use super::*;
    use windows_sys::Win32::Security::Credentials::*;
    use zeroize::Zeroize;

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }
    struct Credential(*mut CREDENTIALW);
    impl Drop for Credential {
        fn drop(&mut self) {
            unsafe {
                let value = &mut *self.0;
                if !value.CredentialBlob.is_null()
                    && value.CredentialBlobSize <= CRED_MAX_CREDENTIAL_BLOB_SIZE
                {
                    std::slice::from_raw_parts_mut(
                        value.CredentialBlob,
                        value.CredentialBlobSize as usize,
                    )
                    .zeroize();
                }
                CredFree(self.0.cast());
            }
        }
    }
    fn read(target: &str) -> Result<Option<Credential>> {
        let target = wide(target);
        let mut value = std::ptr::null_mut();
        if unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut value) } == 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(1168) {
                return Ok(None);
            }
            bail!("Impossibile leggere la chiave da Gestione credenziali di Windows.");
        }
        if value.is_null() {
            bail!("Credenziale OpenAI non disponibile.");
        }
        Ok(Some(Credential(value)))
    }
    fn valid(value: &CREDENTIALW) -> bool {
        value.Type == CRED_TYPE_GENERIC
            && !value.CredentialBlob.is_null()
            && (20..=2048).contains(&value.CredentialBlobSize)
    }
    pub(super) fn has(target: &str) -> bool {
        read(target)
            .ok()
            .flatten()
            .is_some_and(|value| unsafe { valid(&*value.0) })
    }
    pub(super) fn load(target: &str) -> Result<Zeroizing<String>> {
        let credential = read(target)?
            .context("Aggiungi una chiave API OpenAI in Strumenti per attivare la trascrizione.")?;
        let credential = unsafe { &*credential.0 };
        if !valid(credential) {
            bail!(
                "La chiave OpenAI salvata non è valida. Rimuovila e inseriscila nuovamente in Strumenti."
            );
        }
        let bytes = unsafe {
            std::slice::from_raw_parts(
                credential.CredentialBlob,
                credential.CredentialBlobSize as usize,
            )
        };
        let key = std::str::from_utf8(bytes)
            .map_err(|_| anyhow::anyhow!("La chiave OpenAI salvata non è valida."))?;
        validated_key(key)?;
        Ok(Zeroizing::new(key.to_owned()))
    }
    pub(super) fn save(target: &str, key: &str) -> Result<()> {
        let mut target = wide(target);
        let mut username = wide("OpenAI");
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: target.as_mut_ptr(),
            CredentialBlobSize: key.len() as u32,
            CredentialBlob: key.as_ptr().cast_mut(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: username.as_mut_ptr(),
            ..Default::default()
        };
        if unsafe { CredWriteW(&credential, 0) } == 0 {
            bail!("Impossibile salvare la chiave in Gestione credenziali di Windows.");
        }
        Ok(())
    }
    pub(super) fn remove(target: &str) -> Result<()> {
        let target = wide(target);
        if unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) } == 0
            && std::io::Error::last_os_error().raw_os_error() != Some(1168)
        {
            bail!("Impossibile rimuovere la chiave da Gestione credenziali di Windows.");
        }
        Ok(())
    }
    #[cfg(test)]
    #[test]
    fn isolated_credential_roundtrip_does_not_touch_the_user_credential() {
        let target = format!(
            "Corsivo.TheConverter.Test.{}.{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = remove(&self.0);
            }
        }
        let _cleanup = Cleanup(target.clone());
        let key = "sk-local-credential-test-not-a-real-key";
        save(&target, key).unwrap();
        assert!(has(&target));
        let loaded = load(&target).unwrap();
        assert_eq!(loaded.as_str(), key);
        remove(&target).unwrap();
        assert!(!has(&target));
        remove(&target).unwrap();
    }
}

#[cfg(not(windows))]
mod credentials {
    use super::*;
    pub(super) fn has(_: &str) -> bool {
        false
    }
    pub(super) fn load(_: &str) -> Result<Zeroizing<String>> {
        bail!("La trascrizione richiede Gestione credenziali di Windows.")
    }
    pub(super) fn save(_: &str, _: &str) -> Result<()> {
        bail!("Il salvataggio protetto della chiave è disponibile su Windows.")
    }
    pub(super) fn remove(_: &str) -> Result<()> {
        Ok(())
    }
}

fn client_builder() -> reqwest::ClientBuilder {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(600))
        .user_agent(concat!("TheConverter/", env!("CARGO_PKG_VERSION")))
}

/// Upload only after the caller explicitly selected audio -> TXT and configured a credential.
pub fn transcribe(
    input: &Path,
    dir: &Path,
    settings: &ConversionSettings,
    tools: &ToolPaths,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(Progress),
) -> Result<PathBuf> {
    engine::cancelled(cancel)?;
    if let Some(language) = &settings.transcription.language {
        validate_language(language)?;
    }
    let key = credentials::load(CREDENTIAL_TARGET)?;
    let stage = tempfile::Builder::new()
        .prefix(".the-converter-transcription-")
        .tempdir_in(dir)?;
    let parts = prepare_audio(input, stage.path(), tools, cancel, progress)?;
    engine::cancelled(cancel)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| anyhow::anyhow!("Impossibile avviare la connessione per la trascrizione."))?;
    runtime.block_on(async {
        let client = client_builder().https_only(true).build().map_err(|_| {
            anyhow::anyhow!("Impossibile inizializzare la connessione sicura a OpenAI.")
        })?;
        transcribe_parts(
            &client, ENDPOINT, &key, &parts, input, dir, settings, cancel, progress,
        )
        .await
    })
}

fn direct_format(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "flac" | "mp3" | "mp4" | "mpeg" | "mpga" | "m4a" | "ogg" | "wav" | "webm"
    )
}

fn prepare_audio(
    input: &Path,
    stage: &Path,
    tools: &ToolPaths,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(Progress),
) -> Result<Vec<PathBuf>> {
    let length = fs::metadata(input)?.len();
    if length == 0 {
        bail!("Il file audio è vuoto.");
    }
    if direct_format(input) && length <= MAX_UPLOAD_BYTES {
        return Ok(vec![input.to_owned()]);
    }
    let ffmpeg=tools.ffmpeg.as_ref().filter(|path|crate::tools::executable_available(path))
        .context("Per preparare questo formato o un audio superiore a 24 MB serve FFmpeg. Seleziona ffmpeg.exe in Strumenti prima di trascrivere.")?;
    progress(Progress {
        fraction: 0.02,
        message: "Preparazione locale dell’audio per OpenAI…".into(),
    });
    let mut command = Command::new(ffmpeg);
    command
        .args([
            "-hide_banner",
            "-nostdin",
            "-loglevel",
            "error",
            "-y",
            "-progress",
            "pipe:1",
            "-nostats",
            "-protocol_whitelist",
            "file,pipe",
            "-i",
        ])
        .arg(input)
        .args([
            "-map",
            "0:a:0",
            "-vn",
            "-c:a",
            "libmp3lame",
            "-b:a",
            "64k",
            "-ar",
            "16000",
            "-ac",
            "1",
            "-f",
            "segment",
            "-segment_time",
            &CHUNK_SECONDS.to_string(),
            "-reset_timestamps",
            "1",
        ])
        .arg(stage.join("audio-%05d.mp3"));
    engine::run_process(command, cancel, None, &mut |value| {
        progress(Progress {
            fraction: 0.05,
            message: value.message,
        })
    })?;
    engine::cancelled(cancel)?;
    let mut parts = fs::read_dir(stage)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    parts.retain(|path| {
        path.extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("mp3"))
    });
    parts.sort();
    if parts.is_empty() || parts.len() > 1000 {
        bail!("Audio non preparabile per OpenAI. Dividi la registrazione in file più brevi.");
    }
    for part in &parts {
        let length = fs::metadata(part)?.len();
        if length == 0 || length > MAX_UPLOAD_BYTES {
            bail!(
                "Un segmento audio supera il limite di caricamento. Dividi la registrazione in file più brevi."
            );
        }
    }
    Ok(parts)
}

fn read_upload(path: &Path, cancel: &AtomicBool) -> Result<Vec<u8>> {
    let mut file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 65536];
    loop {
        engine::cancelled(cancel)?;
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        if bytes.len() + count > MAX_UPLOAD_BYTES as usize {
            bail!("Il segmento audio supera il limite di caricamento di 24 MB.");
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    if bytes.is_empty() {
        bail!("Il file audio è vuoto.");
    }
    Ok(bytes)
}

fn mime(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "m4a" | "mp4" => "audio/mp4",
        "ogg" => "audio/ogg",
        "webm" => "audio/webm",
        _ => "audio/mpeg",
    }
}

#[allow(clippy::too_many_arguments)]
async fn transcribe_parts(
    client: &Client,
    endpoint: &str,
    key: &str,
    parts: &[PathBuf],
    input: &Path,
    dir: &Path,
    settings: &ConversionSettings,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(Progress),
) -> Result<PathBuf> {
    let mut output = engine::temporary(dir, "txt")?;
    let mut wrote_text = false;
    for (index, path) in parts.iter().enumerate() {
        engine::cancelled(cancel)?;
        progress(Progress {
            fraction: 0.1 + 0.85 * index as f32 / parts.len() as f32,
            message: format!(
                "Trascrizione GPT-Transcribe · parte {} di {} · invio a OpenAI…",
                index + 1,
                parts.len()
            ),
        });
        let bytes = read_upload(path, cancel)?;
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("mp3")
            .to_ascii_lowercase();
        // A generic filename avoids sending the user's original local filename.
        let part = Part::bytes(bytes)
            .file_name(format!("audio.{extension}"))
            .mime_str(mime(path))
            .map_err(|_| anyhow::anyhow!("Formato audio non valido per OpenAI."))?;
        let mut form = Form::new()
            .text("model", MODEL)
            .text("response_format", "json")
            .part("file", part);
        if let Some(language) = settings
            .transcription
            .language
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            // Keep persisted single-language settings compatible, using the model's array field.
            form = form.text("languages[]", language.to_ascii_lowercase());
        }
        let bearer = Zeroizing::new(format!("Bearer {key}"));
        let mut authorization = HeaderValue::from_bytes(bearer.as_bytes())
            .map_err(|_| anyhow::anyhow!("La chiave API salvata non è valida."))?;
        authorization.set_sensitive(true);
        let request = client
            .post(endpoint)
            .header(AUTHORIZATION, authorization)
            .multipart(form);
        let response = wait_http(request.send(), cancel).await?;
        let status = response.status();
        let body = read_response(
            response,
            if status.is_success() {
                MAX_RESPONSE_BYTES
            } else {
                MAX_ERROR_BYTES
            },
            cancel,
        )
        .await?;
        if !status.is_success() {
            return Err(api_error(status, &body));
        }
        let text = parse_transcript(&body)?;
        if !text.is_empty() {
            if wrote_text {
                output.as_file_mut().write_all(b"\n\n")?;
            }
            output.as_file_mut().write_all(text.as_bytes())?;
            wrote_text = true;
        }
        progress(Progress {
            fraction: 0.1 + 0.85 * (index + 1) as f32 / parts.len() as f32,
            message: format!(
                "Trascrizione GPT-Transcribe · parte {} di {} completata",
                index + 1,
                parts.len()
            ),
        });
    }
    engine::cancelled(cancel)?;
    if !wrote_text {
        bail!(
            "OpenAI non ha restituito testo dalla registrazione. Nessun file di testo è stato creato."
        );
    }
    output.as_file_mut().write_all(b"\n")?;
    output.as_file_mut().sync_all()?;
    engine::cancelled(cancel)?;
    engine::commit(output, dir, &engine::stem(input), "txt")
}

fn parse_transcript(body: &[u8]) -> Result<String> {
    #[derive(serde::Deserialize)]
    struct TranscriptResponse {
        text: String,
    }
    // serde structs can also accept positional sequences; the API contract requires an object.
    if body.iter().find(|byte| !byte.is_ascii_whitespace()) != Some(&b'{') {
        bail!(
            "OpenAI ha restituito un risultato di trascrizione non valido. Nessun file di testo è stato creato."
        );
    }
    // Detected-language metadata is optional and not needed to write the transcript.
    // In particular, languages: [] does not imply silence or a failed transcription.
    let response: TranscriptResponse = serde_json::from_slice(body).map_err(|_| {
        anyhow::anyhow!("OpenAI ha restituito un risultato di trascrizione non valido. Nessun file di testo è stato creato.")
    })?;
    // Silence in one chunk is valid; the caller rejects only an entirely empty transcript.
    Ok(response.text.trim().to_owned())
}

async fn wait_http<T>(
    future: impl std::future::Future<Output = std::result::Result<T, reqwest::Error>>,
    cancel: &AtomicBool,
) -> Result<T> {
    tokio::pin!(future);
    loop {
        engine::cancelled(cancel)?;
        tokio::select! {
            result=&mut future=>return result.map_err(|error|if error.is_timeout(){anyhow::anyhow!("Tempo di attesa OpenAI scaduto. Nessun nuovo tentativo è stato avviato automaticamente.")}else{anyhow::anyhow!("Connessione a OpenAI interrotta o non disponibile. Controlla la rete; nessun nuovo tentativo è stato avviato automaticamente.")}),
            _=tokio::time::sleep(Duration::from_millis(50))=>{}
        }
    }
}

async fn read_response(
    mut response: reqwest::Response,
    limit: usize,
    cancel: &AtomicBool,
) -> Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        bail!(
            "La risposta OpenAI supera il limite previsto. Nessun risultato parziale è stato salvato."
        );
    }
    let mut body = Vec::new();
    while let Some(chunk) = wait_http(response.chunk(), cancel).await? {
        if body.len() + chunk.len() > limit {
            bail!(
                "La risposta OpenAI supera il limite previsto. Nessun risultato parziale è stato salvato."
            );
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn api_error(status: StatusCode, body: &[u8]) -> anyhow::Error {
    let json: serde_json::Value = serde_json::from_slice(body).unwrap_or_default();
    let code = json
        .get("error")
        .and_then(|error| {
            error
                .get("code")
                .and_then(|value| value.as_str())
                .or_else(|| error.get("type").and_then(|value| value.as_str()))
        })
        .unwrap_or("");
    let insufficient_quota =
        json.pointer("/error/type").and_then(|value| value.as_str()) == Some("insufficient_quota");
    let message = match status.as_u16() {
        401 => "Chiave API OpenAI non valida o revocata. Aggiornala in Strumenti.",
        403 => {
            "La chiave o il progetto OpenAI non hanno accesso alla trascrizione. Controlla i permessi del progetto."
        }
        429 if insufficient_quota
            || matches!(
                code,
                "insufficient_quota"
                    | "billing_hard_limit_reached"
                    | "billing_not_active"
                    | "credit_balance_exhausted"
                    | "organization_spend_limit_exceeded"
                    | "project_spend_limit_exceeded"
                    | "organization_usage_limit_exceeded"
            ) =>
        {
            "Quota o credito API OpenAI esauriti. Controlla la fatturazione del progetto; l’abbonamento ChatGPT non include il credito API."
        }
        429 => {
            "Limite di richieste OpenAI raggiunto. Attendi prima di riprovare; non sono stati eseguiti tentativi automatici."
        }
        413 => {
            "OpenAI ha rifiutato il file perché troppo grande. Prepara segmenti audio inferiori a 25 MB."
        }
        404 | 410 => {
            "Il modello gpt-transcribe non è disponibile per questo progetto o è stato ritirato. Nessun altro modello è stato usato."
        }
        400 | 422 => {
            "OpenAI non ha accettato l’audio o le impostazioni. Verifica che contenga parlato e che il codice lingua sia supportato."
        }
        500..=599 => {
            "Il servizio di trascrizione OpenAI è temporaneamente non disponibile. Nessun tentativo automatico è stato eseguito."
        }
        _ => {
            "La richiesta di trascrizione non è stata accettata da OpenAI. Controlla account, progetto e connessione."
        }
    };
    anyhow::anyhow!(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        net::TcpListener,
        sync::atomic::{AtomicBool, Ordering},
        thread,
        time::Instant,
    };

    struct Reply {
        status: u16,
        body: String,
        delay: Duration,
        declared_length: Option<usize>,
    }
    impl Reply {
        fn new(status: u16, body: &str) -> Self {
            Self {
                status,
                body: body.to_owned(),
                delay: Duration::ZERO,
                declared_length: None,
            }
        }
        fn transcript(text: &str) -> Self {
            Self::new(
                200,
                &serde_json::json!({"text":text,"languages":[]}).to_string(),
            )
        }
    }
    fn server(replies: Vec<Reply>) -> (String, thread::JoinHandle<Vec<Vec<u8>>>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!(
            "http://{}/v1/audio/transcriptions",
            listener.local_addr().unwrap()
        );
        let handle = thread::spawn(move || {
            let mut requests = Vec::new();
            for reply in replies {
                let deadline = Instant::now() + Duration::from_secs(3);
                let (mut stream, _) = loop {
                    match listener.accept() {
                        Ok(connection) => break connection,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                Instant::now() < deadline,
                                "local mock did not receive a request"
                            );
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("local mock accept failed: {error}"),
                    }
                };
                // Windows accepted sockets inherit the listener's FIONBIO mode. Reads below
                // use blocking timeouts so fragmented requests never fail with WSAEWOULDBLOCK.
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buffer = [0u8; 4096];
                let header_end = loop {
                    let length = stream.read(&mut buffer).unwrap();
                    assert_ne!(length, 0);
                    request.extend_from_slice(&buffer[..length]);
                    if let Some(position) = request.windows(4).position(|part| part == b"\r\n\r\n")
                    {
                        break position + 4;
                    }
                };
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|length| length.trim().parse::<usize>().ok())
                    })
                    .unwrap();
                while request.len() < header_end + length {
                    let count = stream.read(&mut buffer).unwrap();
                    assert_ne!(count, 0);
                    request.extend_from_slice(&buffer[..count]);
                }
                requests.push(request);
                thread::sleep(reply.delay);
                let response = format!(
                    "HTTP/1.1 {} Mock\r\nContent-Length: {}\r\nContent-Type: application/json; charset=utf-8\r\nConnection: close\r\n\r\n{}",
                    reply.status,
                    reply.declared_length.unwrap_or(reply.body.len()),
                    reply.body
                );
                let _ = stream.write_all(response.as_bytes());
            }
            requests
        });
        (endpoint, handle)
    }
    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }
    fn client() -> Client {
        client_builder().no_proxy().build().unwrap()
    }
    fn input(folder: &Path, name: &str) -> PathBuf {
        let path = folder.join(name);
        fs::write(&path, b"RIFF-local-mock-audio").unwrap();
        path
    }
    fn settings() -> ConversionSettings {
        let mut settings = ConversionSettings::default();
        settings.transcription.language = Some("IT".into());
        settings
    }
    const TEST_KEY: &str = "sk-local-http-test-not-a-real-key";

    #[test]
    fn gpt_transcribe_json_preserves_unicode_newlines_and_accepts_empty_detected_languages() {
        let folder = tempfile::tempdir().unwrap();
        let source = input(folder.path(), "nome privato è.wav");
        fs::write(folder.path().join("nome privato è.txt"), b"existing text").unwrap();
        let spoken_text = "Ciao, città!\nSeconda riga: \"perché?\".";
        let (endpoint, server) = server(vec![Reply::transcript(spoken_text)]);
        let output = runtime()
            .block_on(transcribe_parts(
                &client(),
                &endpoint,
                TEST_KEY,
                std::slice::from_ref(&source),
                &source,
                folder.path(),
                &settings(),
                &AtomicBool::new(false),
                &mut |_| {},
            ))
            .unwrap();
        assert_eq!(
            fs::read_to_string(&output).unwrap(),
            format!("{spoken_text}\n")
        );
        assert_eq!(
            fs::read(folder.path().join("nome privato è.txt")).unwrap(),
            b"existing text"
        );
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 1);
        let request = String::from_utf8_lossy(&requests[0]);
        assert!(request.starts_with("POST /v1/audio/transcriptions HTTP/1.1"));
        assert!(request.contains("name=\"model\"\r\n\r\ngpt-transcribe"));
        assert!(request.contains("name=\"response_format\"\r\n\r\njson"));
        assert!(request.contains("name=\"languages[]\"\r\n\r\nit"));
        assert!(!request.contains("name=\"language\""));
        assert!(!request.contains("whisper-1"));
        assert!(request.contains("filename=\"audio.wav\""));
        assert!(!request.contains("nome privato"));
        assert_eq!(fs::read(source).unwrap(), b"RIFF-local-mock-audio");
    }

    #[test]
    fn automatic_detection_omits_both_language_fields_and_accepts_missing_metadata() {
        let folder = tempfile::tempdir().unwrap();
        let source = input(folder.path(), "source.wav");
        let (endpoint, server) =
            server(vec![Reply::new(200, r#"{"text":"Testo senza metadati"}"#)]);
        let settings = ConversionSettings::default();
        let output = runtime()
            .block_on(transcribe_parts(
                &client(),
                &endpoint,
                TEST_KEY,
                std::slice::from_ref(&source),
                &source,
                folder.path(),
                &settings,
                &AtomicBool::new(false),
                &mut |_| {},
            ))
            .unwrap();
        assert_eq!(
            fs::read_to_string(output).unwrap(),
            "Testo senza metadati\n"
        );
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 1);
        let request = String::from_utf8_lossy(&requests[0]);
        assert!(request.contains("name=\"model\"\r\n\r\ngpt-transcribe"));
        assert!(!request.contains("name=\"languages[]\""));
        assert!(!request.contains("name=\"language\""));
        assert!(!request.contains("whisper-1"));
    }

    #[test]
    fn malformed_or_blank_success_responses_never_become_text_files() {
        let folder = tempfile::tempdir().unwrap();
        let source = input(folder.path(), "source.wav");
        let out = folder.path().join("out");
        fs::create_dir(&out).unwrap();
        for response in [
            "plain legacy text",
            "{invalid JSON containing sk-local-http-test-not-a-real-key",
            r#""top-level string""#,
            r#"["positional text must not be accepted"]"#,
            r#"{}"#,
            r#"{"text":null}"#,
            r#"{"text":42}"#,
            r#"{"text":true}"#,
            r#"{"text":[]}"#,
            r#"{"text":""}"#,
            r#"{"text":" \n\t "}"#,
            r#"{"text":"first","text":"duplicate"}"#,
        ] {
            let (endpoint, server) = server(vec![Reply::new(200, response)]);
            let result = runtime().block_on(transcribe_parts(
                &client(),
                &endpoint,
                TEST_KEY,
                std::slice::from_ref(&source),
                &source,
                &out,
                &settings(),
                &AtomicBool::new(false),
                &mut |_| {},
            ));
            let error = result.unwrap_err();
            assert!(!error.to_string().contains(TEST_KEY));
            assert_eq!(server.join().unwrap().len(), 1);
            assert_eq!(fs::read_dir(&out).unwrap().count(), 0);
        }
        assert_eq!(fs::read(source).unwrap(), b"RIFF-local-mock-audio");
    }

    #[test]
    fn malformed_second_part_rolls_back_the_entire_transcription() {
        let folder = tempfile::tempdir().unwrap();
        let source = input(folder.path(), "source.wav");
        let parts = vec![
            input(folder.path(), "part-000.mp3"),
            input(folder.path(), "part-001.mp3"),
        ];
        let out = folder.path().join("out");
        fs::create_dir(&out).unwrap();
        let (endpoint, server) = server(vec![
            Reply::transcript("Valid first part"),
            Reply::new(200, r#"{"text":42}"#),
        ]);
        let error = runtime()
            .block_on(transcribe_parts(
                &client(),
                &endpoint,
                TEST_KEY,
                &parts,
                &source,
                &out,
                &settings(),
                &AtomicBool::new(false),
                &mut |_| {},
            ))
            .unwrap_err();
        assert!(error.to_string().contains("non valido"));
        assert_eq!(server.join().unwrap().len(), 2);
        assert_eq!(fs::read_dir(&out).unwrap().count(), 0);
    }

    #[test]
    fn speech_silence_and_speech_keep_the_complete_nonempty_transcript() {
        let folder = tempfile::tempdir().unwrap();
        let source = input(folder.path(), "source.wav");
        let parts = vec![
            input(folder.path(), "part-000.mp3"),
            input(folder.path(), "part-001.mp3"),
            input(folder.path(), "part-002.mp3"),
        ];
        let out = folder.path().join("out");
        fs::create_dir(&out).unwrap();
        let (endpoint, server) = server(vec![
            Reply::transcript("Prima parte"),
            Reply::transcript(" \n\t "),
            Reply::transcript("Ultima parte"),
        ]);
        let output = runtime()
            .block_on(transcribe_parts(
                &client(),
                &endpoint,
                TEST_KEY,
                &parts,
                &source,
                &out,
                &settings(),
                &AtomicBool::new(false),
                &mut |_| {},
            ))
            .unwrap();
        assert_eq!(
            fs::read_to_string(output).unwrap(),
            "Prima parte\n\nUltima parte\n"
        );
        assert_eq!(server.join().unwrap().len(), 3);
        assert_eq!(fs::read_dir(&out).unwrap().count(), 1);
    }

    #[test]
    fn entirely_silent_chunks_fail_without_publishing_an_empty_file() {
        let folder = tempfile::tempdir().unwrap();
        let source = input(folder.path(), "source.wav");
        let parts = vec![
            input(folder.path(), "part-000.mp3"),
            input(folder.path(), "part-001.mp3"),
        ];
        let out = folder.path().join("out");
        fs::create_dir(&out).unwrap();
        let (endpoint, server) = server(vec![Reply::transcript(""), Reply::transcript(" \n\t ")]);
        let error = runtime()
            .block_on(transcribe_parts(
                &client(),
                &endpoint,
                TEST_KEY,
                &parts,
                &source,
                &out,
                &settings(),
                &AtomicBool::new(false),
                &mut |_| {},
            ))
            .unwrap_err();
        assert!(error.to_string().contains("non ha restituito testo"));
        assert_eq!(server.join().unwrap().len(), 2);
        assert_eq!(fs::read_dir(&out).unwrap().count(), 0);
    }

    #[test]
    fn unavailable_current_model_does_not_fall_back_to_whisper() {
        let folder = tempfile::tempdir().unwrap();
        let source = input(folder.path(), "source.wav");
        let (endpoint, server) = server(vec![Reply::new(
            404,
            r#"{"error":{"code":"model_not_found"}}"#,
        )]);
        let error = runtime()
            .block_on(transcribe_parts(
                &client(),
                &endpoint,
                TEST_KEY,
                std::slice::from_ref(&source),
                &source,
                folder.path(),
                &settings(),
                &AtomicBool::new(false),
                &mut |_| {},
            ))
            .unwrap_err();
        assert!(error.to_string().contains("gpt-transcribe"));
        assert!(!error.to_string().contains("whisper-1"));
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 1);
        let request = String::from_utf8_lossy(&requests[0]);
        assert!(request.contains("name=\"model\"\r\n\r\ngpt-transcribe"));
        assert!(!request.contains("whisper-1"));
        assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 1);
    }

    #[test]
    fn multiple_parts_join_in_order_and_failure_never_publishes_partial_text() {
        let folder = tempfile::tempdir().unwrap();
        let source = input(folder.path(), "source.wav");
        let parts = vec![
            input(folder.path(), "part-000.mp3"),
            input(folder.path(), "part-001.mp3"),
        ];
        let out = folder.path().join("out");
        fs::create_dir(&out).unwrap();
        let (endpoint, server) = server(vec![
            Reply::transcript("prima parte"),
            Reply::new(
                401,
                &format!(
                    r#"{{"error":{{"code":"invalid_api_key","message":"secret {TEST_KEY}"}}}}"#
                ),
            ),
        ]);
        let error = runtime()
            .block_on(transcribe_parts(
                &client(),
                &endpoint,
                TEST_KEY,
                &parts,
                &source,
                &out,
                &settings(),
                &AtomicBool::new(false),
                &mut |_| {},
            ))
            .unwrap_err();
        assert!(error.to_string().contains("Chiave API"));
        assert!(!error.to_string().contains(TEST_KEY));
        assert_eq!(fs::read_dir(&out).unwrap().count(), 0);
        assert_eq!(server.join().unwrap().len(), 2);
        let (endpoint, server) = self::server(vec![
            Reply::transcript("prima parte"),
            Reply::transcript("seconda parte"),
        ]);
        let output = runtime()
            .block_on(transcribe_parts(
                &client(),
                &endpoint,
                TEST_KEY,
                &parts,
                &source,
                &out,
                &settings(),
                &AtomicBool::new(false),
                &mut |_| {},
            ))
            .unwrap();
        assert_eq!(
            fs::read_to_string(output).unwrap(),
            "prima parte\n\nseconda parte\n"
        );
        assert_eq!(server.join().unwrap().len(), 2);
    }

    #[test]
    fn quota_error_is_redacted_and_not_retried() {
        let folder = tempfile::tempdir().unwrap();
        let source = input(folder.path(), "source.wav");
        let (endpoint, server) = server(vec![Reply::new(
            429,
            &format!(r#"{{"error":{{"code":"insufficient_quota","message":"{TEST_KEY}"}}}}"#),
        )]);
        let error = runtime()
            .block_on(transcribe_parts(
                &client(),
                &endpoint,
                TEST_KEY,
                std::slice::from_ref(&source),
                &source,
                folder.path(),
                &settings(),
                &AtomicBool::new(false),
                &mut |_| {},
            ))
            .unwrap_err();
        assert!(error.to_string().contains("credito"));
        assert!(!error.to_string().contains(TEST_KEY));
        assert_eq!(server.join().unwrap().len(), 1);
        assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 1);
    }

    #[test]
    fn cancellation_during_http_wait_is_prompt_and_does_not_publish() {
        let folder = tempfile::tempdir().unwrap();
        let source = input(folder.path(), "source.wav");
        let mut reply = Reply::transcript("must not be saved");
        reply.delay = Duration::from_secs(1);
        let (endpoint, server) = server(vec![reply]);
        let cancel = AtomicBool::new(false);
        thread::scope(|scope| {
            scope.spawn(|| {
                thread::sleep(Duration::from_millis(150));
                cancel.store(true, Ordering::Relaxed);
            });
            let start = Instant::now();
            let result = runtime().block_on(transcribe_parts(
                &client(),
                &endpoint,
                TEST_KEY,
                std::slice::from_ref(&source),
                &source,
                folder.path(),
                &settings(),
                &cancel,
                &mut |_| {},
            ));
            assert!(result.unwrap_err().to_string().contains("annullata"));
            assert!(start.elapsed() < Duration::from_millis(800));
        });
        assert_eq!(server.join().unwrap().len(), 1);
        assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 1);
    }

    #[test]
    fn cancellation_after_response_still_prevents_output_commit() {
        let folder = tempfile::tempdir().unwrap();
        let source = input(folder.path(), "source.wav");
        let (endpoint, server) = server(vec![Reply::transcript("received but cancelled")]);
        let cancel = AtomicBool::new(false);
        let result = runtime().block_on(transcribe_parts(
            &client(),
            &endpoint,
            TEST_KEY,
            std::slice::from_ref(&source),
            &source,
            folder.path(),
            &settings(),
            &cancel,
            &mut |progress| {
                if progress.message.ends_with("completata") {
                    cancel.store(true, Ordering::Relaxed);
                }
            },
        ));
        assert!(result.is_err());
        assert_eq!(server.join().unwrap().len(), 1);
        assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 1);
    }

    #[test]
    fn oversized_response_is_rejected_without_output() {
        let folder = tempfile::tempdir().unwrap();
        let source = input(folder.path(), "source.wav");
        let mut reply = Reply::new(200, "");
        reply.declared_length = Some(MAX_RESPONSE_BYTES + 1);
        let (endpoint, server) = server(vec![reply]);
        let result = runtime().block_on(transcribe_parts(
            &client(),
            &endpoint,
            TEST_KEY,
            std::slice::from_ref(&source),
            &source,
            folder.path(),
            &settings(),
            &AtomicBool::new(false),
            &mut |_| {},
        ));
        assert!(result.unwrap_err().to_string().contains("limite"));
        assert_eq!(server.join().unwrap().len(), 1);
        assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 1);
    }

    #[test]
    fn local_audio_preflight_and_language_validation_do_not_require_a_key_or_network() {
        assert!(validate_language("it").is_ok());
        assert!(validate_language("EN").is_ok());
        assert!(validate_language("").is_ok());
        assert!(validate_language("it-IT").is_err());
        assert!(validate_language("it\r\nx").is_err());
        assert!(validated_key("sk-short").is_err());
        let folder = tempfile::tempdir().unwrap();
        let stage = folder.path().join("stage");
        fs::create_dir(&stage).unwrap();
        let source = input(folder.path(), "source.wav");
        let paths = prepare_audio(
            &source,
            &stage,
            &ToolPaths::default(),
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(paths, vec![source]);
        let source = input(folder.path(), "source.opus");
        assert!(
            prepare_audio(
                &source,
                &stage,
                &ToolPaths::default(),
                &AtomicBool::new(false),
                &mut |_| {}
            )
            .unwrap_err()
            .to_string()
            .contains("FFmpeg")
        );
        let source = input(folder.path(), "large.wav");
        fs::OpenOptions::new()
            .write(true)
            .open(&source)
            .unwrap()
            .set_len(MAX_UPLOAD_BYTES + 1)
            .unwrap();
        assert!(
            prepare_audio(
                &source,
                &stage,
                &ToolPaths::default(),
                &AtomicBool::new(false),
                &mut |_| {}
            )
            .is_err()
        );
        assert_eq!(fs::read_dir(stage).unwrap().count(), 0);
    }

    #[test]
    fn documented_billing_limits_are_not_reported_as_transient_rate_limits() {
        for code in [
            "credit_balance_exhausted",
            "organization_spend_limit_exceeded",
            "project_spend_limit_exceeded",
            "organization_usage_limit_exceeded",
        ] {
            let body =
                serde_json::to_vec(&serde_json::json!({"error":{"code":code,"message":TEST_KEY}}))
                    .unwrap();
            let error = api_error(StatusCode::TOO_MANY_REQUESTS, &body);
            assert!(error.to_string().contains("credito"));
            assert!(!error.to_string().contains(TEST_KEY));
        }
        let body = br#"{"error":{"code":null,"type":"insufficient_quota"}}"#;
        assert!(
            api_error(StatusCode::TOO_MANY_REQUESTS, body)
                .to_string()
                .contains("credito")
        );
    }

    #[test]
    fn ffmpeg_prepares_incompatible_long_audio_as_ordered_bounded_chunks() {
        let tools = ToolPaths::discover();
        let Some(ffmpeg) = tools.ffmpeg.as_ref() else {
            return;
        };
        let folder = tempfile::tempdir().unwrap();
        let source = folder.path().join("long.aiff");
        let mut command = Command::new(ffmpeg);
        let output = engine::hidden(&mut command)
            .args([
                "-hide_banner",
                "-nostdin",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "anullsrc=r=8000:cl=mono",
                "-t",
                &(CHUNK_SECONDS + 1).to_string(),
                "-c:a",
                "pcm_s16be",
            ])
            .arg(&source)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stage = folder.path().join("stage");
        fs::create_dir(&stage).unwrap();
        let parts = prepare_audio(
            &source,
            &stage,
            &tools,
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(parts.len(), 2);
        assert!(parts[0].file_name().unwrap() < parts[1].file_name().unwrap());
        for part in parts {
            let size = fs::metadata(part).unwrap().len();
            assert!(size > 0 && size <= MAX_UPLOAD_BYTES);
        }
        assert!(source.is_file());
    }
}
