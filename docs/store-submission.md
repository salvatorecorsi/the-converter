# Microsoft Store: preparazione della submission

Stato al 27 ottobre 2026: materiale preparatorio locale. Account Partner Center, nome riservato e identita assegnata non sono ancora disponibili. La preview con certificato di sviluppo non e un pacchetto pronto da caricare. Editore pubblico richiesto: **CORSIVO**; autore e sviluppatore: **Salvatore Corsi**. CORSIVO e il brand indicato dall'autore, non una ragione sociale verificata.

## Percorso scelto

Distribuire un **MSIX completo x64**, contenente applicazione, CLI, DLL Explorer, risorse e avvisi delle dipendenze. Il Microsoft Store firma il pacchetto dopo la certificazione; non occorre acquistare un certificato per questa distribuzione. Il vecchio `TheConverter.Identity.msix` sparse e il setup EXE restano artefatti diversi e non sono la submission Store. [Requisiti MSIX e firma Store](https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msix/app-package-requirements#code-signing-for-microsoft-store-submissions).

Il menu moderno usa la DLL `IExplorerCommand`, COM surrogate e `windows.fileExplorerContextMenus` dichiarati nel manifest del pacchetto completo. Non richiede script di registrazione del menu classico nel pacchetto Store. [Integrazione ufficiale con Esplora file](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/integrate-packaged-app-with-file-explorer).

## Dati ancora necessari dal proprietario

| Campo | Stato / azione |
| --- | --- |
| Account sviluppatore Partner Center | Da creare e verificare dal proprietario. |
| Tipo account | Da determinare in base all'attivita: Microsoft distingue hobby/noncommerciale da attivita professionale o commerciale, anche per freelance. Il solo brand non determina la forma giuridica. |
| Nome prodotto riservato | Proposta: The Converter. Disponibilita da verificare in Partner Center. |
| `Package/Identity/@Name` | Copiare esattamente dalla pagina Identita dell'app in Partner Center. |
| `Package/Identity/@Publisher` | Copiare esattamente il valore assegnato; non costruire un `CN=CORSIVO` presunto. |
| Publisher display name | CORSIVO, subordinato a registrazione/verifica dell'account. |
| Identita legale e recapiti del titolare | Da fornire e verificare; non coincidono automaticamente con il brand. |
| URL HTTPS privacy e supporto | Da pubblicare e verificare; la bozza [privacy.md](privacy.md) contiene i campi mancanti. |
| Prezzo, mercati e calendario | Da scegliere dal proprietario. Nessuna pubblicazione automatica prevista. |
| Licenza applicazione | Da scegliere; attualmente manca una licenza propria nel repository. Gli avvisi delle dipendenze non sostituiscono la licenza dell'app. |

Le istruzioni Microsoft includono gli sviluppatori indipendenti e i freelance che distribuiscono nell'ambito del proprio lavoro fra gli account Company; Individual e descritto per attivita personali non commerciali. [Scelta dell'account](https://learn.microsoft.com/en-us/windows/apps/publish/get-started#prerequisite).

## Identita, versione e lingue

### Build locale e GitHub Actions

L'anteprima si genera senza account o certificato:

```powershell
.\scripts\build-store-package.ps1
```

Quando l'account di Corsivo e il prodotto saranno verificati, copiare `packaging/store/identity.partner-center.example.json` in `packaging/store/identity.partner-center.json` e sostituire i placeholder con i valori reali. Per generare il MSIX unsigned destinato alla submission:

```powershell
.\scripts\build-store-package.ps1 -ForSubmission -IdentityConfig .\packaging\store\identity.partner-center.json
```

Il builder rifiuta identita di prova, placeholder e versioni Store non valide. Crea una nuova cartella in `dist/store/preview/` o `dist/store/submission/`, con MSIX, manifest, log, SHA-256 e `submission-report.json`. Non firma, installa, registra l'app o importa certificati.

In alternativa, aprire **Actions → Microsoft Store package → Run workflow** nel repository. Il valore predefinito genera una preview; per un candidato attivare `for_submission` e inserire identita, editore, nome riservato e versione. Gli input sono trasferiti allo script tramite variabili d'ambiente, senza interpretarli come codice. Gli artefatti includono il pacchetto unsigned e le bozze di submission. Microsoft esegue la firma dopo l'approvazione Store.

**Windows checks** esegue automaticamente formattazione, test Rust e fixture PowerShell. Una pipeline verde verifica queste operazioni; non sostituisce la certificazione Microsoft o il collaudo dell'installazione su Windows 11.

- Versione Store iniziale predisposta: `1.0.0.0`, distinta dalla versione applicazione Rust `0.1.0`. Il primo segmento Store deve essere maggiore di zero; il quarto rimane zero. Ogni segmento e al massimo 65535. [Versioni dei pacchetti](https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msix/app-package-requirements#version-numbering-for-windows-10-and-11-packages).
- Nome e Publisher devono corrispondere esattamente ai dati Partner Center, comprese maiuscole e punteggiatura. Il builder ricompila gli eseguibili con il manifest dedicato al pacchetto completo, senza l'elemento di identita sparse dell'installer EXE.
- La UI e attualmente in italiano. La scheda inglese e una traduzione della descrizione, non una traduzione dell'interfaccia. Non dichiarare supporto UI inglese finche non implementato e verificato.
- Target previsto: Windows 11 desktop x64 per il menu moderno. Il limite OS e quello effettivamente impostato e collaudato nel manifest Store; non promettere ARM64 nativo.

## Funzioni incluse e motori opzionali

Il pacchetto leggero offre subito conversioni immagini e immagini in PDF, coda, impostazioni e menu contestuale. FFmpeg serve per audio/video; Poppler per PDF in immagini/testo; LibreOffice per documenti Office in PDF. Sono programmi esterni gia disponibili sul PC o selezionati dall'utente nella finestra Strumenti. La build Store non deve includere il downloader PowerShell del setup EXE.

La dipendenza va dichiarata all'inizio delle descrizioni: la policy 10.2.4 ammette software non integrato con questa trasparenza. E un percorso ragionevole da sottoporre alla certificazione, non un'approvazione garantita. Mantenere le funzioni native utilizzabili senza motori e messaggi chiari quando un motore manca. La policy 10.2.2 vieta l'introduzione dinamica di funzionalita incoerenti con quanto dichiarato. [Store policies](https://learn.microsoft.com/en-us/windows/apps/publish/store-policies#102-security).

## Capability e motivazione per la certificazione

`runFullTrust` e necessario per l'app desktop `mediumIL` e il COM packaged. Motivazione proposta, da confrontare con il manifest finale:

> The Converter is a native Rust desktop file conversion application. It reads files explicitly selected by the user, writes converted outputs to the selected folder, and provides an IExplorerCommand shell extension through packaged COM. Optional conversion engines already installed or explicitly selected by the user run as child processes with the user's permissions. The app does not require administrator privileges, a service, a driver, or elevation.

Non aggiungere `unvirtualizedResources` per abitudine ereditandolo dal pacchetto sparse. Serve a disabilitare la virtualizzazione di AppData/HKCU: conservarlo solo se un requisito verificato lo rende indispensabile, documentandone lo scope e chiedendo l'approvazione necessaria. Le capability ristrette richiedono una spiegazione nella submission. [Dichiarazioni delle capability](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/app-capability-declarations#restricted-capabilities), [virtualizzazione flessibile](https://learn.microsoft.com/en-us/windows/msix/desktop/flexible-virtualization).

## Materiali e verifiche prima dell'invio

- [ ] Identita reale Partner Center configurata; nessun placeholder e nessuna firma di sviluppo presentata come firma pubblica.
- [ ] Privacy completata con identita legale e contatto reali, pubblicata a un URL HTTPS accessibile. La policy 10.5.1 la richiede anche alle app Win32 senza telemetria. [Requisiti privacy](https://learn.microsoft.com/en-us/windows/apps/publish/store-policies#105-personal-information).
- [ ] Schede [italiana](store-listing-it.md) e [inglese](store-listing-en.md) ricontrollate sul pacchetto finale; licenza, disponibilita, categorie e questionario IARC completati dal proprietario.
- [ ] Asset del pacchetto inclusi e risolti dal manifest: Square44x44Logo, Square150x150Logo e StoreLogo con varianti di scala; verificare le varianti richieste e l'aspetto su temi chiaro/scuro. [Icone e risorse](https://learn.microsoft.com/en-us/windows/apps/design/iconography/app-icon-construction).
- [ ] Almeno una schermata PNG reale per scheda, desktop almeno 1366 x 768, massimo 50 MB; consigliate quattro. Preparare separatamente l'icona Store 300 x 300. [Screenshot e immagini](https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/screenshots-and-images).
- [ ] Windows App Certification Kit eseguito sul MSIX completo; conservare il report. Controllare installazione, aggiornamento, disinstallazione e avvio da Start su ambiente pulito.
- [ ] Verificati menu moderno, selezione multipla, conversione rapida senza finestra opzioni, segnaposto, annullamento, nomi duplicati e percorsi con spazi/accenti.
- [ ] Provate le funzioni native senza motori; separatamente provati i motori opzionali e il selettore di eseguibili. Verificati percorsi di dati realmente usati sotto MSIX e comportamento della disinstallazione.
- [ ] Nessun motore scaricato automaticamente, certificato di sviluppo, chiave privata o script di installazione esterno nell'artefatto Store.
- [ ] Invio per certificazione e pubblicazione eseguiti soltanto dopo la revisione del proprietario.

Per il CSV attendere l'esportazione del prodotto reale da Partner Center: preservare colonne Field, ID e Type; trasferire i testi preparati nei campi della lingua corretta. Non inventare un template con ID presunti. [Importazione/esportazione delle schede](https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msix/import-and-export-store-listings).

## Note per il certificatore: bozza inglese

Usare il testo seguente dopo il collaudo del pacchetto completo e adattare i passi solo se la UI cambia:

> The publisher brand is CORSIVO; the application is developed by Salvatore Corsi. The application UI is currently Italian. No account or login is required.
>
> The package includes native image conversion and image-to-PDF conversion. It does not bundle FFmpeg, Poppler or LibreOffice and does not automatically download them. Audio/video conversion requires FFmpeg; PDF page/text export requires Poppler; Office-to-PDF conversion requires LibreOffice. Existing local executables can be selected under Strumenti using Seleziona programma…. Missing engines are reported in the UI. These dependencies are disclosed at the start of the Store description.
>
> To test without dependencies: launch The Converter from Start, add a PNG/JPEG using Aggiungi file or drag and drop, select a different image format or PDF, and press Converti file. Output is written beside the source unless the user selects another folder. Existing files are preserved through numbered output names.
>
> On Windows 11 x64, right-click a supported image in File Explorer and select Converti in, then choose a compatible format. The conversion is queued without opening the settings window. A temporary shortcut with a pending icon appears in the output folder and is replaced by the completed output. Double-clicking the shortcut opens job status. Impostazioni personalizzate opens the main options window. The shell extension is registered declaratively through the MSIX manifest.
>
> There is no OCR, PDF merging or PDF editing. Animated image inputs and multipage TIFF inputs use their first frame/page. Image-to-PDF creates a separate A4 PDF for each image. The app contains no advertising, subscription or in-app purchase code.
