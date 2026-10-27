[CmdletBinding(SupportsShouldProcess)]
param(
    [ValidateSet('Classic', 'Modern', 'All')][string]$Mode = 'All',
    [string]$InstallDirectory
)
$ErrorActionPreference = 'Stop'
$env:PSModulePath = (Join-Path $PSHOME 'Modules') + [IO.Path]::PathSeparator + $env:PSModulePath
if ($Mode -in @('Modern', 'All') -and $PSVersionTable.PSEdition -eq 'Core') {
    $systemDirectory = if ([Environment]::Is64BitProcess) { 'System32' } else { 'Sysnative' }
    $windowsPowerShell = Join-Path $env:WINDIR "$systemDirectory\WindowsPowerShell\v1.0\powershell.exe"
    $forward = @('-NoLogo', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $PSCommandPath, '-Mode', $Mode)
    if ($InstallDirectory) { $forward += @('-InstallDirectory', $InstallDirectory) }
    if ($WhatIfPreference) { $forward += '-WhatIf' }
    if ($PSBoundParameters.ContainsKey('Confirm') -and [bool]$PSBoundParameters['Confirm']) { $forward += '-Confirm' }
    & $windowsPowerShell @forward
    if ($LASTEXITCODE -ne 0) { throw "Rimozione menu Windows 11 fallita (PowerShell: $LASTEXITCODE)." }
    return
}
$installPath = $null
if ($InstallDirectory) { $installPath = (Resolve-Path -LiteralPath $InstallDirectory).Path }
if ($Mode -in @('Classic', 'All')) {
    # Exact, fixed keys owned exclusively by this application.
    $classicKeys = @('HKCU:\Software\Classes\*\shell\TheConverter', 'HKCU:\Software\Classes\CLSID\{67B51BA1-8A78-4B4B-A5F9-08A825BE91F8}')
    $ownsClassic = $true
    if ($installPath) {
        $serverKey = $classicKeys[1] + '\InprocServer32'
        $ownsClassic = (Test-Path -LiteralPath $serverKey) -and
            [string]::Equals([string](Get-Item -LiteralPath $serverKey).GetValue(''), (Join-Path $installPath 'the_converter_shell.dll'), [StringComparison]::OrdinalIgnoreCase)
    }
    foreach ($key in $classicKeys) {
        if (-not $ownsClassic) { continue }
        if ((Test-Path -LiteralPath $key) -and $PSCmdlet.ShouldProcess($key, 'Rimuovi integrazione The Converter')) {
            Remove-Item -LiteralPath $key -Recurse -Force
        }
    }
}
if ($Mode -in @('Modern', 'All')) {
    Import-Module (Join-Path $PSScriptRoot 'modern-menu-package.psm1') -Force
    $savedWhatIf = $WhatIfPreference
    try { $WhatIfPreference = $false; Import-Module Appx -ErrorAction Stop } finally { $WhatIfPreference = $savedWhatIf }
    # The publisher may change when moving from development to production signing.
    # Exact Name plus verified external location identifies our installation.
    foreach ($package in @(Get-AppxPackage -Name 'SalvatoreCorsi.TheConverter' | Where-Object { $_.Name -ceq 'SalvatoreCorsi.TheConverter' })) {
        if ($installPath) {
            $externalPath = Get-ModernMenuExternalPath -Package $package
            if (-not $externalPath) { throw 'Il percorso esterno del menu non e verificabile; nessun pacchetto e stato rimosso.' }
            if (-not [string]::Equals([IO.Path]::GetFullPath($externalPath).TrimEnd('\'), $installPath.TrimEnd('\'), [StringComparison]::OrdinalIgnoreCase)) {
                Write-Verbose 'Identita associata a un altra installazione: conservata.'
                continue
            }
        }
        if ($PSCmdlet.ShouldProcess($package.PackageFullName, 'Rimuovi identita MSIX e menu Windows 11')) {
            Remove-AppxPackage -Package $package.PackageFullName -ErrorAction Stop
        }
    }
}
Write-Host 'Rimozione completata. File applicazione e preferenze sono conservati.'
