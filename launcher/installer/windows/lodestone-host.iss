; The Windows installer for Lodestone Host (Inno Setup 6).
;
; Built by `launcher/scripts/publish_release.py --product host`, which passes the
; Host, the core it runs and the build number:
;
;   ISCC /DSourceExe=<path\to\lodestone-host.exe> /DSourceCore=<path\to\lodestone_core.exe>
;        /DBuild=531 /DVersion=0.1.0 /DOutputDir=<folder>
;        /DOutputName=<file name without .exe> lodestone-host.iss
;
; It installs for the current user only, without asking for administrator rights,
; under %LOCALAPPDATA%\Programs unless the person chooses another folder (another
; drive, say), so the Host can update itself in place: it replaces its own
; executable and the core's (launcher-core/src/update.rs). Setup refuses a folder
; the user can't write to (shared.iss). Upgrading keeps the existing install's folder.
;
; The Host runs Lodestone Core (lodestone_core.exe) as a child process, from the
; same folder. The core's data (servers, worlds, users) is in %USERPROFILE%\.lodestone,
; and the Host's own (settings, logs) in %APPDATA%\Lodestone Host. Installing never
; touches either; uninstalling keeps both, and at the end says so and offers to
; delete them (shared.iss).

#ifndef SourceExe
  #error Pass /DSourceExe=<path to lodestone-host.exe>
#endif
#ifndef SourceCore
  #error Pass /DSourceCore=<path to lodestone_core.exe>
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
  #define OutputName "LodestoneHostSetup"
#endif

#define AppName "Lodestone Host"
#define ExeName "lodestone-host.exe"
#define CoreName "lodestone_core.exe"
; Shown in Settings > Apps. The Host keeps it current after it updates itself
; (launcher-core/src/update.rs, refresh_uninstall_entry_for), in the same format.
#define DisplayVersion Version + " (build " + Build + ")"

[Setup]
; Never change AppId: Windows uses it to find an existing install to upgrade, and
; the Host uses it to find its Settings > Apps entry (HOST_WINDOWS_APP_ID).
AppId={{6F0B2C55-4E0A-4D6B-9C8E-3B1A7D2E5F40}
AppName={#AppName}
AppVersion={#DisplayVersion}
AppVerName={#AppName} {#DisplayVersion}
AppPublisher=Lodestone Team
AppPublisherURL=https://lodestone.cc
AppSupportURL=https://github.com/Lodestone-Team/lodestone/issues
AppUpdatesURL=https://github.com/Lodestone-Team/lodestone/releases?q=host-v&expanded=true
VersionInfoVersion={#Version}.{#Build}
VersionInfoProductName={#AppName}
VersionInfoDescription={#AppName} Setup

; Per user, no administrator rights, and no way to switch to a per-machine install
; (it would break updates).
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=
DefaultDirName={autopf}\{#AppName}
; The folder page shows on a first install only: an upgrade goes where the existing
; install is.
DisableDirPage=auto
DisableProgramGroupPage=yes
DisableReadyPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0

; A running Host has servers running. Setup never closes it (that would stop them
; without warning): it finds the Host's single-instance mutex
; (lodestone-host/src/single_instance.rs) and asks the person to quit it first, from
; its tray icon, which stops the servers cleanly.
AppMutex=Local\LodestoneHost.SingleInstance
CloseApplications=no
RestartApplications=no

OutputDir={#OutputDir}
OutputBaseFilename={#OutputName}
SetupIconFile=host.ico
UninstallDisplayIcon={app}\host.ico
UninstallDisplayName={#AppName}
WizardStyle=modern
Compression=lzma2/ultra64
LZMANumFastBytes=273
LZMAUseSeparateProcess=yes
SolidCompression=yes

[Messages]
SelectDirBrowseLabel=To continue, click Next. To install somewhere else, such as a folder on another drive, click Browse.
ConfirmUninstall=Are you sure you want to remove %1?%n%nYour servers and worlds are kept. At the end you can choose to delete them too.

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; DestName: "{#ExeName}"; Flags: ignoreversion
Source: "{#SourceCore}"; DestDir: "{app}"; DestName: "{#CoreName}"; Flags: ignoreversion
Source: "host.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#ExeName}"; IconFilename: "{app}\host.ico"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#ExeName}"; IconFilename: "{app}\host.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\{#ExeName}"; Description: "Open {#AppName}"; Flags: nowait postinstall skipifsilent

[InstallDelete]
; What an update the Host was in the middle of left behind.
Type: files; Name: "{app}\{#ExeName}.old"
Type: files; Name: "{app}\{#ExeName}.update"
Type: files; Name: "{app}\{#ExeName}.update.part"
Type: files; Name: "{app}\{#CoreName}.old"
Type: files; Name: "{app}\{#CoreName}.update"
Type: files; Name: "{app}\{#CoreName}.update.part"
Type: files; Name: "{app}\{#CoreName}.previous"

[UninstallDelete]
; Files the updater makes next to the executables, which the installer didn't.
Type: files; Name: "{app}\{#ExeName}.old"
Type: files; Name: "{app}\{#ExeName}.update"
Type: files; Name: "{app}\{#ExeName}.update.part"
Type: files; Name: "{app}\{#CoreName}.old"
Type: files; Name: "{app}\{#CoreName}.update"
Type: files; Name: "{app}\{#CoreName}.update.part"
Type: files; Name: "{app}\{#CoreName}.previous"
Type: dirifempty; Name: "{app}"

#include "shared.iss"

[Code]
{ lodestone-host/src/core_process.rs: data_dir and host_dir. }
function CoreDataFolder: String;
begin
  Result := EnvOr('LODESTONE_PATH', ExpandConstant('{%USERPROFILE}\.lodestone'));
end;

function HostDataFolder: String;
begin
  Result := EnvOr('LODESTONE_HOST_DATA_DIR', ExpandConstant('{userappdata}\{#AppName}'));
end;

procedure InitializeWizard;
begin
  AddDataLocationNote('Your servers and worlds are kept in ' + CoreDataFolder +
    ', not in this folder.');
end;

function NextButtonClick(CurPageID: Integer): Boolean;
begin
  Result := CheckWritableDir(CurPageID);
end;

procedure RegisterKeptData;
begin
  AddKeptData('Servers and worlds', CoreDataFolder,
    'Your Minecraft servers with their worlds, mods and backups, and Lodestone''s users ' +
    'and settings. The older Lodestone app, and Lodestone Core if you run it yourself, ' +
    'use this folder too.');
  AddKeptData('Host settings and logs', HostDataFolder,
    'The Host''s own settings and logs. Nothing in it is needed to keep your servers.');
end;

procedure BeforeDeletingKeptData(Index: Integer);
begin
end;
