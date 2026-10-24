#ifndef AppVersion
  #define AppVersion "0.1.0-alpha.1"
#endif

[Setup]
AppId={{507D6630-798C-47E4-9769-E9B90BFC74D5}
AppName=Rusty VTT Alpha
AppVersion={#AppVersion}
AppPublisher=Rusty VTT
AppPublisherURL=https://github.com/Virlez/rusty-vtt
DefaultDirName={localappdata}\Programs\Rusty VTT
DefaultGroupName=Rusty VTT
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir=..\..\dist
OutputBaseFilename=Rusty-VTT-{#AppVersion}-windows-x64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
LicenseFile=..\..\LICENSE
InfoBeforeFile=alpha.txt
UninstallDisplayIcon={app}\rusty-vtt.exe
CloseApplications=yes

[Languages]
Name: "french"; MessagesFile: "compiler:Languages\French.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Créer un raccourci sur le bureau"; Flags: unchecked

[Files]
Source: "..\..\target\release\rusty-vtt.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "Launch-RustyVTT.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\THIRD_PARTY_NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "alpha.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Rusty VTT Alpha"; Filename: "{sys}\WindowsPowerShell\v1.0\powershell.exe"; Parameters: "-NoLogo -NoProfile -ExecutionPolicy Bypass -File ""{app}\Launch-RustyVTT.ps1"""; WorkingDir: "{app}"; IconFilename: "{app}\rusty-vtt.exe"
Name: "{autodesktop}\Rusty VTT Alpha"; Filename: "{sys}\WindowsPowerShell\v1.0\powershell.exe"; Parameters: "-NoLogo -NoProfile -ExecutionPolicy Bypass -File ""{app}\Launch-RustyVTT.ps1"""; WorkingDir: "{app}"; IconFilename: "{app}\rusty-vtt.exe"; Tasks: desktopicon

[Run]
Filename: "{sys}\WindowsPowerShell\v1.0\powershell.exe"; Parameters: "-NoLogo -NoProfile -ExecutionPolicy Bypass -File ""{app}\Launch-RustyVTT.ps1"""; Description: "Lancer Rusty VTT Alpha"; Flags: nowait postinstall skipifsilent
