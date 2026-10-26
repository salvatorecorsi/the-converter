[CmdletBinding(SupportsShouldProcess)]
param([ValidateSet('Classic', 'Modern', 'All')][string]$Mode = 'All')
$ErrorActionPreference = 'Stop'
if ($Mode -in @('Classic', 'All')) {
    # Exact, fixed keys owned exclusively by this application.
    foreach ($key in @('HKCU:\Software\Classes\*\shell\TheConverter', 'HKCU:\Software\Classes\CLSID\{67B51BA1-8A78-4B4B-A5F9-08A825BE91F8}')) {
        if ((Test-Path -LiteralPath $key) -and $PSCmdlet.ShouldProcess($key, 'Rimuovi integrazione The Converter')) {
            Remove-Item -LiteralPath $key -Recurse -Force
        }
    }
}
if ($Mode -in @('Modern', 'All')) {
    foreach ($package in @(Get-AppxPackage -Name 'SalvatoreCorsi.TheConverter')) {
        if ($PSCmdlet.ShouldProcess($package.PackageFullName, 'Rimuovi identita MSIX e menu Windows 11')) {
            Remove-AppxPackage -Package $package.PackageFullName
        }
    }
}
Write-Host 'Rimozione completata. File applicazione e preferenze sono conservati.'
