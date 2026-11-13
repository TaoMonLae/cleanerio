; Inno Setup script for the Cleanerio installer. Built by package_windows.ps1, which passes
; the version:  iscc /DAppVersion=0.2.0 assets\windows\cleanerio.iss
#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{6B0E2B7C-5C0E-4C61-9F0B-2D0B1C4E7A11}
AppName=Cleanerio
AppVersion={#AppVersion}
AppPublisher=Min Htet Min Aung
AppPublisherURL=https://github.com/TaoMonLae/cleanerio
DefaultDirName={autopf}\Cleanerio
DefaultGroupName=Cleanerio
DisableProgramGroupPage=yes
; installs for the current user without asking for an administrator
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
LicenseFile=..\..\LICENSE
SetupIconFile=..\icon.ico
UninstallDisplayIcon={app}\Cleanerio.exe
OutputDir=..\..\dist
OutputBaseFilename=Cleanerio-{#AppVersion}-windows-x64-setup
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked

[Files]
Source: "..\..\dist\windows\Cleanerio.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "..\..\THIRD-PARTY-NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Cleanerio"; Filename: "{app}\Cleanerio.exe"
Name: "{autodesktop}\Cleanerio"; Filename: "{app}\Cleanerio.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\Cleanerio.exe"; Description: "Open Cleanerio"; Flags: nowait postinstall skipifsilent
