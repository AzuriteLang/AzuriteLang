; AzuriteLang Compiler Installer
; Requires Inno Setup 6+ (https://jrsoftware.org/isdl.php)

#define MyAppName "AzuriteLang"
#define MyAppVersion "0.1.0"
#define MyAppPublisher "AzuriteLang"
#define MyAppURL "https://github.com/AzuriteLang"
#define MyAppExeName "azurite.exe"

[Setup]
AppId={{8A7E3C2F-1D5B-4A9E-8C6F-3B2D1A5E7F90}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
PrivilegesRequired=admin
PrivilegesRequiredOverridesAllowed=dialog
OutputDir=.\output
OutputBaseFilename=AzuriteLang-{#MyAppVersion}-setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
DisableProgramGroupPage=yes
ChangesEnvironment=yes
UninstallDisplayIcon={app}\{#MyAppExeName}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "french"; MessagesFile: "compiler:Languages\French.isl"

[Tasks]
Name: "addtopath"; Description: "Add Azurite to &PATH (system-wide)"; Flags: checkedonce
Name: "associate"; Description: "Associate &.az files with Azurite"; Flags: checkedonce
Name: "desktopicon"; Description: "Create &desktop shortcut"; GroupDescription: "Additional shortcuts:"; Flags: checkedonce

[Files]
; Main binary
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion

; LLVM runtime DLLs (bundled by build script)
Source: "..\target\release\LLVM-C.dll"; DestDir: "{app}"; Flags: ignoreversion; Check: LLVMDllExists
Source: "..\target\release\clang.exe"; DestDir: "{app}"; Flags: ignoreversion; Check: ClangExists
Source: "..\target\release\lld.exe"; DestDir: "{app}"; Flags: ignoreversion; Check: LldExists

; Example files
Source: "..\main.az"; DestDir: "{app}\examples"; Flags: ignoreversion

; VS Code extension
Source: "..\azurite-vscode\*.vsix"; DestDir: "{app}\vscode"; Flags: ignoreversion; Check: VsixExists

[Icons]
Name: "{group}\AzuriteLang REPL"; Filename: "{app}\{#MyAppExeName}"; Parameters: "repl"; WorkingDir: "{app}"
Name: "{group}\AzuriteLang Docs"; Filename: "https://azuritelang.github.io"
Name: "{group}\Uninstall AzuriteLang"; Filename: "{uninstallexe}"
Name: "{autodesktop}\AzuriteLang REPL"; Filename: "{app}\{#MyAppExeName}"; Parameters: "repl"; WorkingDir: "{app}"; Tasks: desktopicon

[Registry]
; .az file association
Root: HKLM; Subkey: "Software\Classes\.az"; ValueType: string; ValueName: ""; ValueData: "AzuriteLang.File"; Flags: uninsdeletekey; Tasks: associate
Root: HKLM; Subkey: "Software\Classes\AzuriteLang.File"; ValueType: string; ValueName: ""; ValueData: "Azurite Source File"; Flags: uninsdeletekey; Tasks: associate
Root: HKLM; Subkey: "Software\Classes\AzuriteLang.File\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#MyAppExeName},0"; Tasks: associate
Root: HKLM; Subkey: "Software\Classes\AzuriteLang.File\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" build ""%1"""; Tasks: associate

[Run]
; Install VS Code extension
Filename: "code"; Parameters: "--install-extension ""{app}\vscode\*.vsix"""; Flags: runascurrentuser skipifdoesntexist; Description: "Install VS Code extension"
; Show readme
Filename: "{app}\examples\main.az"; Flags: postinstall shellexec skipifsilent; Description: "Open example file"

[Code]
var
  AddPathToAllUsers: Boolean;

function LLVMDllExists: Boolean;
begin
  Result := FileExists(ExpandConstant('{app}\LLVM-C.dll'));
end;

function ClangExists: Boolean;
begin
  Result := FileExists(ExpandConstant('{app}\clang.exe'));
end;

function LldExists: Boolean;
begin
  Result := FileExists(ExpandConstant('{app}\lld.exe'));
end;

function VsixExists: Boolean;
begin
  Result := FileExists(ExpandConstant('{app}\vscode\*.vsix'));
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  PathStr: String;
begin
  if (CurStep = ssPostInstall) and WizardIsTaskSelected('addtopath') then
  begin
    PathStr := ExpandConstant('{app}');
    if not PathIsInSystemPath(PathStr) then
      AddToSystemPath(PathStr);
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
  begin
    if PathIsInSystemPath(ExpandConstant('{app}')) then
      RemoveFromSystemPath(ExpandConstant('{app}'));
  end;
end;

function PathIsInSystemPath(Path: String): Boolean;
var
  EnvPath: String;
  P: Integer;
begin
  Result := False;
  if not RegQueryStringValue(HKEY_LOCAL_MACHINE, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', EnvPath) then
    Exit;
  P := Pos(UpperCase(Path), UpperCase(EnvPath));
  Result := (P > 0) and ((P = 1) or (EnvPath[P - 1] = ';')) and ((P + Length(Path) - 1 = Length(EnvPath)) or (EnvPath[P + Length(Path)] = ';'));
end;

procedure AddToSystemPath(Path: String);
var
  EnvPath: String;
begin
  if RegQueryStringValue(HKEY_LOCAL_MACHINE, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', EnvPath) then
  begin
    EnvPath := EnvPath + ';' + Path;
    RegWriteStringValue(HKEY_LOCAL_MACHINE, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', EnvPath);
  end;
end;

procedure RemoveFromSystemPath(Path: String);
var
  EnvPath, NewPath: String;
  P: Integer;
begin
  if RegQueryStringValue(HKEY_LOCAL_MACHINE, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', EnvPath) then
  begin
    P := Pos(UpperCase(Path) + ';', UpperCase(EnvPath) + ';');
    if P > 0 then
    begin
      NewPath := EnvPath;
      Delete(NewPath, P, Length(Path) + 1);
      RegWriteStringValue(HKEY_LOCAL_MACHINE, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', NewPath);
    end;
  end;
end;
