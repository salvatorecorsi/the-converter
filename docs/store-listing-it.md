# Scheda Microsoft Store — italiano (it-IT)

Bozza per il MSIX completo, da verificare sul pacchetto finale. Editore: **CORSIVO**. Sviluppatore: **Salvatore Corsi**. Il nome **The Converter** resta subordinato alla disponibilita e alla prenotazione in Partner Center. Non copiare questo preambolo nei campi pubblici.

## Descrizione breve

Conversioni di file dal tasto destro di Windows 11, con coda e opzioni. Trascrizione audio TXT online facoltativa con OpenAI e propria API key. Motori esterni per audio/video e alcune funzioni PDF.

## Descrizione

Immagini e immagini in PDF funzionano subito. Le conversioni di formato audio/video richiedono FFmpeg; l'esportazione PDF richiede Poppler; i documenti Office in PDF richiedono LibreOffice. Questi motori opzionali non sono inclusi. La trascrizione audio TXT con OpenAI e facoltativa: richiede internet, una propria chiave API e fatturazione API separata da ChatGPT; invia l'audio a OpenAI. FFmpeg prepara gli audio che non possono essere inviati direttamente.

The Converter porta le conversioni nella cartella in cui lavori. Dal menu contestuale di Windows 11 scegli Converti in e il formato desiderato: il lavoro entra in coda con le impostazioni salvate, senza aprire ogni volta la finestra delle opzioni.

Un segnaposto con icona di attesa mostra subito dove comparira il risultato. Puoi aprirlo per vedere lo stato o annullare il lavoro. Quando la conversione termina, il file pronto prende il suo posto. Per impostazione predefinita i risultati sono accanto agli originali; se un nome esiste gia viene aggiunto un numero.

Le conversioni incluse comprendono PNG, JPEG, WebP, BMP, TIFF, GIF e ICO, oltre alla creazione di un PDF A4 per ciascuna immagine. Puoi regolare qualita JPEG e larghezza delle immagini.

Con FFmpeg puoi convertire audio in MP3, WAV, FLAC, AAC, OGG, Opus e AIFF, convertire video in MP4, MKV, MOV, WebM, AVI e GIF animato, oppure estrarne l'audio. Le opzioni comprendono preset di qualita, bitrate, frequenza, canali, normalizzazione, qualita video, larghezza, FPS e taglio temporale.

Con Poppler puoi esportare i PDF in immagini PNG/JPEG o estrarne il testo incorporato. Con LibreOffice puoi convertire documenti, fogli e presentazioni compatibili in PDF.

Le conversioni di formato vengono elaborate sul PC. Solo scegliendo la trascrizione audio TXT con OpenAI, dopo averla abilitata, invii l'audio al servizio online usando la tua chiave API. La chiave e protetta dal Gestore credenziali Windows e puo essere rimossa dalle impostazioni. Non serve un account nell'app per le conversioni locali. Preferenze, percorsi dei file, stato della coda e dettagli degli errori vengono salvati localmente, senza includere la chiave.

La trascrizione del parlato crea un file TXT e permette il riconoscimento automatico della lingua o la scelta di una lingua. Non e disponibile direttamente per i video. Rileggi sempre il testo ottenuto: possono esserci errori. I costi delle richieste gia inviate possono maturare anche se il lavoro viene annullato.

Limiti: non sono inclusi OCR, unione o modifica dei PDF. Da GIF animate e TIFF multipagina viene convertito il primo fotogramma o la prima pagina. Ogni immagine genera un PDF separato. L'impaginazione dei documenti Office dipende da LibreOffice e dai font disponibili. L'interfaccia e attualmente in italiano. Il menu moderno richiede Windows 11 x64.

## Funzionalita

Inserire ciascuna riga in un campo separato, senza il trattino iniziale:

```text
Menu Converti in direttamente in Esplora file su Windows 11
Conversioni rapide in coda senza aprire ogni volta le opzioni
Segnaposto nella cartella di destinazione e stato del lavoro su richiesta
Conversioni tra PNG, JPEG, WebP, BMP, TIFF, GIF e ICO
Creazione di un PDF A4 separato per ciascuna immagine
Qualita JPEG, ridimensionamento e cartella di destinazione personalizzabili
Audio e video con FFmpeg opzionale selezionato dall'utente
Esportazione PDF con Poppler e documenti in PDF con LibreOffice opzionali
File originali conservati e nomi numerati in caso di conflitto
Conversioni di formato locali; audio inviato a OpenAI solo per la trascrizione richiesta
Trascrizione audio TXT opzionale con propria API key OpenAI e costi API separati
```

## Termini di ricerca

Un termine per campo; non usare nomi di prodotti terzi:

```text
convertitore file
convertire immagini
immagini PDF
menu contestuale
conversione audio
conversione video
coda conversioni
```

## Requisiti aggiuntivi

```text
Windows 11 x64 per il menu contestuale moderno
FFmpeg per audio e video; Poppler per esportare PDF; LibreOffice per documenti in PDF. Motori opzionali non inclusi.
Trascrizione audio TXT facoltativa: internet, chiave e account OpenAI API con costi separati da ChatGPT; FFmpeg se occorre preparare l'audio.
```

## Novita

Lasciare vuoto per la prima submission Store. Preparare una nota specifica soltanto per gli aggiornamenti successivi.

## Didascalie delle schermate da acquisire

1. Converti in dal menu contestuale di Windows 11.
2. La coda mostra formato, stato e destinazione dei file.
3. Impostazioni di qualita e ridimensionamento per le immagini.
4. Configura gli eseguibili opzionali nella finestra Strumenti.

## Campi da completare

URL privacy e supporto, nome prodotto effettivamente riservato, licenza applicazione e dettagli legali del titolare. Non dichiarare una licenza open source finche non scelta e aggiunta al progetto.

Descrizione entro 10.000 caratteri; breve entro 1.000 (qui meno di 270); massimo 20 funzionalita da 200 caratteri; novita massimo 1.500, vuote alla prima submission. [Limiti ufficiali dei campi MSIX](https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msix/add-and-edit-store-listing-info).
