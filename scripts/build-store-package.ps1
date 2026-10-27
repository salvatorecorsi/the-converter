# Build an unsigned, self-contained app MSIX for Partner Center or a distinct preview.
# This script never signs, installs, registers packages, imports certificates or downloads engines.
[CmdletBinding()]
param(
    [switch]$ForSubmission,
    [string]$IdentityConfig,
    [string]$PreviewVersion = '1.0.0.0',
    [string]$OutputDirectory,
    [string]$BuildDirectory,
    [string]$CargoPath = 'cargo',
    [string]$MakeAppxPath
)
$ErrorActionPreference = 'Stop'
$env:PSModulePath = (Join-Path $PSHOME 'Modules') + [IO.Path]::PathSeparator + $env:PSModulePath
if (-not [Environment]::Is64BitOperatingSystem) { throw 'Store packaging requires a 64-bit Windows build host.' }
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$definitions = Join-Path $root 'packaging\store'
Import-Module (Join-Path $definitions 'package-tools.psm1') -Force
Import-Module (Join-Path $PSScriptRoot 'application-manifest.psm1') -Force

# Validate account metadata before creating output or invoking build tools.
if ($ForSubmission -and -not $IdentityConfig) { $IdentityConfig = Join-Path $definitions 'identity.partner-center.json' }
if ($ForSubmission -and $PSBoundParameters.ContainsKey('PreviewVersion')) { throw '-PreviewVersion cannot override the Partner Center submission version.' }
$identity = Get-StorePackageIdentity -IdentityConfig $IdentityConfig -ForSubmission:$ForSubmission -PreviewVersion $PreviewVersion
$cargoVersionMatch = [regex]::Match((Get-Content -LiteralPath (Join-Path $root 'Cargo.toml') -Raw -Encoding UTF8), '(?m)^version\s*=\s*"([^"\r\n]+)"')
if (-not $cargoVersionMatch.Success) { throw 'Application version not found in Cargo.toml.' }
$cargoVersion = $cargoVersionMatch.Groups[1].Value
$sourceCommit = (& git -C $root rev-parse HEAD 2>$null | Select-Object -First 1)
$sourceWorkingTreeDirty = @(& git -C $root status --porcelain --untracked-files=normal 2>$null).Count -gt 0
$licenseSource = Join-Path $root 'docs\release-licenses'
foreach ($required in @('RUST-THIRD-PARTY-LICENSES.txt', 'rust-components.json', 'rust-runtime\COPYRIGHT-library.html')) {
    if (-not (Test-Path -LiteralPath (Join-Path $licenseSource $required) -PathType Leaf)) { throw "Missing license inventory: $required. Run release-support/collect-rust-licenses.ps1 first." }
}

if (-not $OutputDirectory) { $OutputDirectory = Join-Path $root ('dist\store\' + $identity.Mode) }
$outputRoot = [IO.Path]::GetFullPath($OutputDirectory)
if (-not $BuildDirectory) { $BuildDirectory = Join-Path $root 'target\store-msix' }
$buildRoot = [IO.Path]::GetFullPath($BuildDirectory)
$runName = $identity.Version + '-' + [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssZ') + '-' + [guid]::NewGuid().ToString('N').Substring(0, 8)
$runDirectory = Join-Path $outputRoot $runName
if (Test-Path -LiteralPath $runDirectory) { throw 'Fresh Store staging path unexpectedly already exists.' }
New-Item -ItemType Directory -Path $runDirectory | Out-Null
$stage = Join-Path $runDirectory 'layout'
$logs = Join-Path $runDirectory 'logs'
New-Item -ItemType Directory -Path $stage, $logs | Out-Null

function ConvertTo-NativeArgument([string]$Argument) {
    # Windows argv quoting, including a path ending in backslashes.
    $quoted = New-Object Text.StringBuilder
    [void]$quoted.Append('"')
    $slashes = 0
    foreach ($character in $Argument.ToCharArray()) {
        if ($character -eq '\') { $slashes++; continue }
        if ($character -eq '"') {
            [void]$quoted.Append([char]'\', 2 * $slashes + 1)
            [void]$quoted.Append('"')
        } else {
            [void]$quoted.Append([char]'\', $slashes)
            [void]$quoted.Append($character)
        }
        $slashes = 0
    }
    [void]$quoted.Append([char]'\', 2 * $slashes)
    [void]$quoted.Append('"')
    $quoted.ToString()
}

function Invoke-StoreNativeTool([string]$Executable, [string[]]$Arguments, [string]$LogPath) {
    $start = New-Object Diagnostics.ProcessStartInfo
    $start.FileName = $Executable
    $start.Arguments = (@($Arguments | ForEach-Object { ConvertTo-NativeArgument $_ }) -join ' ')
    $start.WorkingDirectory = $root
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $process = New-Object Diagnostics.Process
    $process.StartInfo = $start
    try {
        if (-not $process.Start()) { throw "Could not start $Executable" }
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        $process.WaitForExit()
        $stdout = $stdoutTask.GetAwaiter().GetResult()
        $stderr = $stderrTask.GetAwaiter().GetResult()
        [IO.File]::WriteAllText($LogPath, ($stdout + [Environment]::NewLine + $stderr), (New-Object Text.UTF8Encoding($false)))
        if ($process.ExitCode -ne 0) {
            Get-Content -LiteralPath $LogPath -Tail 30 | Write-Host
            throw "Build tool failed: $Executable (exit $($process.ExitCode)); see $LogPath"
        }
        return $stdout
    } finally { $process.Dispose() }
}

if ($CargoPath -eq 'cargo') {
    $cargoCommand = Get-Command cargo.exe -ErrorAction SilentlyContinue
    if ($cargoCommand) { $CargoPath = $cargoCommand.Source }
    elseif (Test-Path -LiteralPath (Join-Path $root '.tools\cargo\bin\cargo.exe') -PathType Leaf) { $CargoPath = Join-Path $root '.tools\cargo\bin\cargo.exe' }
    else { throw 'Cargo not found; provide -CargoPath.' }
}
$cargoExecutable = (Resolve-Path -LiteralPath $CargoPath).Path
if (-not $MakeAppxPath) {
    $sdk = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
    $makeAppx = Get-ChildItem -LiteralPath $sdk -Filter makeappx.exe -Recurse -ErrorAction SilentlyContinue |
        Where-Object { $_.Directory.Name -eq 'x64' } | Sort-Object FullName -Descending | Select-Object -First 1
    if (-not $makeAppx) { throw 'Windows SDK MakeAppx x64 not found; provide -MakeAppxPath.' }
    $MakeAppxPath = $makeAppx.FullName
}
$makeAppxExecutable = (Resolve-Path -LiteralPath $MakeAppxPath).Path

[xml]$peManifest = Get-Content -LiteralPath (Join-Path $definitions 'application.manifest') -Raw -Encoding UTF8
$peManifest.assembly.assemblyIdentity.SetAttribute('version', $identity.Version)
$generatedPeManifest = Join-Path $runDirectory 'application.manifest'
$peManifest.Save($generatedPeManifest)
$savedEnvironment = @{}
foreach ($name in @('THE_CONVERTER_APP_MANIFEST', 'CARGO_TARGET_DIR', 'CARGO_HOME', 'RUSTUP_HOME', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'PATH')) { $savedEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
$target = 'x86_64-pc-windows-msvc'
try {
    $env:THE_CONVERTER_APP_MANIFEST = $generatedPeManifest
    $env:CARGO_TARGET_DIR = $buildRoot
    $env:PATH = (Split-Path -Parent $cargoExecutable) + [IO.Path]::PathSeparator + $env:PATH
    if ($cargoExecutable.StartsWith((Join-Path $root '.tools\cargo\'), [StringComparison]::OrdinalIgnoreCase)) {
        $env:CARGO_HOME = Join-Path $root '.tools\cargo'
        $env:RUSTUP_HOME = Join-Path $root '.tools\rustup'
    }
    if ($env:CARGO_ENCODED_RUSTFLAGS) { $env:CARGO_ENCODED_RUSTFLAGS += [char]31 + '-C' + [char]31 + 'target-feature=+crt-static' }
    else { $env:RUSTFLAGS = ($env:RUSTFLAGS + ' -C target-feature=+crt-static').Trim() }

    $components = @(Get-Content -LiteralPath (Join-Path $licenseSource 'rust-components.json') -Raw -Encoding UTF8 | ConvertFrom-Json)
    $noticeKeys = @{}
    foreach ($component in $components) {
        if ($component.notice_count -lt 1) { throw 'Incomplete Rust license notice inventory.' }
        $noticeKeys[($component.name + '@' + $component.version)] = $true
    }
    foreach ($manifest in @((Join-Path $root 'Cargo.toml'), (Join-Path $root 'shell-extension\Cargo.toml'))) {
        $logName = if ($manifest -like '*shell-extension*') { 'metadata-shell.log' } else { 'metadata-app.log' }
        $rawMetadata = Invoke-StoreNativeTool $cargoExecutable @('metadata', '--format-version', '1', '--locked', '--filter-platform', $target, '--manifest-path', $manifest) (Join-Path $logs $logName)
        $metadata = $rawMetadata | ConvertFrom-Json
        foreach ($dependency in @($metadata.packages | Where-Object { $_.source -like 'registry+*' })) {
            if (-not $noticeKeys.ContainsKey($dependency.name + '@' + $dependency.version)) { throw "Missing dependency notice for $($dependency.name) $($dependency.version); regenerate release licenses." }
        }
    }
    Write-Host 'Building Store desktop app (isolated target directory, static CRT)...'
    $null = Invoke-StoreNativeTool $cargoExecutable @('build', '--release', '--locked', '--bin', 'the-converter', '--target', $target, '--manifest-path', (Join-Path $root 'Cargo.toml')) (Join-Path $logs 'cargo-app.log')
    Write-Host 'Building packaged Explorer COM extension...'
    $null = Invoke-StoreNativeTool $cargoExecutable @('build', '--release', '--locked', '--target', $target, '--manifest-path', (Join-Path $root 'shell-extension\Cargo.toml')) (Join-Path $logs 'cargo-shell.log')
} finally {
    foreach ($name in $savedEnvironment.Keys) { [Environment]::SetEnvironmentVariable($name, $savedEnvironment[$name], 'Process') }
}

$imports = [ordered]@{}
foreach ($binary in @('the-converter.exe', 'the_converter_shell.dll')) {
    $source = Join-Path $buildRoot "$target\release\$binary"
    $imports[$binary] = (Get-StorePeImportInfo -Path $source).Imports
    if ($binary -ne 'the_converter_shell.dll') {
        $embedded = Get-ExecutableApplicationManifest -ExecutablePath $source
        if (@($embedded.SelectNodes('//*[local-name()="msix" and namespace-uri()="urn:schemas-microsoft-com:msix.v1"]')).Count -ne 0 -or
            $embedded.assembly.assemblyIdentity.GetAttribute('version') -cne $identity.Version) { throw "Stale sparse or incorrect application manifest in $binary" }
        $privilege = $embedded.SelectSingleNode('//*[local-name()="requestedExecutionLevel"]')
        if (-not $privilege -or $privilege.GetAttribute('level') -cne 'asInvoker') { throw "Store executable must run asInvoker: $binary" }
    }
    Copy-Item -LiteralPath $source -Destination $stage
}
New-StorePackageManifest -TemplatePath (Join-Path $definitions 'AppxManifest.xml') -Identity $identity -OutputPath (Join-Path $stage 'AppxManifest.xml')
New-StorePackageAssets -SourceImage (Join-Path $root 'assets\app.png') -OutputDirectory (Join-Path $stage 'Assets')
[IO.File]::WriteAllText((Join-Path $stage 'store-runtime.json'), (@{ execution_alias = $identity.ExecutionAlias } | ConvertTo-Json), (New-Object Text.UTF8Encoding($false)))
Copy-Item -LiteralPath (Join-Path $definitions 'README.package.txt') -Destination (Join-Path $stage 'README.txt')
Copy-Item -LiteralPath (Join-Path $root 'THIRD-PARTY-NOTICES.md') -Destination $stage
New-Item -ItemType Directory -Path (Join-Path $stage 'Docs') | Out-Null
Copy-Item -LiteralPath $licenseSource -Destination (Join-Path $stage 'Docs\licenses') -Recurse
Assert-StorePackageLayout -StageDirectory $stage -Identity $identity

$fileName = if ($ForSubmission) { "TheConverter-$($identity.Version)-x64-store.msix" } else { "TheConverter-$($identity.Version)-x64-store-PREVIEW.msix" }
$package = Join-Path $runDirectory $fileName
Write-Host 'Packing full MSIX with MakeAppx semantic validation enabled...'
# /nv is deliberately absent: every manifest path must resolve inside this package.
$null = Invoke-StoreNativeTool $makeAppxExecutable @('pack', '/d', $stage, '/p', $package, '/o') (Join-Path $logs 'makeappx.log')
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [IO.Compression.ZipFile]::OpenRead($package)
try {
    if (@($zip.Entries | Where-Object { $_.FullName -ieq 'AppxSignature.p7x' }).Count -ne 0) { throw 'Store upload package must remain unsigned; unexpected package signature.' }
    foreach ($binary in @('the-converter.exe', 'the_converter_shell.dll', 'store-runtime.json')) {
        if (-not $zip.GetEntry($binary)) { throw "Full MSIX is missing packaged content: $binary" }
    }
} finally { $zip.Dispose() }
$payload = @(Get-ChildItem -LiteralPath $stage -File -Recurse | Sort-Object FullName | ForEach-Object {
    [ordered]@{ Path = $_.FullName.Substring($stage.Length + 1).Replace('\', '/'); Bytes = $_.Length; SHA256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
})
$packageHash = (Get-FileHash -LiteralPath $package -Algorithm SHA256).Hash.ToLowerInvariant()
$report = [ordered]@{
    SchemaVersion = 1; Mode = $identity.Mode; BuiltAtUtc = [DateTime]::UtcNow.ToString('o'); SourceCommit = $sourceCommit; SourceDirty = $sourceWorkingTreeDirty
    ApplicationVersion = $cargoVersion; StoreVersion = $identity.Version; Architecture = 'x64'; MinimumOSVersion = '10.0.22000.0'; Identity = $identity
    Package = $fileName; PackageSHA256 = $packageHash; Signed = $false; FullPackage = $true; ExternalContent = $false
    ApplicationCount = 1; ConsoleCliBundled = $false; EnginesBundled = $false; RuntimeDownloaderBundled = $false; CertificateStoreChanged = $false; PackageInstalled = $false
    MakeAppxValidation = 'Passed (no /nv)'; WindowsAppCertificationKit = 'Not run'; PartnerCenterUpload = 'Not performed'
    RunFullTrustJustification = 'Local Win32 file conversions, user-selected installed conversion tools and File Explorer integration. No elevation requested.'
    DependencyNoticeRecords = $components.Count; ImportedLibraries = $imports; Payload = $payload
}
$reportPath = Join-Path $runDirectory 'submission-report.json'
[IO.File]::WriteAllText($reportPath, ($report | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))
$submissionText = @(
    '# The Converter - Microsoft Store package', '',
    ('Mode: ' + $identity.Mode), ('Package: ' + $fileName), ('SHA256: ' + $packageHash), '',
    ('Identity Name: ' + $identity.Name), ('Publisher: ' + $identity.Publisher), ('Display name: ' + $identity.DisplayName),
    ('Publisher display name: ' + $identity.PublisherDisplayName), ('Store version: ' + $identity.Version), ('Cargo version: ' + $cargoVersion),
    ('GUI execution alias: ' + $identity.ExecutionAlias), '',
    'Full Windows 11 x64 MSIX (minimum build 22000), packagedClassicApp/mediumIL, visible app-list entry and packaged Explorer COM command.',
    'MakeAppx semantic validation passed without /nv. All application binaries, runtime metadata, logos and dependency notices are inside the package.',
    'Exactly one GUI application is declared. Its executable and the Explorer DLL are the only binary payloads; the console CLI belongs to the separate GitHub distribution.',
    'No external conversion engines or PowerShell downloader are bundled.',
    'The package is unsigned. Microsoft Store signs accepted submissions; this build did not use a signing certificate or modify trust.', '',
    'Remaining review: Windows App Certification Kit, clean-machine app/menu/queue tests, Store listing and privacy information, runFullTrust justification and Partner Center certification.',
    'No Partner Center upload or package installation was performed.'
)
if (-not $ForSubmission) { $submissionText += @('', 'DO NOT UPLOAD THIS PREVIEW: it uses a deliberately separate preview identity. Reserve the Corsivo app in Partner Center and use its exact identity config with -ForSubmission.') }
$submissionReport = Join-Path $runDirectory 'SUBMISSION-REPORT.md'
[IO.File]::WriteAllLines($submissionReport, $submissionText, (New-Object Text.UTF8Encoding($false)))
$checksums = foreach ($path in @($package, $reportPath, $submissionReport)) { (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() + '  ' + [IO.Path]::GetFileName($path) }
[IO.File]::WriteAllLines((Join-Path $runDirectory 'SHA256SUMS.txt'), $checksums, (New-Object Text.UTF8Encoding($false)))
[pscustomobject]@{ OutputDirectory = $runDirectory; Package = $package; Report = $reportPath; SubmissionReport = $submissionReport; Mode = $identity.Mode; StoreVersion = $identity.Version }
