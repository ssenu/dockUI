# 이름을 Dock UI로 통일 (0.3.0) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 옛 이름(YT Launcher)을 실행 파일·설치 폴더·데이터 폴더·내부 이름·릴리스까지 Dock UI로 바꾸고, 기존 사용자는 자동 업데이트로 그대로 넘어오게 한다.

**Architecture:** 앱은 시작할 때 옛 데이터 폴더를 새 이름으로 옮기고, 업데이터는 새·옛 설치 파일 이름을 모두 알아본다. 설치 프로그램은 새 폴더에 설치한 뒤 옛 설치 폴더와 옛 자동 실행 값을 정리한다. 릴리스는 같은 설치 파일을 새 이름과 옛 이름으로 함께 올린다.

**Tech Stack:** Rust 2024, `windows` 0.62, Inno Setup 6, GitHub Actions(pwsh). 스펙: `docs/superpowers/specs/2026-11-09-rename-dock-ui-design.md`

## Global Constraints

- 브랜치: `feat/rename-dock-ui`. 푸시하지 않는다.
- 코드 주석·문구·커밋 메시지는 한국어. 커밋 머리말 `fix:`, `feat:`, `docs:`, `chore:`.
- 모든 태스크는 끝날 때 `cargo fmt`, `cargo test`, `cargo clippy --all-targets -- -D warnings`를 통과한다.
- 새 의존성을 추가하지 않는다.
- 바꾸지 않는 것: 설치 프로그램 `AppId` `{8F3C2A71-5B4E-4C9D-9A16-3E7D2B0C5F48}`, 앱 ID `ssenu.DockUI`, 업데이트 저장소 `ssenu/dockUI`.
- 새 이름: 실행 파일 `DockUI.exe`, 설치 폴더 `{localappdata}\Programs\DockUI`, 데이터 폴더 `%APPDATA%\DockUI`, Run 값 `DockUI`, 뮤텍스 `DockUIMutex`, 메시지 창 클래스 `DockUIMsg`, 설치 파일 `DockUI-Setup-<버전>.exe`, 릴리스 제목 `Dock UI v<버전>`, Cargo 패키지 `dock-ui`, 바이너리 `DockUI`.
- 옛 이름은 호환 처리에만 남긴다: 옛 데이터 폴더 `YT-Launcher`, 옛 설치 파일 접두어 `YT-Launcher-Setup-`, 옛 설치 폴더 `Programs\YT-Launcher`, 옛 Run 값 `YTLauncher`, 옛 뮤텍스 `YTLauncherMutex`, 옛 실행 파일 `YTLauncher.exe`, 옛 바로가기 `YT Launcher.lnk`.
- 설치된 사용자 앱(`%LOCALAPPDATA%\Programs\YT-Launcher\YTLauncher.exe`)을 끄거나 실행하지 않는다. 설치 프로그램을 실행하지 않는다(설치 시험은 컨트롤러가 한다).

---

### Task 1: 데이터 폴더 이전

**Files:**
- Modify: `src/config.rs` (`default_dir` 대체, 테스트)
- Modify: `src/main.rs`

**Interfaces:**
- Produces: `pub const DATA_DIR_NAME: &str = "DockUI";`, `pub const LEGACY_DATA_DIR_NAME: &str = "YT-Launcher";`, `pub enum DataDirPlan { UseNew, MoveLegacy }`, `pub fn plan_data_dir(new_exists: bool, legacy_exists: bool) -> DataDirPlan`, `pub fn appdata_dir() -> PathBuf`, `pub fn prepare_data_dir(base: &Path) -> (PathBuf, Option<String>)`

- [ ] **Step 1: 실패하는 테스트 작성**

`src/config.rs` 테스트 모듈에 추가:

```rust
    #[test]
    fn data_dir_moves_only_when_new_is_missing_and_legacy_exists() {
        assert_eq!(plan_data_dir(false, true), DataDirPlan::MoveLegacy);
        assert_eq!(plan_data_dir(true, true), DataDirPlan::UseNew);
        assert_eq!(plan_data_dir(true, false), DataDirPlan::UseNew);
        assert_eq!(plan_data_dir(false, false), DataDirPlan::UseNew);
    }

    #[test]
    fn legacy_data_dir_is_moved_with_its_contents() {
        let base = temp_dir("move");
        fs::create_dir_all(base.join(LEGACY_DATA_DIR_NAME).join("icons")).unwrap();
        fs::write(base.join(LEGACY_DATA_DIR_NAME).join(CONFIG_NAME), r#"{"schema":2,"locked":true}"#).unwrap();
        let (dir, note) = prepare_data_dir(&base);
        assert_eq!(dir, base.join(DATA_DIR_NAME));
        assert!(note.is_some_and(|n| n.contains("옮겼습니다")));
        assert!(!base.join(LEGACY_DATA_DIR_NAME).exists());
        assert!(dir.join("icons").is_dir());
        let (config, _) = Storage::new(&dir).load();
        assert!(config.locked, "설정이 그대로");
    }

    #[test]
    fn existing_new_data_dir_wins_and_legacy_is_left_alone() {
        let base = temp_dir("both");
        fs::create_dir_all(base.join(DATA_DIR_NAME)).unwrap();
        fs::create_dir_all(base.join(LEGACY_DATA_DIR_NAME)).unwrap();
        let (dir, note) = prepare_data_dir(&base);
        assert_eq!((dir, note), (base.join(DATA_DIR_NAME), None));
        assert!(base.join(LEGACY_DATA_DIR_NAME).exists());
        // 처음 설치: 아무것도 없으면 새 폴더 경로만 돌려주고 만들지 않는다 (저장할 때 만든다)
        let empty = temp_dir("fresh");
        assert_eq!(prepare_data_dir(&empty), (empty.join(DATA_DIR_NAME), None));
        assert!(!empty.join(DATA_DIR_NAME).exists());
    }
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test data_dir`
Expected: 컴파일 실패 (`plan_data_dir` 등 없음)

- [ ] **Step 3: 구현**

`src/config.rs`의 모듈 주석을 `//! 설정 파일(\`%APPDATA%\DockUI\config.json\`) 읽기·쓰기, 0.1.x(schema 1) 변환, 옛 데이터 폴더 이전.`으로 바꾸고, `default_dir`를 지우고 아래를 넣는다:

```rust
/// 데이터 폴더 이름 (`%APPDATA%` 아래)
pub const DATA_DIR_NAME: &str = "DockUI";
/// 0.2.x까지 쓰던 데이터 폴더 이름. 처음 실행 때 새 이름으로 옮긴다
pub const LEGACY_DATA_DIR_NAME: &str = "YT-Launcher";

/// 데이터 폴더를 정하는 방법
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataDirPlan {
    /// 새 폴더를 쓴다 (이미 있거나 처음 설치)
    UseNew,
    /// 옛 폴더를 새 이름으로 옮긴다
    MoveLegacy,
}

/// 새 폴더가 없고 옛 폴더가 있을 때만 옮긴다 (순수 함수). 둘 다 있으면 새 폴더를 쓰고 옛 폴더는 건드리지 않는다.
pub fn plan_data_dir(new_exists: bool, legacy_exists: bool) -> DataDirPlan {
    if !new_exists && legacy_exists { DataDirPlan::MoveLegacy } else { DataDirPlan::UseNew }
}

/// `%APPDATA%` (없으면 현재 폴더)
pub fn appdata_dir() -> PathBuf {
    std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

/// 쓸 데이터 폴더를 정하고, 필요하면 옛 폴더를 옮긴다. (폴더, 로그에 남길 한 줄)
/// 옮기기에 실패하면 이번 실행은 옛 폴더를 쓰고 다음 실행 때 다시 시도한다 (복사해서 두 벌을 만들지 않는다).
/// 로그를 열기 전에 불리므로 결과는 부른 쪽이 로그에 남긴다.
pub fn prepare_data_dir(base: &Path) -> (PathBuf, Option<String>) {
    let new = base.join(DATA_DIR_NAME);
    let legacy = base.join(LEGACY_DATA_DIR_NAME);
    match plan_data_dir(new.exists(), legacy.exists()) {
        DataDirPlan::UseNew => (new, None),
        DataDirPlan::MoveLegacy => match fs::rename(&legacy, &new) {
            Ok(()) => (new, Some(format!("데이터 폴더를 옮겼습니다: {} → {}", legacy.display(), base.join(DATA_DIR_NAME).display()))),
            Err(e) => (legacy, Some(format!("데이터 폴더를 옮기지 못해 옛 폴더를 씁니다: {e}"))),
        },
    }
}
```

`src/main.rs`:

```rust
    let (data_dir, moved) = config::prepare_data_dir(&config::appdata_dir());
    logging::init(&data_dir.join("logs"));
    if let Some(note) = moved {
        logging::info(&note);
    }
```

(기존 `let data_dir = config::default_dir();`와 `logging::init(...)` 두 줄을 이것으로 바꾼다. 뒤의 뮤텍스 확인·"시작" 로그는 그대로.)

- [ ] **Step 4: 통과 확인**

Run: `cargo test config::` 와 `cargo test` 와 `cargo clippy --all-targets -- -D warnings`
Expected: 모두 통과

- [ ] **Step 5: 커밋**

```bash
cargo fmt
git add src/config.rs src/main.rs
git commit -m "feat: 데이터 폴더를 %APPDATA%\\DockUI로 옮기기"
```

---

### Task 2: 업데이터가 새·옛 설치 파일 이름을 모두 알아보기

**Files:**
- Modify: `src/updater.rs`
- Modify: `src/app.rs` (업데이트 임시 폴더 이름 한 줄)

**Interfaces:**
- Produces: `pub const INSTALLER_PREFIX: &str = "DockUI-Setup-";`, `pub const LEGACY_INSTALLER_PREFIX: &str = "YT-Launcher-Setup-";`, `fn is_installer_name(name: &str, prefix: &str) -> bool`

- [ ] **Step 1: 실패하는 테스트 작성**

`src/updater.rs` 테스트 모듈:

`installer_name_rule_matches_v1_updater`를 아래로 바꾼다:

```rust
    #[test]
    fn installer_name_rule_accepts_new_and_legacy_prefix() {
        for prefix in [INSTALLER_PREFIX, LEGACY_INSTALLER_PREFIX] {
            assert!(is_installer_name(&format!("{prefix}0.2.0.exe"), prefix));
            assert!(is_installer_name(&format!("{prefix}1.0.0-rc_1.exe"), prefix));
            assert!(!is_installer_name(&format!("{prefix}.exe"), prefix));
            assert!(!is_installer_name(&format!("{prefix}0.2.0.exe.sig"), prefix));
            assert!(!is_installer_name(&format!("{prefix}../x.exe"), prefix));
        }
        assert!(!is_installer_name("DockUI-Setup-0.3.0.exe", LEGACY_INSTALLER_PREFIX));
    }
```

추가:

```rust
    fn release_with(names: &[&str]) -> Value {
        let assets: Vec<Value> = names
            .iter()
            .map(|n| json!({"name": n, "browser_download_url": format!("https://dl/{n}"), "size": 1}))
            .collect();
        json!({"tag_name": "v0.3.0", "draft": false, "prerelease": false, "assets": assets})
    }

    #[test]
    fn prefers_new_installer_name_and_falls_back_to_legacy() {
        let both = release_with(&["YT-Launcher-Setup-0.3.0.exe", "DockUI-Setup-0.3.0.exe", "SHA256SUMS.txt"]);
        assert_eq!(parse_release(&both).unwrap().installer.unwrap().name, "DockUI-Setup-0.3.0.exe");
        let legacy = release_with(&["YT-Launcher-Setup-0.2.9.exe", "SHA256SUMS.txt"]);
        assert_eq!(parse_release(&legacy).unwrap().installer.unwrap().name, "YT-Launcher-Setup-0.2.9.exe");
        let new_only = release_with(&["DockUI-Setup-0.3.0.exe", "SHA256SUMS.txt"]);
        assert!(parse_release(&new_only).unwrap().is_installable());
    }

    #[test]
    fn sums_with_both_names_find_the_chosen_installer() {
        let text = format!("{INSTALLER_SHA}  DockUI-Setup-0.3.0.exe\n{INSTALLER_SHA}  YT-Launcher-Setup-0.3.0.exe\n");
        let sums = parse_sums(&text);
        assert_eq!(sums["DockUI-Setup-0.3.0.exe"], INSTALLER_SHA);
        assert_eq!(sums["YT-Launcher-Setup-0.3.0.exe"], INSTALLER_SHA);
    }
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test updater::`
Expected: 컴파일 실패 (`LEGACY_INSTALLER_PREFIX` 없음, `is_installer_name` 인자 수)

- [ ] **Step 3: 구현**

모듈 주석의 둘째 단락을 바꾼다:

```rust
//! 설치 파일은 `DockUI-Setup-<버전>.exe`. 0.2.x 이하 앱은 옛 이름 `YT-Launcher-Setup-<버전>.exe`만 찾으므로
//! 릴리스는 같은 파일을 옛 이름으로도 올리고, 이 업데이터는 둘 다 알아보되 새 이름을 먼저 쓴다.
//! `SHA256SUMS.txt`(`<소문자 해시>  <파일명>`), `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART`는 그대로다.
```

상수:

```rust
pub const INSTALLER_PREFIX: &str = "DockUI-Setup-";
/// 0.2.x 이하가 찾는 옛 설치 파일 이름 (릴리스에 같은 파일을 함께 올린다)
pub const LEGACY_INSTALLER_PREFIX: &str = "YT-Launcher-Setup-";
```

`is_installer_name`:

```rust
fn is_installer_name(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix).and_then(|rest| rest.strip_suffix(".exe")).is_some_and(|ver| {
        !ver.is_empty() && ver.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    })
}
```

`parse_release`의 자산 루프:

```rust
    let mut installer = None;
    let mut legacy = None;
    let mut sums = None;
    for a in data.get("assets").and_then(Value::as_array).into_iter().flatten() {
        let (Some(name), Some(url)) =
            (a.get("name").and_then(Value::as_str), a.get("browser_download_url").and_then(Value::as_str))
        else {
            continue;
        };
        let asset = Asset {
            name: name.to_string(),
            url: url.to_string(),
            size: a.get("size").and_then(Value::as_u64).unwrap_or(0),
        };
        if name == SUMS_NAME {
            sums = Some(asset);
        } else if installer.is_none() && is_installer_name(name, INSTALLER_PREFIX) {
            installer = Some(asset);
        } else if legacy.is_none() && is_installer_name(name, LEGACY_INSTALLER_PREFIX) {
            legacy = Some(asset);
        }
    }
```

`Release { .. installer, .. }`를 `installer: installer.or(legacy),`로 바꾼다.

사용자 에이전트: `.user_agent(format!("DockUI/{}", env!("CARGO_PKG_VERSION")))`

`src/app.rs`: `std::env::temp_dir().join("YT-Launcher-update")`를 `std::env::temp_dir().join("DockUI-update")`로 바꾼다.

다른 기존 테스트(`release_json`, `download_and_verify_success`, `hash_mismatch_leaves_no_installer_or_part`, `parses_sha256sums`)는 옛 이름을 그대로 써도 통과한다. 옛 이름만 있는 릴리스도 설치할 수 있어야 하므로 그대로 둔다.

- [ ] **Step 4: 통과 확인**

Run: `cargo test updater::` 와 `cargo test` 와 `cargo clippy --all-targets -- -D warnings`
Expected: 모두 통과

- [ ] **Step 5: 커밋**

```bash
cargo fmt
git add src/updater.rs src/app.rs
git commit -m "feat: 업데이터가 새 설치 파일 이름(DockUI-Setup)을 먼저 쓰고 옛 이름도 알아보기"
```

---

### Task 3: 내부 이름 바꾸기 (패키지·실행 파일·리소스·뮤텍스·창 클래스·Run 값)

**Files:**
- Modify: `Cargo.toml`, `Cargo.lock`, `build.rs`, `assets/app.manifest`
- Modify: `src/win/instance.rs`, `src/autostart.rs`, `src/ui/panel.rs`, `src/ui/dialogs.rs`, `src/ui/app_list.rs`, `src/win/msg_window.rs`, `src/win/reg.rs`, `src/app.rs`, `src/main.rs`, `src/icons.rs`, `src/logging.rs`

- [ ] **Step 1: 바꾸기**

| 파일 | 지금 | 바꾼 뒤 |
|---|---|---|
| `Cargo.toml` | `name = "yt-launcher"`, `version = "0.2.0"`, `[[bin]] name = "YTLauncher"` | `name = "dock-ui"`, `version = "0.3.0"`, `name = "DockUI"` |
| `build.rs` | `"InternalName", "YTLauncher"`, `"OriginalFilename", "YTLauncher.exe"` | `"DockUI"`, `"DockUI.exe"` |
| `assets/app.manifest` | `name="ssenu.YTLauncher"` | `name="ssenu.DockUI"` |
| `src/win/instance.rs` | `MUTEX_NAME = "YTLauncherMutex"`, `MESSAGE_CLASS = "YTLauncherMsg"`, 테스트 `"YTLauncherTest-{}"` | `"DockUIMutex"`, `"DockUIMsg"`, `"DockUITest-{}"` |
| `src/autostart.rs` | 모듈 주석 `` `YTLauncher` 값``, `VALUE_NAME = "YTLauncher"`, 테스트 `"YTLauncherTest{}"`, `yt-launcher-test.exe` | `` `DockUI` 값``, `"DockUI"`, `"DockUITest{}"`, `dock-ui-test.exe` |
| `src/ui/panel.rs` | `GLASS_CLASS = "YTLauncherGlass"`, `CONTENT_CLASS = "YTLauncherContent"` | `"DockUIGlass"`, `"DockUIContent"` |
| `src/ui/dialogs.rs` | `DIALOG_CLASS = "YTLauncherDialog"`, 테스트의 `"YTLauncherTest…"` | `"DockUIDialog"`, `"DockUITest…"` |
| `src/ui/app_list.rs` | `PICKER_CLASS = "YTLauncherAppPicker"`, 테스트 `"YTLauncherTestPickerOwner"` | `"DockUIAppPicker"`, `"DockUITestPickerOwner"` |
| `src/win/msg_window.rs`, `src/app.rs` 테스트 | `"YTLauncherTest…"` | `"DockUITest…"` |
| `src/win/reg.rs` 테스트 | `r"Software\YT-Launcher-Test"` | `r"Software\DockUI-Test"` |
| `src/main.rs` | `//! YT Launcher 진입점.` | `//! Dock UI 진입점.` |
| `src/icons.rs` | 모듈 주석 `` `%APPDATA%\YT-Launcher\icons\…` ``, `` `YTLauncher.exe --fetch-icons …` `` | `` `%APPDATA%\DockUI\icons\…` ``, `` `DockUI.exe --fetch-icons …` `` |
| `src/logging.rs` | 모듈 주석 `` `%APPDATA%\YT-Launcher\logs\launcher.log` `` | `` `%APPDATA%\DockUI\logs\launcher.log` `` |

`src/app.rs`의 `APP_NAME` 주석 "(exe·설치 파일·데이터 폴더 이름은 업데이트 호환을 위해 그대로 둔다)"를 지운다. 같은 파일의 `startup_notices` 등 다른 코드는 바꾸지 않는다.

`Cargo.lock`은 `cargo build`로 갱신한다(패키지 이름·버전 줄).

- [ ] **Step 2: 남은 옛 이름 확인**

Run: `grep -rn "YTLauncher\|YT-Launcher\|yt-launcher\|YT Launcher" src build.rs Cargo.toml assets/app.manifest`
Expected: 아래만 남는다. 그 밖의 것이 있으면 이 태스크 범위인지 판단해 고치고 보고서에 적는다.
- `src/config.rs`: `LEGACY_DATA_DIR_NAME`과 그 주석, 테스트
- `src/updater.rs`: `LEGACY_INSTALLER_PREFIX`, 모듈 주석, 옛 이름을 쓰는 테스트

- [ ] **Step 3: 확인**

Run: `cargo build` 와 `cargo test` 와 `cargo clippy --all-targets -- -D warnings`
Expected: 모두 통과. `target\debug\DockUI.exe`가 생긴다.

- [ ] **Step 4: 커밋**

```bash
cargo fmt
git add Cargo.toml Cargo.lock build.rs assets/app.manifest src
git commit -m "chore: 패키지·실행 파일·뮤텍스·창 클래스·자동 실행 값을 Dock UI 이름으로"
```

---

### Task 4: 설치 프로그램·빌드 스크립트·릴리스 워크플로

**Files:**
- Modify: `installer/setup.iss`
- Modify: `scripts/build.ps1`
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: `installer/setup.iss`**

머리 주석과 정의를 바꾼다:

```
; Dock UI 설치 스크립트. scripts/build.ps1이 /DAppVersion=<버전>을 넘긴다.
#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#define AppName "Dock UI"
#define AppExe "DockUI.exe"
; 0.2.x(옛 이름 YT Launcher)의 실행 파일·설치 폴더. 업데이트 때 정리한다
#define LegacyExe "YTLauncher.exe"
#define LegacyDir "{localappdata}\Programs\YT-Launcher"
```

`[Setup]`:
- `AppId` 위에 주석 `; AppId는 옛 이름 때부터 같다. 바꾸면 "설치된 앱"에 두 개가 생긴다`
- `DefaultDirName={localappdata}\Programs\DockUI`
- 그 아래 추가:
  ```
  ; 기본값이면 업그레이드가 옛 설치 폴더(YT-Launcher)에 그대로 설치된다. 새 폴더에 설치하고 옛 폴더는 설치 뒤 지운다
  UsePreviousAppDir=no
  ```
- `OutputBaseFilename=DockUI-Setup-{#AppVersion}`

`[InstallDelete]`: `{app}\_internal` 줄과 그 주석을 지운다(새 폴더에는 없다). 옛 바로가기 두 줄은 둔다.

`[Files]`: `Source: "..\dist\DockUI\DockUI.exe"; DestDir: "{app}"; Flags: ignoreversion`

`[Registry]`: `ValueName: "DockUI"` (나머지 그대로)

`[Code]` 전체를 아래로 바꾼다:

```pascal
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
```

- [ ] **Step 2: `scripts/build.ps1`**

- 스테이지: `$stage = Join-Path $root "dist\DockUI"`, `Copy-Item (Join-Path $root "target\release\DockUI.exe") $stage`
- 끝부분을 바꾼다:

```powershell
$setupName = "DockUI-Setup-$Version.exe"
$legacyName = "YT-Launcher-Setup-$Version.exe"
$installerDir = Join-Path $root "dist\installer"
$setupPath = Join-Path $installerDir $setupName
if (-not (Test-Path $setupPath)) { throw "설치 프로그램이 없습니다: $setupPath" }
# 0.2.x 이하 앱은 옛 이름의 설치 파일만 찾는다: 같은 파일을 옛 이름으로도 둔다
Copy-Item $setupPath (Join-Path $installerDir $legacyName) -Force
$hash = (Get-FileHash $setupPath -Algorithm SHA256).Hash.ToLower()
@("$hash  $setupName", "$hash  $legacyName") | Set-Content (Join-Path $installerDir "SHA256SUMS.txt") -Encoding ascii
Write-Host "Done: $setupPath (+ $legacyName)"
```

- [ ] **Step 3: `.github/workflows/release.yml`**

- `Where-Object name -eq "yt-launcher"` → `Where-Object name -eq "dock-ui"`
- `Check asset names` 단계의 `run:`을 바꾼다:

```powershell
          $dir = "dist/installer"
          $names = @("DockUI-Setup-$env:APP_VERSION.exe", "YT-Launcher-Setup-$env:APP_VERSION.exe")
          foreach ($n in $names) { if (-not (Test-Path "$dir/$n")) { throw "설치 파일이 없습니다: $n" } }
          $lines = @(Get-Content "$dir/SHA256SUMS.txt" | Where-Object { $_ })
          if ($lines.Count -ne 2) { throw "SHA256SUMS.txt 줄 수 오류: $($lines.Count)" }
          for ($i = 0; $i -lt 2; $i++) {
            if ($lines[$i] -notmatch "^[0-9a-f]{64}  $([regex]::Escape($names[$i]))$") { throw "SHA256SUMS.txt 형식 오류: $($lines[$i])" }
          }
```

- `Publish release`의 `with:`에 `name: Dock UI v${{ env.APP_VERSION }}`를 더하고 `files:`를 바꾼다:

```yaml
          files: |
            dist/installer/DockUI-Setup-${{ env.APP_VERSION }}.exe
            dist/installer/YT-Launcher-Setup-${{ env.APP_VERSION }}.exe
            dist/installer/SHA256SUMS.txt
```

- [ ] **Step 4: 로컬 빌드로 확인**

Run (PowerShell): `./scripts/build.ps1 -Version 0.3.0`
Expected: `dist\installer\`에 `DockUI-Setup-0.3.0.exe`, `YT-Launcher-Setup-0.3.0.exe`, `SHA256SUMS.txt`(두 줄, 같은 해시)가 생긴다. 이어서 위 `Check asset names` 스크립트를 `$env:APP_VERSION='0.3.0'`로 로컬에서 돌려 통과하는지 본다. **설치 파일을 실행하지 않는다.**

- [ ] **Step 5: 커밋**

```bash
git add installer/setup.iss scripts/build.ps1 .github/workflows/release.yml
git commit -m "feat: 설치 프로그램을 DockUI 폴더로 옮기고 옛 설치·자동 실행 정리, 릴리스에 새·옛 이름 설치 파일"
```

---

### Task 5: 문서

**Files:**
- Modify: `README.md`, `docs/manual-test-checklist.md`

- [ ] **Step 1: README**

- 3번 줄 `(옛 이름 YT Launcher)`는 둔다.
- 설치 단락: `` `YT-Launcher-Setup-<버전>.exe` ``를 `` `DockUI-Setup-<버전>.exe` ``로.
- 0.1.x 문장의 원본 경로 `` `%APPDATA%\YT-Launcher\config.v1.json` ``을 `` `%APPDATA%\DockUI\config.v1.json` ``으로.
- 설치 단락 끝에 추가: `0.2.x를 쓰고 있었다면 업데이트 때 설치 폴더·설정 폴더·자동 실행이 새 이름(DockUI)으로 옮겨진다.`
- 릴리스 단락의 예시 태그를 `v0.3.0`으로.

- [ ] **Step 2: 점검표**

`## 설치·업데이트`:
- `"설정 → 앱 → 설치된 앱"에 YT Launcher와 버전이 보인다` → `"설정 → 앱 → 설치된 앱"에 Dock UI가 하나만 있고 버전이 보인다`
- `` `%APPDATA%\YT-Launcher`는 남는다`` → `` `%APPDATA%\DockUI`는 남는다``
- 0.1.4 → 0.2.0 업데이트 두 줄은 과거 기록이므로 둔다.
- 끝에 추가:
  - `- [ ] 0.2.x에서 업데이트하면 Programs\YT-Launcher가 지워지고 Programs\DockUI\DockUI.exe로 실행되며, %APPDATA%\DockUI로 옮겨진 설정·항목·아이콘이 그대로다`
  - `- [ ] 자동 실행이 켜져 있었다면 업데이트 뒤 Run 값이 DockUI 하나뿐이고 새 실행 파일을 가리킨다`
  - `- [ ] 릴리스에 DockUI-Setup·YT-Launcher-Setup 두 설치 파일과 SHA256SUMS.txt(두 줄)가 있고, 0.2.x 앱도 업데이트 단추를 띄운다`

- [ ] **Step 3: 커밋**

```bash
git add README.md docs/manual-test-checklist.md
git commit -m "docs: Dock UI 이름 변경 안내와 업데이트 점검 항목"
```

---

### Task 6 (컨트롤러): 이 PC에서 업그레이드 시험

서브에이전트에 맡기지 않는다. Task 4의 로컬 설치 파일로 한다.

1. 백업: `%APPDATA%\YT-Launcher`를 스크래치 폴더로 복사, Run 값 `YTLauncher`·`DockUI`와 제거 정보(`HKCU\...\Uninstall\{8F3C2A71-...}_is1`) 기록.
2. `dist\installer\DockUI-Setup-0.3.0.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART` 실행(0.2.10이 실행 중인 상태에서), 끝날 때까지 기다림.
3. 확인: 스펙 "로컬 설치 시험"의 다섯 항목 + 새 앱 로그에 "데이터 폴더를 옮겼습니다".
4. 실패하면: 새 앱을 끄고, 백업 설정을 `%APPDATA%\YT-Launcher`로 되돌리고, `v0.2.10` 설치 파일로 다시 설치해 원상복구한 뒤 원인을 고친다.
