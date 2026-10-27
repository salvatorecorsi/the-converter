# Exercises the real Windows COM ABI without registering a class or a menu.
[CmdletBinding()]
param(
    [string]$LibraryPath,
    # Only point this at the output of an isolated argument-capture test helper.
    [string]$InvokeCaptureFile,
    [switch]$ValidateRegistrationOnly
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$registration = Join-Path $PSScriptRoot 'install-context-menu.ps1'
$installer = Join-Path $repo 'installer\TheConverter.iss'
$shellSource = Join-Path $repo 'shell-extension\src\lib.rs'
if ((Test-Path -LiteralPath $installer) -and (Test-Path -LiteralPath $shellSource)) {
    $installSource = Get-Content -LiteralPath $registration -Raw
    $list = [regex]::Match($installSource, '(?s)\$classicExtensions\s*=\s*@\((.*?)\)')
    if (-not $list.Success) { throw 'Classic registration extension whitelist missing.' }
    $extensions = @([regex]::Matches($list.Groups[1].Value, "'([a-z0-9]+)'") | ForEach-Object { $_.Groups[1].Value })
    $expectedQuery = ($extensions | ForEach-Object { 'System.FileExtension:=".' + $_ + '"' }) -join ' OR '
    $line = @(Get-Content -LiteralPath $installer | Where-Object { $_ -match 'ValueName: "AppliesTo";' })
    if ($line.Count -ne 1) { throw 'Installer must declare one classic AppliesTo filter.' }
    $actualQuery = [regex]::Match($line[0], 'ValueData: "(.*)"; Tasks: contextmenu').Groups[1].Value.Replace('""', '"')
    if ($actualQuery -cne $expectedQuery -or $installSource -notmatch "-Name 'AppliesTo' -Value \`$appliesTo") { throw 'Classic script and installer AppliesTo filters differ.' }
    $rust = Get-Content -LiteralPath $shellSource -Raw
    $maskSource = $rust.Substring($rust.IndexOf('fn target_mask('), $rust.IndexOf('fn common_target_mask') - $rust.IndexOf('fn target_mask('))
    $shellExtensions = @([regex]::Matches($maskSource, '"([a-z0-9]+)"') | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
    if ((Compare-Object @($extensions | Sort-Object -Unique) $shellExtensions)) { throw 'Classic whitelist and shell input extensions differ.' }
    if ($extensions -notcontains 'txt' -or $extensions -notcontains 'mpga' -or $extensions -contains 'exe' -or $extensions -contains 'unknown') { throw 'Unexpected classic whitelist capabilities.' }
    Write-Output "Classic AppliesTo verified: $($extensions.Count) input extensions match DLL and installer; TXT documents included, unknown inputs excluded."
} elseif ($ValidateRegistrationOnly) { throw 'Run registration-only validation from the source checkout.' }
if ($ValidateRegistrationOnly) { return }
if (-not $LibraryPath) {
    $adjacentLibrary = Join-Path $PSScriptRoot '..\the_converter_shell.dll'
    if (Test-Path -LiteralPath $adjacentLibrary -PathType Leaf) { $LibraryPath = $adjacentLibrary }
    else { $LibraryPath = Join-Path $PSScriptRoot '..\shell-extension\target\release\the_converter_shell.dll' }
}
$library = (Resolve-Path -LiteralPath $LibraryPath).Path
if (-not ('ConverterShellSmokeV2' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

public static class ConverterShellSmokeV2 {
    [StructLayout(LayoutKind.Sequential)]
    struct Credential {
        public uint Flags, Type;
        public IntPtr Target, Comment;
        public System.Runtime.InteropServices.ComTypes.FILETIME LastWritten;
        public uint BlobSize;
        public IntPtr Blob;
        public uint Persist, AttributeCount;
        public IntPtr Attributes, TargetAlias, UserName;
    }
    [DllImport("advapi32", CharSet=CharSet.Unicode, SetLastError=true)]
    static extern bool CredReadW(string target, uint type, uint flags, out IntPtr credential);
    [DllImport("advapi32")]
    static extern void CredFree(IntPtr credential);
    static bool ApiKeyPresent() {
        IntPtr pointer;
        if (!CredReadW("Corsivo.TheConverter.OpenAI", 1, 0, out pointer) || pointer == IntPtr.Zero) return false;
        try {
            var credential = (Credential)Marshal.PtrToStructure(pointer, typeof(Credential));
            bool available = credential.Type == 1 && credential.Blob != IntPtr.Zero && credential.BlobSize >= 20 && credential.BlobSize <= 2048;
            if (credential.Blob != IntPtr.Zero && credential.BlobSize <= 2560) {
                for (int offset = 0; offset < credential.BlobSize; offset++) Marshal.WriteByte(credential.Blob, offset, 0);
            }
            return available;
        } finally { CredFree(pointer); }
    }
    [DllImport("kernel32", CharSet=CharSet.Unicode, SetLastError=true)]
    static extern IntPtr LoadLibrary(string path);
    [DllImport("kernel32", CharSet=CharSet.Ansi)]
    static extern IntPtr GetProcAddress(IntPtr library, string name);
    [DllImport("kernel32")]
    static extern bool FreeLibrary(IntPtr library);
    [DllImport("ole32")]
    static extern int CoInitializeEx(IntPtr reserved, uint flags);
    [DllImport("ole32")]
    static extern void CoUninitialize();
    [DllImport("shell32", CharSet=CharSet.Unicode, PreserveSig=true)]
    static extern int SHCreateItemFromParsingName(string path, IntPtr bind, ref Guid iid, out IntPtr item);
    [DllImport("shell32", PreserveSig=true)]
    static extern int SHCreateShellItemArrayFromShellItem(IntPtr item, ref Guid iid, out IntPtr array);
    [DllImport("shell32", CharSet=CharSet.Unicode, PreserveSig=true)]
    static extern int SHParseDisplayName(string path, IntPtr bind, out IntPtr pidl, uint attributes, out uint actual);
    [DllImport("shell32", PreserveSig=true)]
    static extern int SHCreateShellItemArrayFromIDLists(uint count, [In] IntPtr[] pidls, out IntPtr array);
    [UnmanagedFunctionPointer(CallingConvention.Winapi)]
    delegate int GetClassObject(ref Guid clsid, ref Guid iid, out IntPtr instance);
    [UnmanagedFunctionPointer(CallingConvention.Winapi)]
    delegate int CanUnload();
    [UnmanagedFunctionPointer(CallingConvention.Winapi)]
    delegate int CreateInstance(IntPtr self, IntPtr outer, ref Guid iid, out IntPtr instance);
    [UnmanagedFunctionPointer(CallingConvention.Winapi)]
    delegate int GetState(IntPtr self, IntPtr items, int slow, out uint state);
    [UnmanagedFunctionPointer(CallingConvention.Winapi)]
    delegate int GetTitle(IntPtr self, IntPtr items, out IntPtr title);
    [UnmanagedFunctionPointer(CallingConvention.Winapi)]
    delegate int EnumCommands(IntPtr self, out IntPtr enumerator);
    [UnmanagedFunctionPointer(CallingConvention.Winapi)]
    delegate int NextCommand(IntPtr self, uint requested, out IntPtr command, out uint fetched);
    [UnmanagedFunctionPointer(CallingConvention.Winapi)]
    delegate int GetFlags(IntPtr self, out uint flags);
    [UnmanagedFunctionPointer(CallingConvention.Winapi)]
    delegate int InvokeCommand(IntPtr self, IntPtr items, IntPtr bind);

    static Delegate Slot(IntPtr instance, int slot, Type delegateType) {
        return Marshal.GetDelegateForFunctionPointer(Marshal.ReadIntPtr(Marshal.ReadIntPtr(instance), slot * IntPtr.Size), delegateType);
    }
    static void Check(int status) { Marshal.ThrowExceptionForHR(status); }
    static uint State(IntPtr command, string file) {
        Guid shellItem = new Guid("43826d1e-e718-42ee-bc55-a1e261c37bfe");
        Guid shellArray = new Guid("b63ea76d-1f85-456f-a19c-48159efa858b");
        IntPtr item = IntPtr.Zero, array = IntPtr.Zero;
        try {
            Check(SHCreateItemFromParsingName(file, IntPtr.Zero, ref shellItem, out item));
            Check(SHCreateShellItemArrayFromShellItem(item, ref shellArray, out array));
            uint state;
            Check(((GetState)Slot(command, 7, typeof(GetState)))(command, array, 0, out state));
            return state;
        } finally {
            if (array != IntPtr.Zero) Marshal.Release(array);
            if (item != IntPtr.Zero) Marshal.Release(item);
        }
    }
    static string Title(IntPtr command) {
        IntPtr text;
        Check(((GetTitle)Slot(command, 3, typeof(GetTitle)))(command, IntPtr.Zero, out text));
        try { return Marshal.PtrToStringUni(text); } finally { Marshal.FreeCoTaskMem(text); }
    }
    static string[] Visible(IntPtr parent, params string[] files) {
        IntPtr[] pidls = new IntPtr[files.Length];
        IntPtr array = IntPtr.Zero, enumerator = IntPtr.Zero;
        var labels = new List<string>();
        try {
            for (int index = 0; index < files.Length; index++) {
                uint attributes;
                Check(SHParseDisplayName(files[index], IntPtr.Zero, out pidls[index], 0, out attributes));
            }
            Check(SHCreateShellItemArrayFromIDLists((uint)pidls.Length, pidls, out array));
            uint parentState;
            Check(((GetState)Slot(parent, 7, typeof(GetState)))(parent, array, 1, out parentState));
            if (parentState != 0) return labels.ToArray();
            Check(((EnumCommands)Slot(parent, 10, typeof(EnumCommands)))(parent, out enumerator));
            var next = (NextCommand)Slot(enumerator, 3, typeof(NextCommand));
            for (;;) {
                IntPtr child; uint fetched;
                int status = next(enumerator, 1, out child, out fetched);
                Check(status);
                if (fetched == 0) break;
                try {
                    uint state;
                    Check(((GetState)Slot(child, 7, typeof(GetState)))(child, array, 1, out state));
                    if (state == 0) labels.Add(Title(child));
                } finally { Marshal.Release(child); }
            }
            return labels.ToArray();
        } finally {
            if (enumerator != IntPtr.Zero) Marshal.Release(enumerator);
            if (array != IntPtr.Zero) Marshal.Release(array);
            foreach (IntPtr pidl in pidls) if (pidl != IntPtr.Zero) Marshal.FreeCoTaskMem(pidl);
        }
    }
    static void Expect(string label, string[] actual, string[] expected) {
        if (String.Join("|", actual) != String.Join("|", expected)) throw new Exception(label + ": " + String.Join(", ", actual));
    }
    public static void CaptureInvoke(string library, string capture, string[] files, bool custom) {
        int initialized = CoInitializeEx(IntPtr.Zero, 2);
        IntPtr module = LoadLibrary(library), factory = IntPtr.Zero, parent = IntPtr.Zero, enumerator = IntPtr.Zero, child = IntPtr.Zero, array = IntPtr.Zero;
        IntPtr[] pidls = new IntPtr[files.Length];
        if (module == IntPtr.Zero) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
        try {
            var activate = (GetClassObject)Marshal.GetDelegateForFunctionPointer(GetProcAddress(module, "DllGetClassObject"), typeof(GetClassObject));
            Guid clsid = new Guid("67b51ba1-8a78-4b4b-a5f9-08a825be91f8"), factoryId = new Guid("00000001-0000-0000-C000-000000000046");
            Guid commandId = new Guid("a08ce4d0-fa25-44ab-b57c-c7b1c323e0b9");
            Check(activate(ref clsid, ref factoryId, out factory));
            Check(((CreateInstance)Slot(factory, 3, typeof(CreateInstance)))(factory, IntPtr.Zero, ref commandId, out parent));
            Check(((EnumCommands)Slot(parent, 10, typeof(EnumCommands)))(parent, out enumerator));
            var next = (NextCommand)Slot(enumerator, 3, typeof(NextCommand));
            for (int index = 0; index <= (custom ? 21 : 0); index++) {
                uint fetched;
                if (child != IntPtr.Zero) { Marshal.Release(child); child = IntPtr.Zero; }
                Check(next(enumerator, 1, out child, out fetched));
                if (fetched != 1) throw new Exception("Sottocomando mancante");
            }
            for (int index = 0; index < files.Length; index++) {
                uint attributes;
                Check(SHParseDisplayName(files[index], IntPtr.Zero, out pidls[index], 0, out attributes));
            }
            Check(SHCreateShellItemArrayFromIDLists((uint)pidls.Length, pidls, out array));
            Check(((InvokeCommand)Slot(child, 8, typeof(InvokeCommand)))(child, array, IntPtr.Zero));
            var elapsed = System.Diagnostics.Stopwatch.StartNew();
            while (!System.IO.File.Exists(capture) && elapsed.ElapsedMilliseconds < 5000) System.Threading.Thread.Sleep(50);
            if (!System.IO.File.Exists(capture)) throw new Exception("Helper Invoke non avviato");
        } finally {
            if (array != IntPtr.Zero) Marshal.Release(array);
            if (child != IntPtr.Zero) Marshal.Release(child);
            if (enumerator != IntPtr.Zero) Marshal.Release(enumerator);
            if (parent != IntPtr.Zero) Marshal.Release(parent);
            if (factory != IntPtr.Zero) Marshal.Release(factory);
            foreach (IntPtr pidl in pidls) if (pidl != IntPtr.Zero) Marshal.FreeCoTaskMem(pidl);
            FreeLibrary(module);
            if (initialized >= 0) CoUninitialize();
        }
    }
    public static string Run(string library, string supported, string unsupported, string audio, string video, string pdf, string document, string text, string mpga, string directory) {
        int initialized = CoInitializeEx(IntPtr.Zero, 2);
        IntPtr module = LoadLibrary(library), factory = IntPtr.Zero, command = IntPtr.Zero;
        if (module == IntPtr.Zero) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
        try {
            var activate = (GetClassObject)Marshal.GetDelegateForFunctionPointer(GetProcAddress(module, "DllGetClassObject"), typeof(GetClassObject));
            var unload = (CanUnload)Marshal.GetDelegateForFunctionPointer(GetProcAddress(module, "DllCanUnloadNow"), typeof(CanUnload));
            if (unload() != 0) throw new Exception("DLL inizialmente occupata");
            Guid clsid = new Guid("67b51ba1-8a78-4b4b-a5f9-08a825be91f8"), factoryId = new Guid("00000001-0000-0000-C000-000000000046");
            Guid commandId = new Guid("a08ce4d0-fa25-44ab-b57c-c7b1c323e0b9");
            Check(activate(ref clsid, ref factoryId, out factory));
            Check(((CreateInstance)Slot(factory, 3, typeof(CreateInstance)))(factory, IntPtr.Zero, ref commandId, out command));
            string label = Title(command);
            if (label != "Converti in") throw new Exception("Titolo del menu padre errato: " + label);
            uint flags;
            Check(((GetFlags)Slot(command, 9, typeof(GetFlags)))(command, out flags));
            if (flags != 1) throw new Exception("ECF_HASSUBCOMMANDS mancante");
            if (State(command, supported) != 0) throw new Exception("PNG non abilitato");
            if (State(command, unsupported) != 2) throw new Exception("Estensione sconosciuta non nascosta");
            const string custom = "Impostazioni personalizzate…";
            Expect("Audio + video", Visible(command, audio, video), new string[]{"MP3", "WAV", "FLAC", "AAC", "OGG", "Opus", "AIFF", custom});
            Expect("PNG + PDF", Visible(command, supported, pdf), new string[]{"PNG", "JPEG", custom});
            Expect("DOCX + PNG", Visible(command, document, supported), new string[]{"PDF", custom});
            Expect("PNG + audio", Visible(command, supported, audio), new string[]{});
            Expect("Testo", Visible(command, text), new string[]{"PDF", custom});
            Expect("Testo + audio", Visible(command, text, audio), new string[]{});
            Expect("Cartella WAV", Visible(command, directory), new string[]{});
            var audioFormats = new List<string>(new string[]{"MP3", "WAV", "FLAC", "AAC", "OGG", "Opus", "AIFF"});
            bool keyAvailable = ApiKeyPresent();
            if (keyAvailable) audioFormats.Add("TXT con OpenAI");
            audioFormats.Add(custom);
            Expect("Audio e mpga", Visible(command, audio, mpga), audioFormats.ToArray());
            Expect("PDF", Visible(command, pdf), new string[]{"PNG", "JPEG", "TXT", custom});
            Expect("Audio + PDF", Visible(command, audio, pdf), keyAvailable ? new string[]{"TXT con OpenAI", custom} : new string[]{});
            Expect("Sconosciuto", Visible(command, unsupported), new string[]{});
            Marshal.Release(command); command = IntPtr.Zero;
            Marshal.Release(factory); factory = IntPtr.Zero;
            if (unload() != 0) throw new Exception("Oggetti COM non rilasciati");
            return "COM smoke test OK: " + label + "; enumerazione sottomenu, filtri batch audio/video/immagini/PDF/documenti, selezioni reali IShellItemArray e unload OK.";
        } finally {
            if (command != IntPtr.Zero) Marshal.Release(command);
            if (factory != IntPtr.Zero) Marshal.Release(factory);
            FreeLibrary(module);
            if (initialized >= 0) CoUninitialize();
        }
    }
}
'@
}
$token = [Guid]::NewGuid().ToString('N')
$supported = Join-Path ([IO.Path]::GetTempPath()) "the-converter-smoke-$token.png"
$unsupported = Join-Path ([IO.Path]::GetTempPath()) "the-converter-smoke-$token.unknown"
$audio = Join-Path ([IO.Path]::GetTempPath()) "the-converter-smoke-$token.wav"
$video = Join-Path ([IO.Path]::GetTempPath()) "the-converter-smoke-$token.mp4"
$pdf = Join-Path ([IO.Path]::GetTempPath()) "the-converter-smoke-$token.pdf"
$document = Join-Path ([IO.Path]::GetTempPath()) "the-converter-smoke-$token.docx"
$text = Join-Path ([IO.Path]::GetTempPath()) "the-converter-smoke-$token.txt"
$mpga = Join-Path ([IO.Path]::GetTempPath()) "the-converter-smoke-$token.mpga"
$directory = Join-Path ([IO.Path]::GetTempPath()) "the-converter-folder-$token.wav"
$testFiles = @($supported, $unsupported, $audio, $video, $pdf, $document, $text, $mpga)
try {
    foreach ($file in $testFiles) { [IO.File]::WriteAllBytes($file, [byte[]]@()) }
    New-Item -ItemType Directory -Path $directory | Out-Null
    [ConverterShellSmokeV2]::Run($library, $supported, $unsupported, $audio, $video, $pdf, $document, $text, $mpga, $directory)
    if ($InvokeCaptureFile) {
        foreach ($custom in @($false, $true)) {
            if (Test-Path -LiteralPath $InvokeCaptureFile) { throw 'Il file di cattura deve essere assente prima del test.' }
            [ConverterShellSmokeV2]::CaptureInvoke($library, $InvokeCaptureFile, [string[]]@($audio, $video), $custom)
            $captured = [IO.File]::ReadAllLines($InvokeCaptureFile)
            if ($custom) {
                if ($captured.Count -ne 2 -or $captured[0] -ne '--selection-file') { throw 'Invoke custom deve aprire GUI senza --quick.' }
                $request = $captured[1]
            } else {
                if ($captured.Count -ne 5 -or $captured[0] -ne '--quick' -or $captured[1] -ne '--target' -or $captured[2] -ne 'mp3' -or $captured[3] -ne '--selection-file') { throw 'Invoke rapido non valido.' }
                $request = $captured[4]
            }
            try {
                $selection = [IO.File]::ReadAllText($request) | ConvertFrom-Json
                if ($selection.Count -ne 2 -or $selection[0] -ne $audio -or $selection[1] -ne $video) { throw 'Selezione batch non conservata.' }
            } finally {
                if ($request -like (Join-Path ([IO.Path]::GetTempPath()) 'the-converter-selection-*.json')) { Remove-Item -LiteralPath $request -Force }
                Remove-Item -LiteralPath $InvokeCaptureFile -Force
            }
        }
        Write-Output 'Invoke smoke test OK: MP3 usa --quick --target mp3; personalizzate apre GUI; JSON batch conservato.'
    }
} finally {
    foreach ($file in $testFiles) {
        if (Test-Path -LiteralPath $file) { Remove-Item -LiteralPath $file -Force }
    }
    if (Test-Path -LiteralPath $directory -PathType Container) { Remove-Item -LiteralPath $directory -Force }
}
