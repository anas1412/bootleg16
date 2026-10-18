; Windows installer (Inno Setup 6). Built by .github/workflows/installers.yml, or by hand:
;   iscc /DVersion=0.1.0 /DStage=..\..\dist\win packaging\windows\bootleg16.iss
; Stage holds bootleg16.exe, steam_api64.dll (if the build has Steam) and data\ (the game data,
; private until it is our own). Installs per user: no admin prompt, Start menu entry, uninstaller.
#ifndef Version
  #define Version "0.1.0"
#endif
#ifndef Stage
  #define Stage "..\..\dist\win"
#endif

[Setup]
AppName=bootleg16
AppVersion={#Version}
AppPublisher=bootleg16
DefaultDirName={autopf}\bootleg16
DefaultGroupName=bootleg16
PrivilegesRequired=lowest
OutputDir=..\..\dist
OutputBaseFilename=bootleg16-{#Version}-windows-setup
SetupIconFile=bootleg16.ico
UninstallDisplayIcon={app}\bootleg16.ico
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
WizardStyle=modern
DisableProgramGroupPage=yes

[Tasks]
Name: desktopicon; Description: "Create a desktop shortcut"; Flags: unchecked

[Files]
Source: "{#Stage}\bootleg16.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "bootleg16.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Stage}\steam_api64.dll"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "{#Stage}\data\*"; DestDir: "{app}\data"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\bootleg16"; Filename: "{app}\bootleg16.exe"; WorkingDir: "{app}"; IconFilename: "{app}\bootleg16.ico"
Name: "{autodesktop}\bootleg16"; Filename: "{app}\bootleg16.exe"; WorkingDir: "{app}"; IconFilename: "{app}\bootleg16.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\bootleg16.exe"; Description: "Start bootleg16"; Flags: nowait postinstall skipifsilent
; Settings live in %APPDATA%\bootleg16 and stay when the game is uninstalled.
