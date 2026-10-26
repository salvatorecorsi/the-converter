[CmdletBinding()]
param(
    [string]$CargoPath = 'cargo',
    [switch]$SkipBuild,
    [string]$CertificateThumbprint
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$output = Join-Path $root 'dist\TheConverter'
if ($CargoPath -eq 'cargo' -and -not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $localCargo = Join-Path $root '.tools\cargo\bin\cargo.exe'
    if (Test-Path -LiteralPath $localCargo -PathType Leaf) {
        $CargoPath = $localCargo
        $env:CARGO_HOME = Join-Path $root '.tools\cargo'
        $env:RUSTUP_HOME = Join-Path $root '.tools\rustup'
    }
}
if (-not $SkipBuild) {
    Push-Location -LiteralPath $root
    try {
        & $CargoPath build --release --locked
        if ($LASTEXITCODE -ne 0) { throw 'Compilazione applicazione fallita.' }
        & $CargoPath build --release --locked --manifest-path (Join-Path $root 'shell-extension\Cargo.toml')
        if ($LASTEXITCODE -ne 0) { throw 'Compilazione estensione Explorer fallita.' }
    } finally { Pop-Location }
}
New-Item -ItemType Directory -Path $output -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $root 'target\release\the-converter.exe') -Destination $output -Force
Copy-Item -LiteralPath (Join-Path $root 'target\release\the-converter-cli.exe') -Destination $output -Force
Copy-Item -LiteralPath (Join-Path $root 'shell-extension\target\release\the_converter_shell.dll') -Destination $output -Force
$scriptOutput = Join-Path $output 'scripts'
New-Item -ItemType Directory -Path $scriptOutput -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'install-context-menu.ps1'), (Join-Path $PSScriptRoot 'uninstall-context-menu.ps1'), (Join-Path $PSScriptRoot 'test-shell-extension.ps1') -Destination $scriptOutput -Force
# Media tools are optional and are never downloaded by this script.
$mediaTools = Join-Path $root 'tools'
if (Test-Path -LiteralPath $mediaTools -PathType Container) {
    $mediaOutput = Join-Path $output 'tools'
    New-Item -ItemType Directory -Path $mediaOutput -Force | Out-Null
    Copy-Item -Path (Join-Path $mediaTools '*') -Destination $mediaOutput -Recurse -Force
}
if (Test-Path -LiteralPath (Join-Path $root 'README.md')) { Copy-Item -LiteralPath (Join-Path $root 'README.md') -Destination $output -Force }
if (Test-Path -LiteralPath (Join-Path $root 'THIRD-PARTY-NOTICES.md')) { Copy-Item -LiteralPath (Join-Path $root 'THIRD-PARTY-NOTICES.md') -Destination $output -Force }
$docsOutput = Join-Path $output 'docs'
New-Item -ItemType Directory -Path $docsOutput -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $root 'docs\windows-integration.md'), (Join-Path $root 'docs\queued-conversions.md') -Destination $docsOutput -Force

# Render simple native assets for MSIX. No external graphics tools are needed.
Add-Type -AssemblyName System.Drawing
$packageStage = Join-Path $root 'dist\identity-package'
$assetOutput = Join-Path $packageStage 'Assets'
New-Item -ItemType Directory -Path $assetOutput -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $root 'shell-extension\package\AppxManifest.xml') -Destination $packageStage -Force
foreach ($asset in @(@('StoreLogo.png', 50), @('Square44x44Logo.png', 44), @('Square150x150Logo.png', 150))) {
    $size = [int]$asset[1]
    $bitmap = New-Object System.Drawing.Bitmap($size, $size)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $graphics.Clear([System.Drawing.Color]::FromArgb(18, 24, 35))
    $pen = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(67, 139, 242), [single]($size * 0.09))
    $pen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $pen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    $graphics.DrawLine($pen, [single]($size * .22), [single]($size * .35), [single]($size * .76), [single]($size * .35))
    $graphics.DrawLine($pen, [single]($size * .64), [single]($size * .23), [single]($size * .76), [single]($size * .35))
    $graphics.DrawLine($pen, [single]($size * .76), [single]($size * .35), [single]($size * .64), [single]($size * .47))
    $graphics.DrawLine($pen, [single]($size * .78), [single]($size * .65), [single]($size * .24), [single]($size * .65))
    $graphics.DrawLine($pen, [single]($size * .36), [single]($size * .53), [single]($size * .24), [single]($size * .65))
    $graphics.DrawLine($pen, [single]($size * .24), [single]($size * .65), [single]($size * .36), [single]($size * .77))
    $appImagePath = Join-Path $root 'assets\app.png'
    if (Test-Path -LiteralPath $appImagePath -PathType Leaf) {
        $appImage = [System.Drawing.Image]::FromFile($appImagePath)
        try {
            $graphics.Clear([System.Drawing.Color]::Transparent)
            $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $graphics.DrawImage($appImage, 0, 0, $size, $size)
        } finally { $appImage.Dispose() }
    }
    $bitmap.Save((Join-Path $assetOutput $asset[0]), [System.Drawing.Imaging.ImageFormat]::Png)
    $pen.Dispose(); $graphics.Dispose(); $bitmap.Dispose()
}
$installedAssets = Join-Path $output 'Assets'
New-Item -ItemType Directory -Path $installedAssets -Force | Out-Null
Copy-Item -Path (Join-Path $assetOutput '*') -Destination $installedAssets -Force
$sdkBin = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
$makeAppx = Get-ChildItem -LiteralPath $sdkBin -Filter makeappx.exe -Recurse -ErrorAction SilentlyContinue |
    Where-Object { $_.Directory.Name -eq 'x64' } | Sort-Object FullName -Descending | Select-Object -First 1
if (-not $makeAppx) {
    Write-Warning 'Windows SDK MakeAppx non trovato: eseguibile e DLL pronti; pacchetto identita non generato.'
    return
}
$packagePath = Join-Path $output 'TheConverter.Identity.msix'
& $makeAppx.FullName pack /o /nv /d $packageStage /p $packagePath
if ($LASTEXITCODE -ne 0) { throw 'Creazione pacchetto identita fallita.' }
if ($CertificateThumbprint) {
    $signTool = Join-Path $makeAppx.Directory.FullName 'signtool.exe'
    & $signTool sign /fd SHA256 /sha1 $CertificateThumbprint $packagePath
    if ($LASTEXITCODE -ne 0) { throw 'Firma MSIX fallita. Il certificato deve avere Subject CN=Salvatore Corsi.' }
} else {
    Write-Host 'MSIX creato senza firma: firma e trust sono necessari prima di installare il menu moderno.'
}
Write-Host "Distribuzione pronta: $output"
