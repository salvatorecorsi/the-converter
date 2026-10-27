# Full MSIX for Microsoft Store

This packaging path targets **Windows 11 x64, build 22000 or later**, and contains one graphical application, the Explorer COM DLL, execution-alias metadata, app logos and third-party notices. It uses `packagedClassicApp`, a visible app-list entry and `runFullTrust`. It does not use sparse identity or external application content. The console CLI, FFmpeg, Poppler, LibreOffice and the PowerShell runtime downloader are excluded; terminal commands remain available in the separate portable/GitHub distribution.

The publisher brand is **Corsivo**; the developer is **Salvatore Corsi**. A Corsivo Partner Center account and reserved product do not yet exist. Production identity values must be copied from that account after registration; the development publisher subject must not be substituted for those values.

## Preview build

```powershell
.\scripts\test-store-package.ps1
.\scripts\build-store-package.ps1
```

The preview is deliberately named `Corsivo.TheConverter.StorePreview`, displays **The Converter Store Preview**, uses publisher display name **Corsivo** and development publisher `CN=Salvatore Corsi`. Its default package version is `1.0.0.0`. The preview must not be uploaded as the production app.

The script does not sign, install, register or trust the preview. Local installation is a separate action requiring an appropriately signed package and explicit authorization. Because preview and standalone Explorer extensions currently use the same COM CLSID, coexistence can cause duplicate menu entries or ambiguous activation during testing; verify the executable path of the invoked app or use a clean test profile.

## Partner Center candidate

Copy `identity.partner-center.example.json` to a local config file, then replace **Name** and **Publisher** with the exact Package/Identity values from Partner Center. **DisplayName** must match the reserved app name; **PublisherDisplayName** must match the account's publisher display name. Keep the account's actual spelling and punctuation.

```powershell
.\scripts\build-store-package.ps1 -ForSubmission -IdentityConfig C:\Build\corsivo-partner-center.json
```

`-ForSubmission` rejects missing fields, placeholder values and the preview identity before creating build output. The Store package version comes from the JSON config, independently of `Cargo.toml`: it must have a positive first component and the final component must be zero, for example `1.0.0.0`. Increase it for each subsequent submission. The Cargo application version is recorded separately in the report.

Microsoft signs accepted Store submissions, so this build intentionally produces an **unsigned** `.msix`; it needs no production signing certificate. `MakeAppx pack` runs normal semantic validation, without `/nv`. This verifies the package structure, not certification or app behavior.

## Build contract

- Windows x64, Rust MSVC, Visual Studio C++ Build Tools and Windows SDK MakeAppx are required. `-CargoPath` and `-MakeAppxPath` accept explicit executable paths.
- The Cargo cache defaults to `target/store-msix`; it does not overwrite standalone/sparse binaries. `-BuildDirectory` changes this cache.
- The PE application manifest is supplied through `THE_CONVERTER_APP_MANIFEST`. It requests `asInvoker` and omits the sparse `msix` element: the full MSIX grants runtime package identity. The GUI executable resource is checked after compilation.
- `+crt-static` avoids an undeclared Visual C++ runtime requirement. Data-only PE inspection checks x64 and rejects dynamic MSVCP/VCRUNTIME/MSVCR imports.
- Logos retain unqualified base files and provide StoreLogo scales 100/125/150/200/400, Square44/Square150 scales 100/200/400 and light/dark unplated target sizes 16/24/32/48/256. Their actual pixel dimensions are validated.
- Existing dependency notices must cover the locked Windows Cargo graph. If missing, first run `release-support/collect-rust-licenses.ps1`.
- Each run creates a fresh `dist/store/<preview|submission>/<version>-<UTC>-<id>/` directory. `-OutputDirectory` changes the parent directory, not the fresh staging rule. No previous output is recursively removed.

Each run returns the package path and produces `submission-report.json`, `SUBMISSION-REPORT.md`, `SHA256SUMS.txt`, tool logs and the exact package layout. The report records identity, both versions, source commit/dirty state, imported libraries, payload hashes and checks that were not performed. No account upload occurs.

The GUI execution alias is `corsivo-the-converter-preview.exe` for preview and `corsivo-the-converter.exe` for submission. The application declares multiple-instance support so quick conversions, queue workers and status windows can run alongside an existing GUI without activation being redirected to it. `store-runtime.json` next to the executable contains only the matching `execution_alias`; the application uses this stable alias for queue placeholders across Store updates.

A preliminary two-application package passed MakeAppx validation but WACK 1.0.1 reported a failed optional Store application-count test: the Store does not support more than one declared application. That warning was not treated as a clean certification result. This layout therefore declares exactly one GUI application and has no CLI binary or console alias. MakeAppx also rejects multiple app-execution-alias extensions inside one application, and the alias schema has no per-alias executable override, so no unsupported workaround is used. Final WACK testing remains separate from package generation.

The package declares only the implemented Italian UI language (`it-it`). An English Store listing is independent of application localization.

Before submission, complete the listing, privacy policy, screenshots, restricted-capability explanation, Windows App Certification Kit and clean-machine app/menu/queue tests. Package generation does not replace Partner Center certification.

## Microsoft references

- [Generating full MSIX components for desktop apps](https://learn.microsoft.com/en-us/windows/msix/desktop/desktop-to-uwp-manual-conversion)
- [Runtime identity and read-only packaged application files](https://learn.microsoft.com/en-us/windows/msix/desktop/desktop-to-uwp-behind-the-scenes)
- [Store signs submitted packages](https://learn.microsoft.com/en-us/windows/msix/package/sign-msix-package-guide#production-microsoft-store-distribution)
- [Uploading MSIX packages](https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msix/upload-app-packages)
- [App execution alias schema](https://learn.microsoft.com/en-us/uwp/schemas/appxpackage/uapmanifestschema/element-uap5-appexecutionalias)
- [Icon sizes and scale variants](https://learn.microsoft.com/en-us/windows/apps/design/style/iconography/app-icon-construction)
