# Motori esterni: provenienza e scelta di distribuzione

L'installer pubblico e il pacchetto portable di questa release contengono l'app Rust, la CLI e l'estensione Explorer. **Non contengono i binari FFmpeg, Poppler o LibreOffice.** Le conversioni di immagini e immagini in PDF sono implementate nell'app; le altre funzioni usano programmi separati installati dall'utente o scaricati direttamente dai rispettivi distributori.

Il download facoltativo dei motori usa versioni fissate e verifica gli SHA-256 prima dell'estrazione. Conserva la documentazione presente negli archivi di origine.

| Motore | Archivio di origine | SHA-256 |
| --- | --- | --- |
| FFmpeg 9.0.2 essentials, Gyan Doshi | [Archivio upstream](https://github.com/GyanD/codexffmpeg/releases/download/9.0.2/ffmpeg-9.0.2-essentials_build.zip) | `60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba` |
| Poppler 26.09.0-0, oschwartz10612/conda-forge | [Archivio upstream](https://github.com/oschwartz10612/poppler-windows/releases/download/v26.09.0-0/Release-26.09.0-0.zip) | `7a6f256a0ddf7536182246a5733331bf4677cbcc34f4663774947ad34556c8d0` |

La build FFmpeg indicata dichiara GPL v3 nel proprio `README.txt` e include il testo in `LICENSE`. Il distributore collega il [commit FFmpeg corrispondente](https://github.com/FFmpeg/FFmpeg/commit/946fcce07b) e descrive le librerie abilitate nella [pagina delle build](https://www.gyan.dev/ffmpeg/builds/).

L'archivio Poppler è un pacchetto indipendente che raccoglie Poppler e DLL dell'ambiente conda-forge risolto. Il [suo script alla release esatta](https://github.com/oschwartz10612/poppler-windows/blob/v26.09.0-0/package.sh) copia tutte le DLL dell'ambiente: le componenti mantengono le rispettive licenze. I riferimenti di origine sono il [progetto Poppler](https://poppler.freedesktop.org/), [poppler-feedstock](https://github.com/conda-forge/poppler-feedstock) e la [release del distributore](https://github.com/oschwartz10612/poppler-windows/releases/tag/v26.09.0-0).

Per pubblicare in futuro un bundle che ospiti questi binari insieme all'app, la distribuzione deve essere preparata anche con i testi di licenza, gli avvisi e il sorgente corrispondente richiesti per le build e le loro dipendenze, comprese eventuali patch e istruzioni di compilazione. Un solo collegamento al repository FFmpeg non costituisce un inventario completo di tutte le componenti del bundle. La release corrente evita questo ri-impacchettamento e lascia il download ai distributori originali.

Fonti: [condizioni FFmpeg](https://ffmpeg.org/legal.html), [GPL v3, sezione 6](https://www.gnu.org/licenses/gpl-3.0.html#section6), [FAQ GNU sulla redistribuzione di binari invariati](https://www.gnu.org/licenses/gpl-faq.html#UnchangedJustBinary), [MPL 2.0, sezione 3.2](https://www.mozilla.org/en-US/MPL/2.0/).

LibreOffice è facoltativo e non è incluso né scaricato automaticamente da questa release. [Licenze LibreOffice](https://www.libreoffice.org/about-us/licenses/).
