#define TapSendVersion "1.0.1"

[Setup]
AppId={{0F26897D-BA60-49B8-8342-9831DCDFBA50}
AppName=TapSend
AppVersion={#TapSendVersion}
AppPublisher=TapSend
DefaultDirName={localappdata}\Programs\TapSend
DefaultGroupName=TapSend
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#AddBackslash(SourcePath)}..\dist
OutputBaseFilename=TapSend-v{#TapSendVersion}-windows-x64-setup
Compression=lzma
SolidCompression=yes
ChangesEnvironment=yes

[Files]
Source: "{#AddBackslash(SourcePath)}..\dist\windows\tapsend.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#AddBackslash(SourcePath)}..\dist\windows\tabsend.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#AddBackslash(SourcePath)}..\dist\windows\tapsend-tui.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#AddBackslash(SourcePath)}..\dist\windows\tapsend-server.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#AddBackslash(SourcePath)}README.release.md"; DestDir: "{app}"; DestName: "README.md"; Flags: ignoreversion
Source: "{#AddBackslash(SourcePath)}..\release.md"; DestDir: "{app}"; DestName: "release.md"; Flags: ignoreversion
Source: "{#AddBackslash(SourcePath)}licenses\go-qrcode.LICENSE"; DestDir: "{app}\licenses"; Flags: ignoreversion

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional icons:"

[Icons]
Name: "{group}\TapSend"; Filename: "{app}\tapsend.exe"; WorkingDir: "{userdocs}"
Name: "{autodesktop}\TapSend"; Filename: "{app}\tapsend.exe"; WorkingDir: "{userdocs}"; Tasks: desktopicon

[Code]
function InPath(const Value, Directory: String): Boolean;
begin
  Result := Pos(';' + Uppercase(Directory) + ';', ';' + Uppercase(Value) + ';') > 0;
end;

function WithoutPath(const Value, Directory: String): String;
var
  StartAt, SeparatorAt: Integer;
  Segment: String;
begin
  Result := '';
  StartAt := 1;
  while StartAt <= Length(Value) do begin
    SeparatorAt := Pos(';', Copy(Value, StartAt, Length(Value) - StartAt + 1));
    if SeparatorAt = 0 then SeparatorAt := Length(Value) - StartAt + 2;
    Segment := Copy(Value, StartAt, SeparatorAt - 1);
    if (Segment <> '') and (Uppercase(Segment) <> Uppercase(Directory)) then begin
      if Result <> '' then Result := Result + ';';
      Result := Result + Segment;
    end;
    StartAt := StartAt + SeparatorAt;
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Existing, Directory: String;
begin
  if CurStep <> ssPostInstall then Exit;
  Directory := ExpandConstant('{app}');
  if not RegQueryStringValue(HKCU, 'Environment', 'Path', Existing) then Existing := '';
  if not InPath(Existing, Directory) then begin
    if Existing <> '' then Existing := Existing + ';';
    if RegWriteExpandStringValue(HKCU, 'Environment', 'Path', Existing + Directory) then
      RegWriteStringValue(HKCU, 'Software\TapSend', 'PathAdded', Directory);
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Existing, Directory, AddedDirectory: String;
begin
  if CurUninstallStep <> usPostUninstall then Exit;
  Directory := ExpandConstant('{app}');
  if RegQueryStringValue(HKCU, 'Software\TapSend', 'PathAdded', AddedDirectory) and
    (Uppercase(AddedDirectory) = Uppercase(Directory)) then begin
    if RegQueryStringValue(HKCU, 'Environment', 'Path', Existing) and InPath(Existing, Directory) then
      RegWriteExpandStringValue(HKCU, 'Environment', 'Path', WithoutPath(Existing, Directory));
    RegDeleteValue(HKCU, 'Software\TapSend', 'PathAdded');
    RegDeleteKeyIfEmpty(HKCU, 'Software\TapSend');
  end;
end;
