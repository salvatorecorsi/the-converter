# Shared, read-only validation for the sparse identity package.
Set-StrictMode -Version 2

function Get-ModernMenuPackageInfo {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$PackagePath,
        [Parameter(Mandatory = $true)][string]$InstallDirectory,
        [switch]$RequireTrustedSignature
    )
    $packageFile = (Resolve-Path -LiteralPath $PackagePath -ErrorAction Stop).Path
    $installRoot = (Resolve-Path -LiteralPath $InstallDirectory -ErrorAction Stop).Path
    if (-not (Test-Path -LiteralPath $packageFile -PathType Leaf)) { throw 'Il pacchetto identita non e un file.' }
    if (-not (Test-Path -LiteralPath $installRoot -PathType Container)) { throw 'La cartella applicazione non e valida.' }
    Add-Type -AssemblyName System.IO.Compression.FileSystem -ErrorAction Stop
    $archive = [IO.Compression.ZipFile]::OpenRead($packageFile)
    try {
        $entries = @($archive.Entries | Where-Object { $_.FullName -ieq 'AppxManifest.xml' })
        if ($entries.Count -ne 1 -or $entries[0].Length -gt 262144) { throw 'Manifest MSIX mancante, duplicato o troppo grande.' }
        $settings = New-Object System.Xml.XmlReaderSettings
        $settings.DtdProcessing = [Xml.DtdProcessing]::Prohibit
        $settings.XmlResolver = $null
        $settings.MaxCharactersInDocument = 262144
        $stream = $entries[0].Open()
        $reader = $null
        try {
            $reader = [Xml.XmlReader]::Create($stream, $settings)
            $manifest = New-Object System.Xml.XmlDocument
            $manifest.XmlResolver = $null
            $manifest.Load($reader)
        } finally {
            if ($reader) { $reader.Dispose() }
            $stream.Dispose()
        }
    } finally { $archive.Dispose() }

    $ns = New-Object Xml.XmlNamespaceManager($manifest.NameTable)
    $ns.AddNamespace('p', 'http://schemas.microsoft.com/appx/manifest/foundation/windows10')
    $ns.AddNamespace('uap', 'http://schemas.microsoft.com/appx/manifest/uap/windows10')
    $ns.AddNamespace('uap10', 'http://schemas.microsoft.com/appx/manifest/uap/windows10/10')
    $ns.AddNamespace('com', 'http://schemas.microsoft.com/appx/manifest/com/windows10')
    $ns.AddNamespace('desktop4', 'http://schemas.microsoft.com/appx/manifest/desktop/windows10/4')
    $ns.AddNamespace('desktop5', 'http://schemas.microsoft.com/appx/manifest/desktop/windows10/5')
    $identities = @($manifest.SelectNodes('/p:Package/p:Identity', $ns))
    if ($identities.Count -ne 1) { throw 'Identita MSIX non valida.' }
    $identity = $identities[0]
    $name = $identity.GetAttribute('Name')
    $publisher = $identity.GetAttribute('Publisher')
    $versionText = $identity.GetAttribute('Version')
    if ($name -cne 'SalvatoreCorsi.TheConverter' -or [string]::IsNullOrWhiteSpace($publisher) -or
        $identity.GetAttribute('ProcessorArchitecture') -cne 'x64') { throw 'Il pacchetto non e l identita x64 di The Converter.' }
    if ($versionText -notmatch '^\d+\.\d+\.\d+\.\d+$') { throw 'Versione identita MSIX non valida.' }
    $version = [version]$versionText
    foreach ($part in @($version.Major, $version.Minor, $version.Build, $version.Revision)) {
        if ($part -gt 65535) { throw 'Versione identita MSIX fuori intervallo.' }
    }
    $external = $manifest.SelectSingleNode('/p:Package/p:Properties/uap10:AllowExternalContent', $ns)
    if (-not $external -or $external.InnerText -cne 'true') { throw 'Il pacchetto deve usare contenuto esterno.' }
    $apps = @($manifest.SelectNodes('/p:Package/p:Applications/p:Application', $ns))
    if ($apps.Count -ne 1 -or $apps[0].GetAttribute('Id') -cne 'TheConverter' -or
        $apps[0].GetAttribute('Executable') -cne 'the-converter.exe') { throw 'Applicazione MSIX inattesa.' }
    $classes = @($manifest.SelectNodes('/p:Package/p:Applications/p:Application/p:Extensions/com:Extension[@Category="windows.comServer"]/com:ComServer/com:SurrogateServer/com:Class', $ns))
    $classId = '67B51BA1-8A78-4B4B-A5F9-08A825BE91F8'
    if ($classes.Count -ne 1 -or $classes[0].GetAttribute('Id') -ine $classId -or
        $classes[0].GetAttribute('Path') -cne 'the_converter_shell.dll' -or
        $classes[0].GetAttribute('ThreadingModel') -cne 'STA') { throw 'Server COM del menu inatteso.' }
    $verbs = @($manifest.SelectNodes('/p:Package/p:Applications/p:Application/p:Extensions/desktop4:Extension[@Category="windows.fileExplorerContextMenus"]/desktop4:FileExplorerContextMenus/desktop5:ItemType[@Type="*"]/desktop5:Verb', $ns))
    if ($verbs.Count -ne 1 -or $verbs[0].GetAttribute('Clsid') -ine $classId) { throw 'Comando del menu Windows 11 mancante o inatteso.' }

    $assetValues = @()
    $logo = $manifest.SelectSingleNode('/p:Package/p:Properties/p:Logo', $ns)
    if (-not $logo) { throw 'Logo del pacchetto mancante.' }
    $assetValues += $logo.InnerText
    $visual = $apps[0].SelectSingleNode('uap:VisualElements', $ns)
    if (-not $visual) { throw 'VisualElements del pacchetto mancante.' }
    foreach ($attribute in @('Square150x150Logo', 'Square44x44Logo')) { $assetValues += $visual.GetAttribute($attribute) }
    $assets = @()
    $assetRoot = [IO.Path]::GetFullPath((Join-Path $installRoot 'Assets')).TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
    foreach ($asset in $assetValues) {
        $segments = @($asset -split '[\\/]')
        if ($segments.Count -lt 2 -or $segments[0] -cne 'Assets' -or
            @($segments | Where-Object { $_ -eq '' -or $_ -eq '.' -or $_ -eq '..' -or $_.IndexOfAny([IO.Path]::GetInvalidFileNameChars()) -ge 0 }).Count -gt 0 -or
            [IO.Path]::GetExtension($asset) -notin @('.png', '.jpg', '.jpeg')) { throw "Percorso asset non consentito: $asset" }
        $assetPath = [IO.Path]::GetFullPath((Join-Path $installRoot ($segments -join [IO.Path]::DirectorySeparatorChar)))
        if (-not $assetPath.StartsWith($assetRoot, [StringComparison]::OrdinalIgnoreCase) -or
            -not (Test-Path -LiteralPath $assetPath -PathType Leaf)) { throw "Asset esterno mancante: $asset" }
        $assets += $segments -join '\'
    }
    foreach ($binary in @('the-converter.exe', 'the_converter_shell.dll')) {
        if (-not (Test-Path -LiteralPath (Join-Path $installRoot $binary) -PathType Leaf)) { throw "File applicazione mancante: $binary" }
    }
    if ($RequireTrustedSignature) {
        $signature = Get-AuthenticodeSignature -LiteralPath $packageFile -ErrorAction Stop
        if ($signature.Status -ne 'Valid' -or -not $signature.SignerCertificate) {
            throw "Il pacchetto MSIX deve avere una firma valida e attendibile (stato: $($signature.Status)). L archivio certificati non viene modificato."
        }
        if (-not [string]::Equals($signature.SignerCertificate.Subject, $publisher, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Il Publisher del manifest non corrisponde al certificato firmatario.'
        }
    }
    [pscustomobject]@{ Name = $name; Publisher = $publisher; Version = $version; Assets = @($assets | Select-Object -Unique) }
}

function Get-ModernMenuExternalPath {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)]$Package)
    # Get-AppxPackage does not expose these properties on every Windows version.
    foreach ($property in @('EffectiveExternalPath', 'UserExternalPath')) {
        if ($Package.PSObject.Properties[$property] -and $Package.$property) { return [string]$Package.$property }
    }
    if ($Package.PSObject.Properties['EffectiveExternalLocation'] -and $Package.EffectiveExternalLocation) {
        return [string]$Package.EffectiveExternalLocation.Path
    }
    try {
        $manager = [Windows.Management.Deployment.PackageManager,Windows.Management.Deployment,ContentType=WindowsRuntime]::new()
        $runtimePackage = $manager.FindPackageForUser('', [string]$Package.PackageFullName)
        if ($runtimePackage) { return [string]$runtimePackage.EffectiveExternalPath }
    } catch { Write-Verbose "Percorso esterno non verificabile: $($_.Exception.Message)" }
    return $null
}

Export-ModuleMember -Function Get-ModernMenuPackageInfo, Get-ModernMenuExternalPath
