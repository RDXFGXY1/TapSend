#ifndef TapSendVersion
  #define TapSendVersion "1.0.2"
#endif
#ifndef TapSendRepo
  #define TapSendRepo "RDXFGXY1/TapSend"
#endif
#ifndef BundleSHA256
  #error Build the matching ZIP and pass its SHA-256 using packaging/build-windows-installer.ps1.
#endif
#define BundleName "TapSend-v" + TapSendVersion + "-windows-x64.zip"
#define BundleURL "https://github.com/" + TapSendRepo + "/releases/download/v" + TapSendVersion + "/" + BundleName

[Setup]
AppId={{0F26897D-BA60-49B8-8342-9831DCDFBA50}
AppName=TapSend
AppVersion={#TapSendVersion}
AppPublisher=TapSend
DefaultDirName=C:\TapSend
DisableDirPage=no
DefaultGroupName=TapSend
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#AddBackslash(SourcePath)}..\dist
OutputBaseFilename=TapSend-v{#TapSendVersion}-windows-x64-setup
ArchiveExtraction=full
WizardStyle=modern dark hidebevels
WizardBackColor=#0F100F
#if FileExists(AddBackslash(SourcePath) + "tapsend.ico")
SetupIconFile={#AddBackslash(SourcePath)}tapsend.ico
#endif
AppMutex=Local\TapSendDesktopInstance
UninstallDisplayIcon={app}\TapSend-Desktop.exe
ChangesEnvironment=yes

[Files]
Source: "{tmp}\{#BundleName}"; DestDir: "{app}"; Flags: external extractarchive recursesubdirs createallsubdirs ignoreversion; Check: BundleReady

[Tasks]
Name: "desktopicon"; Description: "{cm:DesktopShortcut}"; Flags: unchecked
Name: "clipath"; Description: "{cm:CommandShortcut}"; Flags: unchecked

[Icons]
Name: "{group}\TapSend"; Filename: "{app}\TapSend-Desktop.exe"; WorkingDir: "{userdocs}"; IconFilename: "{code:ShortcutIcon}"
Name: "{autodesktop}\TapSend"; Filename: "{app}\TapSend-Desktop.exe"; WorkingDir: "{userdocs}"; Tasks: desktopicon; IconFilename: "{code:ShortcutIcon}"

[Run]
Filename: "{app}\TapSend-Desktop.exe"; Description: "{cm:OpenTapSend}"; Flags: postinstall nowait skipifsilent unchecked; Check: PreferencesReady

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "arabic"; MessagesFile: "compiler:Languages\Arabic.isl"

[Messages]
english.ButtonBack=[ &Back ]
english.ButtonNext=[ &Next ]
english.ButtonInstall=[ &Install ]
english.ButtonFinish=[ &Finish ]
english.ButtonCancel=[ Cancel ]
arabic.ButtonBack=[ رجوع ]
arabic.ButtonNext=[ التالي ]
arabic.ButtonInstall=[ تثبيت ]
arabic.ButtonFinish=[ إنهاء ]
arabic.ButtonCancel=[ إلغاء ]

[CustomMessages]
english.DesktopShortcut=Create a desktop shortcut
arabic.DesktopShortcut=إنشاء اختصار على سطح المكتب
english.CommandShortcut=Add TapSend commands to PATH
arabic.CommandShortcut=إضافة أوامر TapSend إلى PATH
english.OpenTapSend=Open TapSend
arabic.OpenTapSend=فتح TapSend
english.Destination=Destination
arabic.Destination=مجلد التثبيت
english.ProgramFolder=Program folder
arabic.ProgramFolder=مجلد البرنامج
english.ChooseProgram=Choose where to install the TapSend program files.
arabic.ChooseProgram=اختر مجلد تثبيت ملفات برنامج TapSend.
english.FolderExists=Folder exists
arabic.FolderExists=هل المجلد موجود؟
english.ExistsYes=Yes
arabic.ExistsYes=نعم
english.ExistsNo=No
arabic.ExistsNo=لا
english.Source=Downloads from GitHub Releases when you click Install.
arabic.Source=يبدأ التنزيل من إصدارات GitHub عند الضغط على تثبيت.
english.Browse=[ &Browse ]
arabic.Browse=[ استعراض ]
english.Profile=Existing profile
arabic.Profile=الإعدادات الحالية
english.ProfileHelp=Keep your paired devices, storage locations, and transfer settings.
arabic.ProfileHelp=احتفظ بالأجهزة المقترنة ومجلدات التخزين وإعدادات النقل.
english.Keep=Keep my storage and settings
arabic.Keep=الاحتفاظ بالتخزين والإعدادات الحالية
english.Reconfigure=Choose new storage and transfer settings
arabic.Reconfigure=اختيار إعدادات جديدة للتخزين والنقل
english.Storage=Storage
arabic.Storage=التخزين
english.ChooseStorage=Choose where TapSend saves uploads and pairing data.
arabic.ChooseStorage=اختر مكان حفظ الملفات المستلمة وبيانات الاقتران.
english.Inbox=Computer inbox
arabic.Inbox=مجلد الملفات المستلمة
english.PrivateData=Private data folder
arabic.PrivateData=مجلد البيانات الخاصة
english.Limit=Transfer limit
arabic.Limit=حدّ النقل
english.LimitLabel=Upload limit (GiB)
arabic.LimitLabel=حدّ الرفع (GiB)
english.ChooseLimit=Set the maximum size of one incoming file.
arabic.ChooseLimit=حدّد الحجم الأقصى لكل ملف وارد.
english.LimitRange=Range 1 to 64 GiB   /   Default 8 GiB
arabic.LimitRange=من 1 إلى 64 GiB، والقيمة الافتراضية 8 GiB
english.StartMenu=Start menu
arabic.StartMenu=قائمة ابدأ
english.ChooseStart=Choose the Start menu folder for the TapSend shortcut.
arabic.ChooseStart=اختر مجلد اختصار TapSend في قائمة ابدأ.
english.Options=Optional tasks
arabic.Options=خيارات إضافية
english.Review=Review
arabic.Review=مراجعة
english.Download=Download / verify
arabic.Download=التنزيل والتحقق
english.DownloadHelp=Downloading and verifying the Windows package from GitHub.
arabic.DownloadHelp=جارٍ تنزيل حزمة Windows من GitHub والتحقق منها.
english.Prepare=Preparing
arabic.Prepare=التحضير
english.Installing=Installing
arabic.Installing=التثبيت
english.Keyboard=TAB Move    ENTER Confirm
arabic.Keyboard=TAB للتنقل     ENTER للتأكيد
english.Complete=Installation complete
arabic.Complete=اكتمل التثبيت
english.OpenFromStart=TapSend is ready to open from the Start menu.
arabic.OpenFromStart=يمكنك فتح TapSend من قائمة ابدأ.
english.FinishPrefs=Open TapSend to finish choosing your preferences.
arabic.FinishPrefs=افتح TapSend لإكمال اختيار الإعدادات.
english.FolderError=Enter both folder paths without quotation marks.
arabic.FolderError=أدخل مساري المجلدين بدون علامات اقتباس.
english.LimitError=Enter a whole number between 1 and 64 GiB.
arabic.LimitError=أدخل عدداً صحيحاً من 1 إلى 64 GiB.
english.SaveError=TapSend was installed, but could not save those preferences.
arabic.SaveError=تم تثبيت TapSend، لكن تعذّر حفظ الإعدادات.
english.TryPackage=Check your internet connection and try again. This installer needs %1 from GitHub Releases.
arabic.TryPackage=تحقّق من اتصال الإنترنت وحاول مجدداً. يحتاج المثبّت إلى ملف %1 من إصدارات GitHub.
english.PackageMissing=Setup did not receive the Windows package.
arabic.PackageMissing=لم يستلم المثبّت حزمة Windows.
english.PackageInvalid=The downloaded package could not be verified.
arabic.PackageInvalid=تعذّر التحقق من الحزمة التي تم تنزيلها.

[Code]
const
  TerminalInk = $00EDF2F0;
  TerminalMuted = $00ACB2A5;
  TerminalAccent = $006A70FF;
  TerminalInput = $001B1D1B;

var
  ExistingPage: TInputOptionWizardPage;
  StoragePage: TInputDirWizardPage;
  LimitPage: TInputQueryWizardPage;
  Downloads: TDownloadWizardPage;
  HasSettings, Downloaded, Configured: Boolean;
  SettingsOverride: String;
  FolderStatus: TNewStaticText;
  PreviousDirChange: TNotifyEvent;

function ShortcutIcon(Param: String): String;
begin
  Result := ExpandConstant('{app}\TapSend.ico');
  if not FileExists(Result) then Result := ExpandConstant('{app}\TapSend-Desktop.exe');
end;

procedure Mono(AFont: TFont; PointSize: Integer; Ink: TColor; Bold: Boolean);
begin
  AFont.Name := 'Consolas';
  if ActiveLanguage = 'arabic' then AFont.Name := 'Segoe UI';
  AFont.Size := PointSize;
  AFont.Color := Ink;
  if Bold then AFont.Style := [fsBold] else AFont.Style := [];
end;

function TerminalText(Parent: TWinControl; X, Y, W, Size: Integer;
  Ink: TColor; Bold: Boolean; const Value: String): TNewStaticText;
begin
  Result := TNewStaticText.Create(WizardForm);
  Result.Parent := Parent;
  Result.AutoSize := False;
  Result.ShowAccelChar := False;
  Result.WordWrap := True;
  Result.SetBounds(ScaleX(X), ScaleY(Y), W, ScaleY(28));
  Mono(Result.Font, Size, Ink, Bold);
  Result.Caption := Value;
  Result.AdjustHeight;
end;

procedure TerminalFrame(Parent: TWinControl; TopY, Height: Integer; Width: Integer);
var
  Border: TBevel;
begin
  Border := TBevel.Create(WizardForm);
  Border.Parent := Parent;
  Border.Shape := bsBox;
  Border.SetBounds(0, ScaleY(TopY), Width, ScaleY(Height));
end;

procedure TerminalPathRow(Input: TNewEdit; Browse: TNewButton; TopY, Width: Integer);
begin
  Input.AutoSize := False;
  Input.Color := TerminalInput;
  Mono(Input.Font, 11, TerminalInk, False);
  Input.SetBounds(ScaleX(13), ScaleY(TopY), Width - ScaleX(113), ScaleY(27));
  Mono(Browse.Font, 10, TerminalInk, False);
  Browse.Caption := CustomMessage('Browse');
  Browse.SetBounds(Width - ScaleX(93), ScaleY(TopY), ScaleX(80), ScaleY(27));
end;

procedure DestinationChanged(Sender: TObject);
var
  Exists: String;
begin
  if PreviousDirChange <> nil then PreviousDirChange(Sender);
  if DirExists(Trim(WizardForm.DirEdit.Text)) then Exists := CustomMessage('ExistsYes')
  else Exists := CustomMessage('ExistsNo');
  FolderStatus.Caption := CustomMessage('FolderExists') + '   [ ' + Exists + ' ]';
end;

procedure StyleSetup;
var
  Width, BodyWidth, Row: Integer;
  Hint: TNewStaticText;
  Rule: TBevel;
  AccentBar: TBitmapImage;
begin
  WizardForm.Caption := 'TapSend / Setup';
  WizardForm.WizardSmallBitmapImage.Visible := False;
  WizardForm.WizardBitmapImage2.Visible := False;
  Mono(WizardForm.BackButton.Font, 10, TerminalInk, False);
  Mono(WizardForm.NextButton.Font, 10, TerminalInk, True);
  Mono(WizardForm.CancelButton.Font, 10, TerminalInk, False);
  Mono(WizardForm.PageNameLabel.Font, 15, TerminalInk, True);
  WizardForm.PageNameLabel.Font.Name := 'Consolas';
  WizardForm.PageDescriptionLabel.StyleElements := [seClient, seBorder];
  Mono(WizardForm.PageDescriptionLabel.Font, 10, TerminalAccent, True);
  WizardForm.PageNameLabel.AutoSize := False;
  WizardForm.PageNameLabel.SetBounds(ScaleX(24), ScaleY(14), ScaleX(245), ScaleY(28));
  WizardForm.PageDescriptionLabel.AutoSize := False;
  WizardForm.PageDescriptionLabel.WordWrap := False;
  WizardForm.PageDescriptionLabel.Alignment := taRightJustify;
  WizardForm.PageDescriptionLabel.SetBounds(ScaleX(269), ScaleY(20),
    WizardForm.MainPanel.ClientWidth - ScaleX(295), ScaleY(25));
  Rule := TBevel.Create(WizardForm);
  Rule.Parent := WizardForm.MainPanel;
  Rule.Shape := bsTopLine;
  Rule.SetBounds(ScaleX(24), WizardForm.MainPanel.ClientHeight - ScaleY(7),
    WizardForm.MainPanel.ClientWidth - ScaleX(48), ScaleY(2));
  AccentBar := TBitmapImage.Create(WizardForm);
  AccentBar.Parent := WizardForm.MainPanel;
  AccentBar.AutoSize := False;
  AccentBar.Stretch := True;
  AccentBar.Bitmap.Width := 2;
  AccentBar.Bitmap.Height := 2;
  AccentBar.Bitmap.Canvas.Brush.Color := TerminalAccent;
  AccentBar.Bitmap.Canvas.Pen.Color := TerminalAccent;
  AccentBar.Bitmap.Canvas.Rectangle(0, 0, 2, 2);
  AccentBar.SetBounds(ScaleX(24), WizardForm.MainPanel.ClientHeight - ScaleY(7), ScaleX(86), ScaleY(2));
  Rule := TBevel.Create(WizardForm);
  Rule.Parent := WizardForm;
  Rule.Shape := bsTopLine;
  Rule.SetBounds(ScaleX(10), WizardForm.NextButton.Top - ScaleY(12),
    WizardForm.ClientWidth - ScaleX(20), ScaleY(2));
  Hint := TerminalText(WizardForm, 24, 0, WizardForm.BackButton.Left - ScaleX(42),
    9, TerminalMuted, False, CustomMessage('Keyboard'));
  Hint.Top := WizardForm.NextButton.Top + ScaleY(5);

  Width := WizardForm.SelectDirPage.ClientWidth - ScaleX(2);
  WizardForm.SelectDirLabel.Visible := False;
  WizardForm.SelectDirBrowseLabel.Visible := False;
  WizardForm.SelectDirBitmapImage.Visible := False;
  WizardForm.DiskSpaceLabel.Visible := False;
  TerminalText(WizardForm.SelectDirPage, 0, 5, Width, 11, TerminalInk, False,
    CustomMessage('ChooseProgram'));
  TerminalFrame(WizardForm.SelectDirPage, 43, 110, Width);
  Hint := TerminalText(WizardForm.SelectDirPage, 13, 56, Width - ScaleX(26),
    10, TerminalAccent, True, CustomMessage('ProgramFolder'));
  Hint.FocusControl := WizardForm.DirEdit;
  TerminalPathRow(WizardForm.DirEdit, WizardForm.DirBrowseButton, 80, Width);
  FolderStatus := TerminalText(WizardForm.SelectDirPage, 13, 116, Width - ScaleX(26),
    10, TerminalMuted, False, ' ');
  PreviousDirChange := WizardForm.DirEdit.OnChange;
  WizardForm.DirEdit.OnChange := @DestinationChanged;
  DestinationChanged(WizardForm.DirEdit);
  TerminalText(WizardForm.SelectDirPage, 0, 169, Width, 10, TerminalMuted, False,
    CustomMessage('Source'));

  Mono(ExistingPage.SubCaptionLabel.Font, 10, TerminalMuted, False);
  Mono(ExistingPage.CheckListBox.Font, 11, TerminalInk, False);

  Width := StoragePage.SurfaceWidth - ScaleX(2);
  StoragePage.SubCaptionLabel.Visible := False;
  TerminalText(StoragePage.Surface, 0, 5, Width, 11, TerminalInk, False,
    CustomMessage('ChooseStorage'));
  TerminalFrame(StoragePage.Surface, 39, 145, Width);
  for Row := 0 to 1 do begin
    StoragePage.PromptLabels[Row].Left := ScaleX(13);
    StoragePage.PromptLabels[Row].Top := ScaleY(50 + Row * 62);
    Mono(StoragePage.PromptLabels[Row].Font, 10, TerminalAccent, True);
    StoragePage.Edits[Row].AutoSize := False;
    StoragePage.Edits[Row].Color := TerminalInput;
    Mono(StoragePage.Edits[Row].Font, 11, TerminalInk, False);
    StoragePage.Edits[Row].SetBounds(ScaleX(13), ScaleY(72 + Row * 62), Width - ScaleX(113), ScaleY(27));
    Mono(StoragePage.Buttons[Row].Font, 10, TerminalInk, False);
    StoragePage.Buttons[Row].Caption := CustomMessage('Browse');
    StoragePage.Buttons[Row].SetBounds(Width - ScaleX(93), ScaleY(72 + Row * 62), ScaleX(80), ScaleY(27));
  end;

  Width := LimitPage.SurfaceWidth - ScaleX(2);
  LimitPage.SubCaptionLabel.Visible := False;
  TerminalText(LimitPage.Surface, 0, 5, Width, 11, TerminalInk, False,
    CustomMessage('ChooseLimit'));
  TerminalFrame(LimitPage.Surface, 43, 110, Width);
  LimitPage.PromptLabels[0].Left := ScaleX(13);
  LimitPage.PromptLabels[0].Top := ScaleY(56);
  Mono(LimitPage.PromptLabels[0].Font, 10, TerminalAccent, True);
  LimitPage.Edits[0].AutoSize := False;
  LimitPage.Edits[0].Color := TerminalInput;
  Mono(LimitPage.Edits[0].Font, 12, TerminalInk, False);
  LimitPage.Edits[0].SetBounds(ScaleX(13), ScaleY(80), ScaleX(86), ScaleY(27));
  TerminalText(LimitPage.Surface, 13, 116, Width - ScaleX(26),
    10, TerminalMuted, False, CustomMessage('LimitRange'));

  Width := WizardForm.SelectProgramGroupPage.ClientWidth - ScaleX(2);
  WizardForm.SelectStartMenuFolderLabel.Visible := False;
  WizardForm.SelectStartMenuFolderBrowseLabel.Visible := False;
  WizardForm.SelectGroupBitmapImage.Visible := False;
  TerminalText(WizardForm.SelectProgramGroupPage, 0, 5, Width, 11, TerminalInk, False,
    CustomMessage('ChooseStart'));
  TerminalFrame(WizardForm.SelectProgramGroupPage, 43, 90, Width);
  Hint := TerminalText(WizardForm.SelectProgramGroupPage, 13, 56, Width - ScaleX(26),
    10, TerminalAccent, True, CustomMessage('StartMenu'));
  Hint.FocusControl := WizardForm.GroupEdit;
  TerminalPathRow(WizardForm.GroupEdit, WizardForm.GroupBrowseButton, 80, Width);
  Mono(WizardForm.SelectTasksLabel.Font, 11, TerminalInk, False);
  Mono(WizardForm.TasksList.Font, 11, TerminalInk, False);
  Mono(WizardForm.ReadyLabel.Font, 11, TerminalInk, False);
  Mono(WizardForm.ReadyMemo.Font, 10, TerminalInk, False);
  WizardForm.ReadyMemo.Color := TerminalInput;
  Mono(WizardForm.StatusLabel.Font, 11, TerminalAccent, True);
  Mono(WizardForm.FilenameLabel.Font, 10, TerminalMuted, False);
  Mono(Downloads.Msg1Label.Font, 11, TerminalAccent, True);
  Mono(Downloads.Msg2Label.Font, 10, TerminalMuted, False);
  Mono(Downloads.AbortButton.Font, 10, TerminalInk, False);
  Downloads.ShowBaseNameInsteadOfUrl := True;

  BodyWidth := WizardForm.FinishedPage.ClientWidth - ScaleX(54);
  TerminalText(WizardForm.FinishedPage, 27, 20, BodyWidth, 12, TerminalAccent, True,
    '>_  TapSend  /  SETUP');
  WizardForm.FinishedHeadingLabel.SetBounds(ScaleX(27), ScaleY(65), BodyWidth, ScaleY(39));
  Mono(WizardForm.FinishedHeadingLabel.Font, 19, TerminalInk, True);
  WizardForm.FinishedLabel.SetBounds(ScaleX(27), ScaleY(114), BodyWidth, ScaleY(52));
  Mono(WizardForm.FinishedLabel.Font, 11, TerminalMuted, False);
  WizardForm.RunList.SetBounds(ScaleX(27), ScaleY(184), BodyWidth, ScaleY(70));
  Mono(WizardForm.RunList.Font, 11, TerminalInk, False);
end;

procedure InitializeWizard;
var
  DefaultData: String;
begin
  DefaultData := ExpandConstant('{userappdata}\TapSend\data');
  HasSettings := FileExists(ExpandConstant('{userappdata}\TapSend\settings.json'));
  SettingsOverride := ExpandConstant('{param:CONFIGURE|}');
  ExistingPage := CreateInputOptionPage(wpSelectDir, CustomMessage('Profile'),
    '', CustomMessage('ProfileHelp'), True, False);
  ExistingPage.Add(CustomMessage('Keep'));
  ExistingPage.Add(CustomMessage('Reconfigure'));
  ExistingPage.SelectedValueIndex := 0;

  StoragePage := CreateInputDirPage(ExistingPage.ID, CustomMessage('Storage'),
    '', '', False, '');
  StoragePage.Add(CustomMessage('Inbox'));
  StoragePage.Add(CustomMessage('PrivateData'));
  StoragePage.Values[0] := ExpandConstant('{param:INBOX|}');
  StoragePage.Values[1] := ExpandConstant('{param:DATA|}');
  if StoragePage.Values[0] = '' then StoragePage.Values[0] := DefaultData + '\inbox';
  if StoragePage.Values[1] = '' then StoragePage.Values[1] := DefaultData;

  LimitPage := CreateInputQueryPage(StoragePage.ID, CustomMessage('Limit'), '', '');
  LimitPage.Add(CustomMessage('LimitLabel'), False);
  LimitPage.Values[0] := ExpandConstant('{param:UPLOADLIMIT|8}');
  Downloads := CreateDownloadPage(CustomMessage('Download'), CustomMessage('DownloadHelp'), nil);
  StyleSetup;
end;

procedure CurPageChanged(CurPageID: Integer);
var
  Phase: String;
begin
  if CurPageID = wpSelectDir then Phase := CustomMessage('Destination')
  else if CurPageID = ExistingPage.ID then Phase := CustomMessage('Profile')
  else if CurPageID = StoragePage.ID then Phase := CustomMessage('Storage')
  else if CurPageID = LimitPage.ID then Phase := CustomMessage('Limit')
  else if CurPageID = wpSelectProgramGroup then Phase := CustomMessage('StartMenu')
  else if CurPageID = wpSelectTasks then Phase := CustomMessage('Options')
  else if CurPageID = wpReady then Phase := CustomMessage('Review')
  else if CurPageID = Downloads.ID then Phase := CustomMessage('Download')
  else if CurPageID = wpPreparing then Phase := CustomMessage('Prepare')
  else if CurPageID = wpInstalling then Phase := CustomMessage('Installing')
  else Phase := 'WINDOWS x64';

  WizardForm.PageNameLabel.Caption := #8206 + '>_  TapSend  /  SETUP' + #8206;
  WizardForm.PageDescriptionLabel.Caption := '[ ' + Phase + ' ]   v{#TapSendVersion}';
  if CurPageID = wpFinished then begin
    WizardForm.FinishedHeadingLabel.Caption := CustomMessage('Complete');
    if Configured then
      WizardForm.FinishedLabel.Caption := CustomMessage('OpenFromStart')
    else
      WizardForm.FinishedLabel.Caption := CustomMessage('FinishPrefs');
  end;
end;

function KeepSettings: Boolean;
begin
  Result := HasSettings and (SettingsOverride <> '1') and
    ((SettingsOverride = '0') or (ExistingPage.SelectedValueIndex = 0));
end;

function UpdateReadyMemo(Space, NewLine, MemoUserInfoInfo, MemoDirInfo,
  MemoTypeInfo, MemoComponentsInfo, MemoGroupInfo, MemoTasksInfo: String): String;
begin
  Result := CustomMessage('ProgramFolder') + NewLine + Space + WizardDirValue + NewLine + NewLine;
  if KeepSettings then
    Result := Result + CustomMessage('Profile') + NewLine + Space + CustomMessage('Keep')
  else
    Result := Result + CustomMessage('Inbox') + NewLine + Space + Trim(StoragePage.Values[0]) +
      NewLine + NewLine + CustomMessage('PrivateData') + NewLine + Space + Trim(StoragePage.Values[1]) +
      NewLine + NewLine + CustomMessage('LimitLabel') + NewLine + Space + Trim(LimitPage.Values[0]) + ' GiB';
  if MemoGroupInfo <> '' then Result := Result + NewLine + NewLine + MemoGroupInfo;
  if MemoTasksInfo <> '' then Result := Result + NewLine + NewLine + MemoTasksInfo;
end;

function ShouldSkipPage(PageID: Integer): Boolean;
begin
  Result := ((PageID = ExistingPage.ID) and (not HasSettings or (SettingsOverride <> ''))) or
    (((PageID = StoragePage.ID) or (PageID = LimitPage.ID)) and KeepSettings);
end;

function ValidLimit: Boolean;
var
  Limit: Integer;
begin
  Limit := StrToIntDef(Trim(LimitPage.Values[0]), 0);
  Result := (Limit >= 1) and (Limit <= 64);
end;

function ValidFolders: Boolean;
begin
  Result := (Trim(StoragePage.Values[0]) <> '') and (Trim(StoragePage.Values[1]) <> '') and
    (Pos('"', StoragePage.Values[0]) = 0) and (Pos('"', StoragePage.Values[1]) = 0);
end;

function NextButtonClick(PageID: Integer): Boolean;
begin
  Result := True;
  if (PageID = StoragePage.ID) and not ValidFolders then begin
    MsgBox(CustomMessage('FolderError'), mbError, MB_OK);
    Result := False;
  end else if (PageID = LimitPage.ID) and not ValidLimit then begin
    MsgBox(CustomMessage('LimitError'), mbError, MB_OK);
    Result := False;
  end;
end;

function BundleReady: Boolean;
begin
  Result := Downloaded and FileExists(ExpandConstant('{tmp}\{#BundleName}'));
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  PackagePath: String;
begin
  Result := '';
  if not KeepSettings and not ValidLimit then begin
    Result := CustomMessage('LimitError');
    Exit;
  end;
  if not KeepSettings and not ValidFolders then begin
    Result := CustomMessage('FolderError');
    Exit;
  end;
  if BundleReady then Exit;
  PackagePath := ExpandConstant('{tmp}\{#BundleName}');
  try
    Downloads.Clear;
    Downloads.Add('{#BundleURL}', '{#BundleName}', '{#BundleSHA256}');
    Downloads.Show;
    try
      Downloads.Download;
    finally
      Downloads.Hide;
    end;
    if not FileExists(PackagePath) then RaiseException(CustomMessage('PackageMissing'));
    if CompareText(GetSHA256OfFile(PackagePath), '{#BundleSHA256}') <> 0 then
      RaiseException(CustomMessage('PackageInvalid'));
    Downloaded := True;
  except
    DeleteFile(PackagePath);
    Result := GetExceptionMessage + #13#10 + FmtMessage(CustomMessage('TryPackage'), ['{#BundleName}']);
  end;
end;

function PreferencesReady: Boolean;
begin
  Result := Configured;
end;

function QuoteArg(Value: String): String;
begin
  if (Length(Value) > 0) and (Value[Length(Value)] = '\') then begin
    { A terminal backslash before a quote must be escaped. A dot preserves drive roots. }
    Value := Value + '.';
  end;
  Result := '"' + Value + '"';
end;

function InPath(const Value, Directory: String): Boolean;
begin
  Result := Pos(';' + Uppercase(Directory) + ';', ';' + Uppercase(Value) + ';') > 0;
end;

function WithoutPath(const Value, Directory: String): String;
var
  Remaining, Segment: String;
  Separator: Integer;
begin
  Result := '';
  Remaining := Value;
  while Remaining <> '' do begin
    Separator := Pos(';', Remaining);
    if Separator = 0 then Separator := Length(Remaining) + 1;
    Segment := Copy(Remaining, 1, Separator - 1);
    Delete(Remaining, 1, Separator);
    if (Segment <> '') and (CompareText(Segment, Directory) <> 0) then begin
      if Result <> '' then Result := Result + ';';
      Result := Result + Segment;
    end;
  end;
end;

procedure AddCommandPath(Directory: String);
var
  Existing: String;
begin
  if not WizardIsTaskSelected('clipath') then Exit;
  if not RegQueryStringValue(HKCU, 'Environment', 'Path', Existing) then Existing := '';
  if not InPath(Existing, Directory) then begin
    if Existing <> '' then Existing := Existing + ';';
    if RegWriteExpandStringValue(HKCU, 'Environment', 'Path', Existing + Directory) then
      RegWriteStringValue(HKCU, 'Software\TapSend', 'PathAdded', Directory);
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  ExitCode: Integer;
  Directory, Params: String;
begin
  if CurStep <> ssPostInstall then Exit;
  Directory := ExpandConstant('{app}');
  AddCommandPath(Directory);
  Configured := KeepSettings;
  if Configured then Exit;
  Params := 'settings setup --inbox ' + QuoteArg(Trim(StoragePage.Values[0])) +
    ' --data-dir ' + QuoteArg(Trim(StoragePage.Values[1])) +
    ' --upload-limit-gib ' + IntToStr(StrToIntDef(Trim(LimitPage.Values[0]), 0));
  Configured := Exec(AddBackslash(Directory) + 'tapsend.exe', Params, Directory,
    SW_HIDE, ewWaitUntilTerminated, ExitCode) and (ExitCode = 0);
  if not Configured then
    MsgBox(CustomMessage('SaveError') + ' ' + CustomMessage('FinishPrefs'), mbError, MB_OK);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Existing, Directory, AddedDirectory: String;
begin
  if CurUninstallStep <> usPostUninstall then Exit;
  Directory := ExpandConstant('{app}');
  if RegQueryStringValue(HKCU, 'Software\TapSend', 'PathAdded', AddedDirectory) and
    (CompareText(AddedDirectory, Directory) = 0) then begin
    if RegQueryStringValue(HKCU, 'Environment', 'Path', Existing) and InPath(Existing, Directory) then
      RegWriteExpandStringValue(HKCU, 'Environment', 'Path', WithoutPath(Existing, Directory));
    RegDeleteValue(HKCU, 'Software\TapSend', 'PathAdded');
    RegDeleteKeyIfEmpty(HKCU, 'Software\TapSend');
  end;
end;
