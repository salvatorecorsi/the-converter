# The Converter — informativa sulla privacy / privacy statement

Bozza del 27 ottobre 2026, basata sul codice applicativo attuale. **Prima della pubblicazione occorre completare i dati seguenti e verificare i percorsi dati sul MSIX finale.** Questo file non costituisce un URL privacy gia pubblicato.

| Dato | Valore |
| --- | --- |
| Applicazione | The Converter |
| Brand editore | CORSIVO |
| Autore / sviluppatore | Salvatore Corsi |
| Identita legale del titolare | DA COMPLETARE con i dati reali del proprietario; il brand non identifica automaticamente un soggetto giuridico. |
| Contatto privacy e supporto | DA COMPLETARE con un recapito effettivo del proprietario. |
| URL pubblico della presente informativa | DA COMPLETARE dopo la pubblicazione HTTPS. |

## Italiano

### File e conversioni

The Converter legge i file che scegli tramite Esplora file, selettore o trascinamento e crea i risultati nella destinazione configurata. Le conversioni di formato avvengono sul PC. La trascrizione audio TXT con OpenAI e invece online, facoltativa e descritta qui sotto. L'app non invia file o percorsi a un server di Corsivo e non contiene servizi di telemetria, pubblicita o registrazione di account propri. Gli originali non vengono modificati dall'app.

Se usi FFmpeg, Poppler o LibreOffice, gli eseguibili locali ricevono i percorsi e le opzioni necessari alla conversione. Sono programmi separati, installati o selezionati dall'utente. Possono avere proprie impostazioni e informative; The Converter non controlla tutte le loro funzioni. Non e prevista una trasmissione allo sviluppatore dell'app.

### Trascrizione audio online facoltativa

La trascrizione non e necessaria per le conversioni locali. Per abilitarla inserisci una tua chiave OpenAI API nella finestra Strumenti, dopo l'avviso sull'invio e sui costi. Scegliendo **TXT con OpenAI** per un audio richiedi l'invio a OpenAI: l'app trasmette via HTTPS l'audio selezionato o i segmenti preparati localmente, i parametri della trascrizione e la chiave necessaria ad autenticare la richiesta. Il file e presentato con un nome generico `audio.<estensione>`, senza il nome o il percorso originale. Il destinatario e `api.openai.com/v1/audio/transcriptions`; Corsivo non fa da intermediario del contenuto. Le voci, le parole pronunciate e gli altri suoni presenti nella registrazione vengono quindi comunicati a OpenAI. Non vengono trasmesse automaticamente le altre conversioni o l'intera coda.

L'invio diretto comprende anche gli eventuali metadati incorporati nel file audio. Il modello usato e `whisper-1`. Puoi scegliere una lingua o lasciarne il riconoscimento automatico. Per formati o dimensioni che richiedono preparazione, FFmpeg crea segmenti audio temporanei sul PC e l'app li invia in sequenza. Il testo ricevuto viene salvato nel TXT di destinazione solo dopo il completamento del lavoro. Puoi annullare l'operazione, ma l'annullamento non ritira audio gia inviato e non annulla eventuali addebiti API gia maturati.

Per trattamento, conservazione e controlli sui dati ricevuti da OpenAI si applicano le condizioni e le impostazioni del tuo account API; l'app non promette cancellazione immediata o una specifica area geografica di elaborazione. Consulta i [controlli dati OpenAI API](https://developers.openai.com/api/docs/guides/your-data). L'accesso e gli eventuali costi API appartengono al tuo account OpenAI e sono separati dall'abbonamento ChatGPT.

### Protezione e rimozione della chiave API

Su Windows la chiave e salvata nel Gestore credenziali dell'utente corrente, con voce `Corsivo.TheConverter.OpenAI`, e protetta da Windows. Non e conservata nei file JSON delle preferenze, nei lavori in coda, nei resoconti o nei log. Viene usata in memoria per autenticare le richieste al servizio. Il pacchetto distribuito non contiene una chiave dello sviluppatore.

**Rimuovi chiave** nelle impostazioni elimina la credenziale salvata e disabilita nuove trascrizioni. Per fermare un lavoro gia in corso occorre anche annullarlo. Questa rimozione non revoca la chiave presso OpenAI e non cancella dati o richieste gia inviati: i controlli dell'account API si gestiscono separatamente presso il fornitore. Rimuovere i soli file di preferenze o la coda non equivale a rimuovere la credenziale.

La voce del Gestore credenziali e indipendente dai file del programma e rimane dopo la disinstallazione EXE o MSIX. Per eliminarla usa **Rimuovi chiave** prima di disinstallare oppure, anche dopo la rimozione dell'app, elimina `Corsivo.TheConverter.OpenAI` in **Gestione credenziali → Credenziali Windows → Credenziali generiche**, nell'account Windows con cui l'hai salvata. Questa operazione riguarda soltanto la copia locale della chiave.

### Informazioni conservate sul dispositivo

L'app salva preferenze di conversione, lingua di trascrizione, cartella di destinazione e percorsi degli strumenti nelle proprie cartelle utente. La coda rapida registra nomi e percorsi dei file, destinazioni, formato, impostazioni, motori utilizzati, date, avanzamento ed eventuali errori. L'ultimo resoconto contiene data, formato, risultati e messaggi di errore, che possono includere percorsi o diagnostica dei motori. Nessuno di questi record contiene la chiave API.

Questi dati sono memorizzati come file locali, fra cui `preferences.json`, `report-last.json` e i record `job.json`, nelle cartelle The Converter sotto `%APPDATA%` e `%LOCALAPPDATA%`. In un'installazione MSIX Windows puo reindirizzare tali percorsi nei dati privati del pacchetto. I file di preferenze, stato e risultato non sono cifrati dall'app: sono soggetti ai permessi e alle protezioni dell'account Windows e del dispositivo. La chiave API e conservata separatamente come descritto sopra.

Nella cartella di destinazione vengono creati temporanei, prenotazioni dei nomi e collegamenti segnaposto con informazioni sul lavoro. L'estensione Explorer usa inoltre un JSON temporaneo per passare la selezione all'app. L'app tenta la pulizia dei temporanei al termine; un arresto improvviso puo lasciare residui. I risultati convertiti restano nella cartella scelta.

Non e implementata una scadenza automatica per preferenze e cronologia della coda. Per eliminarle, termina o annulla i lavori, attendi la fine dei processi e chiudi l'app; puoi quindi rimuovere i relativi dati locali. La cancellazione dei record della coda attiva puo impedire il recupero dei lavori. I risultati si gestiscono e si eliminano con Esplora file. La rimozione dell'app e la gestione dei suoi dati da parte di Windows dipendono dal metodo di installazione.

### Collegamenti e servizi esterni

La finestra Strumenti contiene collegamenti ai siti di FFmpeg, Poppler, LibreOffice e alla gestione delle chiavi API OpenAI. Si aprono solo quando li selezioni, nel browser configurato. Il browser, il sito visitato e gli strumenti esterni applicano le proprie regole. Anche cartelle di rete, sincronizzazione cloud, antivirus e funzioni diagnostiche di Windows dipendono dalle impostazioni del tuo dispositivo: questa informativa riguarda il codice di The Converter.

L'edizione Store prevista non scarica automaticamente motori o aggiornamenti eseguibili. Installazione e aggiornamenti Store sono gestiti da Microsoft. L'eventuale script per scaricare motori della distribuzione EXE separata contatta i distributori indicati nello script quando viene avviato; non fa parte del pacchetto Store previsto.

### Controlli e richieste

Puoi scegliere file e destinazioni, configurare i motori, rimuovere la chiave API e annullare i lavori. Non esiste un archivio cloud dei file presso Corsivo; per l'audio inviato a OpenAI valgono i controlli del servizio descritti sopra. Per richieste su questa informativa usa il contatto effettivo da completare sopra. La presente informativa deve essere aggiornata se cambiano le funzioni o i dati trattati.

## English

### Files and local processing

The Converter reads files you select through File Explorer, a file picker or drag and drop, then writes results to your configured destination. Format conversion takes place on your PC. Optional OpenAI audio-to-text transcription is online and is described below. The app does not send files or paths to a Corsivo server and contains no telemetry service, advertising or its own account-registration system. The app does not modify source files.

Optional local FFmpeg, Poppler and LibreOffice executables receive the paths and options needed for conversion. They are separate programs installed or selected by the user and may have their own settings and privacy statements. The Converter does not control all their behavior. No transfer to the app developer is part of this workflow.

### Optional online audio transcription

Transcription is not required for local conversions. To enable it, provide your own OpenAI API key in Strumenti after the audio-upload and cost notice. Selecting **TXT con OpenAI** for an audio file requests upload to OpenAI. The app sends the selected audio or locally prepared segments, transcription parameters and the authentication key over HTTPS to `api.openai.com/v1/audio/transcriptions`. Files use a generic `audio.<extension>` name without the original filename or path. Corsivo does not relay the content. Voices, spoken words and other sounds in the recording are therefore disclosed to OpenAI. Other conversions and the entire queue are not automatically uploaded.

Direct uploads also include any metadata embedded in the audio file. The model is `whisper-1`. Choose a language or use automatic detection. When preparation is needed for a file format or size, FFmpeg creates temporary audio segments on your PC; the app uploads them sequentially. Received text is saved to the output TXT only after the job completes. Cancelling does not withdraw audio already uploaded or reverse API charges already incurred.

OpenAI's handling, retention and available controls depend on its terms and your API-account settings. The app does not promise immediate deletion or processing in a particular region. See [OpenAI API data controls](https://developers.openai.com/api/docs/guides/your-data). Access and any API charges belong to your OpenAI account and are separate from a ChatGPT subscription.

### API key protection and removal

On Windows, the key is stored in Windows Credential Manager for the current user under `Corsivo.TheConverter.OpenAI`, protected by Windows. It is not saved in preferences JSON, queued jobs, reports or logs. It is used in memory to authenticate service requests. Distributed packages contain no developer API key.

**Rimuovi chiave** removes the saved credential and disables new transcriptions. Also cancel any job already running if you want it to stop. Removing the saved key does not revoke it with OpenAI or delete previously submitted data or requests; manage those controls separately through your API account. Deleting only preferences or queue files does not remove the saved credential.

The Credential Manager entry is independent of the program files and remains after EXE or MSIX uninstall. Use **Rimuovi chiave** before uninstalling, or remove `Corsivo.TheConverter.OpenAI` under **Credential Manager → Windows Credentials → Generic Credentials** in the Windows account that saved it, including after the app has been removed. This deletes only the local copy of the key.

### Information stored on your device

The app stores conversion preferences, transcription language, output-folder and tool paths. Its persistent quick-conversion queue stores source and destination paths, formats, settings, tool paths, timestamps, progress and errors. The latest report stores its date, format, results and error messages, which may contain paths or engine diagnostics. None of these records contains the API key.

Local files include `preferences.json`, `report-last.json` and `job.json` records in The Converter user folders under `%APPDATA%` and `%LOCALAPPDATA%`. In an MSIX installation, Windows may redirect these locations into private package data. The app does not encrypt preferences, state or output files; Windows account permissions and device protections apply. The API key is stored separately as described above.

Temporary conversion files, name reservations and placeholder shortcuts are created in the output folder. The Explorer extension also uses a temporary selection JSON file. The app attempts to remove temporary files after use; an unexpected shutdown may leave residual files. Completed results remain in the selected folder.

Preferences and queue history have no automatic expiry implemented by the app. To remove them, finish or cancel jobs, wait for processing to stop, and close the app before deleting the relevant local data. Removing active queue records can prevent job recovery. You can manage and delete outputs in File Explorer. Windows handling of app data during uninstall depends on the installation method.

### External links and services

The Strumenti window links to the FFmpeg, Poppler and LibreOffice websites and to OpenAI API-key management. Selecting a link opens your configured browser; the browser and destination website apply their own policies. External engines, network folders, cloud synchronization, antivirus and Windows diagnostics are governed by their own behavior and your device settings. This statement describes The Converter's application code.

The planned Store edition does not automatically download conversion engines or executable updates. Microsoft manages Store installation and updates. A separate EXE distribution may offer an engine-download script that contacts the distributors named in that script when run; that script is not part of the planned Store package.

### Controls and questions

You choose the files, destination and optional engines, can remove your saved API key, and can cancel jobs. Corsivo does not hold a cloud copy of your files. Audio submitted to OpenAI is subject to the service controls described above. For privacy questions use the real contact details that must be completed above. This statement must be updated when the app's functionality or data practices change.
