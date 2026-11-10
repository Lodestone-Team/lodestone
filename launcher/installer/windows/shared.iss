; Shared by lodestone-launcher.iss and lodestone-host.iss (Inno Setup 6): where to
; install, and what uninstalling leaves behind.
;
; Installing: the person can install anywhere they can write to, such as a games
; folder on another drive. Setup runs without administrator rights (so the app can
; update itself in place), so Next checks that the chosen folder is writable and
; explains when it isn't (Program Files, say), instead of failing halfway through.
; Upgrades keep the folder of the existing install (DisableDirPage=auto).
;
; Uninstalling removes the program but keeps the person's data (worlds and so on).
; At the end it lists each data folder that is still there, with its size and a
; way to open it, and lets the person delete it too. Nothing is deleted unless they
; tick it and confirm. A silent uninstall keeps everything, unless /DELETEDATA is
; passed.
;
; The including script defines, after the #include, in its own [Code]:
;   procedure RegisterKeptData;
;     Calls AddKeptData(Title, Folder, Note) for each data folder.
;   procedure BeforeDeletingKeptData(Index: Integer);
;     Runs just before the folder AddKeptData added as Index is deleted.
; and calls CheckWritableDir from NextButtonClick (shared below).

[Code]
function SetFileAttributes(FileName: String; Attributes: DWORD): BOOL;
  external 'SetFileAttributesW@kernel32.dll stdcall';

procedure RegisterKeptData; forward;
procedure BeforeDeletingKeptData(Index: Integer); forward;

var
  KeptTitles, KeptFolders, KeptNotes: TArrayOfString;
  KeptSizes: array of Int64;
  KeptChecks: array of TNewCheckBox;
  { What the person chose to delete, by index. }
  KeptChosen: array of Boolean;
  KeptFinishButton: TNewButton;

{ --- Installing ----------------------------------------------------------- }

{ Whether this user can make Dir and write a file in it: what installing needs.
  Folders it made to find out are removed again. }
function CanWriteTo(Dir: String): Boolean;
var
  Made: TArrayOfString;
  Parent, Probe: String;
  I: Integer;
begin
  Dir := RemoveBackslashUnlessRoot(Dir);
  { The folders that don't exist yet, deepest first. }
  Parent := Dir;
  while (Parent <> '') and not DirExists(Parent) do
  begin
    SetArrayLength(Made, GetArrayLength(Made) + 1);
    Made[GetArrayLength(Made) - 1] := Parent;
    if ExtractFileDir(Parent) = Parent then
      Break;
    Parent := ExtractFileDir(Parent);
  end;
  Result := ForceDirectories(Dir);
  if Result then
  begin
    Probe := AddBackslash(Dir) + '.lodestone-setup-write-test.tmp';
    Result := SaveStringToFile(Probe, 'test', False);
    DeleteFile(Probe);
  end;
  for I := 0 to GetArrayLength(Made) - 1 do
    RemoveDir(Made[I]);
end;

{ Whether Dir is Root or a folder in it. }
function IsUnder(const Dir, Root: String): Boolean;
begin
  Result := (Root <> '') and
    (Pos(Lowercase(AddBackslash(Root)), Lowercase(AddBackslash(Dir))) = 1);
end;

{ Program Files and the Windows folder, which need administrator rights for every
  change. Setup started with "Run as administrator" can write to them, so the write
  test alone would let the app be installed where it can never update itself. }
function IsSystemFolder(const Dir: String): Boolean;
begin
  Result := IsUnder(Dir, ExpandConstant('{commonpf32}')) or
    IsUnder(Dir, ExpandConstant('{commonpf64}')) or
    IsUnder(Dir, ExpandConstant('{win}'));
end;

{ For NextButtonClick: on the folder page, refuses a folder this user can't write to. }
function CheckWritableDir(CurPageID: Integer): Boolean;
var
  Dir: String;
begin
  Result := True;
  if CurPageID <> wpSelectDir then
    Exit;
  Dir := WizardDirValue;
  if not IsSystemFolder(Dir) and CanWriteTo(Dir) then
    Exit;
  Result := False;
  SuppressibleMsgBox(
    'You can''t install to this folder without administrator rights:' + #13#10#13#10 +
    Dir + #13#10#13#10 +
    '{#AppName} installs without them, so that it can update itself. ' +
    'Program Files and the Windows folder need them, and so can folders other people own.' + #13#10#13#10 +
    'Choose a folder of your own, for example D:\Games\{#AppName}.',
    mbError, MB_OK, IDOK);
end;

{ Under the folder box: where the data goes, which isn't the install folder. }
procedure AddDataLocationNote(const Text: String);
var
  Note: TNewStaticText;
begin
  Note := TNewStaticText.Create(WizardForm);
  Note.Parent := WizardForm.SelectDirPage;
  Note.Left := WizardForm.DirEdit.Left;
  Note.Top := WizardForm.DirEdit.Top + WizardForm.DirEdit.Height + ScaleY(16);
  Note.Width := WizardForm.SelectDirPage.ClientWidth - Note.Left;
  Note.AutoSize := False;
  Note.WordWrap := True;
  Note.Height := ScaleY(60);
  Note.Caption := Text;
end;

{ --- Uninstalling --------------------------------------------------------- }

{ The size of the files under Dir. Linked folders (junctions, symbolic links) are
  someone else's, so they aren't counted. }
function FolderSize(const Dir: String): Int64;
var
  Rec: TFindRec;
  Path: String;
begin
  Result := 0;
  if not FindFirst(AddBackslash(Dir) + '*', Rec) then
    Exit;
  try
    repeat
      if (Rec.Name <> '.') and (Rec.Name <> '..') then
      begin
        Path := AddBackslash(Dir) + Rec.Name;
        if (Rec.Attributes and FILE_ATTRIBUTE_DIRECTORY) <> 0 then
        begin
          if (Rec.Attributes and FILE_ATTRIBUTE_REPARSE_POINT) = 0 then
            Result := Result + FolderSize(Path);
        end
        else
          Result := Result + Int64(Rec.SizeHigh) * 4294967296 + Int64(Rec.SizeLow);
      end;
    until not FindNext(Rec);
  finally
    FindClose(Rec);
  end;
end;

function FormatSize(Bytes: Int64): String;
begin
  if Bytes >= 1073741824 then
    Result := Format('%.1f GB', [Bytes / 1073741824.0])
  else if Bytes >= 1048576 then
    Result := Format('%.0f MB', [Bytes / 1048576.0])
  else if Bytes >= 1024 then
    Result := Format('%.0f KB', [Bytes / 1024.0])
  else
    Result := Format('%d bytes', [Bytes]);
end;

{ Deletes Dir and everything in it; returns how many files or folders it couldn't
  delete (in use, usually). A linked folder is removed as a link: what it points to
  is never touched. }
function DeleteFolder(const Dir: String): Integer;
var
  Rec: TFindRec;
  Path: String;
begin
  Result := 0;
  if FindFirst(AddBackslash(Dir) + '*', Rec) then
  try
    repeat
      if (Rec.Name <> '.') and (Rec.Name <> '..') then
      begin
        Path := AddBackslash(Dir) + Rec.Name;
        if (Rec.Attributes and FILE_ATTRIBUTE_DIRECTORY) <> 0 then
        begin
          if (Rec.Attributes and FILE_ATTRIBUTE_REPARSE_POINT) <> 0 then
          begin
            if not RemoveDir(Path) then
              Result := Result + 1;
          end
          else
            Result := Result + DeleteFolder(Path);
        end
        else
        begin
          if (Rec.Attributes and FILE_ATTRIBUTE_READONLY) <> 0 then
            SetFileAttributes(Path, FILE_ATTRIBUTE_NORMAL);
          if not DeleteFile(Path) then
            Result := Result + 1;
        end;
      end;
    until not FindNext(Rec);
  finally
    FindClose(Rec);
  end;
  if (Result = 0) and not RemoveDir(Dir) then
    Result := 1;
end;

procedure AddKeptData(const Title, Folder, Note: String);
var
  N: Integer;
begin
  if (Trim(Folder) = '') or not DirExists(Folder) then
    Exit;
  for N := 0 to GetArrayLength(KeptFolders) - 1 do
    if CompareText(KeptFolders[N], Folder) = 0 then
      Exit;
  N := GetArrayLength(KeptFolders);
  SetArrayLength(KeptTitles, N + 1);
  SetArrayLength(KeptFolders, N + 1);
  SetArrayLength(KeptNotes, N + 1);
  SetArrayLength(KeptSizes, N + 1);
  KeptTitles[N] := Title;
  KeptFolders[N] := RemoveBackslashUnlessRoot(Folder);
  KeptNotes[N] := Note;
  KeptSizes[N] := FolderSize(Folder);
end;

{ An environment variable a developer may set to move a folder, else Default. }
function EnvOr(const Name, Default: String): String;
begin
  Result := Trim(GetEnv(Name));
  if Result = '' then
    Result := Default;
end;

procedure OpenKeptFolder(Sender: TObject);
var
  Code: Integer;
begin
  ShellExec('open', KeptFolders[TNewButton(Sender).Tag], '', '', SW_SHOWNORMAL, ewNoWait, Code);
end;

procedure KeptCheckClick(Sender: TObject);
var
  I: Integer;
  Any: Boolean;
begin
  Any := False;
  for I := 0 to GetArrayLength(KeptChecks) - 1 do
    if KeptChecks[I].Checked then
      Any := True;
  if Any then
    KeptFinishButton.Caption := 'Delete and close'
  else
    KeptFinishButton.Caption := 'Keep and close';
end;

function NewText(Form: TSetupForm; const Caption: String; Left, Top, Width: Integer;
  Bold: Boolean): TNewStaticText;
begin
  Result := TNewStaticText.Create(Form);
  Result.Parent := Form;
  Result.Left := Left;
  Result.Top := Top;
  Result.AutoSize := False;
  Result.WordWrap := True;
  Result.Width := Width;
  Result.Caption := Caption;
  if Bold then
    Result.Font.Style := [fsBold];
  Result.AutoSize := True;
  { AutoSize with WordWrap keeps the width and fits the height. }
  Result.Width := Width;
end;

{ Shows what was kept and lets the person tick what to delete. Returns whether
  they asked for anything to be deleted (and confirmed it). }
function AskWhatToDelete: Boolean;
var
  Form: TSetupForm;
  Intro: TNewStaticText;
  Text: TNewStaticText;
  Open: TNewButton;
  Check: TNewCheckBox;
  Margin, Y, W, I, ButtonW: Integer;
  Chosen: String;
  Total: Int64;
begin
  Result := False;
  SetArrayLength(KeptChosen, GetArrayLength(KeptFolders));
  Margin := ScaleX(16);
  W := ScaleX(460);
  ButtonW := ScaleX(96);
#if Ver >= EncodeVer(6, 6, 0)
  { The size unscaled: CreateCustomForm scales it, and KeepSize stops it widening
    the form further by WizardSizePercent, past the controls placed below. }
  Form := CreateCustomForm(460 + 2 * 16, 200, True, True);
#else
  Form := CreateCustomForm;
  Form.ClientWidth := W + 2 * Margin;
#endif
  try
    Form.Caption := '{#AppName} was uninstalled';
    Y := Margin;
    Intro := NewText(Form,
      '{#AppName} was removed, but your data is still on this computer, so nothing is lost ' +
      'if you install it again. To remove it too, tick it below. ' +
      'Copy anything you want to keep, such as a world, before deleting it.',
      Margin, Y, W, False);
    Y := Y + Intro.Height + ScaleY(14);

    SetArrayLength(KeptChecks, GetArrayLength(KeptFolders));
    for I := 0 to GetArrayLength(KeptFolders) - 1 do
    begin
      Text := NewText(Form, KeptTitles[I] + '  (' + FormatSize(KeptSizes[I]) + ')',
        Margin, Y, W - ButtonW - ScaleX(8), True);
      Open := TNewButton.Create(Form);
      Open.Parent := Form;
      Open.Caption := 'Open folder';
      Open.Width := ButtonW;
      Open.Height := ScaleY(23);
      Open.Left := Margin + W - ButtonW;
      Open.Top := Y - ScaleY(3);
      Open.Tag := I;
      Open.OnClick := @OpenKeptFolder;
      Y := Y + Text.Height + ScaleY(2);

      { A long path is shortened in the middle; the full one is in the confirmation. }
      Text := NewText(Form, '', Margin, Y, W, False);
      Text.Caption := MinimizePathName(KeptFolders[I], Text.Font, W);
      Y := Y + Text.Height + ScaleY(4);
      Text := NewText(Form, KeptNotes[I], Margin, Y, W, False);
      Y := Y + Text.Height + ScaleY(6);

      Check := TNewCheckBox.Create(Form);
      Check.Parent := Form;
      Check.Left := Margin;
      Check.Top := Y;
      Check.Width := W;
      Check.Height := ScaleY(17);
      Check.Caption := 'Delete this folder';
      Check.Checked := False;
      Check.OnClick := @KeptCheckClick;
      KeptChecks[I] := Check;
      Y := Y + Check.Height + ScaleY(16);
    end;

    KeptFinishButton := TNewButton.Create(Form);
    KeptFinishButton.Parent := Form;
    KeptFinishButton.Width := ScaleX(120);
    KeptFinishButton.Height := ScaleY(23);
    KeptFinishButton.Left := Margin + W - KeptFinishButton.Width;
    KeptFinishButton.Top := Y;
    KeptFinishButton.ModalResult := mrOk;
    KeptFinishButton.Default := True;
    KeptFinishButton.Cancel := True;
    KeptCheckClick(nil);
    Form.ClientHeight := Y + KeptFinishButton.Height + Margin;
    Form.ActiveControl := KeptFinishButton;

    Form.ShowModal;

    Chosen := '';
    Total := 0;
    for I := 0 to GetArrayLength(KeptFolders) - 1 do
      if KeptChecks[I].Checked then
      begin
        Chosen := Chosen + #13#10 + '  ' + KeptFolders[I] + '  (' + FormatSize(KeptSizes[I]) + ')';
        Total := Total + KeptSizes[I];
      end;
    if Chosen = '' then
      Exit;
    Result := MsgBox('Permanently delete these folders and everything in them?' + #13#10 +
      Chosen + #13#10#13#10 + 'They don''t go to the Recycle Bin, and this can''t be undone.',
      mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES;
    if Result then
      for I := 0 to GetArrayLength(KeptChecks) - 1 do
        KeptChosen[I] := KeptChecks[I].Checked;
  finally
    Form.Free;
  end;
end;

procedure DeleteKeptFolder(Index: Integer; var Failed: String);
var
  Left: Integer;
begin
  BeforeDeletingKeptData(Index);
  Left := DeleteFolder(KeptFolders[Index]);
  Log(Format('Deleted %s: %d left', [KeptFolders[Index], Left]));
  if Left > 0 then
    Failed := Failed + #13#10 + '  ' + KeptFolders[Index];
end;

procedure HandleKeptData;
var
  I: Integer;
  ToDelete: array of Boolean;
  Failed: String;
begin
  RegisterKeptData;
  if GetArrayLength(KeptFolders) = 0 then
    Exit;
  SetArrayLength(ToDelete, GetArrayLength(KeptFolders));
  if UninstallSilent then
  begin
    { Silent: keep everything, unless asked on the command line. }
    for I := 0 to GetArrayLength(ToDelete) - 1 do
      ToDelete[I] := Pos('/DELETEDATA', Uppercase(GetCmdTail)) > 0;
  end
  else
  begin
    if not AskWhatToDelete then
      Exit;
    for I := 0 to GetArrayLength(ToDelete) - 1 do
      ToDelete[I] := KeptChosen[I];
  end;
  Failed := '';
  for I := 0 to GetArrayLength(ToDelete) - 1 do
    if ToDelete[I] then
      DeleteKeptFolder(I, Failed);
  if (Failed <> '') and not UninstallSilent then
    MsgBox('Some files couldn''t be deleted, probably because a program is using them ' +
      '(Minecraft, or a Lodestone server). Close it, then delete what is left of:' + #13#10 +
      Failed, mbError, MB_OK);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    HandleKeptData;
end;
