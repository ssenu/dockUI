; Dock UI 설치 스크립트. scripts/build.ps1이 /DAppVersion=<버전>을 넘긴다.
#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#define AppName "Dock UI"
#define AppExe "DockUI.exe"
; 0.2.x(옛 이름 YT Launcher)의 실행 파일·설치 폴더. 업데이트 때 정리한다
#define LegacyExe "YTLauncher.exe"
#define LegacyDir "{localappdata}\Programs\YT-Launcher"

[Setup]
; AppId는 옛 이름 때부터 같다. 바꾸면 "설치된 앱"에 두 개가 생긴다
AppId={{8F3C2A71-5B4E-4C9D-9A16-3E7D2B0C5F48}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=ssenu
AppPublisherURL=https://github.com/ssenu/dockUI
AppUpdatesURL=https://github.com/ssenu/dockUI/releases
DefaultDirName={localappdata}\Programs\DockUI
; 기본값이면 업그레이드가 옛 설치 폴더(YT-Launcher)에 그대로 설치된다. 새 폴더에 설치하고 옛 폴더는 설치 뒤 지운다
UsePreviousAppDir=no
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
DisableDirPage=yes
PrivilegesRequired=lowest
OutputDir=..\dist\installer
OutputBaseFilename=DockUI-Setup-{#AppVersion}
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
CloseApplications=force
RestartApplications=no
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "korean"; MessagesFile: "compiler:Languages\Korean.isl"

[Tasks]
Name: "autostart"; Description: "윈도우 시작 시 자동 실행"; GroupDescription: "추가 옵션:"
Name: "desktopicon"; Description: "바탕화면 바로가기 만들기"; GroupDescription: "추가 옵션:"; Flags: unchecked

[InstallDelete]
; 옛 이름(YT Launcher) 바로가기
Type: files; Name: "{autoprograms}\YT Launcher.lnk"
Type: files; Name: "{autodesktop}\YT Launcher.lnk"

[Files]
Source: "..\dist\DockUI\DockUI.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"; AppUserModelID: "ssenu.DockUI"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon; AppUserModelID: "ssenu.DockUI"

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "DockUI"; ValueData: """{app}\{#AppExe}"""; Tasks: autostart; Flags: uninsdeletevalue

[Run]
; 일반 설치: 마지막 화면의 "실행" 체크박스
Filename: "{app}\{#AppExe}"; Description: "{#AppName} 실행"; Flags: nowait postinstall skipifsilent
; 조용한 설치(앱 내 자동 업데이트): 끝나면 바로 다시 실행
Filename: "{app}\{#AppExe}"; Flags: nowait; Check: WizardSilent

[Code]
const
  { 새 이름과 0.2.x 옛 이름 뮤텍스 (쉼표로 여러 개) }
  AppMutexNames = 'DockUIMutex,YTLauncherMutex';
  RunKey = 'Software\Microsoft\Windows\CurrentVersion\Run';
  RunValue = 'DockUI';
  LegacyRunValue = 'YTLauncher';

procedure CloseRunningApp(WaitFirst: Boolean);
var
  I, ResultCode: Integer;
begin
  if WaitFirst then
  begin
    { 자동 업데이트 시 앱이 스스로 종료할 시간을 준다 (최대 10초). }
    for I := 1 to 50 do
    begin
      if not CheckForMutexes(AppMutexNames) then
        Exit;
      Sleep(200);
    end;
  end;
  if CheckForMutexes(AppMutexNames) then
    Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM {#AppExe} /IM {#LegacyExe}', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

{ 앱 메뉴로 켠 옛 자동 실행 값을 새 이름·새 경로로 옮긴다 }
procedure MoveLegacyAutostart;
begin
  if RegValueExists(HKEY_CURRENT_USER, RunKey, LegacyRunValue) then
  begin
    RegDeleteValue(HKEY_CURRENT_USER, RunKey, LegacyRunValue);
    if not RegValueExists(HKEY_CURRENT_USER, RunKey, RunValue) then
      RegWriteStringValue(HKEY_CURRENT_USER, RunKey, RunValue, '"' + ExpandConstant('{app}\{#AppExe}') + '"');
  end;
end;

{ 0.2.x 설치 폴더(옛 실행 파일·옛 제거 프로그램)를 지운다. 실패해도 설치는 성공으로 둔다 }
procedure RemoveLegacyInstallDir;
var
  Legacy: String;
begin
  Legacy := ExpandConstant('{#LegacyDir}');
  if (CompareText(Legacy, ExpandConstant('{app}')) <> 0) and DirExists(Legacy) then
    DelTree(Legacy, True, True, True);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  CloseRunningApp(WizardSilent);
  Result := '';
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    MoveLegacyAutostart;
    RemoveLegacyInstallDir;
  end;
end;

function InitializeUninstall(): Boolean;
begin
  CloseRunningApp(False);
  Result := True;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  { 앱 메뉴에서 켠 자동 실행도 함께 지운다(새·옛 이름). 설정(%APPDATA%\DockUI)은 남긴다. }
  if CurUninstallStep = usPostUninstall then
  begin
    RegDeleteValue(HKEY_CURRENT_USER, RunKey, RunValue);
    RegDeleteValue(HKEY_CURRENT_USER, RunKey, LegacyRunValue);
  end;
end;
