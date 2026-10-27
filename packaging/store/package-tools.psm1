Set-StrictMode -Version 2

function Get-StorePackageIdentity {
    [CmdletBinding()]
    param([string]$IdentityConfig, [switch]$ForSubmission, [string]$PreviewVersion = '1.0.0.0')
    if ($ForSubmission) {
        if (-not $IdentityConfig -or -not (Test-Path -LiteralPath $IdentityConfig -PathType Leaf)) {
            throw 'Submission requires -IdentityConfig with exact identity values from the Corsivo Partner Center account.'
        }
        $config = Get-Content -LiteralPath $IdentityConfig -Raw -Encoding UTF8 | ConvertFrom-Json -ErrorAction Stop
        foreach ($field in @('Name', 'Publisher', 'DisplayName', 'PublisherDisplayName', 'Version')) {
            if (-not $config.PSObject.Properties[$field] -or $config.$field -isnot [string] -or [string]::IsNullOrWhiteSpace($config.$field)) { throw "Missing Store identity field: $field" }
            if ($config.$field -match '(?i)(REPLACE_WITH|PLACEHOLDER|CHANGE_ME|TODO|YOUR_|StorePreview|[<>])') { throw "Store identity still contains a placeholder: $field" }
        }
        $identity = [pscustomobject]@{ Name = $config.Name; Publisher = $config.Publisher; DisplayName = $config.DisplayName; PublisherDisplayName = $config.PublisherDisplayName; Version = $config.Version; Mode = 'submission'; ExecutionAlias = 'corsivo-the-converter.exe'; CliExecutionAlias = 'corsivo-the-converter-cli.exe' }
    } else {
        if ($IdentityConfig) { throw 'IdentityConfig is only accepted with -ForSubmission. Preview uses an explicitly separate identity.' }
        $identity = [pscustomobject]@{ Name = 'Corsivo.TheConverter.StorePreview'; Publisher = 'CN=Salvatore Corsi'; DisplayName = 'The Converter Store Preview'; PublisherDisplayName = 'Corsivo'; Version = $PreviewVersion; Mode = 'preview'; ExecutionAlias = 'corsivo-the-converter-preview.exe'; CliExecutionAlias = 'corsivo-the-converter-preview-cli.exe' }
    }
    if ($identity.Name -notmatch '^[A-Za-z0-9][A-Za-z0-9.-]{1,48}[A-Za-z0-9]$') { throw 'Store package Name must contain 3-50 letters, digits, periods or hyphens.' }
    if ($identity.Publisher -notmatch '^CN=\S' -or $identity.Publisher.Length -gt 8192) { throw 'Store Publisher must be the exact Partner Center X.500 distinguished name.' }
    try { $null = New-Object Security.Cryptography.X509Certificates.X500DistinguishedName($identity.Publisher) } catch { throw 'Invalid X.500 Store Publisher.' }
    foreach ($field in @('DisplayName', 'PublisherDisplayName')) {
        if ($identity.$field.Length -gt 256 -or $identity.$field -match '[\x00-\x1F]') { throw "Invalid Store identity text: $field" }
    }
    if ($identity.Version -notmatch '^\d+\.\d+\.\d+\.0$') { throw 'Store Version must be major.minor.build.0; the final component is reserved by the Store.' }
    $version = [version]$identity.Version
    if ($version.Major -lt 1 -or $version.Major -gt 65535 -or $version.Minor -gt 65535 -or $version.Build -gt 65535) { throw 'Store Version must have major > 0 and components at most 65535.' }
    $identity.Version = $version.ToString()
    return $identity
}

function New-StorePackageManifest {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$TemplatePath, [Parameter(Mandatory = $true)]$Identity, [Parameter(Mandatory = $true)][string]$OutputPath)
    [xml]$manifest = Get-Content -LiteralPath $TemplatePath -Raw -Encoding UTF8
    $manifest.Package.Identity.SetAttribute('Name', $Identity.Name)
    $manifest.Package.Identity.SetAttribute('Publisher', $Identity.Publisher)
    $manifest.Package.Identity.SetAttribute('Version', $Identity.Version)
    $manifest.Package.Properties.DisplayName = $Identity.DisplayName
    $manifest.Package.Properties.PublisherDisplayName = $Identity.PublisherDisplayName
    $ns = New-Object Xml.XmlNamespaceManager($manifest.NameTable)
    $ns.AddNamespace('p', 'http://schemas.microsoft.com/appx/manifest/foundation/windows10')
    $ns.AddNamespace('uap', 'http://schemas.microsoft.com/appx/manifest/uap/windows10')
    $ns.AddNamespace('uap5', 'http://schemas.microsoft.com/appx/manifest/uap/windows10/5')
    $visual = $manifest.SelectSingleNode('/p:Package/p:Applications/p:Application[@Id="TheConverter"]/uap:VisualElements', $ns)
    $visual.SetAttribute('DisplayName', $Identity.DisplayName)
    $manifest.SelectSingleNode('/p:Package/p:Applications/p:Application[@Id="TheConverterCli"]/uap:VisualElements', $ns).SetAttribute('DisplayName', $Identity.DisplayName + ' CLI')
    $manifest.SelectSingleNode('//uap5:Extension[@Executable="the-converter.exe"]/uap5:AppExecutionAlias/uap5:ExecutionAlias', $ns).SetAttribute('Alias', $Identity.ExecutionAlias)
    $manifest.SelectSingleNode('//uap5:Extension[@Executable="the-converter-cli.exe"]/uap5:AppExecutionAlias/uap5:ExecutionAlias', $ns).SetAttribute('Alias', $Identity.CliExecutionAlias)
    $manifest.Save($OutputPath)
}

function Get-StoreAssetSpecifications {
    foreach ($base in @(@('StoreLogo', 50), @('Square44x44Logo', 44), @('Square150x150Logo', 150))) {
        [pscustomobject]@{ File = $base[0] + '.png'; Size = [int]$base[1] }
        $scales = if ($base[0] -eq 'StoreLogo') { @(100, 125, 150, 200, 400) } else { @(100, 200, 400) }
        foreach ($scale in $scales) {
            [pscustomobject]@{ File = $base[0] + '.scale-' + $scale + '.png'; Size = [int][Math]::Round($base[1] * $scale / 100.0, [MidpointRounding]::AwayFromZero) }
        }
    }
    foreach ($size in @(16, 24, 32, 48, 256)) {
        foreach ($form in @('unplated', 'lightunplated')) {
            [pscustomobject]@{ File = "Square44x44Logo.targetsize-$($size)_altform-$form.png"; Size = $size }
        }
    }
}

function New-StorePackageAssets {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$SourceImage, [Parameter(Mandatory = $true)][string]$OutputDirectory)
    Add-Type -AssemblyName System.Drawing
    New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
    $source = [Drawing.Image]::FromFile((Resolve-Path -LiteralPath $SourceImage).Path)
    try {
        foreach ($asset in @(Get-StoreAssetSpecifications)) {
            $size = $asset.Size
            $bitmap = New-Object Drawing.Bitmap($size, $size, [Drawing.Imaging.PixelFormat]::Format32bppArgb)
            $graphics = [Drawing.Graphics]::FromImage($bitmap)
            try {
                $graphics.Clear([Drawing.Color]::Transparent)
                $graphics.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
                $graphics.PixelOffsetMode = [Drawing.Drawing2D.PixelOffsetMode]::HighQuality
                $scale = [Math]::Min($size / [double]$source.Width, $size / [double]$source.Height)
                $width = [int][Math]::Round($source.Width * $scale)
                $height = [int][Math]::Round($source.Height * $scale)
                $graphics.DrawImage($source, [int](($size - $width) / 2), [int](($size - $height) / 2), $width, $height)
                $bitmap.Save((Join-Path $OutputDirectory $asset.File), [Drawing.Imaging.ImageFormat]::Png)
            } finally { $graphics.Dispose(); $bitmap.Dispose() }
        }
    } finally { $source.Dispose() }
}

function Assert-StorePackageLayout {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$StageDirectory, [Parameter(Mandatory = $true)]$Identity)
    $stage = (Resolve-Path -LiteralPath $StageDirectory).Path
    Add-Type -AssemblyName System.Drawing
    [xml]$manifest = Get-Content -LiteralPath (Join-Path $stage 'AppxManifest.xml') -Raw -Encoding UTF8
    $ns = New-Object Xml.XmlNamespaceManager($manifest.NameTable)
    foreach ($entry in @(
        @('p', 'http://schemas.microsoft.com/appx/manifest/foundation/windows10'),
        @('uap', 'http://schemas.microsoft.com/appx/manifest/uap/windows10'),
        @('uap10', 'http://schemas.microsoft.com/appx/manifest/uap/windows10/10'),
        @('uap5', 'http://schemas.microsoft.com/appx/manifest/uap/windows10/5'),
        @('com', 'http://schemas.microsoft.com/appx/manifest/com/windows10'),
        @('desktop4', 'http://schemas.microsoft.com/appx/manifest/desktop/windows10/4'),
        @('desktop5', 'http://schemas.microsoft.com/appx/manifest/desktop/windows10/5'),
        @('rescap', 'http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities')
    )) { $ns.AddNamespace($entry[0], $entry[1]) }
    foreach ($field in @('Name', 'Publisher', 'Version')) {
        if ($manifest.Package.Identity.GetAttribute($field) -cne $Identity.$field) { throw "Unexpected Store manifest identity: $field" }
    }
    if ($manifest.Package.Properties.DisplayName -cne $Identity.DisplayName -or $manifest.Package.Properties.PublisherDisplayName -cne $Identity.PublisherDisplayName) { throw 'Store package display metadata does not match the configured identity.' }
    if ($manifest.Package.Identity.GetAttribute('ProcessorArchitecture') -cne 'x64') { throw 'Store shell package must target x64.' }
    $deviceFamilies = @($manifest.SelectNodes('/p:Package/p:Dependencies/p:TargetDeviceFamily', $ns))
    if ($deviceFamilies.Count -ne 1 -or $deviceFamilies[0].GetAttribute('Name') -cne 'Windows.Desktop' -or
        $deviceFamilies[0].GetAttribute('MinVersion') -cne '10.0.22000.0') { throw 'Store package requires Windows 11 desktop (build 22000) or later.' }
    $languages = @($manifest.SelectNodes('/p:Package/p:Resources/p:Resource', $ns))
    if ($languages.Count -ne 1 -or $languages[0].GetAttribute('Language') -cne 'it-it') { throw 'Store manifest must advertise only the implemented Italian UI language.' }
    if (@($manifest.SelectNodes('//*[local-name()="AllowExternalContent"]')).Count -ne 0) { throw 'Store package must contain its files; AllowExternalContent is forbidden.' }
    $capabilities = @($manifest.SelectNodes('/p:Package/p:Capabilities/*', $ns))
    if ($capabilities.Count -ne 1 -or $capabilities[0].NamespaceURI -cne 'http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities' -or
        $capabilities[0].GetAttribute('Name') -cne 'runFullTrust') { throw 'Unexpected Store capabilities; only runFullTrust is allowed.' }
    $apps = @($manifest.SelectNodes('/p:Package/p:Applications/p:Application', $ns))
    if ($apps.Count -ne 2) { throw 'Store package must contain a visible GUI application and a hidden CLI application.' }
    foreach ($expectedApp in @(@('TheConverter', 'the-converter.exe'), @('TheConverterCli', 'the-converter-cli.exe'))) {
        $matches = @($apps | Where-Object { $_.GetAttribute('Id') -ceq $expectedApp[0] })
        if ($matches.Count -ne 1 -or $matches[0].GetAttribute('Executable') -cne $expectedApp[1] -or
            $matches[0].GetAttribute('RuntimeBehavior', 'http://schemas.microsoft.com/appx/manifest/uap/windows10/10') -cne 'packagedClassicApp' -or
            $matches[0].GetAttribute('TrustLevel', 'http://schemas.microsoft.com/appx/manifest/uap/windows10/10') -cne 'mediumIL' -or
            $matches[0].GetAttribute('SupportsMultipleInstances', 'http://schemas.microsoft.com/appx/manifest/uap/windows10/10') -cne 'true') { throw "Invalid full-trust or multiple-instance packaged application: $($expectedApp[0])" }
    }
    $visual = $manifest.SelectSingleNode('/p:Package/p:Applications/p:Application[@Id="TheConverter"]/uap:VisualElements', $ns)
    if (-not $visual -or $visual.GetAttribute('AppListEntry') -ceq 'none' -or $visual.GetAttribute('DisplayName') -cne $Identity.DisplayName) { throw 'Store application must be visible in the app list with the configured display name.' }
    $cliVisual = $manifest.SelectSingleNode('/p:Package/p:Applications/p:Application[@Id="TheConverterCli"]/uap:VisualElements', $ns)
    if (-not $cliVisual -or $cliVisual.GetAttribute('AppListEntry') -cne 'none' -or $cliVisual.GetAttribute('DisplayName') -cne ($Identity.DisplayName + ' CLI')) { throw 'CLI application must remain hidden from the app list.' }
    $cliApp = $cliVisual.ParentNode
    if ($cliApp.GetAttribute('Subsystem', 'http://schemas.microsoft.com/appx/manifest/uap/windows10/10') -cne 'console' -or
        $cliApp.GetAttribute('SupportsMultipleInstances', 'http://schemas.microsoft.com/appx/manifest/uap/windows10/10') -cne 'true') { throw 'Console activation requires multiple-instance support.' }
    $aliasExtensions = @($manifest.SelectNodes('/p:Package/p:Applications/p:Application/p:Extensions/uap5:Extension[@Category="windows.appExecutionAlias"]', $ns))
    if ($aliasExtensions.Count -ne 2) { throw 'Store package must declare separate GUI and CLI execution alias extensions.' }
    foreach ($expected in @(@('the-converter.exe', $Identity.ExecutionAlias, 'windows', 'TheConverter'), @('the-converter-cli.exe', $Identity.CliExecutionAlias, 'console', 'TheConverterCli'))) {
        $extension = @($aliasExtensions | Where-Object { $_.GetAttribute('Executable') -ceq $expected[0] })
        if ($extension.Count -ne 1) { throw "Execution alias target is missing or duplicated: $($expected[0])" }
        if ($extension[0].ParentNode.ParentNode.GetAttribute('Id') -cne $expected[3]) { throw 'Execution alias is attached to the wrong packaged application.' }
        $aliasContainer = $extension[0].SelectSingleNode('uap5:AppExecutionAlias', $ns)
        $aliases = @($extension[0].SelectNodes('uap5:AppExecutionAlias/uap5:ExecutionAlias', $ns))
        if (-not $aliasContainer -or $aliasContainer.GetAttribute('Subsystem', 'http://schemas.microsoft.com/appx/manifest/uap/windows10/10') -cne $expected[2] -or
            $aliases.Count -ne 1 -or $aliases[0].GetAttribute('Alias') -cne $expected[1]) { throw "Stable Store alias or subsystem is inconsistent: $($expected[0])" }
    }
    $runtime = Get-Content -LiteralPath (Join-Path $stage 'store-runtime.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    if (@($runtime.PSObject.Properties).Count -ne 1 -or -not $runtime.PSObject.Properties['execution_alias'] -or
        $runtime.execution_alias -cne $Identity.ExecutionAlias) { throw 'store-runtime.json must contain only the declared execution_alias.' }
    $classId = '67B51BA1-8A78-4B4B-A5F9-08A825BE91F8'
    $classes = @($manifest.SelectNodes('//com:Class', $ns))
    $verbs = @($manifest.SelectNodes('//desktop5:Verb', $ns))
    if ($classes.Count -ne 1 -or $classes[0].GetAttribute('Id') -ine $classId -or $classes[0].GetAttribute('Path') -cne 'the_converter_shell.dll' -or
        $verbs.Count -ne 1 -or $verbs[0].GetAttribute('Clsid') -ine $classId) { throw 'Invalid packaged Explorer command registration.' }
    $required = @('AppxManifest.xml', 'the-converter.exe', 'the-converter-cli.exe', 'the_converter_shell.dll', 'store-runtime.json', 'README.txt', 'THIRD-PARTY-NOTICES.md', 'Docs\licenses\RUST-THIRD-PARTY-LICENSES.txt', 'Docs\licenses\rust-components.json')
    foreach ($relative in $required) {
        if (-not (Test-Path -LiteralPath (Join-Path $stage $relative) -PathType Leaf)) { throw "Store layout file missing: $relative" }
    }
    if ($manifest.Package.Properties.Logo -cne 'Assets\StoreLogo.png' -or $visual.GetAttribute('Square150x150Logo') -cne 'Assets\Square150x150Logo.png' -or $visual.GetAttribute('Square44x44Logo') -cne 'Assets\Square44x44Logo.png') { throw 'Unexpected Store logo references.' }
    foreach ($logo in @(Get-StoreAssetSpecifications)) {
        $image = [Drawing.Image]::FromFile((Join-Path $stage ('Assets\' + $logo.File)))
        try { if ($image.Width -ne $logo.Size -or $image.Height -ne $logo.Size) { throw "Invalid Store logo dimensions: $($logo.File)" } } finally { $image.Dispose() }
    }
    $allowedBinaries = @('the-converter.exe', 'the-converter-cli.exe', 'the_converter_shell.dll')
    foreach ($file in @(Get-ChildItem -LiteralPath $stage -Recurse -Force)) {
        if ($file.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Reparse point in Store layout: $($file.FullName)" }
        if ($file.PSIsContainer) { continue }
        $relative = $file.FullName.Substring($stage.Length + 1)
        if ($file.Extension -in @('.exe', '.dll') -and $relative -notin $allowedBinaries) { throw "Unexpected Store executable or engine: $relative" }
        if ($file.Extension -in @('.ps1', '.psm1', '.bat', '.cmd', '.vbs', '.js', '.pfx', '.cer', '.msix', '.msixbundle', '.zip', '.lnk')) { throw "Forbidden script, certificate or nested package: $relative" }
    }
}

function Get-StorePeImportInfo {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$Path)
    if (-not ('TheConverter.StorePackaging.PeImports' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.IO;
using System.Text;
namespace TheConverter.StorePackaging {
    public static class PeImports {
        static ushort U16(byte[] b, int p) { if (p < 0 || p > b.Length - 2) throw new InvalidDataException("Truncated PE header"); return BitConverter.ToUInt16(b, p); }
        static uint U32(byte[] b, int p) { if (p < 0 || p > b.Length - 4) throw new InvalidDataException("Truncated PE header"); return BitConverter.ToUInt32(b, p); }
        static int Offset(byte[] b, uint rva, int sections, int count) {
            for (int i = 0; i < count; i++) {
                int p = checked(sections + i * 40);
                uint start = U32(b, p + 12), rawSize = U32(b, p + 16), size = Math.Max(U32(b, p + 8), rawSize);
                if (rva >= start && (ulong)rva < (ulong)start + size) {
                    uint delta = rva - start;
                    if (delta >= rawSize) throw new InvalidDataException("PE RVA has no file data");
                    return checked((int)(U32(b, p + 20) + delta));
                }
            }
            throw new InvalidDataException("PE RVA outside sections");
        }
        public static string[] Read(string path) {
            byte[] b = File.ReadAllBytes(path);
            if (U16(b, 0) != 0x5a4d) throw new InvalidDataException("Not a PE file");
            int pe = checked((int)U32(b, 0x3c));
            if (U32(b, pe) != 0x4550 || U16(b, pe + 4) != 0x8664 || U16(b, pe + 24) != 0x20b) throw new InvalidDataException("Expected x64 PE32+ binary");
            int count = U16(b, pe + 6), optional = pe + 24, sections = checked(optional + U16(b, pe + 20));
            if (U32(b, optional + 108) < 2) throw new InvalidDataException("Missing PE data directories");
            uint importsRva = U32(b, optional + 120);
            var names = new SortedSet<string>(StringComparer.OrdinalIgnoreCase);
            if (importsRva == 0) return new string[0];
            int descriptor = Offset(b, importsRva, sections, count);
            for (int i = 0; i < 4096; i++, descriptor = checked(descriptor + 20)) {
                uint nameRva = U32(b, descriptor + 12);
                if (nameRva == 0 && U32(b, descriptor) == 0 && U32(b, descriptor + 16) == 0) return new List<string>(names).ToArray();
                int name = Offset(b, nameRva, sections, count), end = name;
                while (end < b.Length && end - name < 260 && b[end] != 0) end++;
                if (end == b.Length || end - name >= 260) throw new InvalidDataException("Invalid PE import name");
                names.Add(Encoding.ASCII.GetString(b, name, end - name));
            }
            throw new InvalidDataException("Unterminated PE import table");
        }
    }
}
'@
    }
    $imports = @([TheConverter.StorePackaging.PeImports]::Read((Resolve-Path -LiteralPath $Path).Path))
    if (@($imports | Where-Object { $_ -match '(?i)^(vcruntime|msvcp|msvcr)\d.*\.dll$' }).Count -ne 0) { throw "Undeclared Visual C++ runtime dependency in $Path. Build with +crt-static." }
    [pscustomobject]@{ Architecture = 'x64'; Imports = $imports }
}

Export-ModuleMember -Function Get-StorePackageIdentity, New-StorePackageManifest, New-StorePackageAssets, Assert-StorePackageLayout, Get-StorePeImportInfo
