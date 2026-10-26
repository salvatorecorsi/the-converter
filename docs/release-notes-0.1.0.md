# The Converter 0.1.0 — Windows x64

Prima release della riscrittura Rust di Final Converter, con conversioni locali e integrazione in Esplora file.

## Installazione

- **Setup consigliato:** `TheConverter-0.1.0-windows-x64-setup.exe`. Installa per l'utente corrente app, menu contestuale e disinstaller.
- **Portable:** `TheConverter-0.1.0-windows-x64-portable.zip`, da estrarre in una cartella stabile.
- **Integrità dei download:** `SHA256SUMS.txt` contiene gli SHA-256 dei due artefatti.

Il setup è leggero e può scaricare FFmpeg e Poppler direttamente dagli upstream (circa 160 MB, hash verificati). I motori non sono inclusi negli allegati di questa release. Le conversioni di immagini funzionano anche senza scaricarli. Nel menu Start è disponibile il comando per installare o aggiornare i motori in seguito.

## Funzioni

- Audio MP3, WAV, FLAC, AAC, OGG, Opus e AIFF; video MP4, MKV, MOV, WebM, AVI e GIF animato; estrazione dell'audio dai video.
- Immagini PNG, JPEG, WebP, BMP, TIFF, GIF e ICO, più immagini in PDF; PDF in immagini per pagina o testo incorporato.
- Preset e impostazioni personalizzate per qualità, bitrate, frequenza, canali, risoluzione, FPS e taglio temporale.
- Dal tasto destro, conversione rapida senza aprire le opzioni. Coda condivisa, segnaposto con clessidra nella destinazione, avanzamento e annullamento su richiesta.
- Nomi assegnati subito, pubblicazione del risultato completo e nessuna sovrascrittura degli originali.
- Disinstallazione dalle App installate di Windows o dal menu Start, preservando preferenze e risultati.

## Requisiti e limiti

Windows 10 build 19041 o successivo, x64. Su Windows 11 il menu di questa release è in **Mostra altre opzioni**. Il menu moderno richiede un pacchetto MSIX firmato attendibile e non viene attivato da questo installer.

Installer ed eseguibili non sono firmati Authenticode. Non sono richiesti Python, Node, WebView o Rust sul computer dell'utente. LibreOffice è opzionale per i documenti Office. OCR, unione e modifica PDF non sono inclusi; GIF/TIFF in ingresso sono convertiti dal primo fotogramma/pagina.

Verificati conversioni reali con FFmpeg/Poppler, coda da più processi, annullamento, collisioni dei nomi e contratti COM dell'estensione Windows. Gli avvisi delle dipendenze Rust e dei font sono inclusi negli artefatti.
