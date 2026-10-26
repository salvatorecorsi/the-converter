# Downloads pinned, checksum-verified Windows x64 conversion engines locally.
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$cache = Join-Path $root '.tools\downloads'
$toolRoot = Join-Path $root 'tools'
New-Item -ItemType Directory -Force -Path $cache, $toolRoot | Out-Null
$packages = @(
    @{
        Name = 'ffmpeg'
        Url = 'https://github.com/GyanD/codexffmpeg/releases/download/9.0.2/ffmpeg-9.0.2-essentials_build.zip'
        Hash = '60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba'
        Folder = 'ffmpeg-9.0.2-essentials_build'
    },
    @{
        Name = 'poppler'
        Url = 'https://github.com/oschwartz10612/poppler-windows/releases/download/v26.09.0-0/Release-26.09.0-0.zip'
        Hash = '7a6f256a0ddf7536182246a5733331bf4677cbcc34f4663774947ad34556c8d0'
        Folder = 'poppler-26.09.0'
    }
)
foreach ($package in $packages) {
    $archive = Join-Path $cache ($package.Name + '.zip')
    if (-not (Test-Path -LiteralPath $archive)) {
        Write-Host "Download $($package.Name)..."
        Invoke-WebRequest -Uri $package.Url -OutFile $archive
    }
    if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $package.Hash) {
        throw "SHA-256 non corrisponde: $archive. Nessun eseguibile di questo archivio e stato copiato."
    }
    $stage = Join-Path $cache ($package.Name + '-' + [guid]::NewGuid().ToString('N'))
    Expand-Archive -LiteralPath $archive -DestinationPath $stage
    $source = Join-Path $stage $package.Folder
    $destination = Join-Path $toolRoot $package.Name
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    # Keep the upstream layout, documentation and any license notices included in the archive.
    Copy-Item -Path (Join-Path $source '*') -Destination $destination -Recurse -Force
    Write-Host "$($package.Name): hash verificato, pronto in $destination"
}
Write-Host 'Motori pronti. Nessuna modifica a PATH, registro o installazioni di sistema.'
