; The Windows installer for Lodestone Launcher (Inno Setup 6).
;
; Built by launcher/scripts/publish_release.py, which passes the binary and its
; build number:
;
;   ISCC /DSourceExe=<path\to\lodestone-launcher.exe> /DBuild=531 /DVersion=0.1.0
;        /DOutputDir=<folder> /DOutputName=<file name without .exe> lodestone-launcher.iss
;
; It installs for the current user only, under %LOCALAPPDATA%\Programs, without
; asking for administrator rights. That matters for updates: the launcher updates
; itself by replacing its own executable (launcher-core/src/update.rs), which it
; can only do in a folder the user can write to. Program Files would need
; administrator rights for every update.
;
; The launcher's data (instances, worlds, accounts) is in %APPDATA%\Lodestone
; Launcher, a different folder. Installing and uninstalling never touch it.

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
AppUpdatesURL=https://github.com/Lodestone-Team/lodestone/releases/tag/launcher-latest
VersionInfoVersion={#Version}.{#Build}
VersionInfoProductName={#AppName}
VersionInfoDescription={#AppName} Setup

; Per user, no administrator rights, and no way to switch to a per-machine install
; (it would break updates).
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=
DefaultDirName={autopf}\{#AppName}
DisableDirPage=yes
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
Compression=lzma2/max
SolidCompression=yes

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; DestName: "{#ExeName}"; Flags: ignoreversion
Source: "lodestone.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#ExeName}"; IconFilename: "{app}\lodestone.ico"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#ExeName}"; IconFilename: "{app}\lodestone.ico"; Tasks: desktopicon

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
