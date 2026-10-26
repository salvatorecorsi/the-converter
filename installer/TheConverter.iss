; Small, per-user installer. Conversion runtimes are obtained from their upstream publishers.
#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#ifndef FileVersion
  #define FileVersion "0.1.0.0"
#endif
#ifndef SourceDir
  #define SourceDir "..\dist\TheConverter"
#endif
#ifndef OutputPath
  #define OutputPath "..\dist\release"
#endif
#ifndef ModernMenuAvailable
  #define ModernMenuAvailable "0"
#endif
#define RepositoryUrl "https://github.com/salvatorecorsi/the-converter"
#define ConverterClass "{67B51BA1-8A78-4B4B-A5F9-08A825BE91F8}"

[Setup]
AppId={{441C8D1C-A78E-4D93-9221-2FA8B207E408}
AppName=The Converter
AppVersion={#AppVersion}
AppVerName=The Converter {#AppVersion}
AppPublisher=Salvatore Corsi
AppPublisherURL={#RepositoryUrl}
AppSupportURL={#RepositoryUrl}/issues
AppUpdatesURL={#RepositoryUrl}/releases
VersionInfoVersion={#FileVersion}
VersionInfoDescription=The Converter Setup
DefaultDirName={localappdata}\Programs\The Converter
DefaultGroupName=The Converter
PrivilegesRequired=lowest
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
MinVersion=10.0.19041
DisableProgramGroupPage=yes
WizardStyle=modern
SetupIconFile=..\assets\app.ico
UninstallDisplayIcon={app}\the-converter.exe
UninstallDisplayName=The Converter
OutputDir={#OutputPath}
OutputBaseFilename=TheConverter-{#AppVersion}-windows-x64-setup
Compression=lzma2/max
SolidCompression=yes
CloseApplications=yes
CloseApplicationsFilter=the-converter.exe,the-converter-cli.exe
RestartApplications=no
ChangesAssociations=yes
UsePreviousAppDir=yes
UsePreviousTasks=yes
Uninstallable=yes
UninstallLogMode=append
SetupLogging=yes

[Languages]
Name: "italian"; MessagesFile: "compiler:Languages\Italian.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
italian.AdditionalIntegration=Integrazione con Windows:
italian.ContextMenu=Aggiungi "Converti in" al menu del tasto destro (su Windows 11: Mostra altre opzioni)
italian.DownloadTools=Scarica FFmpeg e Poppler per audio, video e PDF (consigliato; circa 160 MB, richiede Internet)
italian.ModernContextMenu=Attiva anche il menu moderno di Windows 11 (pacchetto firmato)
italian.DesktopShortcut=Crea un collegamento sul desktop
italian.LaunchApp=Apri The Converter
italian.UninstallApp=Disinstalla The Converter
italian.InstallTools=Installa o aggiorna motori di conversione
italian.DownloadingTools=Download e verifica dei motori di conversione...
italian.ToolsFailed=The Converter è installato, ma non è stato possibile scaricare FFmpeg e Poppler.%n%nLe conversioni di immagini restano disponibili. Per riprovare in seguito, apri "Installa o aggiorna motori di conversione" dalla cartella The Converter nel menu Start.%n%nVuoi riprovare il download adesso?
italian.ModernFailed=Non è stato possibile registrare il menu moderno di Windows 11. Il pacchetto deve avere una firma attendibile su questo computer. Il menu classico resta disponibile se selezionato.%n%nConsulta docs\windows-integration.md per i dettagli.
italian.PendingCleanup=Il menu è stato rimosso. Windows sta ancora usando la DLL dell'estensione: la pulizia verrà ritentata al prossimo accesso, senza riavviare Esplora file.
english.AdditionalIntegration=Windows integration:
english.ContextMenu=Add "Convert to" to the right-click menu (Windows 11: Show more options)
english.DownloadTools=Download FFmpeg and Poppler for audio, video and PDF (recommended; about 160 MB, requires Internet)
english.ModernContextMenu=Enable the modern Windows 11 menu as well (signed package)
english.DesktopShortcut=Create a desktop shortcut
english.LaunchApp=Open The Converter
english.UninstallApp=Uninstall The Converter
english.InstallTools=Install or update conversion engines
english.DownloadingTools=Downloading and verifying conversion engines...
english.ToolsFailed=The Converter is installed, but FFmpeg and Poppler could not be downloaded.%n%nImage conversions remain available. To retry later, open "Install or update conversion engines" in the The Converter Start menu folder.%n%nRetry the download now?
english.ModernFailed=The modern Windows 11 menu could not be registered. The package must have a trusted signature on this computer. The classic menu remains available if selected.%n%nSee docs\windows-integration.md for details.
english.PendingCleanup=The menu has been removed. Windows is still using the extension DLL: cleanup will be retried at your next sign-in without restarting File Explorer.

[Tasks]
Name: "contextmenu"; Description: "{cm:ContextMenu}"; GroupDescription: "{cm:AdditionalIntegration}"
Name: "downloadtools"; Description: "{cm:DownloadTools}"; GroupDescription: "{cm:AdditionalIntegration}"
#if ModernMenuAvailable == "1"
Name: "moderncontextmenu"; Description: "{cm:ModernContextMenu}"; GroupDescription: "{cm:AdditionalIntegration}"; Flags: unchecked; Check: IsWindows11
#endif
Name: "desktopicon"; Description: "{cm:DesktopShortcut}"; GroupDescription: "{cm:AdditionalIntegration}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\the-converter.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\the-converter-cli.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\the_converter_shell.dll"; DestDir: "{app}"; Flags: ignoreversion uninsneveruninstall
Source: "{#SourceDir}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\THIRD-PARTY-NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\docs\*"; DestDir: "{app}\docs"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#SourceDir}\scripts\install-context-menu.ps1"; DestDir: "{app}\scripts"; Flags: ignoreversion
Source: "{#SourceDir}\scripts\uninstall-context-menu.ps1"; DestDir: "{app}\scripts"; Flags: ignoreversion
Source: "..\scripts\install-runtime-tools.ps1"; DestDir: "{app}\scripts"; Flags: ignoreversion
#if ModernMenuAvailable == "1"
Source: "{#SourceDir}\TheConverter.Identity.msix"; DestDir: "{app}"; Flags: ignoreversion
#endif

[Icons]
Name: "{group}\The Converter"; Filename: "{app}\the-converter.exe"
Name: "{group}\{cm:InstallTools}"; Filename: "{sys}\WindowsPowerShell\v1.0\powershell.exe"; Parameters: "-NoLogo -NoProfile -NoExit -ExecutionPolicy Bypass -File ""{app}\scripts\install-runtime-tools.ps1"" -InstallDirectory ""{app}"""; WorkingDir: "{app}"; IconFilename: "{app}\the-converter.exe"
Name: "{group}\{cm:UninstallApp}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\The Converter"; Filename: "{app}\the-converter.exe"; Tasks: desktopicon

[Registry]
; Only private application keys. Uninstall removes them in code after checking ownership.
Root: HKCU; Subkey: "Software\Classes\CLSID\{{#ConverterClass}"; ValueType: string; ValueData: "The Converter Explorer Command"; Tasks: contextmenu
Root: HKCU; Subkey: "Software\Classes\CLSID\{{#ConverterClass}\InprocServer32"; ValueType: string; ValueData: "{app}\the_converter_shell.dll"; Tasks: contextmenu
Root: HKCU; Subkey: "Software\Classes\CLSID\{{#ConverterClass}\InprocServer32"; ValueType: string; ValueName: "ThreadingModel"; ValueData: "Apartment"; Tasks: contextmenu
Root: HKCU; Subkey: "Software\Classes\*\shell\TheConverter"; ValueType: string; ValueName: "MUIVerb"; ValueData: "Converti in"; Tasks: contextmenu
Root: HKCU; Subkey: "Software\Classes\*\shell\TheConverter"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\the-converter.exe"",0"; Tasks: contextmenu
Root: HKCU; Subkey: "Software\Classes\*\shell\TheConverter"; ValueType: string; ValueName: "ExplorerCommandHandler"; ValueData: "{{#ConverterClass}"; Tasks: contextmenu
Root: HKCU; Subkey: "Software\Classes\*\shell\TheConverter"; ValueType: string; ValueName: "MultiSelectModel"; ValueData: "Player"; Tasks: contextmenu

[Run]
Filename: "{app}\the-converter.exe"; Description: "{cm:LaunchApp}"; Flags: nowait postinstall skipifsilent unchecked

[UninstallDelete]
; Downloaded runtime binaries belong exclusively to the installation directory.
Type: filesandordirs; Name: "{app}\tools"

[Code]
const
  ClassKey = 'Software\Classes\CLSID\{#ConverterClass}';
  MenuKey = 'Software\Classes\*\shell\TheConverter';
  SHCNE_ASSOCCHANGED = $08000000;
  SHCNF_IDLIST = $0000;

procedure SHChangeNotify(EventID: Integer; Flags: Cardinal; Item1, Item2: Integer);
  external 'SHChangeNotify@shell32.dll stdcall';

function IsWindows11: Boolean;
var
  Version: TWindowsVersion;
begin
  GetWindowsVersionEx(Version);
  Result := Version.Build >= 22000;
end;

function Quoted(const Text: String): String;
begin
  Result := '"' + Text + '"';
end;

function PowerShellLiteral(const Text: String): String;
var
  Escaped: String;
begin
  Escaped := Text;
  StringChangeEx(Escaped, '''', '''''', True);
  { PowerShell also treats smart apostrophes as delimiters, even in an ASCII literal. }
  StringChangeEx(Escaped, #$2018, #$2018#$2018, True);
  StringChangeEx(Escaped, #$2019, #$2019#$2019, True);
  Result := '''' + Escaped + '''';
end;

function CleanupId: String;
begin
  Result := Copy(GetSHA256OfUnicodeString(LowerCase(RemoveBackslashUnlessRoot(ExpandConstant('{app}')))), 1, 16);
end;

function CleanupScriptPath: String;
begin
  Result := ExpandConstant('{localappdata}\SalvatoreCorsi\TheConverter\cleanup\') + CleanupId + '.ps1';
end;

function CleanupShortcutPath: String;
begin
  Result := ExpandConstant('{userstartup}\The Converter cleanup ') + CleanupId + '.lnk';
end;

procedure CancelDeferredCleanup;
begin
  { These two exact, installation-specific paths are exclusively installer-owned. }
  DeleteFile(CleanupShortcutPath);
  DeleteFile(CleanupScriptPath);
end;

procedure ScheduleShellCleanup;
var
  Script: TArrayOfString;
  ScriptPath, ShortcutPath, Ignored: String;
begin
  ScriptPath := CleanupScriptPath;
  ShortcutPath := CleanupShortcutPath;
  ForceDirectories(ExtractFileDir(ScriptPath));
  SetArrayLength(Script, 16);
  Script[0] := '# The Converter one-shot uninstall cleanup; exact paths only.';
  Script[1] := '$ErrorActionPreference = ''SilentlyContinue''';
  Script[2] := '$app = ' + PowerShellLiteral(ExpandConstant('{app}\the-converter.exe'));
  Script[3] := '$dll = ' + PowerShellLiteral(ExpandConstant('{app}\the_converter_shell.dll'));
  Script[4] := '$dir = ' + PowerShellLiteral(ExpandConstant('{app}'));
  Script[5] := '$shortcut = ' + PowerShellLiteral(ShortcutPath);
  Script[6] := 'for ($attempt = 0; $attempt -lt 3; $attempt++) {';
  Script[7] := '  if ([IO.File]::Exists($app)) { break }';
  Script[8] := '  try { if (-not [IO.File]::Exists($app)) { [IO.File]::Delete($dll) } } catch {}';
  Script[9] := '  if (-not [IO.File]::Exists($dll) -or [IO.File]::Exists($app)) { break }';
  Script[10] := '  Start-Sleep -Seconds 1';
  Script[11] := '}';
  Script[12] := 'if (-not [IO.File]::Exists($app)) { try { [IO.Directory]::Delete($dir, $false) } catch {} }';
  Script[13] := 'if (-not [IO.File]::Exists($dll) -or [IO.File]::Exists($app)) {';
  Script[14] := '  try { [IO.File]::Delete($shortcut) } catch {}; try { [IO.File]::Delete($PSCommandPath) } catch {}';
  Script[15] := '}';
  { SaveStringsToUTF8File writes a BOM so Windows PowerShell preserves Unicode paths. }
  if not SaveStringsToUTF8File(ScriptPath, Script, False) then
    RaiseException('Could not save the deferred shell cleanup helper.');
  Ignored := CreateShellLink(ShortcutPath, 'The Converter temporary uninstall cleanup',
    ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),
    '-NoLogo -NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File ' + Quoted(ScriptPath),
    ExtractFileDir(ScriptPath), '', 0, SW_HIDE);
  Log(CustomMessage('PendingCleanup'));
end;

function RunScript(const Name, ExtraArguments: String): Boolean;
var
  ExitCode: Integer;
  ProgramPath, Parameters: String;
  Started: Boolean;
begin
  ProgramPath := ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe');
  Parameters := '-NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File ' +
    Quoted(ExpandConstant('{app}\scripts\') + Name) + ' ' + ExtraArguments;
  Log('RunScript native x64: ' + ProgramPath + ' ' + Parameters);
  ExitCode := -1;
  Started := False;
  try
    { Inno 7's plain Exec does not redirect a 32-bit setup to native System32.
      This helper explicitly starts x64 PowerShell and captures stdout/stderr. }
    Started := ExecAndLogOutputWithNativeSysDir(ProgramPath, Parameters,
      ExpandConstant('{app}'), SW_SHOWNORMAL, ewWaitUntilTerminated, ExitCode, nil);
  except
    Log('RunScript capture/launch exception: ' + GetExceptionMessage);
  end;
  if Started then
    Log('RunScript exit code: ' + IntToStr(ExitCode))
  else
    Log('RunScript launch failed: ' + IntToStr(ExitCode) + ' ' + SysErrorMessage(ExitCode));
  Result := Started and (ExitCode = 0);
end;

procedure UnregisterClassic;
var
  Server, Icon: String;
begin
  if RegQueryStringValue(HKCU, ClassKey + '\InprocServer32', '', Server) and
    (CompareText(Server, ExpandConstant('{app}\the_converter_shell.dll')) = 0) then
  begin
    if RegQueryStringValue(HKCU, MenuKey, 'Icon', Icon) and
      (CompareText(Icon, Quoted(ExpandConstant('{app}\the-converter.exe')) + ',0') = 0) then
      RegDeleteKeyIncludingSubkeys(HKCU, MenuKey);
    RegDeleteKeyIncludingSubkeys(HKCU, ClassKey);
  end;
  SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, 0, 0);
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Retry: Integer;
  ModernInstalled: Boolean;
begin
  if CurStep = ssInstall then
    CancelDeferredCleanup;
  if CurStep = ssPostInstall then
  begin
    if not WizardIsTaskSelected('contextmenu') then
      UnregisterClassic;
    if WizardIsTaskSelected('downloadtools') then
    begin
      WizardForm.StatusLabel.Caption := CustomMessage('DownloadingTools');
      repeat
        if RunScript('install-runtime-tools.ps1', '-InstallDirectory ' + Quoted(ExpandConstant('{app}'))) then
          Retry := IDNO
        else
          Retry := SuppressibleMsgBox(CustomMessage('ToolsFailed'), mbError, MB_YESNO, IDNO);
      until Retry <> IDYES;
    end;
    ModernInstalled := False;
#if ModernMenuAvailable == "1"
    if WizardIsTaskSelected('moderncontextmenu') then
    begin
      ModernInstalled := RunScript('install-context-menu.ps1', '-Mode Modern -InstallDirectory ' + Quoted(ExpandConstant('{app}')));
      if not ModernInstalled then
        SuppressibleMsgBox(CustomMessage('ModernFailed'), mbError, MB_OK, IDOK);
    end;
#endif
    if ModernInstalled then
      SetIniString('Integration', 'ModernMenuInstalled', '1', ExpandConstant('{app}\installer-state.ini'));
    SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, 0, 0);
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  LibraryPath: String;
begin
  if CurUninstallStep = usUninstall then
  begin
    UnregisterClassic;
    if GetIniString('Integration', 'ModernMenuInstalled', '0', ExpandConstant('{app}\installer-state.ini')) = '1' then
      RunScript('uninstall-context-menu.ps1', '-Mode Modern');
  end;
  if CurUninstallStep = usPostUninstall then
  begin
    DeleteFile(ExpandConstant('{app}\installer-state.ini'));
    LibraryPath := ExpandConstant('{app}\the_converter_shell.dll');
    { The DLL is handled explicitly, avoiding an admin-only reboot queue. }
    DeleteFile(LibraryPath);
    if FileExists(LibraryPath) then
      ScheduleShellCleanup
    else
      RemoveDir(ExpandConstant('{app}'));
  end;
end;
