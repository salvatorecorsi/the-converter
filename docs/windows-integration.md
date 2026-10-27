# Integrazione con Esplora file

The Converter usa una DLL COM Rust con `IExplorerCommand`, lo stesso contratto di Windows per i comandi di Esplora file. Il sottomenu **Converti in** offre i formati compatibili con tutti i file selezionati: scegliere MP3, WAV, PDF o un altro formato avvia direttamente la conversione con le impostazioni salvate. L'ultima voce **Impostazioni personalizzate…** apre la finestra per modificare qualita e destinazione. La conversione viene eseguita in un processo separato da Esplora file.

## Compilazione x64

Serve Rust per `x86_64-pc-windows-msvc` con i Build Tools C++. Il pacchetto identita si genera con MakeAppx del Windows SDK. Dalla cartella del progetto:

```powershell
cargo build --release
cargo build --release --manifest-path .\shell-extension\Cargo.toml
.\scripts\build-release.ps1 -SkipBuild
```

Il risultato e `dist\TheConverter\`: `the-converter.exe` e `the_converter_shell.dll` devono stare nella stessa cartella. Mantieni quella cartella in una posizione stabile prima di registrare il menu. La DLL x64 deve corrispondere all'architettura di Esplora file; il pacchetto corrente non e per Explorer ARM64.

## Menu classico, senza privilegi amministrativi

```powershell
.\scripts\install-context-menu.ps1 -Mode Classic -InstallDirectory .\dist\TheConverter
.\scripts\uninstall-context-menu.ps1 -Mode Classic
```

Su Windows 10 il comando compare nel menu contestuale. Su Windows 11 questo metodo compare in **Mostra altre opzioni**. Registra la DLL e il verbo sotto `HKCU\Software\Classes`; non modifica le associazioni predefinite dei file. `-WhatIf` permette di esaminare l'azione senza installarla. La disinstallazione rimuove esclusivamente le due chiavi dell'app.

## Menu moderno di Windows 11

Il menu moderno richiede **package identity**, oltre a `IExplorerCommand`. La sola scrittura nel registro non e sufficiente. Il progetto include il manifest di un pacchetto MSIX con contenuto esterno, COM surrogate e `windows.fileExplorerContextMenus`.

`build-release.ps1` prepara `TheConverter.Identity.msix` senza installarlo. Per la distribuzione il pacchetto deve essere firmato con un certificato attendibile sul computer destinatario. Lo script ricava il Publisher dal Subject del certificato selezionato in `Cert:\CurrentUser\My`, lo applica sia al manifest MSIX sia al manifest incorporato negli eseguibili e ricompila l'app. La versione del pacchetto deriva da `Cargo.toml`. E possibile firmare durante la preparazione:

```powershell
.\scripts\build-release.ps1 -CertificateThumbprint 'IMPRONTA_DEL_CERTIFICATO'
.\scripts\install-context-menu.ps1 -Mode Modern -InstallDirectory .\dist\TheConverter
```

Non usare `-SkipBuild` per cambiare Publisher: gli eseguibili devono incorporare la stessa identita del pacchetto. Anche quando si riusano binari compilati, lo script controlla il manifest effettivamente incorporato prima di produrre la distribuzione.

La firma usa SHA-256 e un timestamp RFC 3161 per conservare la validita dopo la scadenza del certificato. Il server predefinito e `http://timestamp.digicert.com`; `-TimestampServer` consente di scegliere quello del proprio servizio di firma. Un errore di firma o timestamp interrompe la preparazione.

Lo script di installazione verifica firma, Publisher, versione, COM server e presenza dei loghi nella cartella esterna, poi chiama `Add-AppxPackage -ExternalLocation` per l'utente corrente. Una registrazione identica e gia presente viene conservata. **Non crea certificati, non importa certificati attendibili e non riavvia Esplora file.** Dopo una nuova registrazione puo essere necessario uscire e rientrare in Windows.

Il setup include MSIX e cartella `Assets` solo quando la firma del pacchetto e attendibile sulla macchina di compilazione. In questo caso, su Windows 11 l'opzione per il menu moderno e selezionata per default. Dopo una registrazione riuscita rimuove la precedente voce classica gestita dall'app. Il disinstaller rimuove prima l'identita associata alla propria cartella; se Windows rifiuta la rimozione, interrompe la disinstallazione conservando i file.

Per produrre una release pubblica con il menu moderno, usa una firma di produzione riconosciuta dai computer destinatari e richiedi esplicitamente il pacchetto firmato:

```powershell
.\scripts\prepare-public-release.ps1 -RequireModernMenu
```

Questo comando si interrompe prima di creare gli artefatti se manca una firma attendibile; non produce in sua vece un setup con il solo menu classico. Include il pacchetto identita e tutti i loghi referenziati anche nell'archivio portable. Un certificato autofirmato e rifiutato per la release pubblica.

Per una prova sul proprio PC e possibile usare un certificato di sviluppo. La guida Microsoft per i pacchetti con contenuto esterno prevede l'importazione del solo certificato pubblico `.cer` nell'archivio **Utente corrente → Persone attendibili** (`Cert:\CurrentUser\TrustedPeople`). E una modifica alla fiducia dell'utente, da eseguire consapevolmente e separatamente dall'installer; la chiave privata `.pfx` resta riservata. Un certificato di test non e una firma di produzione e va rimosso quando non serve piu. Non sono necessari Developer Mode o modifiche globali al menu di Windows.

```powershell
.\scripts\uninstall-context-menu.ps1 -Mode Modern -InstallDirectory .\dist\TheConverter
```

Questo rimuove il pacchetto identita dell'utente corrente soltanto se appartiene alla cartella indicata, lasciando applicazione e preferenze. Non rimuove certificati. Puoi registrare separatamente anche il menu classico, se desideri averlo in entrambi i menu.

## Selezioni multiple e sicurezza del processo

Il comando accetta fino a 10.000 elementi e passa i percorsi assoluti con un file JSON UTF-8 temporaneo. I formati rapidi lanciano direttamente `the-converter.exe --quick --target <formato> --selection-file <percorso>`; la voce personalizzate omette `--quick` e apre la finestra. In questo modo spazi, Unicode e selezioni lunghe non diventano una riga di comando enorme. Nessun percorso selezionato viene interpolato in PowerShell o `cmd`. Il processo applicazione convalida i percorsi e rimuove il file richiesta dopo averlo letto.

Il padre espone `ECF_HASSUBCOMMANDS` e un vero enumeratore `IEnumExplorerCommand` con `Next`, `Skip`, `Reset` e `Clone`. Il `GetState` di ogni formato controlla la selezione completa e nasconde le conversioni incompatibili. Ad esempio, selezionando audio e video compaiono i sette formati audio comuni; selezionando un'immagine e un PDF compaiono PNG e JPEG. Per piu di 16 elementi la chiamata rapida torna `E_PENDING`, richiedendo a Windows di rieseguire il controllo in background con `fOkToBeSlow`. La DLL non sonda codec o strumenti esterni e non apre i documenti. Elementi virtuali, selezioni vuote ed estensioni sconosciute nascondono il comando.

## Stato di verifica

Compilazione e test COM sono eseguibili senza registrare la DLL:

```powershell
cargo test --manifest-path .\shell-extension\Cargo.toml
.\scripts\test-shell-extension.ps1
.\scripts\test-modern-menu-package.ps1
```

Lo smoke test carica la DLL reale senza registrarla, attiva gli oggetti COM e passa veri `IShellItemArray` creati da Windows: verifica il sottomenu e i formati comuni a batch audio/video, immagini/PDF e documenti/immagini, allocazione stringhe e rilascio degli oggetti. La modalita opzionale `-InvokeCaptureFile` e riservata a un helper di test isolato che registra gli argomenti del processo lanciato; verifica separatamente la conversione rapida e l'apertura delle impostazioni.

I test del pacchetto moderno usano fixture e comandi Appx simulati in un processo Windows PowerShell separato: coprono manifest e asset non validi, firma incoerente, `WhatIf`, registrazione ripetuta, cambio cartella e Publisher, rimozione della sola installazione indicata e propagazione degli errori. Non modificano certificati, registro o pacchetti reali.

La presenza effettiva nel menu moderno va verificata dopo aver firmato e registrato il pacchetto su una macchina Windows 11. Il repository fornisce l'implementazione e la procedura di distribuzione; un MSIX non firmato non e una installazione completata. Nessuna registrazione viene eseguita automaticamente all'avvio dell'app o durante la compilazione.

## Fonti Microsoft

- [Comandi di Esplora file con IExplorerCommand e manifest MSIX](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/integrate-packaged-app-with-file-explorer)
- [Package identity con contenuto esterno: compilazione, firma e registrazione](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/grant-identity-to-nonpackaged-apps)
- [Firma di sviluppo e trust per pacchetti con contenuto esterno](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/grant-identity-to-nonpackaged-apps#build-and-sign-the-identity-package)
- [Requisiti e comportamento del menu di Windows 11](https://blogs.windows.com/blog/2021/07/19/extending-the-context-menu-and-share-dialog-in-windows-11/)
