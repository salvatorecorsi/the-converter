# Download pinned conversion engines directly from their original distributors.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$InstallDirectory,
    [string]$ArchiveDirectory
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
# Setup may inherit PowerShell 7's module path. Prefer this host's own modules,
# including Windows PowerShell 5.1's Get-FileHash and Expand-Archive functions.
$env:PSModulePath = (Join-Path $PSHOME 'Modules') + [IO.Path]::PathSeparator + $env:PSModulePath
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
$installRoot = (Resolve-Path -LiteralPath $InstallDirectory).Path
if (-not (Test-Path -LiteralPath (Join-Path $installRoot 'the-converter.exe') -PathType Leaf)) {
    throw 'La cartella selezionata non contiene The Converter.'
}
$toolsRoot = Join-Path $installRoot 'tools'
New-Item -ItemType Directory -Force -Path $toolsRoot | Out-Null
$transcriptStarted = $false
try {
    $logDirectory = Join-Path $env:LOCALAPPDATA 'SalvatoreCorsi\TheConverter\logs'
    New-Item -ItemType Directory -Force -Path $logDirectory | Out-Null
    Start-Transcript -LiteralPath (Join-Path $logDirectory 'runtime-install.log') -Force | Out-Null
    $transcriptStarted = $true
} catch { Write-Verbose 'Registro del download non disponibile.' }
$tempParent = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')
$stageRoot = Join-Path $tempParent ('TheConverter-Runtimes-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $stageRoot | Out-Null

function Assert-ChildPath([string]$Path, [string]$Parent) {
    $resolved = [IO.Path]::GetFullPath($Path)
    $prefix = [IO.Path]::GetFullPath($Parent).TrimEnd('\') + '\'
    if (-not $resolved.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Percorso fuori dalla cartella prevista: $resolved"
    }
    return $resolved
}
function Read-PinnedArchive($Package, [string]$Destination) {
    if ($ArchiveDirectory) {
        $cached = Join-Path (Resolve-Path -LiteralPath $ArchiveDirectory).Path ($Package.Name + '.zip')
        if (Test-Path -LiteralPath $cached -PathType Leaf) { Copy-Item -LiteralPath $cached -Destination $Destination }
    }
    if (-not (Test-Path -LiteralPath $Destination)) {
        for ($attempt = 1; $attempt -le 3; $attempt++) {
            try {
                Write-Host "Download $($Package.Label), tentativo $attempt/3..."
                Invoke-WebRequest -UseBasicParsing -Uri $Package.Url -OutFile $Destination
                break
            } catch {
                if ($attempt -eq 3) { throw }
                Start-Sleep -Seconds 2
            }
        }
    }
    if ((Get-FileHash -LiteralPath $Destination -Algorithm SHA256).Hash -ne $Package.Hash) {
        throw "Verifica SHA-256 non riuscita per $($Package.Label). Nessun file di questo archivio viene installato."
    }
}

$packages = @(
    @{
        Name = 'ffmpeg'; Label = 'FFmpeg 9.0.2'; Folder = 'ffmpeg-9.0.2-essentials_build'
        Url = 'https://github.com/GyanD/codexffmpeg/releases/download/9.0.2/ffmpeg-9.0.2-essentials_build.zip'
        Hash = '60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba'
        Required = @('bin\ffmpeg.exe', 'bin\ffprobe.exe')
    },
    @{
        Name = 'poppler'; Label = 'Poppler 26.09.0'; Folder = 'poppler-26.09.0'
        Url = 'https://github.com/oschwartz10612/poppler-windows/releases/download/v26.09.0-0/Release-26.09.0-0.zip'
        Hash = '7a6f256a0ddf7536182246a5733331bf4677cbcc34f4663774947ad34556c8d0'
        Required = @('Library\bin\pdftoppm.exe', 'Library\bin\pdftotext.exe', 'Library\bin\pdfinfo.exe')
    }
)
try {
    foreach ($package in $packages) {
        $destination = Assert-ChildPath (Join-Path $toolsRoot $package.Name) $toolsRoot
        $marker = Join-Path $destination '.the-converter-runtime.json'
        $installed = $false
        if (Test-Path -LiteralPath $marker -PathType Leaf) {
            try {
                $metadata = Get-Content -LiteralPath $marker -Raw | ConvertFrom-Json
                $installed = $metadata.sha256 -eq $package.Hash
                foreach ($required in $package.Required) {
                    $installed = $installed -and (Test-Path -LiteralPath (Join-Path $destination $required) -PathType Leaf)
                }
            } catch { $installed = $false }
        }
        if ($installed) { Write-Host "$($package.Label): gia pronto."; continue }
        $archive = Join-Path $stageRoot ($package.Name + '.zip')
        Read-PinnedArchive $package $archive
        $extract = Join-Path $stageRoot ($package.Name + '-extracted')
        Write-Host "Estrazione $($package.Label)..."
        Expand-Archive -LiteralPath $archive -DestinationPath $extract
        $source = Join-Path $extract $package.Folder
        $prepared = Assert-ChildPath (Join-Path $toolsRoot ('.install-' + $package.Name + '-' + [guid]::NewGuid().ToString('N'))) $toolsRoot
        New-Item -ItemType Directory -Path $prepared | Out-Null
        $backup = $null
        try {
            if ($package.Name -eq 'ffmpeg') {
                New-Item -ItemType Directory -Path (Join-Path $prepared 'bin') | Out-Null
                foreach ($file in $package.Required) {
                    Copy-Item -LiteralPath (Join-Path $source $file) -Destination (Join-Path $prepared $file)
                }
                foreach ($name in @('LICENSE', 'README.txt', 'doc')) {
                    $item = Join-Path $source $name
                    if (Test-Path -LiteralPath $item) { Copy-Item -LiteralPath $item -Destination $prepared -Recurse }
                }
            } else {
                $binaryDir = Join-Path $prepared 'Library\bin'
                New-Item -ItemType Directory -Path $binaryDir -Force | Out-Null
                Copy-Item -Path (Join-Path $source 'Library\bin\*.dll') -Destination $binaryDir
                foreach ($file in $package.Required) {
                    Copy-Item -LiteralPath (Join-Path $source $file) -Destination (Join-Path $prepared $file)
                }
                foreach ($name in @('share', 'Library\share')) {
                    $item = Join-Path $source $name
                    if (Test-Path -LiteralPath $item) {
                        $parent = Split-Path -Parent (Join-Path $prepared $name)
                        New-Item -ItemType Directory -Path $parent -Force | Out-Null
                        Copy-Item -LiteralPath $item -Destination $parent -Recurse
                    }
                }
            }
            foreach ($required in $package.Required) {
                if (-not (Test-Path -LiteralPath (Join-Path $prepared $required) -PathType Leaf)) {
                    throw "Archivio incompleto: $required"
                }
            }
            @{ name = $package.Label; url = $package.Url; sha256 = $package.Hash } |
                ConvertTo-Json | Set-Content -LiteralPath (Join-Path $prepared '.the-converter-runtime.json') -Encoding UTF8
            if (Test-Path -LiteralPath $destination) {
                $backup = Assert-ChildPath (Join-Path $toolsRoot ('.previous-' + $package.Name + '-' + [guid]::NewGuid().ToString('N'))) $toolsRoot
                Move-Item -LiteralPath (Assert-ChildPath $destination $toolsRoot) -Destination $backup
            }
            try { Move-Item -LiteralPath (Assert-ChildPath $prepared $toolsRoot) -Destination $destination }
            catch {
                if ($backup -and (Test-Path -LiteralPath $backup)) {
                    Move-Item -LiteralPath (Assert-ChildPath $backup $toolsRoot) -Destination $destination
                    $backup = $null
                }
                throw
            }
            Write-Host "$($package.Label): installato e verificato."
        } finally {
            if (Test-Path -LiteralPath $prepared) {
                Remove-Item -LiteralPath (Assert-ChildPath $prepared $toolsRoot) -Recurse -Force
            }
            if ($backup -and (Test-Path -LiteralPath $backup)) {
                try { Remove-Item -LiteralPath (Assert-ChildPath $backup $toolsRoot) -Recurse -Force }
                catch { Write-Warning 'Una vecchia versione del motore e ancora in uso; verra rimossa dal disinstaller.' }
            }
        }
    }
    Write-Host 'Motori pronti. Puoi usare le conversioni audio, video e PDF.'
} finally {
    try {
        $checkedStage = Assert-ChildPath $stageRoot $tempParent
        if ([IO.Path]::GetFileName($checkedStage) -notlike 'TheConverter-Runtimes-*') { throw 'Cartella temporanea non riconosciuta' }
        if (Test-Path -LiteralPath $checkedStage) { Remove-Item -LiteralPath $checkedStage -Recurse -Force }
    } finally {
        if ($transcriptStarted) { Stop-Transcript | Out-Null }
    }
}
