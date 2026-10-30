#ifndef AppVersion
  #define AppVersion "0.4.0"
#endif
[Setup]
AppId={{A4B8E289-F1D3-4D8D-B1D1-623C020ED582}
AppName=Clavunora
AppVersion={#AppVersion}
DefaultDirName={localappdata}\Programs\Clavunora
DefaultGroupName=Clavunora
UsePreviousGroup=no
PrivilegesRequired=lowest
OutputDir=..\..\dist
OutputBaseFilename=Clavunora-{#AppVersion}-windows-x64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
SetupIconFile=..\..\pianoflow\resources\icons\pianoflow.ico
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\Clavunora.exe
[InstallDelete]
Type: files; Name: "{app}\PianoFlow.exe"
Type: files; Name: "{userprograms}\PianoFlow\PianoFlow.lnk"
Type: dirifempty; Name: "{userprograms}\PianoFlow"
Type: files; Name: "{userdesktop}\PianoFlow.lnk"
[Files]
Source: "..\..\dist\Clavunora\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
[Icons]
Name: "{group}\Clavunora"; Filename: "{app}\Clavunora.exe"
Name: "{autodesktop}\Clavunora"; Filename: "{app}\Clavunora.exe"; Tasks: desktopicon
[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked
[Run]
Filename: "{app}\Clavunora.exe"; Description: "Launch Clavunora"; Flags: nowait postinstall skipifsilent
