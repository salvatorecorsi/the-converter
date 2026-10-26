# Installer Windows

`TheConverter.iss` genera un installer Inno Setup per Windows x64 (Windows 10 build 19041 o successivo), installato per l'utente corrente in `%LOCALAPPDATA%\Programs\The Converter`. La disinstallazione è disponibile nelle App installate di Windows e nel menu Start.

L'installer contiene l'applicazione Rust, la DLL di Esplora file, la CLI e la documentazione. Il task consigliato scarica FFmpeg e Poppler direttamente dai rispettivi distributori tramite `scripts/install-runtime-tools.ps1`, che verifica gli hash dei pacchetti. Nessun runtime Python, Node o Rust è richiesto per usare l'applicazione.

Il collegamento **Installa o aggiorna motori di conversione** nel menu Start permette di ripetere il download su richiesta. La console resta visibile per leggere avanzamento ed eventuali errori e può essere chiusa al termine.

La voce **Converti in** del menu classico viene aggiunta per default senza privilegi amministrativi; su Windows 11 compare in **Mostra altre opzioni**. L'opzione del menu moderno viene compilata nel setup solo quando `build-installer.ps1` trova un pacchetto MSIX con firma valida; il computer destinatario verifica nuovamente la firma prima della registrazione. Non vengono creati certificati né modificati gli archivi attendibili.

## Build

Serve Inno Setup 7 o successivo. Dopo la preparazione della distribuzione:

```powershell
.\scripts\build-installer.ps1 -CompilerPath C:\Percorso\ISCC.exe
```

Lo script legge la versione da `Cargo.toml` e produce `dist\release\TheConverter-<versione>-windows-x64-setup.exe`. Accetta `-SourceDirectory`, `-OutputDirectory` e `-Version` per una pipeline di release.

Gli script eseguiti dal setup usano esplicitamente PowerShell x64, anche quando il compilatore genera un setup a 32 bit. Output, errori e codice di uscita sono inclusi nel log dell'installazione.

Per una prova silenziosa senza download di motori:

```powershell
.\TheConverter-0.1.0-windows-x64-setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /TASKS="contextmenu" /LOG="install.log"
```

La disinstallazione conserva le preferenze, la coda e i file convertiti nelle loro cartelle. Rimuove il contenuto applicativo e la cartella privata `tools` nell'installazione. Non usare la cartella dell'applicazione o `tools` come cartella per salvare documenti personali.

L'installer usa Restart Manager solo per gli eseguibili dell'app e non forza la chiusura di Esplora file. Se la DLL della shell resta caricata durante una disinstallazione, prepara un collegamento temporaneo nella cartella Esecuzione automatica dell'utente e uno script privato nei dati locali dell'app. Al successivo accesso il helper nascosto tenta tre volte di rimuovere esclusivamente quella DLL, quindi la cartella solo se vuota. Prima di ogni tentativo controlla che un nuovo `the-converter.exe` non sia stato installato nello stesso percorso. Dopo la pulizia o il rilevamento di una nuova installazione elimina soltanto il proprio script e il proprio collegamento. Una nuova installazione annulla preventivamente il helper precedente. Questo percorso funziona anche per gli account standard senza una coda di riavvio amministrativa. Non vengono terminati processi FFmpeg estranei o cancellati file personali.

Fonti ufficiali: [privilegi per utente](https://jrsoftware.org/ishelp/topic_setup_privilegesrequired.htm), [architetture](https://jrsoftware.org/ishelp/topic_setup_architecturesallowed.htm), [Restart Manager](https://jrsoftware.org/ishelp/topic_setup_closeapplications.htm), [limiti delle cancellazioni al riavvio](https://jrsoftware.org/ishelp/topic_filessection.htm).
