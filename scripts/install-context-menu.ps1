# Installs only the selected Explorer integration, for the current user.
[CmdletBinding(SupportsShouldProcess)]
param(
    [ValidateSet('Classic', 'Modern')][string]$Mode = 'Classic',
    [string]$InstallDirectory,
    [string]$PackagePath
)
$ErrorActionPreference = 'Stop'
$env:PSModulePath = (Join-Path $PSHOME 'Modules') + [IO.Path]::PathSeparator + $env:PSModulePath
if ($Mode -eq 'Modern' -and $PSVersionTable.PSEdition -eq 'Core') {
    # Appx is a Windows PowerShell module; avoid PowerShell 7 compatibility remoting.
    $systemDirectory = if ([Environment]::Is64BitProcess) { 'System32' } else { 'Sysnative' }
    $windowsPowerShell = Join-Path $env:WINDIR "$systemDirectory\WindowsPowerShell\v1.0\powershell.exe"
    $forward = @('-NoLogo', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $PSCommandPath, '-Mode', $Mode)
    if ($InstallDirectory) { $forward += @('-InstallDirectory', $InstallDirectory) }
    if ($PackagePath) { $forward += @('-PackagePath', $PackagePath) }
    if ($WhatIfPreference) { $forward += '-WhatIf' }
    # powershell.exe 5.1 -File cannot bind explicit Boolean switch arguments.
    if ($PSBoundParameters.ContainsKey('Confirm') -and [bool]$PSBoundParameters['Confirm']) { $forward += '-Confirm' }
    & $windowsPowerShell @forward
    if ($LASTEXITCODE -ne 0) { throw "Registrazione menu Windows 11 fallita (PowerShell: $LASTEXITCODE)." }
    return
}
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
    Import-Module (Join-Path $PSScriptRoot 'modern-menu-package.psm1') -Force
    $info = Get-ModernMenuPackageInfo -PackagePath $resolvedPackage -InstallDirectory $installPath -RequireTrustedSignature
    $savedWhatIf = $WhatIfPreference
    try { $WhatIfPreference = $false; Import-Module Appx -ErrorAction Stop } finally { $WhatIfPreference = $savedWhatIf }
    $allRegistered = @(Get-AppxPackage -Name $info.Name | Where-Object { $_.Name -ceq $info.Name })
    foreach ($otherIdentity in @($allRegistered | Where-Object { $_.Publisher -ine $info.Publisher })) {
        $otherPath = Get-ModernMenuExternalPath -Package $otherIdentity
        if (-not $otherPath -or [string]::Equals([IO.Path]::GetFullPath($otherPath).TrimEnd('\'), $installPath.TrimEnd('\'), [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Una precedente identita usa un Publisher diverso. Rimuovi prima il menu con uninstall-context-menu.ps1 -Mode Modern -InstallDirectory e il percorso di questa installazione.'
        }
    }
    $registered = @($allRegistered | Where-Object { $_.Publisher -ieq $info.Publisher })
    if ($registered.Count -gt 1) { throw 'Sono presenti piu identita The Converter per questo utente; registrazione interrotta.' }
    $existing = $registered | Select-Object -First 1
    $existingPath = $null
    if ($existing) {
        if ([version]$existing.Version -gt $info.Version) { throw 'E gia registrata una versione piu recente del menu Windows 11.' }
        $existingPath = Get-ModernMenuExternalPath -Package $existing
        if ([version]$existing.Version -eq $info.Version -and $existingPath -and
            [string]::Equals([IO.Path]::GetFullPath($existingPath).TrimEnd('\'), $installPath.TrimEnd('\'), [StringComparison]::OrdinalIgnoreCase)) {
            Write-Host 'Il menu Windows 11 e gia registrato per questa installazione.'
            return
        }
    }
    if ($PSCmdlet.ShouldProcess($resolvedPackage, 'Registra identita MSIX e menu Windows 11 per questo utente')) {
        try {
            Add-AppxPackage -Path $resolvedPackage -ExternalLocation $installPath -ErrorAction Stop
        } catch {
            $registrationError = $_
            $errorText = [string]$_.Exception + ' ' + [string]$_.ErrorDetails + ' ' + [string]$_
            # Never remove a working registration on trust, schema or unrelated errors.
            # Only an exact-version conflict with a verified different location is repairable.
            $conflict = $errorText -match '(?i)\b0x80073CF[9B]\b'
            $stillRegistered = @(Get-AppxPackage -Name $info.Name | Where-Object {
                $_.Name -ceq $info.Name -and $_.Publisher -ieq $info.Publisher -and [version]$_.Version -eq $info.Version
            })
            $conflictPath = $null
            if ($stillRegistered.Count -eq 1) { $conflictPath = Get-ModernMenuExternalPath -Package $stillRegistered[0] }
            if (-not $conflict -or -not $existing -or [version]$existing.Version -ne $info.Version -or
                -not $existingPath -or -not $conflictPath -or
                -not [string]::Equals([IO.Path]::GetFullPath($existingPath), [IO.Path]::GetFullPath($conflictPath), [StringComparison]::OrdinalIgnoreCase) -or
                [string]::Equals([IO.Path]::GetFullPath($conflictPath).TrimEnd('\'), $installPath.TrimEnd('\'), [StringComparison]::OrdinalIgnoreCase)) {
                throw $registrationError
            }
            if ($PSCmdlet.ShouldProcess($stillRegistered[0].PackageFullName, "Aggiorna il percorso esterno da $conflictPath a $installPath")) {
                Remove-AppxPackage -Package $stillRegistered[0].PackageFullName -ErrorAction Stop
                Add-AppxPackage -Path $resolvedPackage -ExternalLocation $installPath -ErrorAction Stop
            } else { return }
        }
        Write-Host 'Menu Windows 11 registrato. Se non appare, riavvia Esplora file o accedi nuovamente a Windows.'
    }
    return
}
$classId = '{67B51BA1-8A78-4B4B-A5F9-08A825BE91F8}'
$classKey = "HKCU:\Software\Classes\CLSID\$classId"
$menuKey = 'HKCU:\Software\Classes\*\shell\TheConverter'
# Canonical AQS uses only the fast file-extension property, independent of locale.
# Keep this input whitelist aligned with target_mask in the Explorer DLL.
$classicExtensions = @('mp3', 'mpga', 'wav', 'flac', 'aac', 'm4a', 'ogg', 'opus', 'wma', 'aiff', 'aif', 'alac', 'ape', 'mp4', 'mkv', 'mov', 'avi', 'webm', 'wmv', 'm4v', 'mpeg', 'mpg', 'ts', 'mts', 'm2ts', '3gp', 'flv', 'png', 'jpg', 'jpeg', 'webp', 'bmp', 'tif', 'tiff', 'gif', 'ico', 'pdf', 'doc', 'docx', 'odt', 'rtf', 'txt', 'xlsx', 'xls', 'ods', 'ppt', 'pptx', 'odp')
$appliesTo = ($classicExtensions | ForEach-Object { 'System.FileExtension:=".' + $_ + '"' }) -join ' OR '
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
    New-ItemProperty -LiteralPath $menuKey -Name 'AppliesTo' -Value $appliesTo -PropertyType String -Force | Out-Null
    Write-Host 'Menu classico installato per questo utente. Su Windows 11 si trova in Mostra altre opzioni.'
}
