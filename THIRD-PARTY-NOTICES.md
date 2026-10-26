# Motori esterni

L'installer e l'archivio portable pubblici contengono l'app Rust e l'estensione Windows, con gli [avvisi delle dipendenze e dei font](docs/release-licenses/README.md). Non contengono binari FFmpeg, Poppler o LibreOffice.

L'applicazione esegue i motori come processi separati. L'opzione di installazione e lo script `install-runtime-tools.ps1` scaricano gli archivi qui indicati direttamente dai rispettivi distributori e ne verificano gli SHA-256. La cartella locale di sviluppo `tools/` non è tracciata in Git; il bundle locale creato da `build-release.ps1` non è quello pubblicato su GitHub.

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
- Le DLL scaricate hanno anche le rispettive licenze. Le release pubbliche di The Converter non ri-ospitano questo bundle; i riferimenti upstream e le modalità di acquisizione sono in [EXTERNAL-ENGINES.md](docs/release-licenses/EXTERNAL-ENGINES.md).

## LibreOffice

Non incluso. L'app usa un'installazione disponibile sul computer. [Sorgenti e licenze](https://www.libreoffice.org/about-us/licenses/).

## Dipendenze Rust

Le versioni esatte sono in `Cargo.lock` e `shell-extension/Cargo.lock`. I testi delle licenze, gli avvisi dei font e del runtime, e il sorgente originale del componente MPL-2.0 `option-ext` sono in `docs/release-licenses/`. Il pacchetto non contiene un'installazione Rust separata. Questi avvisi non assegnano una nuova licenza al codice originale del progetto.
