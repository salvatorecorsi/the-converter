# Isolated fixture tests. No real Appx, registry, certificate or Explorer changes.
[CmdletBinding()]
param([switch]$RunInWorker)
$ErrorActionPreference = 'Stop'
if (-not $RunInWorker) {
    $systemDirectory = if ([Environment]::Is64BitProcess) { 'System32' } else { 'Sysnative' }
    & (Join-Path $env:WINDIR "$systemDirectory\WindowsPowerShell\v1.0\powershell.exe") -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $PSCommandPath -RunInWorker
    if ($LASTEXITCODE -ne 0) { throw "Verifica menu moderno fallita (exit: $LASTEXITCODE)." }
    return
}
if ($PSVersionTable.PSEdition -ne 'Desktop') { throw 'I test Appx simulati devono usare Windows PowerShell 5.1.' }
$env:PSModulePath = (Join-Path $PSHOME 'Modules') + [IO.Path]::PathSeparator + $env:PSModulePath
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')
$stage = Join-Path $temporaryRoot ('converter-modern-tests-' + [guid]::NewGuid().ToString('N'))
$mockNames = @('Get-AuthenticodeSignature', 'Get-AppxPackage', 'Add-AppxPackage', 'Remove-AppxPackage')
$previousFunctions = @{}
foreach ($name in $mockNames) {
    $existing = Get-Command $name -CommandType Function -ErrorAction SilentlyContinue
    if ($existing) { $previousFunctions[$name] = $existing.ScriptBlock }
}
$testVariableNames = @('ConverterModernTestRegistered', 'ConverterModernTestAddCalls', 'ConverterModernTestRemoveCalls', 'ConverterModernTestAddMode', 'ConverterModernTestRemoveFails', 'ConverterModernTestSubject')
$previousVariables = @{}
foreach ($name in $testVariableNames) {
    $existing = Get-Variable -Name $name -Scope Global -ErrorAction SilentlyContinue
    if ($existing) { $previousVariables[$name] = $existing.Value }
}
$checks = 0
function Assert([bool]$Condition, [string]$Message) { if (-not $Condition) { throw $Message }; $script:checks++ }
function Assert-Fails([scriptblock]$Code, [string]$Message) {
    $failed = $false
    try { & $Code | Out-Null } catch { $failed = $true }
    Assert $failed $Message
}
try {
    New-Item -ItemType Directory -Path (Join-Path $stage 'Assets') -Force | Out-Null
    foreach ($name in @('the-converter.exe', 'the_converter_shell.dll', 'Assets\StoreLogo.png', 'Assets\Square150x150Logo.png', 'Assets\Square44x44Logo.png')) {
        [IO.File]::WriteAllBytes((Join-Path $stage $name), [byte[]]@(1, 2, 3))
    }
    Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
    $manifestText = Get-Content -LiteralPath (Join-Path $root 'shell-extension\package\AppxManifest.xml') -Raw
    function New-Package([string]$Name, [string]$Xml) {
        $path = Join-Path $stage $Name
        $archive = [IO.Compression.ZipFile]::Open($path, [IO.Compression.ZipArchiveMode]::Create)
        try {
            $entry = $archive.CreateEntry('AppxManifest.xml')
            $writer = New-Object IO.StreamWriter($entry.Open())
            try { $writer.Write($Xml) } finally { $writer.Dispose() }
        } finally { $archive.Dispose() }
        return $path
    }
    $packagePath = New-Package 'valid.msix' $manifestText
    Import-Module (Join-Path $PSScriptRoot 'modern-menu-package.psm1') -Force
    $info = Get-ModernMenuPackageInfo -PackagePath $packagePath -InstallDirectory $stage
    Assert ($info.Name -eq 'SalvatoreCorsi.TheConverter' -and $info.Assets.Count -eq 3) 'Valid fixture rejected.'
    $badAsset = New-Package 'traversal.msix' ($manifestText.Replace('Assets\StoreLogo.png', 'Assets\..\the-converter.exe'))
    Assert-Fails { Get-ModernMenuPackageInfo -PackagePath $badAsset -InstallDirectory $stage } 'Asset traversal accepted.'
    $badName = New-Package 'wrong-name.msix' ($manifestText.Replace('SalvatoreCorsi.TheConverter', 'Other.App'))
    Assert-Fails { Get-ModernMenuPackageInfo -PackagePath $badName -InstallDirectory $stage } 'Wrong identity accepted.'
    $badClass = New-Package 'wrong-class.msix' ($manifestText.Replace('67B51BA1-8A78-4B4B-A5F9-08A825BE91F8', '00000000-0000-0000-0000-000000000001'))
    Assert-Fails { Get-ModernMenuPackageInfo -PackagePath $badClass -InstallDirectory $stage } 'Wrong COM class accepted.'
    $badDtd = New-Package 'dtd.msix' ($manifestText.Replace('<Package', '<!DOCTYPE Package [<!ENTITY injected "bad">]><Package'))
    Assert-Fails { Get-ModernMenuPackageInfo -PackagePath $badDtd -InstallDirectory $stage } 'DTD accepted.'
    $badLogo = New-Package 'missing-asset.msix' ($manifestText.Replace('Assets\StoreLogo.png', 'Assets\Missing.png'))
    Assert-Fails { Get-ModernMenuPackageInfo -PackagePath $badLogo -InstallDirectory $stage } 'Missing external asset accepted.'
    Assert-Fails { Get-ModernMenuPackageInfo -PackagePath $packagePath -InstallDirectory $stage -RequireTrustedSignature } 'Unsigned fixture accepted as trusted.'

    function global:Get-AuthenticodeSignature {
        param($LiteralPath)
        [pscustomobject]@{ Status = 'Valid'; SignerCertificate = [pscustomobject]@{ Subject = $global:ConverterModernTestSubject } }
    }
    function global:Get-AppxPackage { param($Name) return $global:ConverterModernTestRegistered }
    function global:Add-AppxPackage {
        param($Path, $ExternalLocation, $ErrorAction)
        $global:ConverterModernTestAddCalls++
        if ($global:ConverterModernTestAddMode -eq 'Conflict' -and $global:ConverterModernTestAddCalls -eq 1) { throw 'Deployment conflict 0x80073CFB' }
        if ($global:ConverterModernTestAddMode -eq 'Unrelated') { throw 'Trust failure 0x800B0109' }
    }
    function global:Remove-AppxPackage {
        param($Package, $ErrorAction)
        $global:ConverterModernTestRemoveCalls++
        if ($global:ConverterModernTestRemoveFails) { throw 'Simulated removal failure.' }
        $global:ConverterModernTestRegistered = $null
    }
    foreach ($command in $mockNames) { Assert ((Get-Command $command).CommandType -eq 'Function') "Required mock absent: $command" }
    function Reset-Registration([string]$Path, [version]$Version) {
        $global:ConverterModernTestAddCalls = 0
        $global:ConverterModernTestRemoveCalls = 0
        $global:ConverterModernTestAddMode = 'Success'
        $global:ConverterModernTestRemoveFails = $false
        $global:ConverterModernTestSubject = $info.Publisher
        $global:ConverterModernTestRegistered = [pscustomobject]@{
            Name = $info.Name; Publisher = $info.Publisher; Version = $Version
            PackageFullName = 'test-package'; EffectiveExternalPath = $Path
        }
    }
    $install = Join-Path $PSScriptRoot 'install-context-menu.ps1'
    $uninstall = Join-Path $PSScriptRoot 'uninstall-context-menu.ps1'
    Reset-Registration $stage $info.Version
    $global:ConverterModernTestSubject = 'CN=Wrong Publisher'
    Assert-Fails { Get-ModernMenuPackageInfo -PackagePath $packagePath -InstallDirectory $stage -RequireTrustedSignature } 'Signer/publisher mismatch accepted.'
    Reset-Registration $stage $info.Version
    & $install -Mode Modern -PackagePath $packagePath -InstallDirectory $stage
    Assert ($global:ConverterModernTestAddCalls -eq 0 -and $global:ConverterModernTestRemoveCalls -eq 0) 'Same-version same-location was changed.'
    Reset-Registration (Join-Path $stage 'elsewhere') $info.Version
    & $install -Mode Modern -PackagePath $packagePath -InstallDirectory $stage -WhatIf
    Assert ($global:ConverterModernTestAddCalls -eq 0 -and $global:ConverterModernTestRemoveCalls -eq 0) 'WhatIf mutated registration.'
    Reset-Registration (Join-Path $stage 'elsewhere') $info.Version
    $global:ConverterModernTestAddMode = 'Conflict'
    & $install -Mode Modern -PackagePath $packagePath -InstallDirectory $stage
    Assert ($global:ConverterModernTestAddCalls -eq 2 -and $global:ConverterModernTestRemoveCalls -eq 1) 'Verified exact-version location conflict was not repaired.'
    Reset-Registration (Join-Path $stage 'elsewhere') $info.Version
    $global:ConverterModernTestAddMode = 'Unrelated'
    Assert-Fails { & $install -Mode Modern -PackagePath $packagePath -InstallDirectory $stage } 'Unrelated deployment error accepted.'
    Assert ($global:ConverterModernTestRemoveCalls -eq 0) 'Unrelated error removed a package.'
    Reset-Registration $stage $info.Version
    $global:ConverterModernTestRegistered.Publisher = 'CN=Previous Publisher'
    Assert-Fails { & $install -Mode Modern -PackagePath $packagePath -InstallDirectory $stage } 'Publisher migration silently created duplicate registrations.'
    Assert ($global:ConverterModernTestAddCalls -eq 0 -and $global:ConverterModernTestRemoveCalls -eq 0) 'Publisher conflict mutated registration.'
    & $uninstall -Mode Modern -InstallDirectory $stage
    Assert ($global:ConverterModernTestRemoveCalls -eq 1) 'Previous publisher at the same external location was retained.'
    Reset-Registration (Join-Path $stage 'elsewhere') $info.Version
    & $uninstall -Mode Modern -InstallDirectory $stage
    Assert ($global:ConverterModernTestRemoveCalls -eq 0) 'Another installation was removed.'
    Reset-Registration $stage $info.Version
    & $uninstall -Mode Modern -InstallDirectory $stage -WhatIf
    Assert ($global:ConverterModernTestRemoveCalls -eq 0) 'Uninstall WhatIf mutated registration.'
    & $uninstall -Mode Modern -InstallDirectory $stage
    Assert ($global:ConverterModernTestRemoveCalls -eq 1) 'Matching installation was not removed.'
    Reset-Registration $stage $info.Version
    $global:ConverterModernTestRemoveFails = $true
    Assert-Fails { & $uninstall -Mode Modern -InstallDirectory $stage } 'Removal failure was ignored.'
    Assert (Test-Path -LiteralPath (Join-Path $stage 'the-converter.exe')) 'Removal failure deleted application files.'
    Write-Host "PASS: $checks checks; no real Appx, registry, certificate or Explorer changes."
} finally {
    foreach ($name in $mockNames) {
        if ($previousFunctions.ContainsKey($name)) { Set-Item -LiteralPath ('Function:global:' + $name) -Value $previousFunctions[$name] }
        else { Remove-Item -LiteralPath ('Function:global:' + $name) -ErrorAction SilentlyContinue }
    }
    foreach ($name in $testVariableNames) {
        if ($previousVariables.ContainsKey($name)) { Set-Variable -Name $name -Scope Global -Value $previousVariables[$name] }
        else { Remove-Variable -Name $name -Scope Global -ErrorAction SilentlyContinue }
    }
    $resolvedStage = [IO.Path]::GetFullPath($stage)
    if (-not $resolvedStage.StartsWith($temporaryRoot + '\', [StringComparison]::OrdinalIgnoreCase) -or
        [IO.Path]::GetFileName($resolvedStage) -notmatch '^converter-modern-tests-[0-9a-f]{32}$') { throw 'Percorso fixture non sicuro: pulizia interrotta.' }
    if (Test-Path -LiteralPath $resolvedStage) { Remove-Item -LiteralPath $resolvedStage -Recurse -Force }
}
