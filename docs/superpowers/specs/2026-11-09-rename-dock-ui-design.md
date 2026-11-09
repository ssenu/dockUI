# 이름을 Dock UI로 통일 (0.3.0)

작성: 2026-11-09 · 대상 버전: 0.3.0

릴리스·설치 파일·실행 파일·설치 폴더·데이터 폴더·내부 이름에 남은 옛 이름(YT Launcher, `YTLauncher`, `YT-Launcher`, `yt-launcher`)을 Dock UI로 바꾼다. 이미 설치된 사용자는 아무것도 하지 않아도 자동 업데이트로 새 이름으로 넘어와야 한다.

## 정한 것

- 다음 버전은 `0.3.0`이다.
- 과거 릴리스(v0.2.x)의 제목·파일은 그대로 둔다.
- 0.2.10 이하 앱이 업데이트를 찾을 수 있도록, 릴리스마다 같은 설치 파일을 옛 이름으로도 올린다. 기한은 두지 않는다.
- 설치 프로그램 ID(`AppId`)와 앱 ID(`ssenu.DockUI`)는 바꾸지 않는다. `AppId`를 바꾸면 "설치된 앱"에 두 개가 생긴다.

## 바꿀 이름

| 항목 | 지금 | 바꾼 뒤 |
|---|---|---|
| 설치 파일 | `YT-Launcher-Setup-<버전>.exe` | `DockUI-Setup-<버전>.exe` (옛 이름 사본도 함께) |
| 릴리스 제목 | `v<버전>` | `Dock UI v<버전>` |
| 실행 파일 | `YTLauncher.exe` | `DockUI.exe` |
| 설치 폴더 | `%LOCALAPPDATA%\Programs\YT-Launcher` | `%LOCALAPPDATA%\Programs\DockUI` |
| 데이터 폴더 (설정·로그·아이콘) | `%APPDATA%\YT-Launcher` | `%APPDATA%\DockUI` |
| 자동 실행 값 (HKCU Run) | `YTLauncher` | `DockUI` |
| 뮤텍스 | `YTLauncherMutex` | `DockUIMutex` |
| 메시지 창 클래스 | `YTLauncherMsg` | `DockUIMsg` |
| 화면 창 클래스 | `YTLauncherGlass`, `YTLauncherContent`, `YTLauncherDialog`, `YTLauncherAppPicker` | `DockUIGlass`, `DockUIContent`, `DockUIDialog`, `DockUIAppPicker` |
| Cargo 패키지 / 바이너리 | `yt-launcher` / `YTLauncher` | `dock-ui` / `DockUI` |
| 리소스 이름 (`build.rs`, `app.manifest`) | `YTLauncher`, `ssenu.YTLauncher` | `DockUI`, `ssenu.DockUI` |
| 업데이트 임시 폴더 | `%TEMP%\YT-Launcher-update` | `%TEMP%\DockUI-update` |
| HTTP 사용자 에이전트 | `YT-Launcher/<버전>` | `DockUI/<버전>` |
| 테스트용 이름 (창 클래스, 레지스트리 키) | `YTLauncherTest…`, `Software\YT-Launcher-Test` | `DockUITest…`, `Software\DockUI-Test` |

문서(README, 점검표, 코드 주석)의 옛 이름도 고친다. 옛 이름을 일부러 다루는 곳(호환 처리, 이전 코드, 그 테스트)은 남긴다.

## 업데이트·설치 경로

### 릴리스 (`release.yml`, `scripts/build.ps1`)

- `build.ps1`이 `DockUI-Setup-<버전>.exe`를 만들고, 같은 파일을 `YT-Launcher-Setup-<버전>.exe`로 복사한다.
- `SHA256SUMS.txt`에 두 줄(새 이름, 옛 이름)을 적는다. 해시는 같다.
- 릴리스 워크플로는 두 파일과 `SHA256SUMS.txt`를 올리고, 제목을 `Dock UI v<버전>`으로 한다.
- 자산 이름 검사: 두 설치 파일이 있고, `SHA256SUMS.txt`의 두 줄이 각각 `^[0-9a-f]{64}  <이름>$` 형식이어야 한다.
- `cargo metadata`의 패키지 이름 검사는 `dock-ui`로 바꾼다.

### 업데이터 (`src/updater.rs`)

- 설치 파일 이름 규칙: `DockUI-Setup-<버전>.exe`와 `YT-Launcher-Setup-<버전>.exe`를 모두 받아들인다. 버전 부분 검사는 지금과 같다.
- 한 릴리스에 둘 다 있으면 새 이름을 쓴다. 옛 이름만 있는 과거 릴리스도 계속 설치할 수 있다.
- `SHA256SUMS.txt`에서 고른 파일 이름의 해시를 찾는다 (지금과 같음).

### 설치 프로그램 (`installer/setup.iss`)

- `AppExe`는 `DockUI.exe`, `DefaultDirName`은 `{localappdata}\Programs\DockUI`, `OutputBaseFilename`은 `DockUI-Setup-{#AppVersion}`.
- `UsePreviousAppDir=no`: 기본값(yes)이면 업그레이드가 옛 폴더에 설치된다.
- 실행 중 감지·종료: 뮤텍스 `YTLauncherMutex`와 `DockUIMutex` 둘 다 확인하고(`CheckForMutexes`는 쉼표로 여러 이름을 받는다), 남아 있으면 `YTLauncher.exe`와 `DockUI.exe`를 모두 종료한다. 자동 업데이트(조용한 설치)에서는 지금처럼 최대 10초 기다린 뒤 종료한다.
- 자동 실행 작업(`autostart`)은 Run 값 `DockUI`에 새 실행 파일 경로를 쓴다.
- 설치가 끝나면(`ssPostInstall`):
  - 옛 Run 값 `YTLauncher`가 있으면 지우고, `DockUI` 값이 없으면 새 실행 파일 경로(따옴표 포함)로 쓴다. 앱 메뉴로 자동 실행을 켰던 사용자도 그대로 자동 실행된다.
  - 옛 설치 폴더 `{localappdata}\Programs\YT-Launcher`가 새 폴더와 다르고 존재하면 통째로 지운다 (옛 실행 파일, 옛 제거 프로그램).
- 바로가기(`Dock UI`)는 새 실행 파일을 가리키게 다시 만든다. 옛 이름(`YT Launcher`) 바로가기 삭제는 지금처럼 둔다.
- 제거할 때 Run 값 `DockUI`와 `YTLauncher`를 모두 지운다. 데이터 폴더는 지금처럼 남긴다.
- 끝나면 지금처럼 새 실행 파일을 다시 실행한다.

### 데이터 폴더 이전 (`src/config.rs`, `src/main.rs`)

앱이 시작할 때, 로그를 열기 전에 데이터 폴더를 정한다.

| `%APPDATA%\DockUI` | `%APPDATA%\YT-Launcher` | 동작 |
|---|---|---|
| 있음 | 상관없음 | 새 폴더를 쓴다. 옛 폴더는 건드리지 않는다 |
| 없음 | 있음 | 옛 폴더를 새 이름으로 옮긴다(`rename`). 실패하면 이번 실행은 옛 폴더를 쓴다 |
| 없음 | 없음 | 새 폴더를 쓴다 (처음 설치) |

- 판단은 순수 함수로 두고, 옮기기 결과(옮김 / 실패와 오류 / 할 일 없음)를 로그를 연 뒤 한 줄로 남긴다.
- 옮기기에 실패하면 다음 실행 때 다시 시도한다. 설정을 복사하지 않는다(두 벌이 생기지 않게).
- 0.1.x 설정 변환(`config.v1.json`)은 이전된 폴더 안에서 지금처럼 동작한다.

## 오류·예외 처리

| 상황 | 동작 |
|---|---|
| 옛 앱이 실행 중에 업데이트 | 설치 프로그램이 옛 뮤텍스로 감지해 기다리고 종료한 뒤 설치한다 |
| 데이터 폴더를 다른 프로그램이 잡고 있어 옮기기 실패 | 옛 폴더로 실행하고 로그에 남긴다. 다음 실행 때 다시 시도 |
| 두 데이터 폴더가 모두 있음 | 새 폴더를 쓴다. 옛 폴더는 지우지 않는다 |
| 옛 설치 폴더 지우기 실패 | 설치는 성공으로 둔다. 남은 옛 실행 파일은 실행되지 않는다(바로가기·Run 값이 새 파일을 가리킴) |
| 0.2.10 이하 앱이 새 릴리스를 확인 | 옛 이름 설치 파일과 그 해시로 업데이트한다 |

## 테스트

### 단위

- 데이터 폴더 판단: 세 경우의 결정, 그리고 임시 폴더로 실제 옮기기(새 폴더 생김·옛 폴더 사라짐·내용 유지)
- 업데이터: 새 이름·옛 이름 설치 파일 인식, 둘 다 있을 때 새 이름 선택, 옛 이름만 있을 때 옛 이름 선택, 두 이름이 섞인 `SHA256SUMS.txt`
- 기존 테스트의 이름·경로 갱신

### 로컬 설치 시험 (릴리스 전, 이 PC)

로컬에서 만든 0.3.0 설치 파일을 지금 설치된 0.2.10 위에 조용한 모드(`/VERYSILENT /SUPPRESSMSGBOXES /NORESTART`)로 설치한다. 시작 전에 `%APPDATA%\YT-Launcher`를 백업하고 Run 값과 제거 정보를 기록한다. 확인할 것:
- `Programs\DockUI\DockUI.exe`가 실행 중이고 `Programs\YT-Launcher`가 없다
- `%APPDATA%\DockUI`에 설정·아이콘·로그가 있고 항목이 그대로다. `%APPDATA%\YT-Launcher`가 없다
- Run 값 `DockUI`가 새 경로를 가리키고 `YTLauncher` 값이 없다
- "설치된 앱"에 Dock UI가 하나만 있고 버전이 0.3.0이다
- 시작 메뉴·바탕화면 바로가기가 새 실행 파일을 가리킨다

### 릴리스 뒤

- 릴리스에 `DockUI-Setup-0.3.0.exe`, `YT-Launcher-Setup-0.3.0.exe`, `SHA256SUMS.txt`(두 줄)가 있고 제목이 `Dock UI v0.3.0`이다.

### 수동 점검표 (`docs/manual-test-checklist.md`)

설치·업데이트 항목의 옛 이름을 고치고 다음을 추가한다:
- 0.2.x에서 업데이트하면 설치 폴더·데이터 폴더·자동 실행 값이 새 이름으로 옮겨지고 항목·설정이 그대로다
- 새 버전 확인 시 0.2.x 앱도 업데이트 단추를 띄운다(옛 이름 설치 파일)
