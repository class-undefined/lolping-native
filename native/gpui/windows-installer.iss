; Parameters are supplied by bundle_windows.py. Install per-user without elevation.
[Setup]
AppId={{046C93EF-2BFA-4DD4-94A1-4C5B94F90410}
AppName=lolPing Native
AppVersion={#AppVersion}
DefaultDirName={localappdata}\Programs\lolPing Native
DefaultGroupName=lolPing Native
PrivilegesRequired=lowest
ArchitecturesAllowed={#InstallerArchitecture}
ArchitecturesInstallIn64BitMode={#InstallerArchitecture}
MinVersion={#MinimumWindowsVersion}
OutputDir={#OutputDir}
OutputBaseFilename=lolPing-Native-{#AppVersion}-windows-{#AppArchitecture}-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\lolping-native.exe
CloseApplications=yes
RestartApplications=no
LicenseFile={#SourceDir}\LICENSE

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\lolPing Native"; Filename: "{app}\lolping-native.exe"
Name: "{group}\Uninstall lolPing Native"; Filename: "{uninstallexe}"

[Run]
Filename: "{app}\lolping-native.exe"; Description: "Launch lolPing Native"; Flags: nowait postinstall skipifsilent
