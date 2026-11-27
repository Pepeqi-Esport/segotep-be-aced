; BeAced Windows installer (Inno Setup 7; also works with 6)
; Build: cargo release   (or .\scripts\build-installer.ps1)
; Compile twice with /DMyArch=64 and /DMyArch=86.
; Expects portable binaries:
;   dist\segotep-be-aced-{version}_portable_windows_64.exe
;   dist\segotep-be-aced-{version}_portable_windows_86.exe
; Installed process name stays segotep-be-aced.exe (shortcuts / taskkill / autostart).

#define MyAppName "BeAced"
#ifndef MyAppVersion
  #define MyAppVersion "1.0.0"
#endif
#ifndef MyArch
  #define MyArch "64"
#endif
#define MyAppPublisher "Segotep BeAced (community)"
#define MyAppExeName "segotep-be-aced.exe"
#define MyPortableName "segotep-be-aced-" + MyAppVersion + "_portable_windows_" + MyArch + ".exe"
#define MySetupName "segotep-be-aced-" + MyAppVersion + "_installer_windows_" + MyArch

[Setup]
#if MyArch == "64"
AppId={{A8F3C2E1-7B4D-4E9A-9C1F-2D6E8B0A5F47}
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#else
AppId={{B9E4D3F2-8C5E-4F0B-0D2A-3E7F9C1B6A58}
ArchitecturesAllowed=x86compatible
#endif
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion} ({#MyArch}-bit)
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
; User picks the install folder on this page.
DisableDirPage=no
PrivilegesRequired=admin
PrivilegesRequiredOverridesAllowed=dialog
OutputDir=..\dist
OutputBaseFilename={#MySetupName}
SetupIconFile=app.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
MinVersion=10.0
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
; One arch per installer; install as segotep-be-aced.exe.
Source: "..\dist\{#MyPortableName}"; DestDir: "{app}"; DestName: "{#MyAppExeName}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent runascurrentuser

[UninstallDelete]
; Runtime extracts (sensor-bridge + deps) and config live beside the exe.
Type: filesandordirs; Name: "{app}"

[Code]
function InitializeUninstall(): Boolean;
var
  ResultCode: Integer;
begin
  // Stop a running instance so files can be removed.
  Exec('taskkill.exe', '/F /IM {#MyAppExeName}', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec('taskkill.exe', '/F /IM sensor-bridge.exe', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  // LHM kernel driver locks sensor-bridge.sys until unloaded.
  Exec('sc.exe', 'stop R0sensor-bridge', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec('sc.exe', 'delete R0sensor-bridge', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Result := True;
end;
