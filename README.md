# The Converter

Versione Rust per Windows del progetto [Final Converter](https://github.com/salvatorecorsi/final-converter) di Salvatore Corsi. Conversioni locali, selezioni multiple e integrazione con Esplora file. Nessun server e nessun upload.

## Scarica e installa

Scarica l'[ultima release Windows x64](https://github.com/salvatorecorsi/the-converter/releases/latest) e avvia il file `TheConverter-<versione>-windows-x64-setup.exe`. L'installer aggiunge il programma, il disinstaller e la voce **Converti in** del tasto destro per il tuo utente, senza richiedere privilegi amministrativi.

Il setup è leggero. L'opzione consigliata scarica FFmpeg e Poppler direttamente dai distributori originali (circa 160 MB, con verifica SHA-256). Servono per audio, video e PDF; le conversioni di immagini e immagini in PDF funzionano anche senza questi download. Per riprovare o installare i motori in seguito, nel menu Start trovi **Installa o aggiorna motori di conversione**.

Su Windows 11 il menu incluso in questa release compare in **Mostra altre opzioni**. Il menu moderno richiede un'identità MSIX firmata attendibile ed è descritto in [windows-integration.md](docs/windows-integration.md); la release iniziale non include un pacchetto firmato.

Per rimuovere il programma usa **Impostazioni Windows → App → App installate → The Converter → Disinstalla**, oppure il collegamento nel menu Start. Le preferenze e i file convertiti restano conservati. Se Windows mantiene caricata la DLL del menu, la pulizia viene completata al successivo accesso senza riavviare Esplora file durante il lavoro.

È disponibile anche l'archivio portable: estrailo in una cartella stabile, esegui `scripts/install-runtime-tools.ps1 -InstallDirectory <cartella>` per i motori e, se desideri il menu, `scripts/install-context-menu.ps1 -Mode Classic -InstallDirectory <cartella>`.

## Uso quotidiano

Dal tasto destro scegli **Converti in → MP3**, **WAV**, **PDF**, ecc. La conversione rapida entra in una coda condivisa, con le preferenze salvate, e **non apre la finestra delle opzioni**. Nella cartella di destinazione compare subito un segnaposto con il nome previsto e una clessidra blu. Il menu propone i formati compatibili con la selezione.

Il segnaposto è un collegamento Windows temporaneo: Esplora file normalmente ne nasconde l'estensione `.lnk`. Un doppio clic apre solo lo stato del lavoro, con avanzamento e annullamento. A conversione conclusa lascia il posto al file effettivo; in caso di errore mostra una X rossa e permette di aprire i dettagli. La clessidra in Esplora file è un'icona statica. [Dettagli della coda](docs/queued-conversions.md).

**Converti in → Impostazioni personalizzate…** apre la finestra con coda, opzioni, avanzamento, annullamento e ultimo resoconto rapido. Puoi anche avviare `the-converter.exe` e trascinare i file nella finestra.

Per default i risultati sono accanto agli originali. Se il nome esiste già, viene aggiunto un numero: gli originali e i risultati precedenti non vengono sovrascritti. Le preferenze, inclusa la cartella di destinazione, restano salvate tra gli avvii.

## Conversioni

| Sorgente | Destinazioni | Motore |
| --- | --- | --- |
| Audio: MP3, WAV, FLAC, AAC, M4A, OGG, Opus, AIFF, WMA, ALAC, APE | MP3, WAV, FLAC, AAC, OGG, Opus, AIFF | FFmpeg |
| Video: MP4, MKV, MOV, WebM, AVI, WMV, MPEG, M4V, TS, MTS, M2TS, 3GP, FLV | MP4, MKV, MOV, WebM, AVI, GIF animato; estrazione audio nei sette formati sopra | FFmpeg |
| Immagini: PNG, JPEG, WebP, BMP, TIFF, GIF, ICO | PNG, JPEG, WebP, BMP, TIFF, GIF, ICO, PDF | Rust, senza programmi esterni |
| PDF | PNG/JPEG, una immagine per pagina; TXT con testo incorporato | Poppler |
| DOC/DOCX, ODT, RTF, TXT, XLS/XLSX, ODS, PPT/PPTX, ODP | PDF | LibreOffice opzionale |

L'estensione determina la categoria proposta; il motore verifica il contenuto durante la conversione. File protetti, corrotti o con codec non disponibili producono un errore leggibile.

Impostazioni disponibili: preset leggero/bilanciato/alta qualità, bitrate, frequenza e canali audio, normalizzazione, H.264/H.265, qualità video, larghezza, FPS, taglio temporale, qualità JPEG, ridimensionamento immagini e DPI dei PDF. L'interfaccia mostra solo i controlli pertinenti. Per WebM si usa VP9/Opus, per AVI MPEG-4/MP3; WAV e AIFF sono PCM 16 bit. I PDF creati da immagini sono A4 con proporzioni mantenute e sfondo bianco.

### Limiti espliciti

- Nessun OCR: un PDF scansionato può essere esportato in immagini; per TXT occorre testo incorporato.
- Le immagini GIF animate e i TIFF multipagina sono letti dal primo fotogramma/pagina. La conversione da video a GIF conserva l'animazione.
- WebP è codificato senza perdita. La qualità JPEG non influisce sui formati senza perdita.
- Ogni immagine genera il proprio PDF. Unione, modifica e compressione di PDF non sono incluse.
- I documenti Office richiedono LibreOffice installato o configurato; l'impaginazione dipende anche dai font disponibili.
- La coda interattiva usa un formato comune ai file selezionati. Se le categorie non hanno una destinazione comune, vanno convertite in gruppi separati.
- Le conversioni rapide lavorano in coda in background. La finestra di stato si apre su richiesta dal segnaposto, mentre la finestra principale mostra la coda completa. Il resoconto conserva l'ultima esecuzione rapida; chiudere la finestra di stato non interrompe il lavoro.

## Eseguibile e menu contestuale

La distribuzione locale pronta è in `dist\TheConverter\`. Mantieni questa cartella in una posizione stabile: contiene applicazione, CLI, DLL Explorer e motori.

Per installare il menu classico per il solo utente corrente, da PowerShell:

```powershell
.\dist\TheConverter\scripts\install-context-menu.ps1 -Mode Classic
```

Per rimuoverlo:

```powershell
.\dist\TheConverter\scripts\uninstall-context-menu.ps1 -Mode Classic
```

Su Windows 11 il menu classico compare in **Mostra altre opzioni**. Il progetto include anche la DLL `IExplorerCommand` e un pacchetto MSIX sparse per il **menu moderno di Windows 11**. Per registrarlo serve prima una firma attendibile del pacchetto. La build non registra DLL, non installa pacchetti e non modifica l'archivio dei certificati. La procedura completa e lo stato di verifica sono in [docs/windows-integration.md](docs/windows-integration.md).

## Motori e preferenze

L'app cerca gli strumenti nella cartella `tools` accanto all'eseguibile, nel `PATH`, nelle impostazioni salvate e nella cartella indicata dalla variabile `THE_CONVERTER_TOOLS`. I percorsi possono essere scelti nella finestra **Strumenti**. LibreOffice viene cercato anche nella sua posizione standard di installazione.

La distribuzione locale di sviluppo può includere FFmpeg e Poppler. L'installer e l'archivio portable pubblici li scaricano separatamente dagli upstream su richiesta. L'app non richiede Python, Node, WebView o un'installazione Rust per essere eseguita. LibreOffice non è incluso.

Le preferenze si trovano nella cartella di configurazione utente `SalvatoreCorsi\TheConverter\config` sotto `%APPDATA%`: `preferences.json` e `report-last.json`. Il resoconto include percorsi locali ed errori e non viene trasmesso. La conversione interattiva mantiene i propri risultati nella coda corrente.

La coda rapida è salvata nei dati locali dell'app sotto `%LOCALAPPDATA%`. Un solo processo elabora i lavori, anche quando provengono da più comandi del menu contestuale. Dopo un'interruzione forzata, i lavori già in corso vengono segnalati come interrotti e i lavori ancora in attesa ripartono al successivo avvio dell'app o invio. I nomi sono prenotati prima di iniziare e rimangono quelli mostrati nei segnaposto. Per i PDF esportati in immagini viene prenotato un nome per pagina. Le cartelle temporanee nella destinazione sono nascoste.

## Compilazione

Windows x64, Rust stable MSVC e Visual Studio C++ Build Tools/Windows SDK:

```powershell
.\scripts\fetch-tools.ps1
cargo build --release --locked
cargo build --release --locked --manifest-path .\shell-extension\Cargo.toml
.\scripts\build-release.ps1 -SkipBuild
```

`fetch-tools.ps1` scarica versioni fissate di FFmpeg e Poppler e ne verifica gli SHA-256 prima di copiarle; non installa nulla nel sistema. È opzionale se usi strumenti già disponibili. `scripts\build-release.ps1` esegue entrambe le compilazioni. `-CertificateThumbprint` permette di firmare eseguibili, DLL e MSIX con un certificato già disponibile, ricavandone il Publisher per entrambi i manifest. MakeAppx è necessario per generare il pacchetto identità; l'applicazione può essere usata anche senza.

Per preparare gli artefatti pubblici, dopo la build:

```powershell
.\release-support\collect-rust-licenses.ps1
.\scripts\prepare-public-release.ps1 -CompilerPath C:\Percorso\InnoSetup\ISCC.exe
```

Lo staging pubblico usa una lista esplicita di file e una verifica che rifiuta motori esterni o DLL aggiuntive. L'output in `dist/releases/v<versione>/` contiene setup, archivio portable e `SHA256SUMS.txt`. Gli avvisi delle dipendenze Rust e dei font sono inclusi in entrambi gli artefatti.

Per una distribuzione con il menu moderno, usa `prepare-public-release.ps1 -RequireModernMenu` dopo una build firmata con un certificato di produzione. Il comando include MSIX e loghi esterni, oppure si interrompe se manca una firma attendibile. Il relativo setup abilita per default la voce diretta su Windows 11 e la rimuove durante la disinstallazione. Non importa certificati di prova: la procedura locale e i requisiti di firma sono descritti in [docs/windows-integration.md](docs/windows-integration.md).

## Riga di comando

Usa l'eseguibile console dedicato, che restituisce output, diagnostica e codici di uscita affidabili:

```powershell
.\the-converter-cli.exe --diagnose
.\the-converter-cli.exe --convert --target mp3 --preset quality --files "C:\Video\clip.mp4"
.\the-converter-cli.exe --convert --target png --output-dir "C:\Risultati" --files "C:\Documenti\file.pdf"
.\the-converter-cli.exe --convert --settings .\settings.json --files "C:\Audio\registrazione.wav"
```

`--convert` lavora senza UI, continua sugli altri file dopo un errore e termina con codice non zero se almeno una conversione fallisce. Ctrl+C annulla il processo e pulisce i temporanei. I percorsi sono passati direttamente ai processi senza interpolazione in una shell.

Esempio di impostazioni JSON (i campi omessi usano i default):

```json
{
  "format": "Mp3",
  "audio": { "bitrate_kbps": 256, "sample_rate": 48000, "channels": 2, "normalize": true },
  "video": { "trim_start": 5.0, "trim_end": 35.0 }
}
```

## Verifica

```powershell
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --manifest-path .\shell-extension\Cargo.toml
.\scripts\test-shell-extension.ps1
```

I test esterni usano i motori trovati localmente; se FFmpeg/Poppler mancano, i relativi fixture vengono saltati esplicitamente. Le verifiche del menu caricano la DLL e usano le API COM di Windows senza registrarla. La comparsa effettiva nel menu moderno richiede un'installazione firmata e una verifica su Windows 11.

Vedi anche [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) per provenienza e licenze dei motori locali.
