[CmdletBinding()]
param(
    [string]$SourceDirectory,
    [string]$OutputDirectory,
    [string]$CompilerPath,
    [string]$Version
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
if (-not $SourceDirectory) { $SourceDirectory = Join-Path $root 'dist\TheConverter' }
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $root 'dist\release' }
$source = (Resolve-Path -LiteralPath $SourceDirectory).Path
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$output = (Resolve-Path -LiteralPath $OutputDirectory).Path
if (-not $Version) {
    $packageSection = (Get-Content -LiteralPath (Join-Path $root 'Cargo.toml') -Raw) -split '\[dependencies\]', 2 | Select-Object -First 1
    $versionMatch = [regex]::Match($packageSection, '(?m)^version\s*=\s*"([^"]+)"')
    if (-not $versionMatch.Success) { throw 'Versione non trovata in Cargo.toml.' }
    $Version = $versionMatch.Groups[1].Value
}
if ($Version -notmatch '^\d+\.\d+\.\d+(?:[-+][A-Za-z0-9.-]+)?$') { throw "Versione non valida: $Version" }
$fileVersion = ($Version -split '[-+]', 2)[0] + '.0'
foreach ($required in @('the-converter.exe', 'the-converter-cli.exe', 'the_converter_shell.dll', 'README.md', 'THIRD-PARTY-NOTICES.md')) {
    if (-not (Test-Path -LiteralPath (Join-Path $source $required) -PathType Leaf)) { throw "Distribuzione incompleta: manca $required. Esegui prima build-release.ps1." }
}
if (-not (Test-Path -LiteralPath (Join-Path $PSScriptRoot 'install-runtime-tools.ps1') -PathType Leaf)) {
    throw 'Manca scripts/install-runtime-tools.ps1.'
}
if (-not $CompilerPath) {
    $found = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($found) { $CompilerPath = $found.Source }
    else {
        foreach ($candidate in @(
            (Join-Path $root '.tools\inno\ISCC.exe'),
            (Join-Path $root '.tools\innosetup\ISCC.exe'),
            (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 7\ISCC.exe'),
            (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe')
        )) {
            if (Test-Path -LiteralPath $candidate -PathType Leaf) { $CompilerPath = $candidate; break }
        }
    }
}
if (-not $CompilerPath) { throw 'Inno Setup Compiler non trovato. Passa -CompilerPath con il percorso di ISCC.exe.' }
$compiler = (Resolve-Path -LiteralPath $CompilerPath).Path
$modern = 0
$package = Join-Path $source 'TheConverter.Identity.msix'
if (Test-Path -LiteralPath $package -PathType Leaf) {
    if ((Get-AuthenticodeSignature -LiteralPath $package).Status -eq 'Valid') { $modern = 1 }
}
# The installer deliberately includes no FFmpeg/Poppler binaries from tools/.
# Those are downloaded directly from upstream by the selected installation task.
& $compiler ('/DAppVersion=' + $Version) ('/DFileVersion=' + $fileVersion) ('/DSourceDir=' + $source) ('/DOutputPath=' + $output) ('/DModernMenuAvailable=' + $modern) (Join-Path $root 'installer\TheConverter.iss')
if ($LASTEXITCODE -ne 0) { throw "Compilazione installer fallita (ISCC: $LASTEXITCODE)." }
$artifact = Join-Path $output "TheConverter-$Version-windows-x64-setup.exe"
if (-not (Test-Path -LiteralPath $artifact -PathType Leaf)) { throw 'ISCC non ha prodotto il setup atteso.' }
Get-Item -LiteralPath $artifact | Select-Object FullName, Length
