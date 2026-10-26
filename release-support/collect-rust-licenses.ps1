# Collect the exact Windows dependency notices from the locked Cargo graph.
[CmdletBinding()]
param([string]$CargoPath = 'cargo')
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$output = Join-Path $root 'docs\release-licenses'
New-Item -ItemType Directory -Force -Path $output | Out-Null
if ($CargoPath -eq 'cargo' -and -not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $CargoPath = Join-Path $root '.tools\cargo\bin\cargo.exe'
    $env:CARGO_HOME = Join-Path $root '.tools\cargo'
    $env:RUSTUP_HOME = Join-Path $root '.tools\rustup'
}
$packages = @{}
foreach ($manifest in @((Join-Path $root 'Cargo.toml'), (Join-Path $root 'shell-extension\Cargo.toml'))) {
    $raw = & $CargoPath metadata --format-version 1 --locked --filter-platform x86_64-pc-windows-msvc --manifest-path $manifest
    if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed.' }
    $metadata = ($raw -join "`n") | ConvertFrom-Json
    foreach ($package in $metadata.packages) {
        if ($package.source -like 'registry+*') { $packages[($package.name + '@' + $package.version)] = $package }
    }
}
$upstream = @{
    'clipboard-win' = @{ Repository = 'DoumanAsh/clipboard-win'; Files = @('LICENSE') }
    'ecolor' = @{ Repository = 'emilk/egui'; Files = @('LICENSE-MIT', 'LICENSE-APACHE') }
    'eframe' = @{ Repository = 'emilk/egui'; Files = @('LICENSE-MIT', 'LICENSE-APACHE') }
    'egui' = @{ Repository = 'emilk/egui'; Files = @('LICENSE-MIT', 'LICENSE-APACHE') }
    'egui_glow' = @{ Repository = 'emilk/egui'; Files = @('LICENSE-MIT', 'LICENSE-APACHE') }
    'egui-winit' = @{ Repository = 'emilk/egui'; Files = @('LICENSE-MIT', 'LICENSE-APACHE') }
    'emath' = @{ Repository = 'emilk/egui'; Files = @('LICENSE-MIT', 'LICENSE-APACHE') }
    'epaint' = @{ Repository = 'emilk/egui'; Files = @('LICENSE-MIT', 'LICENSE-APACHE') }
    'epaint_default_fonts' = @{ Repository = 'emilk/egui'; Files = @('LICENSE-MIT', 'LICENSE-APACHE') }
    'gl_generator' = @{ Repository = 'brendanzab/gl-rs'; Files = @('LICENSE') }
    'khronos_api' = @{ Repository = 'brendanzab/gl-rs'; Files = @('LICENSE') }
    'profiling' = @{ Repository = 'aclysma/profiling'; Files = @('LICENSE-MIT', 'LICENSE-APACHE') }
}
$text = [Text.StringBuilder]::new()
[void]$text.AppendLine('The Converter - third-party Rust dependency notices')
[void]$text.AppendLine('Generated from Cargo.lock and shell-extension/Cargo.lock for x86_64-pc-windows-msvc.')
[void]$text.AppendLine('Includes build-time dependencies as a conservative superset. The application has its own rights; these texts concern the identified third-party components only.')
$index = @()
foreach ($package in @($packages.Values | Sort-Object name,version)) {
    $source = Split-Path -Parent $package.manifest_path
    $files = @(Get-ChildItem -LiteralPath $source -Recurse -File | Where-Object {
        $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE|COPYRIGHT)([._-]|$)' -or
        $_.Name -match '^(OFL|UFL)\.txt$' -or $_.Name -eq 'Hack-Regular.txt' -or $_.Name -eq 'emoji-icon-font-mit-license.txt'
    } | Sort-Object FullName)
    $notices = @()
    foreach ($file in $files) {
        $relative = $file.FullName.Substring($source.Length + 1).Replace('\','/')
        $notices += @{ Name = $relative; Text = [IO.File]::ReadAllText($file.FullName); Origin = ('crate:' + $relative) }
    }
    if ($upstream.ContainsKey($package.name)) {
        $definition = $upstream[$package.name]
        $vcs = Get-Content -LiteralPath (Join-Path $source '.cargo_vcs_info.json') -Raw | ConvertFrom-Json
        foreach ($name in $definition.Files) {
            $url = 'https://raw.githubusercontent.com/' + $definition.Repository + '/' + $vcs.git.sha1 + '/' + $name
            $response = Invoke-WebRequest -Uri $url
            $notices += @{ Name = $name; Text = $response.Content; Origin = $url }
        }
    }
    if ($notices.Count -eq 0) { throw "No license text collected for $($package.name) $($package.version)" }
    [void]$text.AppendLine("`n================================================================================")
    [void]$text.AppendLine($package.name + ' ' + $package.version)
    [void]$text.AppendLine('Declared license: ' + $package.license)
    [void]$text.AppendLine('Authors: ' + ($package.authors -join '; '))
    [void]$text.AppendLine('Repository: ' + $package.repository)
    [void]$text.AppendLine('Exact package source: https://crates.io/api/v1/crates/' + $package.name + '/' + $package.version + '/download')
    foreach ($notice in $notices) {
        [void]$text.AppendLine("`n--- " + $notice.Name + ' ---')
        [void]$text.AppendLine('Origin: ' + $notice.Origin)
        [void]$text.AppendLine($notice.Text)
    }
    $index += [ordered]@{ name=$package.name; version=$package.version; license=$package.license; repository=$package.repository; source=('https://crates.io/api/v1/crates/'+$package.name+'/'+$package.version+'/download'); notice_count=$notices.Count }
}
[IO.File]::WriteAllText((Join-Path $output 'RUST-THIRD-PARTY-LICENSES.txt'), $text.ToString(), [Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText((Join-Path $output 'rust-components.json'), ($index | ConvertTo-Json -Depth 8), [Text.UTF8Encoding]::new($false))

# This unmodified MPL-2.0 dependency is distributed in source form alongside its notice.
$mpl = @($packages.Values | Where-Object name -eq 'option-ext')
foreach ($package in $mpl) {
    $destination = Join-Path $output ('option-ext-' + $package.version)
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    $source = Split-Path -Parent $package.manifest_path
    foreach ($file in @(Get-ChildItem -LiteralPath $source -Recurse -File | Where-Object Name -ne '.cargo-ok')) {
        $relative = $file.FullName.Substring($source.Length+1)
        $target = Join-Path $destination $relative
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $target) | Out-Null
        Copy-Item -LiteralPath $file.FullName -Destination $target -Force
    }
}
# Rust's statically linked standard library also ships its own copyright inventory.
$cargoCommand = Get-Command $CargoPath -ErrorAction Stop
$rustc = Join-Path (Split-Path -Parent $cargoCommand.Source) 'rustc.exe'
$sysroot = (& $rustc --print sysroot | Select-Object -First 1)
if ($LASTEXITCODE -ne 0) { throw 'Unable to locate Rust runtime notices.' }
$runtimeSource = Join-Path $sysroot 'share\doc\rust'
$runtimeDestination = Join-Path $output 'rust-runtime'
New-Item -ItemType Directory -Force -Path $runtimeDestination | Out-Null
Copy-Item -LiteralPath (Join-Path $runtimeSource 'COPYRIGHT-library.html') -Destination $runtimeDestination -Force
$runtimeLicenses = Join-Path $runtimeDestination 'licenses'
New-Item -ItemType Directory -Force -Path $runtimeLicenses | Out-Null
Copy-Item -Path (Join-Path $runtimeSource 'licenses\*') -Destination $runtimeLicenses -Force
Set-Content -LiteralPath (Join-Path $runtimeDestination 'compiler-version.txt') -Value (& $rustc --version) -Encoding utf8
Write-Host "Collected notices for $($packages.Count) locked Windows crates in $output"
