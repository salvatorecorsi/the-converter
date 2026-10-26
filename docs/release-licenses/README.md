# Avvisi di terze parti della release Windows

`RUST-THIRD-PARTY-LICENSES.txt` raccoglie i testi originali di licenza e gli avvisi dei crate individuati dai lockfile dell'app e dell'estensione Explorer per Windows x64. Comprende anche dipendenze di compilazione come insieme conservativo. `rust-components.json` contiene nomi, versioni, espressioni di licenza dichiarate, repository e collegamenti ai pacchetti sorgenti esatti.

Le licenze dei font incorporati da `epaint_default_fonts` sono incluse: Hack, Ubuntu Light, Noto Emoji e le icone emoji. Segoe UI viene letto dall'installazione Windows e non viene redistribuito.

La cartella `option-ext-0.2.0` contiene il sorgente **non modificato** del componente MPL-2.0 usato dalla libreria delle cartelle utente, con la propria licenza. Questi file sono disponibili anche nel [pacchetto sorgente originale](https://crates.io/api/v1/crates/option-ext/0.2.0/download). I diritti previsti dalla MPL-2.0 su questo componente restano quelli del suo testo di licenza.

`rust-runtime` conserva l'inventario copyright della libreria standard del toolchain usato per compilare e i testi di licenza che lo accompagnano. Tale inventario comprende anche componenti opzionali del runtime che potrebbero non essere usati dal binario Windows.

Questi avvisi non assegnano una nuova licenza al codice originale di The Converter. L'autore del progetto resta titolare dei propri diritti.

Per rigenerare l'inventario a partire dai lockfile:

```powershell
.\release-support\collect-rust-licenses.ps1
```

Il raccoglitore usa i file del registro Cargo locale e, per pochi crate che non includono i testi nel pacchetto, le licenze del repository al commit indicato dal loro `.cargo_vcs_info.json`.
