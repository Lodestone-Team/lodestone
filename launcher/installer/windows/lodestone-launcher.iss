; The Windows installer for Lodestone Launcher (Inno Setup 6).
;
; Built by launcher/scripts/publish_release.py, which passes the binary and its
; build number:
;
;   ISCC /DSourceExe=<path\to\lodestone-launcher.exe> /DBuild=531 /DVersion=0.1.0
;        /DOutputDir=<folder> /DOutputName=<file name without .exe> lodestone-launcher.iss
;
; It installs for the current user only, without asking for administrator rights,
; under %LOCALAPPDATA%\Programs unless the person chooses another folder (another
; drive's games folder, say). That matters for updates: the launcher updates
; itself by replacing its own executable (launcher-core/src/update.rs), which it
; can only do in a folder the user can write to, so Setup refuses folders they
; can't (shared.iss). Program Files would need administrator rights for every
; update. Upgrading keeps the folder of the install it upgrades.
;
; The launcher's data (instances, worlds, accounts) is in %APPDATA%\Lodestone
; Launcher, a different folder. Installing never touches it; uninstalling keeps it,
; and at the end says so and offers to delete it, with the saved Microsoft sign-ins
; (shared.iss).

#ifndef SourceExe
  #error Pass /DSourceExe=<path to lodestone-launcher.exe>
#endif
#ifndef Build
  #error Pass /DBuild=<build number>
#endif
#ifndef Version
  #define Version "0.1.0"
#endif
#ifndef OutputDir
  #define OutputDir "."
#endif
#ifndef OutputName
  #define OutputName "LodestoneLauncherSetup"
#endif

#define AppName "Lodestone Launcher"
#define ExeName "lodestone-launcher.exe"
; Shown in Settings > Apps. The launcher keeps it current after it updates itself
; (launcher-core/src/update.rs, refresh_uninstall_entry), in the same format.
#define DisplayVersion Version + " (build " + Build + ")"

[Setup]
; Never change AppId: Windows uses it to find an existing install to upgrade, and
; the launcher uses it to find its Settings > Apps entry.
AppId={{19BD6256-FCCA-4D23-B640-06275F57BD67}
AppName={#AppName}
AppVersion={#DisplayVersion}
AppVerName={#AppName} {#DisplayVersion}
AppPublisher=Lodestone Team
AppPublisherURL=https://github.com/Lodestone-Team/lodestone
AppSupportURL=https://github.com/Lodestone-Team/lodestone/issues
AppUpdatesURL=https://github.com/Lodestone-Team/lodestone/releases?q=LodestoneLauncherSetup.exe&expanded=true
VersionInfoVersion={#Version}.{#Build}
VersionInfoProductName={#AppName}
VersionInfoDescription={#AppName} Setup

; Per user, no administrator rights, and no way to switch to a per-machine install
; (it would break updates).
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=
; %LOCALAPPDATA%\Programs\Lodestone Launcher by default. The folder page shows on a
; first install only: an upgrade goes where the existing install is.
DefaultDirName={autopf}\{#AppName}
DisableDirPage=auto
DisableProgramGroupPage=yes
DisableReadyPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0

; A running launcher is closed (and reopened afterwards) when installing over it.
CloseApplications=yes
RestartApplications=no

OutputDir={#OutputDir}
OutputBaseFilename={#OutputName}
SetupIconFile=lodestone.ico
UninstallDisplayIcon={app}\lodestone.ico
UninstallDisplayName={#AppName}
WizardStyle=modern
; The strongest LZMA2 settings: about 1% smaller than lzma2/max for the launcher,
; for a few more seconds of compiling. A larger dictionary gains nothing, since it
; already holds the whole executable.
Compression=lzma2/ultra64
LZMANumFastBytes=273
LZMAUseSeparateProcess=yes
SolidCompression=yes

[Messages]
SelectDirBrowseLabel=To continue, click Next. To install somewhere else, such as a games folder on another drive, click Browse.
ConfirmUninstall=Are you sure you want to remove %1?%n%nYour instances, worlds and accounts are kept. At the end you can choose to delete them too.

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; DestName: "{#ExeName}"; Flags: ignoreversion
Source: "lodestone.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#ExeName}"; IconFilename: "{app}\lodestone.ico"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#ExeName}"; IconFilename: "{app}\lodestone.ico"; Tasks: desktopicon

[Registry]
; The lodestone:// URL scheme, for this user only (HKA is HKEY_CURRENT_USER in a
; per-user install): a friend's invite page opens lodestone://join?invite=… in the
; launcher, which gets the link as its first argument. A launcher already running is
; brought forward and takes the link (see the launcher's deep_link module).
Root: HKA; Subkey: "Software\Classes\lodestone"; ValueType: string; ValueName: ""; ValueData: "URL:Lodestone Launcher"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\lodestone"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\lodestone\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\lodestone.ico"
Root: HKA; Subkey: "Software\Classes\lodestone\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#ExeName}"" ""%1"""

[Run]
Filename: "{app}\{#ExeName}"; Description: "Open {#AppName}"; Flags: nowait postinstall skipifsilent

[InstallDelete]
; What an update the launcher was in the middle of left behind.
Type: files; Name: "{app}\{#ExeName}.old"
Type: files; Name: "{app}\{#ExeName}.update"
Type: files; Name: "{app}\{#ExeName}.update.part"

[UninstallDelete]
; Files the updater makes next to the executable, which the installer didn't.
Type: files; Name: "{app}\{#ExeName}.old"
Type: files; Name: "{app}\{#ExeName}.update"
Type: files; Name: "{app}\{#ExeName}.update.part"
Type: dirifempty; Name: "{app}"

#include "shared.iss"

[Code]
function CredDelete(TargetName: String; CredType: DWORD; Flags: DWORD): BOOL;
  external 'CredDeleteW@advapi32.dll stdcall';

const
  CRED_TYPE_GENERIC = 1;

function DataFolder: String;
begin
  { launcher-control/src/paths.rs: LODESTONE_LAUNCHER_DATA_DIR, else %APPDATA%. }
  Result := EnvOr('LODESTONE_LAUNCHER_DATA_DIR', ExpandConstant('{userappdata}\{#AppName}'));
end;

procedure InitializeWizard;
begin
  AddDataLocationNote('Your instances, worlds and accounts are kept in ' + DataFolder +
    ', not in this folder.');
end;

function NextButtonClick(CurPageID: Integer): Boolean;
begin
  Result := CheckWritableDir(CurPageID);
end;

{ A running launcher would keep its executable from being deleted, and could write
  its data again after it was deleted. }
function InitializeUninstall: Boolean;
begin
  Result := True;
  while Result and CheckForMutexes('Local\LodestoneLauncher.SingleInstance') do
    Result := MsgBox('{#AppName} is open. Close it, then click OK to uninstall it.',
      mbError, MB_OKCANCEL) = IDOK;
end;

procedure RegisterKeptData;
begin
  AddKeptData('Instances, worlds and accounts', DataFolder,
    'Your instances with their worlds, mods, resource packs and screenshots, your ' +
    'accounts and settings, and downloaded game files. Deleting it also signs your ' +
    'Microsoft accounts out of this computer.');
end;

{ The Microsoft accounts' tokens are in Windows' Credential Manager, not the data
  folder: one generic credential per account, "zed:url=lodestone-launcher:<id>"
  (launcher-core/src/accounts.rs's secret_key, as GPUI's write_credentials names
  it). The ids come from accounts.json, before it is deleted. }
procedure BeforeDeletingKeptData(Index: Integer);
var
  Json: AnsiString;
  Rest, Id: String;
  At, Stop: Integer;
begin
  if not LoadStringFromFile(AddBackslash(KeptFolders[Index]) + 'accounts.json', Json) then
    Exit;
  Rest := String(Json);
  At := Pos('"microsoft:', Rest);
  while At > 0 do
  begin
    Rest := Copy(Rest, At + 1, Length(Rest));
    Stop := Pos('"', Rest);
    if Stop = 0 then
      Break;
    Id := Copy(Rest, 1, Stop - 1);
    Rest := Copy(Rest, Stop + 1, Length(Rest));
    if CredDelete('zed:url=lodestone-launcher:' + Id, CRED_TYPE_GENERIC, 0) then
      Log('Removed the saved sign-in of ' + Id);
    At := Pos('"microsoft:', Rest);
  end;
end;
