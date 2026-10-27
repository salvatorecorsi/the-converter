# Read PE resources as data only; never load executable code during release validation.
Set-StrictMode -Version 2

function Get-ExecutableApplicationManifest {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$ExecutablePath)
    $path = (Resolve-Path -LiteralPath $ExecutablePath -ErrorAction Stop).Path
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw 'Eseguibile da verificare non valido.' }
    if (-not ('TheConverter.ReleaseValidation.PeResources' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
namespace TheConverter.ReleaseValidation {
    public static class PeResources {
        [DllImport("kernel32.dll", EntryPoint = "LoadLibraryExW", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern IntPtr LoadLibraryEx(string path, IntPtr file, uint flags);
        [DllImport("kernel32.dll", EntryPoint = "FindResourceW", SetLastError = true)]
        private static extern IntPtr FindResource(IntPtr module, IntPtr name, IntPtr type);
        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern uint SizeofResource(IntPtr module, IntPtr resource);
        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern IntPtr LoadResource(IntPtr module, IntPtr resource);
        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern IntPtr LockResource(IntPtr resource);
        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool FreeLibrary(IntPtr module);

        public static byte[] ReadManifest(string path) {
            // LOAD_LIBRARY_AS_DATAFILE: DllMain, imports and executable code are not run.
            IntPtr module = LoadLibraryEx(path, IntPtr.Zero, 0x00000002);
            if (module == IntPtr.Zero) throw new Win32Exception(Marshal.GetLastWin32Error(), "Impossibile leggere le risorse PE");
            try {
                IntPtr resource = FindResource(module, new IntPtr(1), new IntPtr(24));
                if (resource == IntPtr.Zero) throw new Win32Exception(Marshal.GetLastWin32Error(), "RT_MANIFEST #1 mancante");
                uint length = SizeofResource(module, resource);
                if (length == 0 || length > 262144) throw new InvalidOperationException("Dimensione RT_MANIFEST non valida");
                IntPtr loaded = LoadResource(module, resource);
                if (loaded == IntPtr.Zero) throw new Win32Exception(Marshal.GetLastWin32Error(), "Impossibile caricare RT_MANIFEST");
                IntPtr data = LockResource(loaded);
                if (data == IntPtr.Zero) throw new InvalidOperationException("RT_MANIFEST non leggibile");
                byte[] bytes = new byte[(int)length];
                Marshal.Copy(data, bytes, 0, bytes.Length);
                return bytes;
            } finally {
                FreeLibrary(module);
            }
        }
    }
}
'@ -ErrorAction Stop
    }
    $bytes = [TheConverter.ReleaseValidation.PeResources]::ReadManifest($path)
    $stream = New-Object IO.MemoryStream(, $bytes)
    $reader = $null
    try {
        $settings = New-Object Xml.XmlReaderSettings
        $settings.DtdProcessing = [Xml.DtdProcessing]::Prohibit
        $settings.XmlResolver = $null
        $settings.MaxCharactersInDocument = 262144
        $reader = [Xml.XmlReader]::Create($stream, $settings)
        $document = New-Object Xml.XmlDocument
        $document.XmlResolver = $null
        $document.Load($reader)
        return $document
    } finally {
        if ($reader) { $reader.Dispose() }
        $stream.Dispose()
    }
}

function Assert-ExecutableApplicationManifest {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$ExecutablePath,
        [Parameter(Mandatory = $true)][xml]$ExpectedManifest
    )
    $actual = Get-ExecutableApplicationManifest -ExecutablePath $ExecutablePath
    $fields = @(
        @('publisher', '/a:assembly/m:msix/@publisher'),
        @('packageName', '/a:assembly/m:msix/@packageName'),
        @('applicationId', '/a:assembly/m:msix/@applicationId'),
        @('assembly version', '/a:assembly/a:assemblyIdentity/@version')
    )
    $actualNs = New-Object Xml.XmlNamespaceManager($actual.NameTable)
    $expectedNs = New-Object Xml.XmlNamespaceManager($ExpectedManifest.NameTable)
    foreach ($ns in @($actualNs, $expectedNs)) {
        $ns.AddNamespace('a', 'urn:schemas-microsoft-com:asm.v1')
        $ns.AddNamespace('m', 'urn:schemas-microsoft-com:msix.v1')
    }
    foreach ($field in $fields) {
        $actualValues = @($actual.SelectNodes($field[1], $actualNs))
        $expectedValues = @($ExpectedManifest.SelectNodes($field[1], $expectedNs))
        if ($actualValues.Count -ne 1 -or $expectedValues.Count -ne 1 -or
            [string]::IsNullOrWhiteSpace($expectedValues[0].Value) -or
            -not [string]::Equals($actualValues[0].Value, $expectedValues[0].Value, [StringComparison]::Ordinal)) {
            throw "Manifest incorporato non coerente: $ExecutablePath ($($field[0])). Ricompila gli eseguibili con l identita prevista."
        }
    }
}

Export-ModuleMember -Function Get-ExecutableApplicationManifest, Assert-ExecutableApplicationManifest
