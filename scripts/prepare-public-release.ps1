[CmdletBinding()]
param(
    [string]$DistributionDirectory,
    [string]$CompilerPath,
    [string]$Version,
    [string]$IdentityPackagePath,
    [switch]$RequireModernMenu
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
if (-not $DistributionDirectory) { $DistributionDirectory = Join-Path $root 'dist\TheConverter' }
$source = (Resolve-Path -LiteralPath $DistributionDirectory).Path
if (-not $Version) {
    $match = [regex]::Match((Get-Content -LiteralPath (Join-Path $root 'Cargo.toml') -Raw), '(?m)^version\s*=\s*"([^"]+)"')
    if (-not $match.Success) { throw 'Versione non trovata.' }
    $Version = $match.Groups[1].Value
}
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw 'La release richiede una versione numerica x.y.z.' }
Import-Module (Join-Path $PSScriptRoot 'modern-menu-package.psm1') -Force
if (-not $IdentityPackagePath) { $IdentityPackagePath = Join-Path $source 'TheConverter.Identity.msix' }
$modernInfo = $null
if (Test-Path -LiteralPath $IdentityPackagePath -PathType Leaf) {
    $candidate = Get-ModernMenuPackageInfo -PackagePath $IdentityPackagePath -InstallDirectory $source
    if ($candidate.Version -ne [version]($Version + '.0')) { throw 'Versione MSIX diversa dalla versione della release.' }
    $signature = Get-AuthenticodeSignature -LiteralPath $IdentityPackagePath
    if ($signature.Status -eq 'Valid') {
        if ($signature.SignerCertificate.Subject -eq $signature.SignerCertificate.Issuer) {
            throw 'Un certificato locale autofirmato non e adatto alla release pubblica. Usa build-installer.ps1 per una prova privata.'
        }
        $modernInfo = Get-ModernMenuPackageInfo -PackagePath $IdentityPackagePath -InstallDirectory $source -RequireTrustedSignature
    }
}
if ($RequireModernMenu -and -not $modernInfo) {
    throw 'La release con menu moderno richiede un pacchetto MSIX firmato attendibile. Nessun installer classico viene prodotto al suo posto.'
}
$variant = if ($modernInfo) { 'modern' } else { 'classic' }
$stage = Join-Path $root "dist\public-stage\$Version\$variant\TheConverter"
$output = Join-Path $root "dist\releases\v$Version"
New-Item -ItemType Directory -Force -Path $stage, $output | Out-Null
# An explicit allowlist keeps local GPL runtime bundles and test artifacts out of public assets.
foreach ($name in @('the-converter.exe', 'the-converter-cli.exe', 'the_converter_shell.dll')) {
    Copy-Item -LiteralPath (Join-Path $source $name) -Destination $stage -Force
}
foreach ($name in @('README.md', 'THIRD-PARTY-NOTICES.md')) {
    Copy-Item -LiteralPath (Join-Path $root $name) -Destination $stage -Force
}
$scripts = Join-Path $stage 'scripts'
$docs = Join-Path $stage 'docs'
New-Item -ItemType Directory -Force -Path $scripts, $docs | Out-Null
foreach ($name in @('install-context-menu.ps1', 'uninstall-context-menu.ps1', 'install-runtime-tools.ps1', 'modern-menu-package.psm1')) {
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination $scripts -Force
}
Copy-Item -Path (Join-Path $root 'docs\*') -Destination $docs -Recurse -Force
if ($modernInfo) {
    Copy-Item -LiteralPath $IdentityPackagePath -Destination (Join-Path $stage 'TheConverter.Identity.msix') -Force
    foreach ($relative in $modernInfo.Assets) {
        $target = Join-Path $stage $relative
        New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force | Out-Null
        Copy-Item -LiteralPath (Join-Path $source $relative) -Destination $target -Force
    }
}
& (Join-Path $root 'release-support\verify-thin-release.ps1') -SourceDirectory $stage
& (Join-Path $PSScriptRoot 'build-installer.ps1') -SourceDirectory $stage -OutputDirectory $output -CompilerPath $CompilerPath -Version $Version
$portable = Join-Path $output "TheConverter-$Version-windows-x64-portable.zip"
Compress-Archive -LiteralPath $stage -DestinationPath $portable -CompressionLevel Optimal -Force
$setup = Join-Path $output "TheConverter-$Version-windows-x64-setup.exe"
$checksums = foreach ($path in @($setup, $portable)) {
    $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $([IO.Path]::GetFileName($path))"
}
$checksums | Set-Content -LiteralPath (Join-Path $output 'SHA256SUMS.txt') -Encoding ASCII
Get-Item -LiteralPath $setup, $portable, (Join-Path $output 'SHA256SUMS.txt') | Select-Object FullName, Length
