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

The Converter legge i file che scegli tramite Esplora file, selettore o trascinamento e crea i risultati nella destinazione configurata. L'elaborazione avviene sul PC. Il codice applicativo non invia i file, il loro contenuto o i percorsi a un server dello sviluppatore e non contiene servizi di telemetria, pubblicita o registrazione di account. Gli originali non vengono modificati dall'app.

Se usi FFmpeg, Poppler o LibreOffice, gli eseguibili locali ricevono i percorsi e le opzioni necessari alla conversione. Sono programmi separati, installati o selezionati dall'utente. Possono avere proprie impostazioni e informative; The Converter non controlla tutte le loro funzioni. Non e prevista una trasmissione allo sviluppatore dell'app.

### Informazioni conservate sul dispositivo

L'app salva preferenze di conversione, cartella di destinazione e percorsi degli strumenti nelle proprie cartelle utente. La coda rapida registra nomi e percorsi dei file, destinazioni, formato, impostazioni, motori utilizzati, date, avanzamento ed eventuali errori. L'ultimo resoconto contiene data, formato, risultati e messaggi di errore, che possono includere percorsi o diagnostica dei motori.

I dati sono memorizzati come file locali, fra cui `preferences.json`, `report-last.json` e i record `job.json`, nelle cartelle The Converter sotto `%APPDATA%` e `%LOCALAPPDATA%`. In un'installazione MSIX Windows puo reindirizzare tali percorsi nei dati privati del pacchetto. I file non sono cifrati dall'app: sono soggetti ai permessi e alle protezioni dell'account Windows e del dispositivo.

Nella cartella di destinazione vengono creati temporanei, prenotazioni dei nomi e collegamenti segnaposto con informazioni sul lavoro. L'estensione Explorer usa inoltre un JSON temporaneo per passare la selezione all'app. L'app tenta la pulizia dei temporanei al termine; un arresto improvviso puo lasciare residui. I risultati convertiti restano nella cartella scelta.

Non e implementata una scadenza automatica per preferenze e cronologia della coda. Per eliminarle, termina o annulla i lavori, attendi la fine dei processi e chiudi l'app; puoi quindi rimuovere i relativi dati locali. La cancellazione dei record della coda attiva puo impedire il recupero dei lavori. I risultati si gestiscono e si eliminano con Esplora file. La rimozione dell'app e la gestione dei suoi dati da parte di Windows dipendono dal metodo di installazione.

### Collegamenti e servizi esterni

La finestra Strumenti contiene collegamenti ai siti di FFmpeg, Poppler e LibreOffice. Si aprono solo quando li selezioni, nel browser configurato. Il browser, il sito visitato e gli strumenti esterni applicano le proprie regole. Anche cartelle di rete, sincronizzazione cloud, antivirus e funzioni diagnostiche di Windows dipendono dalle impostazioni del tuo dispositivo: questa informativa riguarda il codice di The Converter.

L'edizione Store prevista non scarica automaticamente motori o aggiornamenti eseguibili. Installazione e aggiornamenti Store sono gestiti da Microsoft. L'eventuale script per scaricare motori della distribuzione EXE separata contatta i distributori indicati nello script quando viene avviato; non fa parte del pacchetto Store previsto.

### Controlli e richieste

Puoi scegliere file e destinazioni, configurare i motori e annullare le conversioni. Non esiste un archivio cloud dei file presso lo sviluppatore che debba essere cancellato. Per richieste su questa informativa usa il contatto effettivo da completare sopra. La presente informativa deve essere aggiornata se cambiano le funzioni o i dati trattati.

## English

### Files and local processing

The Converter reads files you select through File Explorer, a file picker or drag and drop, then writes results to your configured destination. Processing takes place on your PC. The application code does not send your files, their contents or paths to a developer server. It contains no telemetry service, advertising or account-registration system. The app does not modify source files.

Optional local FFmpeg, Poppler and LibreOffice executables receive the paths and options needed for conversion. They are separate programs installed or selected by the user and may have their own settings and privacy statements. The Converter does not control all their behavior. No transfer to the app developer is part of this workflow.

### Information stored on your device

The app stores conversion preferences, output-folder and tool paths. Its persistent quick-conversion queue stores source and destination paths, formats, settings, tool paths, timestamps, progress and errors. The latest report stores its date, format, results and error messages, which may contain paths or engine diagnostics.

Local files include `preferences.json`, `report-last.json` and `job.json` records in The Converter user folders under `%APPDATA%` and `%LOCALAPPDATA%`. In an MSIX installation, Windows may redirect these locations into private package data. The app does not encrypt these records; Windows account permissions and device protections apply.

Temporary conversion files, name reservations and placeholder shortcuts are created in the output folder. The Explorer extension also uses a temporary selection JSON file. The app attempts to remove temporary files after use; an unexpected shutdown may leave residual files. Completed results remain in the selected folder.

Preferences and queue history have no automatic expiry implemented by the app. To remove them, finish or cancel jobs, wait for processing to stop, and close the app before deleting the relevant local data. Removing active queue records can prevent job recovery. You can manage and delete outputs in File Explorer. Windows handling of app data during uninstall depends on the installation method.

### External links and services

The Strumenti window links to the FFmpeg, Poppler and LibreOffice websites. Selecting a link opens your configured browser; the browser and destination website apply their own policies. External engines, network folders, cloud synchronization, antivirus and Windows diagnostics are governed by their own behavior and your device settings. This statement describes The Converter's application code.

The planned Store edition does not automatically download conversion engines or executable updates. Microsoft manages Store installation and updates. A separate EXE distribution may offer an engine-download script that contacts the distributors named in that script when run; that script is not part of the planned Store package.

### Controls and questions

You choose the files, destination and optional engines, and can cancel conversions. The developer does not hold a cloud copy of your files to delete. For privacy questions use the real contact details that must be completed above. This statement must be updated when the app's functionality or data practices change.
