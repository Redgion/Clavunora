; SPDX-License-Identifier: GPL-3.0-only
; Copyright (C) 2026 redgion and Clavunora contributors
#ifndef AppVersion
  #define AppVersion "1.0.0-rc.1"
#endif
[Setup]
AppId={{607B1275-3977-4FC5-A49F-3C00CB5791F7}
AppName=Clavunora
AppVersion={#AppVersion}
AppVerName=Clavunora {#AppVersion}
DefaultDirName={localappdata}\Programs\ClavunoraRust
DefaultGroupName=Clavunora
PrivilegesRequired=lowest
OutputDir=..\..\dist
OutputBaseFilename=Clavunora-{#AppVersion}-windows-x64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
SetupIconFile=..\..\pianoflow\resources\icons\pianoflow.ico
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\clavunora-rust.exe
LicenseFile=..\..\LICENSE
[Files]
Source: "..\..\dist\Clavunora-Windows\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
[Icons]
Name: "{group}\Clavunora"; Filename: "{app}\clavunora-rust.exe"
Name: "{autodesktop}\Clavunora"; Filename: "{app}\clavunora-rust.exe"; Tasks: desktopicon
[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked
[Run]
Filename: "{app}\clavunora-rust.exe"; Description: "Launch Clavunora"; Flags: nowait postinstall skipifsilent
