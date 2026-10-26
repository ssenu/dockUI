; Dock UI(옛 이름 YT Launcher) 설치 스크립트. scripts/build.ps1이 /DAppVersion=<버전>을 넘긴다.
#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
; 보이는 이름만 바꾼다. AppId·설치 폴더·exe·설치 파일 이름·뮤텍스·Run 값은 업데이트 호환을 위해 그대로
#define AppName "Dock UI"
#define AppExe "YTLauncher.exe"

[Setup]
AppId={{8F3C2A71-5B4E-4C9D-9A16-3E7D2B0C5F48}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=ssenu
AppPublisherURL=https://github.com/ssenu/dockUI
AppUpdatesURL=https://github.com/ssenu/dockUI/releases
DefaultDirName={localappdata}\Programs\YT-Launcher
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
DisableDirPage=yes
PrivilegesRequired=lowest
OutputDir=..\dist\installer
OutputBaseFilename=YT-Launcher-Setup-{#AppVersion}
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
; 0.1.x(파이썬)에서 올라올 때 남은 라이브러리 폴더를 지운다. 0.2.0부터는 exe 하나뿐이다.
Type: filesandordirs; Name: "{app}\_internal"
; 옛 이름(YT Launcher) 바로가기
Type: files; Name: "{autoprograms}\YT Launcher.lnk"
Type: files; Name: "{autodesktop}\YT Launcher.lnk"

[Files]
Source: "..\dist\YTLauncher\YTLauncher.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "YTLauncher"; ValueData: """{app}\{#AppExe}"""; Tasks: autostart; Flags: uninsdeletevalue

[Run]
; 일반 설치: 마지막 화면의 "실행" 체크박스
Filename: "{app}\{#AppExe}"; Description: "{#AppName} 실행"; Flags: nowait postinstall skipifsilent
; 조용한 설치(앱 내 자동 업데이트): 끝나면 바로 다시 실행
Filename: "{app}\{#AppExe}"; Flags: nowait; Check: WizardSilent

[Code]
const
  AppMutexName = 'YTLauncherMutex';
  RunKey = 'Software\Microsoft\Windows\CurrentVersion\Run';

procedure CloseRunningApp(WaitFirst: Boolean);
var
  I, ResultCode: Integer;
begin
  if WaitFirst then
  begin
    { 자동 업데이트 시 앱이 스스로 종료할 시간을 준다 (최대 10초). }
    for I := 1 to 50 do
    begin
      if not CheckForMutexes(AppMutexName) then
        Exit;
      Sleep(200);
    end;
  end;
  if CheckForMutexes(AppMutexName) then
    Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM {#AppExe}', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  CloseRunningApp(WizardSilent);
  Result := '';
end;

function InitializeUninstall(): Boolean;
begin
  CloseRunningApp(False);
  Result := True;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  { 앱 메뉴에서 켠 자동 실행도 함께 지운다. 설정(%APPDATA%\YT-Launcher)은 남긴다. }
  if CurUninstallStep = usPostUninstall then
    RegDeleteValue(HKEY_CURRENT_USER, RunKey, 'YTLauncher');
end;
