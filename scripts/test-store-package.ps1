# Fixture tests only. Does not build Rust, execute fixture binaries, install packages or touch certificate stores.
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$definitions = Join-Path $root 'packaging\store'
Import-Module (Join-Path $definitions 'package-tools.psm1') -Force
$temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')
$fixture = Join-Path $temporaryRoot ('converter-store-tests-' + [guid]::NewGuid().ToString('N'))
$checks = 0
function Assert([bool]$Condition, [string]$Message) { if (-not $Condition) { throw $Message }; $script:checks++ }
function Assert-Fails([scriptblock]$Code, [string]$Message) { $failed = $false; try { & $Code | Out-Null } catch { $failed = $true }; Assert $failed $Message }
function Write-Json([string]$Path, $Value) { [IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 8), (New-Object Text.UTF8Encoding($false))) }
function New-PeFixture([string]$Path, [string]$Import = 'kernel32.dll', [uint16]$Machine = 0x8664) {
    # Data-only minimal PE32+ headers and one import descriptor; never executable code.
    $bytes = New-Object byte[] 1536
    [Array]::Copy([BitConverter]::GetBytes([uint16]0x5a4d), 0, $bytes, 0, 2)
    [Array]::Copy([BitConverter]::GetBytes([uint32]0x80), 0, $bytes, 0x3c, 4)
    [Array]::Copy([BitConverter]::GetBytes([uint32]0x4550), 0, $bytes, 0x80, 4)
    [Array]::Copy([BitConverter]::GetBytes($Machine), 0, $bytes, 0x84, 2)
    [Array]::Copy([BitConverter]::GetBytes([uint16]1), 0, $bytes, 0x86, 2)
    [Array]::Copy([BitConverter]::GetBytes([uint16]240), 0, $bytes, 0x94, 2)
    [Array]::Copy([BitConverter]::GetBytes([uint16]0x20b), 0, $bytes, 0x98, 2)
    [Array]::Copy([BitConverter]::GetBytes([uint32]16), 0, $bytes, (0x98 + 108), 4)
    [Array]::Copy([BitConverter]::GetBytes([uint32]0x1000), 0, $bytes, (0x98 + 120), 4)
    $section = 0x98 + 240
    foreach ($value in @(@(8, 512), @(12, 0x1000), @(16, 512), @(20, 0x400))) { [Array]::Copy([BitConverter]::GetBytes([uint32]$value[1]), 0, $bytes, ($section + $value[0]), 4) }
    [Array]::Copy([BitConverter]::GetBytes([uint32]0x1040), 0, $bytes, (0x400 + 12), 4)
    [Array]::Copy([BitConverter]::GetBytes([uint32]0x1080), 0, $bytes, (0x400 + 16), 4)
    $name = [Text.Encoding]::ASCII.GetBytes($Import)
    [Array]::Copy($name, 0, $bytes, 0x440, $name.Length)
    [IO.File]::WriteAllBytes($Path, $bytes)
}
try {
    New-Item -ItemType Directory -Path $fixture | Out-Null
    $preview = Get-StorePackageIdentity
    Assert ($preview.Name -ceq 'Corsivo.TheConverter.StorePreview' -and $preview.DisplayName -ceq 'The Converter Store Preview' -and $preview.PublisherDisplayName -ceq 'Corsivo') 'Preview identity is not clearly separate.'
    Assert ($preview.Version -eq '1.0.0.0' -and $preview.ExecutionAlias -eq 'corsivo-the-converter-preview.exe') 'Incorrect preview version or alias.'
    Assert ($preview.CliExecutionAlias -eq 'corsivo-the-converter-preview-cli.exe') 'Incorrect preview CLI alias.'
    Assert-Fails { Get-StorePackageIdentity -ForSubmission -IdentityConfig (Join-Path $definitions 'identity.partner-center.example.json') } 'Submission accepted placeholder identity.'
    Assert-Fails { Get-StorePackageIdentity -ForSubmission -IdentityConfig (Join-Path $fixture 'missing.json') } 'Submission accepted absent account identity.'
    Assert-Fails { Get-StorePackageIdentity -IdentityConfig (Join-Path $definitions 'identity.partner-center.example.json') } 'Preview silently used submission identity.'
    foreach ($version in @('0.1.0.0', '1.0.0.1', '1.0.65536.0', '1.2.3', 'text')) {
        Assert-Fails { Get-StorePackageIdentity -PreviewVersion $version } "Invalid Store version accepted: $version"
    }
    Assert ((Get-StorePackageIdentity -PreviewVersion '2.4.1.0').Version -eq '2.4.1.0') 'Explicit preview version ignored.'
    $configPath = Join-Path $fixture 'fixture-identity.json'
    $config = [ordered]@{ Name = '12345Corsivo.Fixture'; Publisher = 'CN=00000000-0000-0000-0000-000000000001'; DisplayName = 'Fixture Converter'; PublisherDisplayName = 'Corsivo'; Version = '1.0.0.0' }
    Write-Json $configPath $config
    $submission = Get-StorePackageIdentity -ForSubmission -IdentityConfig $configPath
    Assert ($submission.Name -eq $config.Name -and $submission.ExecutionAlias -eq 'corsivo-the-converter.exe' -and $submission.Mode -eq 'submission') 'Explicit submission identity was not preserved.'
    Assert ($submission.CliExecutionAlias -eq 'corsivo-the-converter-cli.exe') 'Incorrect production CLI alias.'
    $neverOutput = Join-Path $fixture 'must-not-be-created'
    Assert-Fails { & (Join-Path $PSScriptRoot 'build-store-package.ps1') -ForSubmission -IdentityConfig (Join-Path $definitions 'identity.partner-center.example.json') -OutputDirectory $neverOutput } 'Builder accepted placeholder submission config.'
    Assert (-not (Test-Path -LiteralPath $neverOutput)) 'Placeholder submission created output before failing.'

    $layout = Join-Path $fixture 'layout'
    New-Item -ItemType Directory -Path (Join-Path $layout 'Docs\licenses') -Force | Out-Null
    foreach ($binary in @('the-converter.exe', 'the-converter-cli.exe', 'the_converter_shell.dll')) { New-PeFixture (Join-Path $layout $binary) }
    foreach ($name in @('README.txt', 'THIRD-PARTY-NOTICES.md', 'Docs\licenses\RUST-THIRD-PARTY-LICENSES.txt')) { [IO.File]::WriteAllText((Join-Path $layout $name), 'Fixture text') }
    Write-Json (Join-Path $layout 'Docs\licenses\rust-components.json') @(@{ name = 'fixture'; version = '1'; notice_count = 1 })
    Write-Json (Join-Path $layout 'store-runtime.json') @{ execution_alias = $preview.ExecutionAlias }
    New-StorePackageAssets -SourceImage (Join-Path $root 'assets\app.png') -OutputDirectory (Join-Path $layout 'Assets')
    $scaledLogo = [Drawing.Image]::FromFile((Join-Path $layout 'Assets\StoreLogo.scale-125.png'))
    try { Assert ($scaledLogo.Width -eq 63 -and $scaledLogo.Height -eq 63) 'StoreLogo scale-125 must round up to 63x63.' } finally { $scaledLogo.Dispose() }
    $largeTile = [Drawing.Image]::FromFile((Join-Path $layout 'Assets\Square150x150Logo.scale-400.png'))
    try { Assert ($largeTile.Width -eq 600 -and $largeTile.Height -eq 600) 'Store large tile variant must be 600x600.' } finally { $largeTile.Dispose() }
    function Reset-Manifest { New-StorePackageManifest -TemplatePath (Join-Path $definitions 'AppxManifest.xml') -Identity $preview -OutputPath (Join-Path $layout 'AppxManifest.xml') }
    function Change-Manifest([scriptblock]$Change) { [xml]$document = Get-Content -LiteralPath (Join-Path $layout 'AppxManifest.xml') -Raw -Encoding UTF8; & $Change $document; $document.Save((Join-Path $layout 'AppxManifest.xml')) }
    Reset-Manifest
    Assert-StorePackageLayout -StageDirectory $layout -Identity $preview
    Assert $true 'Valid full package layout rejected.'
    $variant = Join-Path $layout 'Assets\Square44x44Logo.scale-200.png'
    [IO.File]::WriteAllText($variant, 'Invalid PNG fixture')
    Assert-Fails { Assert-StorePackageLayout -StageDirectory $layout -Identity $preview } 'Invalid scale variant accepted.'
    New-StorePackageAssets -SourceImage (Join-Path $root 'assets\app.png') -OutputDirectory (Join-Path $layout 'Assets')
    $pe = Get-StorePeImportInfo -Path (Join-Path $layout 'the-converter.exe')
    Assert ($pe.Architecture -eq 'x64' -and $pe.Imports -contains 'kernel32.dll') 'PE architecture/import inspection failed.'
    New-PeFixture (Join-Path $fixture 'dynamic-crt.dll') 'VCRUNTIME140.dll'
    Assert-Fails { Get-StorePeImportInfo -Path (Join-Path $fixture 'dynamic-crt.dll') } 'Undeclared VC runtime import accepted.'
    New-PeFixture (Join-Path $fixture 'wrong-machine.exe') 'kernel32.dll' 0x14c
    Assert-Fails { Get-StorePeImportInfo -Path (Join-Path $fixture 'wrong-machine.exe') } 'Non-x64 PE accepted.'
    Change-Manifest { param($doc) $element = $doc.CreateElement('uap10', 'AllowExternalContent', 'http://schemas.microsoft.com/appx/manifest/uap/windows10/10'); $element.InnerText = 'true'; [void]$doc.Package.Properties.AppendChild($element) }
    Assert-Fails { Assert-StorePackageLayout -StageDirectory $layout -Identity $preview } 'Sparse external content accepted.'
    Reset-Manifest
    Change-Manifest { param($doc) $doc.SelectSingleNode('//*[local-name()="VisualElements"]').SetAttribute('AppListEntry', 'none') }
    Assert-Fails { Assert-StorePackageLayout -StageDirectory $layout -Identity $preview } 'Hidden app-list entry accepted.'
    Reset-Manifest
    Change-Manifest { param($doc) [void]$doc.SelectSingleNode('//*[local-name()="Application" and @Id="TheConverter"]').SetAttribute('RuntimeBehavior', 'http://schemas.microsoft.com/appx/manifest/uap/windows10/10', 'win32App') }
    Assert-Fails { Assert-StorePackageLayout -StageDirectory $layout -Identity $preview } 'Wrong app runtime accepted.'
    Reset-Manifest
    Change-Manifest { param($doc) $doc.Package.Identity.SetAttribute('Version', '2.0.0.0') }
    Assert-Fails { Assert-StorePackageLayout -StageDirectory $layout -Identity $preview } 'Mismatched package version accepted.'
    Reset-Manifest
    Change-Manifest { param($doc) $doc.Package.Dependencies.TargetDeviceFamily.SetAttribute('MinVersion', '10.0.19041.0') }
    Assert-Fails { Assert-StorePackageLayout -StageDirectory $layout -Identity $preview } 'Store manifest silently targeted Windows 10.'
    Reset-Manifest
    Change-Manifest { param($doc) $resource = $doc.CreateElement('Resource', 'http://schemas.microsoft.com/appx/manifest/foundation/windows10'); $resource.SetAttribute('Language', 'en-us'); [void]$doc.Package.Resources.AppendChild($resource) }
    Assert-Fails { Assert-StorePackageLayout -StageDirectory $layout -Identity $preview } 'Unimplemented UI language advertised.'
    Reset-Manifest
    Change-Manifest { param($doc) $alias = $doc.SelectSingleNode('//*[local-name()="Extension" and @Executable="the-converter-cli.exe"]'); [void]$alias.ParentNode.RemoveChild($alias) }
    Assert-Fails { Assert-StorePackageLayout -StageDirectory $layout -Identity $preview } 'Missing console alias accepted.'
    Reset-Manifest
    Change-Manifest { param($doc) $doc.SelectSingleNode('//*[local-name()="Extension" and @Executable="the-converter-cli.exe"]').SetAttribute('Executable', 'the-converter.exe') }
    Assert-Fails { Assert-StorePackageLayout -StageDirectory $layout -Identity $preview } 'CLI alias targeting GUI executable accepted.'
    Reset-Manifest
    Change-Manifest { param($doc) $container = $doc.SelectSingleNode('//*[local-name()="Extension" and @Executable="the-converter-cli.exe"]/*[local-name()="AppExecutionAlias"]'); [void]$container.SetAttribute('Subsystem', 'http://schemas.microsoft.com/appx/manifest/uap/windows10/10', 'windows') }
    Assert-Fails { Assert-StorePackageLayout -StageDirectory $layout -Identity $preview } 'CLI alias with windows subsystem accepted.'
    Reset-Manifest
    Change-Manifest { param($doc) $doc.SelectSingleNode('//*[local-name()="Application" and @Id="TheConverterCli"]/*[local-name()="VisualElements"]').SetAttribute('AppListEntry', 'default') }
    Assert-Fails { Assert-StorePackageLayout -StageDirectory $layout -Identity $preview } 'CLI exposed a duplicate Start menu entry.'
    Reset-Manifest
    Change-Manifest { param($doc) [void]$doc.SelectSingleNode('//*[local-name()="Application" and @Id="TheConverterCli"]').SetAttribute('SupportsMultipleInstances', 'http://schemas.microsoft.com/appx/manifest/uap/windows10/10', 'false') }
    Assert-Fails { Assert-StorePackageLayout -StageDirectory $layout -Identity $preview } 'Console activation without multiple-instance support accepted.'
    Reset-Manifest
    Change-Manifest { param($doc) [void]$doc.SelectSingleNode('//*[local-name()="Application" and @Id="TheConverter"]').SetAttribute('SupportsMultipleInstances', 'http://schemas.microsoft.com/appx/manifest/uap/windows10/10', 'false') }
    Assert-Fails { Assert-StorePackageLayout -StageDirectory $layout -Identity $preview } 'GUI alias could redirect worker/quick activation to an existing GUI instance.'
    Reset-Manifest
    Change-Manifest { param($doc) $doc.SelectSingleNode('//*[local-name()="Class"]').SetAttribute('Path', '..\external.dll') }
    Assert-Fails { Assert-StorePackageLayout -StageDirectory $layout -Identity $preview } 'External COM server path accepted.'
    Reset-Manifest
    Change-Manifest { param($doc) $element = $doc.CreateElement('rescap', 'Capability', 'http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities'); $element.SetAttribute('Name', 'unvirtualizedResources'); [void]$doc.Package.Capabilities.AppendChild($element) }
    Assert-Fails { Assert-StorePackageLayout -StageDirectory $layout -Identity $preview } 'Unexpected restricted capability accepted.'
    Reset-Manifest
    Write-Json (Join-Path $layout 'store-runtime.json') @{ execution_alias = 'stale.exe' }
    Assert-Fails { Assert-StorePackageLayout -StageDirectory $layout -Identity $preview } 'Stale runtime alias accepted.'
    Write-Json (Join-Path $layout 'store-runtime.json') @{ execution_alias = $preview.ExecutionAlias; extra = $true }
    Assert-Fails { Assert-StorePackageLayout -StageDirectory $layout -Identity $preview } 'Undeclared runtime metadata accepted.'
    Write-Json (Join-Path $layout 'store-runtime.json') @{ execution_alias = $preview.ExecutionAlias }
    foreach ($name in @('ffmpeg.exe', 'install-runtime-tools.ps1', 'certificate.cer')) {
        $unexpected = Join-Path $layout $name
        [IO.File]::WriteAllText($unexpected, 'Fixture')
        Assert-Fails { Assert-StorePackageLayout -StageDirectory $layout -Identity $preview } "Unexpected executable/script/certificate accepted: $name"
        Remove-Item -LiteralPath $unexpected
    }
    Assert-StorePackageLayout -StageDirectory $layout -Identity $preview
    $sdkRoot = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
    $makeAppx = Get-ChildItem -LiteralPath $sdkRoot -Filter makeappx.exe -Recurse -ErrorAction SilentlyContinue |
        Where-Object { $_.Directory.Name -eq 'x64' } | Sort-Object FullName -Descending | Select-Object -First 1
    if ($makeAppx) {
        $packagePath = Join-Path $fixture 'fixture-full-package.msix'
        $sdkOutput = & $makeAppx.FullName pack /d $layout /p $packagePath /o 2>&1
        if ($LASTEXITCODE -ne 0) { $sdkOutput | Write-Host; throw 'MakeAppx semantic validation rejected full package fixture.' }
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        $archive = [IO.Compression.ZipFile]::OpenRead($packagePath)
        try {
            Assert ($null -ne $archive.GetEntry('the-converter.exe') -and $null -ne $archive.GetEntry('the_converter_shell.dll')) 'Full MSIX did not contain its binaries.'
            Assert ($null -eq $archive.GetEntry('AppxSignature.p7x')) 'Fixture package unexpectedly signed.'
        } finally { $archive.Dispose() }
        Write-Host 'PASS: MakeAppx semantic validation, no /nv, using data-only fixtures.'
    } else { Write-Warning 'Windows SDK MakeAppx unavailable: semantic SDK fixture test skipped.' }
    Write-Host "PASS: $checks Store packaging checks; no Rust build, executable fixture invocation, package registration or certificate changes."
} finally {
    $resolvedFixture = [IO.Path]::GetFullPath($fixture)
    if (-not $resolvedFixture.StartsWith($temporaryRoot + '\', [StringComparison]::OrdinalIgnoreCase) -or
        [IO.Path]::GetFileName($resolvedFixture) -notmatch '^converter-store-tests-[0-9a-f]{32}$') { throw 'Unsafe Store fixture cleanup path.' }
    if (Test-Path -LiteralPath $resolvedFixture) { Remove-Item -LiteralPath $resolvedFixture -Recurse -Force }
}
