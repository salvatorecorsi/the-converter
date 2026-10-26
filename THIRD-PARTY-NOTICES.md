# Motori esterni

L'applicazione Rust esegue i motori come processi separati. I sorgenti Rust non incorporano FFmpeg, Poppler o LibreOffice. La cartella locale `tools/` non è tracciata in Git e viene copiata nella distribuzione solo se presente.

## FFmpeg 9.0.2 essentials, Windows x64

Build di Gyan Doshi, uno dei distributori Windows collegati dalla [pagina ufficiale FFmpeg](https://ffmpeg.org/download.html).

- [Archivio esatto](https://github.com/GyanD/codexffmpeg/releases/download/9.0.2/ffmpeg-9.0.2-essentials_build.zip).
- SHA-256 verificato: `60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba`.
- Licenza della build: GPL v3, copia in `tools/ffmpeg/LICENSE`.
- Configurazione, librerie e riferimenti sorgenti: `tools/ffmpeg/README.txt`.
- [Sorgente FFmpeg della build](https://github.com/FFmpeg/FFmpeg/commit/946fcce07b).
- [Distribuzione e informazioni sui sorgenti della build](https://www.gyan.dev/ffmpeg/builds/).

## Poppler 26.09.0, Windows x64

Distribuzione binaria indipendente di oschwartz10612, basata su pacchetti conda-forge; non è una build ufficiale del progetto Poppler.

- [Release e archivio esatto](https://github.com/oschwartz10612/poppler-windows/releases/tag/v26.09.0-0).
- SHA-256 verificato: `7a6f256a0ddf7536182246a5733331bf4677cbcc34f4663774947ad34556c8d0`.
- [Progetto e sorgenti Poppler](https://poppler.freedesktop.org/), distribuito sotto GPL.
- [Ricetta, dipendenze e licenze della build conda-forge](https://github.com/conda-forge/poppler-feedstock).
- Le DLL nella distribuzione hanno anche le rispettive licenze. Prima di redistribuire pubblicamente questo bundle occorre includere gli avvisi, le licenze e il codice sorgente corrispondente richiesti dalle sue componenti.

## LibreOffice

Non incluso. L'app usa un'installazione disponibile sul computer. [Sorgenti e licenze](https://www.libreoffice.org/about-us/licenses/).

## Dipendenze Rust

Le versioni esatte sono in `Cargo.lock` e `shell-extension/Cargo.lock`. Ogni crate conserva la propria licenza. Il bundle non contiene un runtime Rust separato.
