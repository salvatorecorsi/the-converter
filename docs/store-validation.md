# Verifiche tecniche — 27 ottobre 2026

Queste verifiche accompagnano la preparazione Store di Corsivo. Non equivalgono alla certificazione Microsoft e non sostituiscono l'identita reale di Partner Center.

## Migrazione GPT-Transcribe

Il backend corrente usa `gpt-transcribe` sullo stesso endpoint HTTPS di trascrizione. Richiede `response_format=json`, legge il campo `text` e conserva l'output finale TXT UTF-8. Il suggerimento di lingua gia salvato resta interoperabile: viene inviato come una sola voce `languages[]`; in modalita automatica il parametro e omesso. [Contratto ufficiale di migrazione](https://developers.openai.com/cookbook/examples/migrating_from_whisper_to_gpt_transcribe).

Sul codice migrato sono superati `cargo test --locked --all-targets` con **75 test** e `cargo clippy --locked --lib --bins -- -D warnings`. I test della trascrizione sono 16 e usano un server HTTP locale: verificano richiesta GPT-Transcribe/JSON, `languages[]` senza parametro singolare, riconoscimento automatico, Unicode e righe del TXT, metadati lingua assenti o vuoti, compatibilita delle preferenze, risposte malformate e protezione della chiave. Non sono state effettuate richieste con chiavi reali o a pagamento.

Sono verificati anche annullamento, errori redatti, assenza di fallback a Whisper e mancata pubblicazione di TXT parziali quando una parte fallisce. I segmenti silenziosi vengono saltati senza perdere il parlato delle altre parti; una trascrizione complessivamente vuota non crea un file. Preparazione FFmpeg e invio sequenziale rimangono invariati. Le prove GUI e i conteggi nella sezione seguente documentano la revisione precedente, non una prova del servizio GPT-Transcribe reale.

## Verifiche della revisione precedente, con Whisper

Eseguiti su Windows 11 x64, con Rust 1.99.1:

- `cargo test --locked --all-targets`: 69 test superati.
- `cargo test --locked --manifest-path shell-extension/Cargo.toml`: 3 test superati.
- `cargo clippy --locked --lib --bins -- -D warnings`: superato.
- Formattazione Rust applicazione/DLL e sintassi PowerShell: superate.
- `scripts/test-modern-menu-package.ps1`: 25 controlli isolati superati.
- `scripts/test-store-package.ps1`: 44 controlli superati, compresa la validazione semantica MakeAppx di fixture senza `/nv`.
- `scripts/test-shell-extension.ps1`: ABI COM reale, selezioni `IShellItemArray`, enumerazione, intersezioni dei formati e whitelist di 48 estensioni coerente fra DLL, script e installer.

I test di compatibilita verificano anche tutti gli abbinamenti non supportati delle categorie di ingresso: vengono rifiutati prima di creare cartelle di uscita o prenotazioni. I documenti TXT possono essere esportati in PDF; non possono diventare MP4. Le selezioni miste propongono soltanto destinazioni comuni. Il TXT da PDF resta disponibile senza OpenAI; il TXT da audio richiede la chiave facoltativa.

La trascrizione e stata provata con un server HTTP locale e una credenziale sintetica in una voce casuale separata del Gestore credenziali: nessuna chiave utente letta nei test, nessuna registrazione inviata a OpenAI e nessuna richiesta API a pagamento. Le prove coprono multipart Whisper, lingua, nome file generico, risultati UTF-8, unione di segmenti, quota, errori redatti, cancellazione durante la connessione e prima del salvataggio, dimensioni delle risposte e assenza di TXT parziali. Il mock e stato ripetuto anche sotto esecuzione parallela. Una prova con FFmpeg reale verifica la preparazione di un audio lungo in segmenti ordinati sotto la soglia di caricamento.

La build locale installata e stata controllata anche tramite la GUI, senza inserire una chiave: pannello OpenAI, avviso di invio/costi, campo password e assenza dei formati finche non si aggiunge un file. Ulteriori prove sugli eseguibili installati verificano il rifiuto TXT → MP4, immagine → PDF e conversione rapida immagine → JPEG, con rimozione del segnaposto e della cartella delle prenotazioni. Il menu Windows 11 sparse e stato ripristinato; la DLL installata supera lo smoke test COM. Nessuna modifica agli archivi dei certificati durante questo aggiornamento.

L'autenticazione, l'accesso al modello e la qualita della trascrizione sul servizio OpenAI reale restano da verificare con una chiave e un audio scelti dal proprietario. Non usare una chiave dello sviluppatore nel pacchetto o nei materiali pubblici.

## Pacchetti e certificazione

Il workflow [Windows checks](https://github.com/salvatorecorsi/the-converter/actions/workflows/windows-ci.yml) ripete i controlli sui sorgenti pubblicati. Il workflow [Microsoft Store package](https://github.com/salvatorecorsi/the-converter/actions/workflows/microsoft-store.yml) produce separatamente il MSIX unsigned e il report con commit, stato della working tree e checksum. Verificare il run relativo al commit che si intende distribuire.

La preview completa 1.0.2.0 precedente a Whisper e stata installata localmente con una copia firmata esclusivamente per il test, dopo un aggiornamento dalla preview 1.0.1.0. Al termine e stata rimossa ed e stata ripristinata l'integrazione sparse dell'installazione locale. Quella prova non certifica la successiva integrazione HTTP/Whisper.

Il report WACK locale della precedente preview **1.0.1.0**, eseguito con kit 10.0.26100.8249 su Windows 10.0.26200.0, ha risultato complessivo **WARNING**: 21 test PASS, due FAIL opzionali e un WARNING obbligatorio. I rilievi riguardavano il numero di applicazioni dichiarate, il rilevamento di eseguibili/avvio processi e la verifica DPI. Il pacchetto successivo dichiara una sola applicazione, esclude la CLI e usa API native per aprire cartelle e siti. L'uso legittimo di processi per i motori esterni resta parte del prodotto.

**Il precedente report WACK non verifica il codice migrato a GPT-Transcribe e non viene dichiarato superato.** Prima della submission occorrono il report del candidato effettivo, la prova dell'interfaccia e del menu moderno, installazione/aggiornamento/disinstallazione su un ambiente pulito e le verifiche con i motori opzionali. Microsoft considera il WACK un controllo locale deprecato: il suo esito non garantisce l'approvazione Store. [Validazione locale MSIX](https://learn.microsoft.com/en-us/windows/msix/package/packaging-uwp-apps#validate-your-app-package-locally).

I dati dell'account Corsivo, l'identita del prodotto, gli URL privacy/supporto, i contatti reali e l'invio a Microsoft restano da completare secondo la [procedura di submission](store-submission.md).
