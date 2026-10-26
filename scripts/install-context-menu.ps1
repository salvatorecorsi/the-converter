# Installs only the selected Explorer integration, for the current user.
[CmdletBinding(SupportsShouldProcess)]
param(
    [ValidateSet('Classic', 'Modern')][string]$Mode = 'Classic',
    [string]$InstallDirectory,
    [string]$PackagePath
)
$ErrorActionPreference = 'Stop'
if (-not $InstallDirectory) {
    $adjacentApp = Join-Path $PSScriptRoot '..\the-converter.exe'
    if (Test-Path -LiteralPath $adjacentApp -PathType Leaf) { $InstallDirectory = Join-Path $PSScriptRoot '..' }
    else { $InstallDirectory = Join-Path $PSScriptRoot '..\dist\TheConverter' }
}
$installPath = (Resolve-Path -LiteralPath $InstallDirectory).Path
$executable = Join-Path $installPath 'the-converter.exe'
$library = Join-Path $installPath 'the_converter_shell.dll'
foreach ($file in @($executable, $library)) {
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw "File mancante: $file. Esegui prima build-release.ps1." }
}
if (-not [Environment]::Is64BitOperatingSystem -or -not [Environment]::Is64BitProcess -or $env:PROCESSOR_ARCHITECTURE -eq 'ARM64' -or $env:PROCESSOR_ARCHITEW6432 -eq 'ARM64') {
    throw 'Questa distribuzione shell e x64; usa un build della stessa architettura di Esplora file.'
}
if ($Mode -eq 'Modern') {
    if ([Environment]::OSVersion.Version.Build -lt 22000) { throw 'Il menu moderno richiede Windows 11.' }
    if (-not $PackagePath) { $PackagePath = Join-Path $installPath 'TheConverter.Identity.msix' }
    $resolvedPackage = (Resolve-Path -LiteralPath $PackagePath).Path
    $signature = Get-AuthenticodeSignature -LiteralPath $resolvedPackage
    if ($signature.Status -ne 'Valid') {
        throw "Il pacchetto MSIX deve avere una firma valida e attendibile (stato: $($signature.Status)). Non viene modificato l'archivio certificati. Vedi docs/windows-integration.md."
    }
    if ($PSCmdlet.ShouldProcess($resolvedPackage, 'Registra identita MSIX e menu Windows 11 per questo utente')) {
        Add-AppxPackage -Path $resolvedPackage -ExternalLocation $installPath
        Write-Host 'Menu Windows 11 registrato. Se non appare, riavvia Esplora file o accedi nuovamente a Windows.'
    }
    return
}
$classId = '{67B51BA1-8A78-4B4B-A5F9-08A825BE91F8}'
$classKey = "HKCU:\Software\Classes\CLSID\$classId"
$menuKey = 'HKCU:\Software\Classes\*\shell\TheConverter'
if ($PSCmdlet.ShouldProcess('HKCU\Software\Classes', 'Installa il comando classico The Converter')) {
    New-Item -Path $classKey -Force | Out-Null
    Set-Item -LiteralPath $classKey -Value 'The Converter Explorer Command'
    $server = New-Item -Path "$classKey\InprocServer32" -Force
    Set-Item -LiteralPath $server.PSPath -Value $library
    New-ItemProperty -LiteralPath $server.PSPath -Name 'ThreadingModel' -Value 'Apartment' -PropertyType String -Force | Out-Null
    New-Item -Path $menuKey -Force | Out-Null
    New-ItemProperty -LiteralPath $menuKey -Name 'MUIVerb' -Value 'Converti in' -PropertyType String -Force | Out-Null
    New-ItemProperty -LiteralPath $menuKey -Name 'Icon' -Value ('"' + $executable + '",0') -PropertyType String -Force | Out-Null
    New-ItemProperty -LiteralPath $menuKey -Name 'ExplorerCommandHandler' -Value $classId -PropertyType String -Force | Out-Null
    New-ItemProperty -LiteralPath $menuKey -Name 'MultiSelectModel' -Value 'Player' -PropertyType String -Force | Out-Null
    Write-Host 'Menu classico installato per questo utente. Su Windows 11 si trova in Mostra altre opzioni.'
}
