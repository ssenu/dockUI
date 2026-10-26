# Rust 독 런처 — 1단계(기반) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 화면 없이 트레이 아이콘만으로 실행되는 Rust 런처 기반을 만든다. 설정(0.1.x 변환 포함), 항목 모델, 단일 인스턴스, 트레이 메뉴, 자동 실행, GitHub 업데이트(확인·다운로드·해시 검증·조용한 설치), 크롬 실행 모듈, 파일 로그까지.

**Architecture:** 저장소 루트에 Cargo 크레이트(`yt-launcher`, 바이너리 `YTLauncher.exe`)를 추가한다. 파이썬 0.1.x 코드는 5단계까지 그대로 둔다. 판단 로직(`items`, `config`, `updater`, `chrome`의 URL, `app`의 결정 함수)은 순수 Rust로 두고 `cargo test`로 검증한다. Win32 호출은 `src/win/` 아래 얇은 래퍼에 모은다. 숨은 메시지 창 하나가 트레이·타이머·백그라운드 작업 결과를 받는다.

**Tech Stack:** Rust 1.99 stable (edition 2024, `x86_64-pc-windows-msvc`), `windows` 0.62, `serde`/`serde_json`, `ureq` 3 (native-tls = Windows schannel), `sha2` 0.11, `embed-resource` 3 (빌드).

**Spec:** `docs/superpowers/specs/2026-10-26-dock-launcher-rust-design.md` (10장의 1단계)

**검증 메모:** 이 계획의 코드는 별도 프로토타입 크레이트에서 그대로 빌드해 확인했다. `cargo test` 35개 통과, `cargo clippy --all-targets -- -D warnings` 통과, 릴리스 exe 약 0.9MB, 실행 시 작업 집합 약 12.6MB, 두 번째 실행은 즉시 종료.

## Global Constraints

- 작업 브랜치 `rust-rewrite`. 파이썬 코드(`src/yt_launcher`, `tests/*.py`, `pyproject.toml`, `scripts`, `installer`)는 이 단계에서 건드리지 않는다.
- 크레이트 이름 `yt-launcher`, 버전 `0.2.0`, 바이너리 이름 `YTLauncher` (→ `YTLauncher.exe`).
- 모든 명령은 저장소 루트 `C:\MyCode\YT-Launcher`에서 PowerShell로 실행. cargo가 PATH에 없으면 먼저 `$env:Path += ";$env:USERPROFILE\.cargo\bin"`.
- 저장소 `ssenu/YT-launcher`, 뮤텍스 `YTLauncherMutex`, 숨은 창 클래스 `YTLauncherMsg`, Run 값 이름 `YTLauncher`, Run 키 `Software\Microsoft\Windows\CurrentVersion\Run`.
- 설정 `%APPDATA%\YT-Launcher\config.json` (schema 2), 손상 시 `config.json.bak`, 0.1.x 원본 백업 `config.v1.json`. 로그 `%APPDATA%\YT-Launcher\logs\launcher.log` (1MB, 백업 3개).
- 업데이트 호환 계약: 정식 릴리스만, 자산 `YT-Launcher-Setup-<버전>.exe` + `SHA256SUMS.txt`(`<소문자 해시>  <파일명>`), 조용한 설치 `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART`, 첫 확인 30초 후, 이후 24시간마다.
- 유튜브 검색 URL: `https://www.youtube.com/results?search_query=` + 파이썬 `quote_plus`와 같은 인코딩.
- UI 문구와 코드 주석은 한국어.
- 각 태스크 끝에 `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`가 모두 통과해야 커밋한다.
- `rustfmt.toml`: `max_width = 120`, `use_small_heuristics = "Max"`.

## File Structure

```
Cargo.toml, Cargo.lock, rustfmt.toml, build.rs, app.rc
assets/icon.ico, assets/app.manifest
testdata/config_v1.json
src/main.rs           진입점 (단일 인스턴스 → 메시지 창 → 메시지 루프)
src/items.rs          Item(유튜브/앱/웹), URL 정규화, id 생성
src/config.rs         Config(schema 2), Storage(원자적 저장·복구), schema 1 변환
src/updater.rs        릴리스 조회·버전 비교·다운로드·해시 검증·설치 실행 (Http 트레이트)
src/autostart.rs      Run 키 자동 실행
src/chrome.rs         검색 URL, 크롬 탐색·실행·최대화
src/logging.rs        파일 로그와 패닉 훅
src/tray.rs           트레이 아이콘·풍선 알림·팝업 메뉴
src/app.rs            App 컨트롤러 (메뉴, 타이머, 업데이트 흐름, 자동 실행 동기화)
src/win/mod.rs        래퍼 모듈 목록
src/win/reg.rs        레지스트리 문자열 값
src/win/windows.rs    최상위 창 목록, 최대화·복원·전면화
src/win/shell.rs      ShellExecute
src/win/instance.rs   뮤텍스, 기존 인스턴스 알림
src/win/msg_window.rs 숨은 창과 메시지 루프 (재진입 안전)
```

---
### Task 1: Rust 개발 환경과 크레이트 뼈대

**Files:**
- Create: `Cargo.toml`, `rustfmt.toml`, `build.rs`, `app.rc`, `assets/app.manifest`, `src/main.rs`
- Create: `assets/icon.ico` (기존 `src/yt_launcher/assets/icon.ico` 복사)
- Modify: `.gitignore`

**Interfaces:**
- Consumes: 없음
- Produces: 빌드 가능한 `YTLauncher.exe` (아이콘 리소스 1번, 매니페스트: PerMonitorV2 DPI, 공용 컨트롤 v6, UTF-8 코드 페이지). Task 7의 `tray::ICON_RESOURCE_ID = 1`이 이 리소스를 읽는다.

- [ ] **Step 1: Rust 설치 확인**

Run:
```powershell
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { winget install -e --id Rustlang.Rustup --silent --accept-source-agreements --accept-package-agreements }
$env:Path += ";$env:USERPROFILE\.cargo\bin"
rustup default stable
cargo --version
```
Expected: `cargo 1.99` 이상. (MSVC 빌드 도구는 이 PC에 설치되어 있음)

- [ ] **Step 2: Cargo.toml 작성**

```toml
[package]
name = "yt-launcher"
version = "0.2.0"
edition = "2024"
description = "바탕화면 독에서 유튜브·앱·웹 주소를 여는 런처"

[[bin]]
name = "YTLauncher"
path = "src/main.rs"

[dependencies]
serde = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.154"
sha2 = "0.11.1"
ureq = { version = "3.5.0", default-features = false, features = ["native-tls"] }
windows = { version = "0.62.2", features = [
    "Win32_Foundation",
    "Win32_Graphics_Gdi",
    "Win32_Security",
    "Win32_System_LibraryLoader",
    "Win32_System_Registry",
    "Win32_System_SystemInformation",
    "Win32_System_Threading",
    "Win32_UI_Shell",
    "Win32_UI_WindowsAndMessaging",
] }

[build-dependencies]
embed-resource = "3.0.12"

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

- [ ] **Step 3: rustfmt.toml, build.rs, app.rc, 매니페스트 작성**

`rustfmt.toml`:
```toml
max_width = 120
use_small_heuristics = "Max"
```

`build.rs`:
```rust
fn main() {
    println!("cargo:rerun-if-changed=app.rc");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=assets/app.manifest");
    embed_resource::compile("app.rc", embed_resource::NONE).manifest_required().unwrap();
}
```

`app.rc`:
```text
// 앱 아이콘(리소스 1번)과 매니페스트(DPI 인식, 공용 컨트롤 v6)
1 ICON "assets/icon.ico"
1 24 "assets/app.manifest"
```

`assets/app.manifest`:
```xml
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity type="win32" name="ssenu.YTLauncher" version="1.0.0.0"/>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0"
        processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
      <activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>
    </windowsSettings>
  </application>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
</assembly>
```

- [ ] **Step 4: 아이콘 복사와 .gitignore**

Run:
```powershell
New-Item -ItemType Directory -Force assets | Out-Null
Copy-Item src\yt_launcher\assets\icon.ico assets\icon.ico
Add-Content .gitignore "/target/"
```

- [ ] **Step 5: 최소 main.rs**

```rust
#![cfg_attr(not(test), windows_subsystem = "windows")]
//! YT Launcher 진입점.
// 크롬 실행·항목 모델은 2단계(화면)에서 연결된다. 그때 이 줄을 지운다.
#![allow(dead_code)]


fn main() {}
```

- [ ] **Step 6: 빌드 확인**

Run: `cargo build`
Expected: `Finished` 출력, `target\debug\YTLauncher.exe` 생성. 실행하면 아무 창 없이 바로 종료된다(정상).

- [ ] **Step 7: Commit**

```powershell
cargo fmt
git add Cargo.toml Cargo.lock rustfmt.toml build.rs app.rc assets src/main.rs .gitignore
git commit -m "chore: Rust 크레이트 뼈대 추가"
```

---

### Task 2: 항목 모델 (items.rs)

**Files:**
- Create: `src/items.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `sha2`
- Produces: `enum Item { Youtube{id,query,name}, App{id,name,target,exe:Option<String>}, Web{id,url,name} }` (serde `kind` 태그, 소문자), `Item::id(&self)->&str`, `Item::display_name(&self)->String`, `Item::youtube(query,name)->Result<Item,ItemError>`, `Item::web(url,name)->Result<Item,ItemError>`, `enum ItemError{EmptyQuery,InvalidUrl}`, `normalize_url(&str)->Result<String,ItemError>`, `host_of(&str)->Option<&str>`, `new_id()->String`(32자 16진수)

- [ ] **Step 1: src/items.rs 작성 (구현 + 테스트)**

```rust
//! 독에 놓이는 항목(유튜브 / 앱 / 웹 주소)과 입력 검증.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Item {
    Youtube {
        id: String,
        query: String,
        #[serde(default)]
        name: String,
    },
    App {
        id: String,
        name: String,
        /// shell:AppsFolder 파싱 이름(Store 앱은 AUMID) 또는 직접 고른 exe/lnk 경로
        target: String,
        /// 실행 중 판정에 쓰는 실제 exe 경로(알 수 있을 때만)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exe: Option<String>,
    },
    Web {
        id: String,
        url: String,
        #[serde(default)]
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemError {
    EmptyQuery,
    InvalidUrl,
}

impl Item {
    pub fn id(&self) -> &str {
        match self {
            Item::Youtube { id, .. } | Item::App { id, .. } | Item::Web { id, .. } => id,
        }
    }

    /// 말풍선에 보이는 이름. 비어 있으면 검색어나 호스트명을 쓴다.
    pub fn display_name(&self) -> String {
        match self {
            Item::Youtube { query, name, .. } => non_empty(name).unwrap_or(query).to_string(),
            Item::App { name, .. } => name.clone(),
            Item::Web { url, name, .. } => {
                non_empty(name).map(str::to_string).unwrap_or_else(|| host_of(url).unwrap_or(url).to_string())
            }
        }
    }

    pub fn youtube(query: &str, name: &str) -> Result<Item, ItemError> {
        let query = query.trim();
        if query.is_empty() {
            return Err(ItemError::EmptyQuery);
        }
        Ok(Item::Youtube { id: new_id(), query: query.to_string(), name: name.trim().to_string() })
    }

    pub fn web(url: &str, name: &str) -> Result<Item, ItemError> {
        Ok(Item::Web { id: new_id(), url: normalize_url(url)?, name: name.trim().to_string() })
    }
}

fn non_empty(s: &str) -> Option<&str> {
    let t = s.trim();
    (!t.is_empty()).then_some(t)
}

/// `naver.com` → `https://naver.com`. http/https가 아니거나 호스트가 없으면 거부.
pub fn normalize_url(input: &str) -> Result<String, ItemError> {
    let input = input.trim();
    if input.is_empty() || input.chars().any(char::is_whitespace) {
        return Err(ItemError::InvalidUrl);
    }
    let url = if input.contains("://") { input.to_string() } else { format!("https://{input}") };
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err(ItemError::InvalidUrl);
    }
    let host_ok = host_of(&url)
        .is_some_and(|h| !h.starts_with('.') && h.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-'));
    let port_ok = port_of(&url).is_none_or(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()));
    if host_ok && port_ok { Ok(url) } else { Err(ItemError::InvalidUrl) }
}

fn authority_of(url: &str) -> Option<&str> {
    let rest = url.split_once("://")?.1;
    let authority = rest.split(['/', '?', '#']).next()?;
    Some(authority.rsplit_once('@').map_or(authority, |(_, h)| h))
}

fn port_of(url: &str) -> Option<&str> {
    authority_of(url)?.split_once(':').map(|(_, port)| port)
}

/// `https://www.github.com:443/a?b` → `www.github.com`
pub fn host_of(url: &str) -> Option<&str> {
    let host = authority_of(url)?.split(':').next()?;
    (!host.is_empty()).then_some(host)
}

/// 32자리 16진수 id (시간 + 프로세스 + 카운터를 SHA-256으로 섞음)
pub fn new_id() -> String {
    use sha2::{Digest, Sha256};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let seed = format!("{nanos}-{}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed));
    let digest = Sha256::digest(seed.as_bytes());
    digest.iter().take(16).map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn youtube_requires_query_and_trims() {
        assert_eq!(Item::youtube("   ", ""), Err(ItemError::EmptyQuery));
        let item = Item::youtube("  로파이 음악 ", " ").unwrap();
        match &item {
            Item::Youtube { query, name, id } => {
                assert_eq!(query, "로파이 음악");
                assert_eq!(name, "");
                assert_eq!(id.len(), 32);
            }
            _ => panic!("wrong kind"),
        }
        assert_eq!(item.display_name(), "로파이 음악");
    }

    #[test]
    fn normalize_url_adds_https_and_rejects_bad_input() {
        assert_eq!(normalize_url("naver.com").unwrap(), "https://naver.com");
        assert_eq!(normalize_url(" http://a.b/c ").unwrap(), "http://a.b/c");
        assert_eq!(normalize_url("HTTPS://GitHub.com").unwrap(), "HTTPS://GitHub.com");
        assert_eq!(normalize_url(""), Err(ItemError::InvalidUrl));
        assert_eq!(normalize_url("ftp://x.com"), Err(ItemError::InvalidUrl));
        assert_eq!(normalize_url("javascript:alert(1)"), Err(ItemError::InvalidUrl));
        assert_eq!(normalize_url("https://"), Err(ItemError::InvalidUrl));
        assert_eq!(normalize_url("has space.com"), Err(ItemError::InvalidUrl));
    }

    #[test]
    fn web_display_name_defaults_to_host() {
        let item = Item::web("https://user@www.github.com:443/x?y", "").unwrap();
        assert_eq!(item.display_name(), "www.github.com");
        let named = Item::web("github.com", "깃허브").unwrap();
        assert_eq!(named.display_name(), "깃허브");
    }

    #[test]
    fn ids_are_unique() {
        assert_ne!(new_id(), new_id());
    }

    #[test]
    fn serde_uses_kind_tag() {
        let item = Item::App { id: "a".into(), name: "계산기".into(), target: "Calc!App".into(), exe: None };
        let json = serde_json::to_string(&item).unwrap();
        assert_eq!(json, r#"{"kind":"app","id":"a","name":"계산기","target":"Calc!App"}"#);
        let back: Item = serde_json::from_str(&json).unwrap();
        assert_eq!(back, item);
    }
}
```

- [ ] **Step 2: main.rs에 모듈 등록**

```rust
#![cfg_attr(not(test), windows_subsystem = "windows")]
//! YT Launcher 진입점.
// 크롬 실행·항목 모델은 2단계(화면)에서 연결된다. 그때 이 줄을 지운다.
#![allow(dead_code)]

mod items;

fn main() {}
```

- [ ] **Step 3: 테스트**

Run: `cargo test items`
Expected: 5 passed

- [ ] **Step 4: 전체 검사**

Run: `cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test`
Expected: clippy 오류 없음, `test result: ok`

- [ ] **Step 5: Commit**

```powershell
git add src/items.rs src/main.rs
git commit -m "feat: 항목 모델과 웹 주소 정규화"
```

---

### Task 3: 설정과 0.1.x 변환 (config.rs)

**Files:**
- Create: `src/config.rs`, `testdata/config_v1.json`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `items::{Item, new_id}`
- Produces: `Config`(schema, items, mode, dock_edge, panel_window, theme, locked, autostart:Option<bool>, last_run_version, notified_version, chrome_notice_shown), `enum Mode{Dock,Panel}`, `enum DockEdge{Bottom,Top,Left,Right}`, `enum Theme{Light,Dark,Sepia,Glass}`, `Rect{x,y,w,h}`, `LoadReport{recovered,migrated}`, `default_dir()->PathBuf`, `Storage::new(dir)`, `Storage::load()->(Config,LoadReport)`, `Storage::save(&Config)->io::Result<()>`, `migrate_v1(&Value)->Option<Config>`

- [ ] **Step 1: 0.1.4 형식 샘플: testdata/config_v1.json**

```json
{
  "schema": 1,
  "tiles": [
    {"query": "로파이 음악", "name": "", "color": "#E53935", "id": "0f3c9a8e2b7d4c1e9a6b5d4c3b2a1f0e"},
    {"query": "뉴스", "name": "아침 뉴스", "color": "", "id": "1a2b3c4d5e6f708192a3b4c5d6e7f809"}
  ],
  "window": {"x": 1544, "y": 16, "w": 360, "h": 480},
  "locked": true,
  "autostart": false,
  "last_run_version": "0.1.4",
  "notified_version": "0.1.4",
  "chrome_notice_shown": true,
  "theme": "sepia"
}
```

- [ ] **Step 2: src/config.rs 작성 (구현 + 테스트)**

```rust
//! 설정 파일(`%APPDATA%\YT-Launcher\config.json`) 읽기·쓰기와 0.1.x(schema 1) 변환.

use crate::items::Item;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const SCHEMA: u32 = 2;
pub const CONFIG_NAME: &str = "config.json";
pub const BACKUP_NAME: &str = "config.json.bak";
pub const V1_BACKUP_NAME: &str = "config.v1.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Dock,
    Panel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DockEdge {
    #[default]
    Bottom,
    Top,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    Sepia,
    #[default]
    Glass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub schema: u32,
    pub items: Vec<Item>,
    #[serde(deserialize_with = "lenient")]
    pub mode: Mode,
    #[serde(deserialize_with = "lenient")]
    pub dock_edge: DockEdge,
    pub panel_window: Option<Rect>,
    #[serde(deserialize_with = "lenient")]
    pub theme: Theme,
    pub locked: bool,
    pub autostart: Option<bool>,
    pub last_run_version: String,
    pub notified_version: String,
    pub chrome_notice_shown: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            schema: SCHEMA,
            items: Vec::new(),
            mode: Mode::Dock,
            dock_edge: DockEdge::Bottom,
            panel_window: None,
            theme: Theme::Glass,
            locked: false,
            autostart: None,
            last_run_version: String::new(),
            notified_version: String::new(),
            chrome_notice_shown: false,
        }
    }
}

/// 알 수 없는 enum 값은 오류 대신 기본값으로 읽는다.
fn lenient<'de, D, T>(de: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    let value = Value::deserialize(de)?;
    Ok(T::deserialize(value).unwrap_or_default())
}

/// 불러오기 결과에 덧붙는 정보 (알림을 띄울지 결정)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LoadReport {
    /// 파일이 깨져서 .bak으로 옮기고 기본값으로 시작함
    pub recovered: bool,
    /// 0.1.x 설정을 변환함
    pub migrated: bool,
}

pub fn default_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("YT-Launcher")
}

pub struct Storage {
    pub dir: PathBuf,
}

impl Storage {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Storage { dir: dir.into() }
    }

    pub fn path(&self) -> PathBuf {
        self.dir.join(CONFIG_NAME)
    }

    pub fn load(&self) -> (Config, LoadReport) {
        let path = self.path();
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return (Config::default(), LoadReport::default()),
            Err(_) => return self.recover(&path),
        };
        let parsed: Option<Value> = std::str::from_utf8(&bytes)
            .ok()
            .map(|s| s.trim_start_matches('\u{feff}'))
            .and_then(|s| serde_json::from_str(s).ok());
        let Some(value) = parsed else { return self.recover(&path) };
        if value.get("schema").and_then(Value::as_u64) == Some(1) {
            return match migrate_v1(&value) {
                Some(config) => {
                    let _ = fs::copy(&path, self.dir.join(V1_BACKUP_NAME));
                    let _ = self.save(&config);
                    (config, LoadReport { recovered: false, migrated: true })
                }
                None => self.recover(&path),
            };
        }
        match serde_json::from_value::<Config>(value) {
            Ok(mut config) => {
                config.schema = SCHEMA;
                (config, LoadReport::default())
            }
            Err(_) => self.recover(&path),
        }
    }

    fn recover(&self, path: &Path) -> (Config, LoadReport) {
        let _ = fs::rename(path, self.dir.join(BACKUP_NAME));
        (Config::default(), LoadReport { recovered: true, migrated: false })
    }

    /// 임시 파일에 다 쓴 뒤 교체한다 (저장 중 꺼져도 파일이 깨지지 않음).
    pub fn save(&self, config: &Config) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let tmp = self.dir.join("config.json.tmp");
        let json = serde_json::to_string_pretty(config).map_err(io::Error::other)?;
        fs::write(&tmp, json)?;
        fs::rename(&tmp, self.path())
    }
}

/// 0.1.x 설정 → schema 2. 타일은 유튜브 항목으로, 창 위치는 패널 위치로, 모드는 패널.
pub fn migrate_v1(v1: &Value) -> Option<Config> {
    let tiles = v1.get("tiles")?.as_array()?;
    let mut items = Vec::with_capacity(tiles.len());
    for tile in tiles {
        let query = tile.get("query")?.as_str()?.trim().to_string();
        if query.is_empty() {
            continue;
        }
        let id = tile
            .get("id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(crate::items::new_id);
        let name = tile.get("name").and_then(Value::as_str).unwrap_or("").to_string();
        items.push(Item::Youtube { id, query, name });
    }
    let panel_window = v1.get("window").and_then(|w| {
        Some(Rect {
            x: i32::try_from(w.get("x")?.as_i64()?).ok()?,
            y: i32::try_from(w.get("y")?.as_i64()?).ok()?,
            w: i32::try_from(w.get("w")?.as_i64()?).ok()?,
            h: i32::try_from(w.get("h")?.as_i64()?).ok()?,
        })
    });
    let theme = v1.get("theme").and_then(|t| serde_json::from_value::<Theme>(t.clone()).ok()).unwrap_or(Theme::Dark);
    let text = |key: &str| v1.get(key).and_then(Value::as_str).unwrap_or("").to_string();
    Some(Config {
        schema: SCHEMA,
        items,
        mode: Mode::Panel,
        dock_edge: DockEdge::Bottom,
        panel_window,
        theme,
        locked: v1.get("locked").and_then(Value::as_bool).unwrap_or(false),
        autostart: v1.get("autostart").and_then(Value::as_bool),
        last_run_version: text("last_run_version"),
        notified_version: text("notified_version"),
        chrome_notice_shown: v1.get("chrome_notice_shown").and_then(Value::as_bool).unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ytl-test-{name}-{}", crate::items::new_id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_file_gives_defaults() {
        let storage = Storage::new(temp_dir("missing"));
        let (config, report) = storage.load();
        assert_eq!(config, Config::default());
        assert_eq!(config.mode, Mode::Dock);
        assert_eq!(config.theme, Theme::Glass);
        assert_eq!(report, LoadReport::default());
    }

    #[test]
    fn roundtrip() {
        let storage = Storage::new(temp_dir("roundtrip"));
        let config = Config {
            items: vec![Item::youtube("로파이", "").unwrap(), Item::web("github.com", "").unwrap()],
            mode: Mode::Panel,
            dock_edge: DockEdge::Left,
            panel_window: Some(Rect { x: 1, y: 2, w: 300, h: 400 }),
            theme: Theme::Sepia,
            locked: true,
            autostart: Some(false),
            last_run_version: "0.2.0".into(),
            ..Config::default()
        };
        storage.save(&config).unwrap();
        let (loaded, report) = storage.load();
        assert_eq!(loaded, config);
        assert_eq!(report, LoadReport::default());
        assert!(!storage.dir.join("config.json.tmp").exists());
    }

    #[test]
    fn migrates_real_v1_config_and_backs_it_up() {
        let dir = temp_dir("v1");
        let v1 = include_str!("../testdata/config_v1.json");
        fs::write(dir.join(CONFIG_NAME), v1).unwrap();
        let storage = Storage::new(&dir);
        let (config, report) = storage.load();
        assert!(report.migrated);
        assert_eq!(config.schema, 2);
        assert_eq!(config.mode, Mode::Panel);
        assert_eq!(config.theme, Theme::Sepia);
        assert!(config.locked);
        assert_eq!(config.autostart, Some(false));
        assert_eq!(config.last_run_version, "0.1.4");
        assert!(config.chrome_notice_shown);
        assert_eq!(config.panel_window, Some(Rect { x: 1544, y: 16, w: 360, h: 480 }));
        assert_eq!(
            config.items,
            vec![
                Item::Youtube {
                    id: "0f3c9a8e2b7d4c1e9a6b5d4c3b2a1f0e".into(),
                    query: "로파이 음악".into(),
                    name: "".into()
                },
                Item::Youtube {
                    id: "1a2b3c4d5e6f708192a3b4c5d6e7f809".into(),
                    query: "뉴스".into(),
                    name: "아침 뉴스".into()
                },
            ]
        );
        assert_eq!(fs::read_to_string(dir.join(V1_BACKUP_NAME)).unwrap(), v1);
        // 변환 결과가 schema 2로 저장되어 다음 실행에는 변환하지 않는다
        let (again, report2) = storage.load();
        assert_eq!(again, config);
        assert!(!report2.migrated);
    }

    #[test]
    fn corrupt_file_is_backed_up() {
        let dir = temp_dir("corrupt");
        fs::write(dir.join(CONFIG_NAME), b"{not json").unwrap();
        let (config, report) = Storage::new(&dir).load();
        assert_eq!(config, Config::default());
        assert!(report.recovered);
        assert_eq!(fs::read(dir.join(BACKUP_NAME)).unwrap(), b"{not json");
    }

    #[test]
    fn non_utf8_file_is_backed_up() {
        let dir = temp_dir("utf8");
        fs::write(dir.join(CONFIG_NAME), b"\xff\xfe\x00bad").unwrap();
        let (_, report) = Storage::new(&dir).load();
        assert!(report.recovered);
    }

    #[test]
    fn unknown_enum_values_fall_back_to_defaults() {
        let dir = temp_dir("enums");
        fs::write(dir.join(CONFIG_NAME), r#"{"schema":2,"mode":"floating","theme":"neon","dock_edge":"middle"}"#)
            .unwrap();
        let (config, report) = Storage::new(&dir).load();
        assert!(!report.recovered);
        assert_eq!((config.mode, config.theme, config.dock_edge), (Mode::Dock, Theme::Glass, DockEdge::Bottom));
    }

    #[test]
    fn utf8_bom_is_accepted() {
        let dir = temp_dir("bom");
        fs::write(dir.join(CONFIG_NAME), "\u{feff}{\"schema\":2,\"locked\":true}").unwrap();
        let (config, report) = Storage::new(&dir).load();
        assert!(!report.recovered);
        assert!(config.locked);
    }
}
```

- [ ] **Step 3: main.rs에 모듈 등록**

```rust
#![cfg_attr(not(test), windows_subsystem = "windows")]
//! YT Launcher 진입점.
// 크롬 실행·항목 모델은 2단계(화면)에서 연결된다. 그때 이 줄을 지운다.
#![allow(dead_code)]

mod config;
mod items;

fn main() {}
```

- [ ] **Step 4: 테스트**

Run: `cargo test config`
Expected: 7 passed

- [ ] **Step 5: 전체 검사**

Run: `cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test`
Expected: clippy 오류 없음, `test result: ok`

- [ ] **Step 6: Commit**

```powershell
git add src/config.rs testdata src/main.rs
git commit -m "feat: 설정 schema 2와 0.1.x 설정 변환"
```

---

### Task 4: 레지스트리 래퍼와 자동 실행 (win/reg.rs, autostart.rs)

**Files:**
- Create: `src/win/mod.rs`, `src/win/reg.rs`, `src/autostart.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `items::new_id` (테스트용)
- Produces: `win::reg::{get_string(root,subkey,value)->Option<String>, set_string(...)->windows::core::Result<()>, delete_value(...), HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE}` (value가 빈 문자열이면 기본값), `autostart::{RUN_KEY, VALUE_NAME, trait RunBackend{get,set,delete}, RegistryRun{value_name}, AutoStart<B>{backend}, AutoStart::is_enabled(), AutoStart::set_enabled(bool)->io::Result<()>, launch_command()->String}`

- [ ] **Step 1: src/win/mod.rs**

```rust
//! Win32 API를 감싼 얇은 래퍼들.

pub mod reg;
```

- [ ] **Step 2: src/win/reg.rs (구현 + 테스트)**

```rust
//! 레지스트리 문자열 값 읽기·쓰기·삭제 (자동 실행, 크롬 경로 탐색에 사용).

use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    HKEY, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
};
use windows::core::{HSTRING, PCWSTR};

// HKEY_LOCAL_MACHINE은 크롬 경로 탐색(Task 6)에서 쓰기 시작한다. Task 6에서 이 allow 두 줄을 지운다.
#[allow(unused_imports)]
pub use windows::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// `value`가 빈 문자열이면 키의 기본값을 읽는다.
pub fn get_string(root: HKEY, subkey: &str, value: &str) -> Option<String> {
    let subkey = HSTRING::from(subkey);
    let name = wide(value);
    let name_ptr = if value.is_empty() { PCWSTR::null() } else { PCWSTR(name.as_ptr()) };
    let mut size = 0u32;
    // SAFETY: 크기만 묻는 호출, 버퍼 없음
    let status = unsafe { RegGetValueW(root, &subkey, name_ptr, RRF_RT_REG_SZ, None, None, Some(&mut size)) };
    if status != ERROR_SUCCESS || size == 0 {
        return None;
    }
    let mut buf = vec![0u16; (size as usize).div_ceil(2)];
    // SAFETY: buf는 size 바이트 이상
    let status = unsafe {
        RegGetValueW(root, &subkey, name_ptr, RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr().cast()), Some(&mut size))
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]))
}

pub fn set_string(root: HKEY, subkey: &str, value: &str, data: &str) -> windows::core::Result<()> {
    let data = wide(data);
    // SAFETY: data는 NUL로 끝나는 UTF-16, 길이는 바이트 단위
    unsafe {
        RegSetKeyValueW(
            root,
            &HSTRING::from(subkey),
            &HSTRING::from(value),
            REG_SZ.0,
            Some(data.as_ptr().cast()),
            (data.len() * 2) as u32,
        )
    }
    .ok()
}

/// 값이 없으면 아무 일도 하지 않는다.
pub fn delete_value(root: HKEY, subkey: &str, value: &str) {
    // SAFETY: 단순 삭제 호출
    let _ = unsafe { RegDeleteKeyValueW(root, &HSTRING::from(subkey), &HSTRING::from(value)) };
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = r"Software\YT-Launcher-Test";

    #[test]
    fn string_roundtrip_in_hkcu() {
        let name = format!("v{}", crate::items::new_id());
        set_string(HKEY_CURRENT_USER, KEY, &name, "한글 값 \"C:\\x.exe\"").unwrap();
        assert_eq!(get_string(HKEY_CURRENT_USER, KEY, &name).as_deref(), Some("한글 값 \"C:\\x.exe\""));
        delete_value(HKEY_CURRENT_USER, KEY, &name);
        assert_eq!(get_string(HKEY_CURRENT_USER, KEY, &name), None);
        delete_value(HKEY_CURRENT_USER, KEY, &name); // 없는 값 삭제도 문제없음
    }
}
```

- [ ] **Step 3: src/autostart.rs (구현 + 테스트)**

```rust
//! 윈도우 시작 시 자동 실행 (HKCU Run 키의 `YTLauncher` 값). 설치 프로그램과 같은 값을 쓴다.

use crate::win::reg::{self, HKEY_CURRENT_USER};

pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
pub const VALUE_NAME: &str = "YTLauncher";

pub trait RunBackend {
    fn get(&self) -> Option<String>;
    fn set(&self, value: &str) -> std::io::Result<()>;
    fn delete(&self);
}

pub struct RegistryRun {
    pub value_name: String,
}

impl Default for RegistryRun {
    fn default() -> Self {
        RegistryRun { value_name: VALUE_NAME.to_string() }
    }
}

impl RunBackend for RegistryRun {
    fn get(&self) -> Option<String> {
        reg::get_string(HKEY_CURRENT_USER, RUN_KEY, &self.value_name)
    }
    fn set(&self, value: &str) -> std::io::Result<()> {
        reg::set_string(HKEY_CURRENT_USER, RUN_KEY, &self.value_name, value).map_err(std::io::Error::other)
    }
    fn delete(&self) {
        reg::delete_value(HKEY_CURRENT_USER, RUN_KEY, &self.value_name);
    }
}

/// Run 키에 넣을 명령: 따옴표로 감싼 현재 exe 경로 (설치 프로그램이 쓰는 형식과 같다)
pub fn launch_command() -> String {
    let exe = std::env::current_exe().unwrap_or_default();
    format!("\"{}\"", exe.display())
}

pub struct AutoStart<B: RunBackend> {
    pub backend: B,
}

impl<B: RunBackend> AutoStart<B> {
    pub fn is_enabled(&self) -> bool {
        self.backend.get().is_some()
    }

    pub fn set_enabled(&self, enabled: bool) -> std::io::Result<()> {
        if enabled {
            self.backend.set(&launch_command())
        } else {
            self.backend.delete();
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct Fake(RefCell<Option<String>>);
    impl RunBackend for Fake {
        fn get(&self) -> Option<String> {
            self.0.borrow().clone()
        }
        fn set(&self, value: &str) -> std::io::Result<()> {
            *self.0.borrow_mut() = Some(value.to_string());
            Ok(())
        }
        fn delete(&self) {
            *self.0.borrow_mut() = None;
        }
    }

    #[test]
    fn enable_writes_quoted_exe_and_disable_removes() {
        let auto = AutoStart { backend: Fake::default() };
        assert!(!auto.is_enabled());
        auto.set_enabled(true).unwrap();
        let value = auto.backend.get().unwrap();
        assert!(value.starts_with('"') && value.ends_with(".exe\""), "{value}");
        auto.set_enabled(false).unwrap();
        assert!(!auto.is_enabled());
    }

    #[test]
    fn real_run_key_roundtrip() {
        let backend = RegistryRun { value_name: format!("YTLauncherTest{}", crate::items::new_id()) };
        backend.set("\"C:\\nonexistent\\yt-launcher-test.exe\"").unwrap();
        assert_eq!(backend.get().as_deref(), Some("\"C:\\nonexistent\\yt-launcher-test.exe\""));
        backend.delete();
        assert_eq!(backend.get(), None);
    }
}
```

- [ ] **Step 4: main.rs에 모듈 등록**

```rust
#![cfg_attr(not(test), windows_subsystem = "windows")]
//! YT Launcher 진입점.
// 크롬 실행·항목 모델은 2단계(화면)에서 연결된다. 그때 이 줄을 지운다.
#![allow(dead_code)]

mod autostart;
mod config;
mod items;
mod win;

fn main() {}
```

- [ ] **Step 5: 테스트**

Run: `cargo test reg; cargo test autostart`
Expected: reg 1 passed, autostart 2 passed. 테스트는 HKCU에 임시 값을 쓰고 지운다. 이후 `Get-ItemProperty HKCU:\Software\Microsoft\Windows\CurrentVersion\Run` 결과에 `YTLauncherTest`로 시작하는 값이 남아 있지 않아야 한다.

- [ ] **Step 6: 전체 검사**

Run: `cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test`
Expected: clippy 오류 없음, `test result: ok`

- [ ] **Step 7: Commit**

```powershell
git add src/win src/autostart.rs src/main.rs
git commit -m "feat: 레지스트리 래퍼와 자동 실행"
```

---

### Task 5: 업데이트 (updater.rs)

**Files:**
- Create: `src/updater.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `items::new_id` (테스트용)
- Produces: `REPO`, `SUMS_NAME`, `SILENT_ARGS`, `Asset{name,url,size}`, `Release{tag,version,html_url,installer,sums}` + `is_installable()`, `UpdateError`(Display 한국어), `trait Http{get(url,accept)->Result<Box<dyn Read+Send>,HttpFailure>}`, `HttpFailure{Status(u16),Transport(String)}`, `UreqHttp::new()`, `parse_release(&Value)->Option<Release>`, `is_newer(cand,cur)->bool`, `fetch_latest(&dyn Http,repo)->Result<Option<Release>,UpdateError>`(404→None), `parse_sums`, `sha256_file`, `download(http,asset,dest,expected,progress)`, `download_and_verify(http,release,dir,progress)->Result<PathBuf,UpdateError>`, `launch_installer(&Path)->io::Result<()>`

- [ ] **Step 1: src/updater.rs (구현 + 테스트)**

```rust
//! GitHub Releases 기반 업데이트: 최신 릴리스 확인 → 설치 파일 다운로드·SHA-256 검증 → 조용한 설치 실행.
//!
//! 0.1.x(파이썬) 업데이터와 같은 규칙을 지킨다: 정식 릴리스, `YT-Launcher-Setup-<버전>.exe`,
//! `SHA256SUMS.txt`(`<소문자 해시>  <파일명>`), `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART`.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const REPO: &str = "ssenu/YT-launcher";
pub const SUMS_NAME: &str = "SHA256SUMS.txt";
pub const INSTALLER_PREFIX: &str = "YT-Launcher-Setup-";
pub const SILENT_ARGS: [&str; 3] = ["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"];
const CHUNK: usize = 64 * 1024;
const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub tag: String,
    pub version: String,
    pub html_url: String,
    pub installer: Option<Asset>,
    pub sums: Option<Asset>,
}

impl Release {
    /// 설치 파일과 해시 파일이 둘 다 있어야 업데이트 배지를 띄운다.
    pub fn is_installable(&self) -> bool {
        self.installer.is_some() && self.sums.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateError {
    Network(String),
    BadResponse(String),
    MissingAssets,
    HashMismatch,
    Io(String),
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateError::Network(e) => write!(f, "네트워크 오류: {e}"),
            UpdateError::BadResponse(e) => write!(f, "릴리스 정보를 읽지 못했습니다: {e}"),
            UpdateError::MissingAssets => write!(f, "릴리스에 설치 파일 또는 SHA256SUMS.txt가 없습니다."),
            UpdateError::HashMismatch => write!(f, "설치 파일 해시가 일치하지 않습니다."),
            UpdateError::Io(e) => write!(f, "파일 오류: {e}"),
        }
    }
}

/// HTTP GET 실패 종류. 404는 "릴리스 없음"으로 따로 처리한다.
#[derive(Debug)]
pub enum HttpFailure {
    Status(u16),
    Transport(String),
}

pub trait Http: Send + Sync {
    fn get(&self, url: &str, accept: &str) -> Result<Box<dyn Read + Send>, HttpFailure>;
}

/// 실제 네트워크용. Windows 기본 TLS(schannel)를 쓴다.
pub struct UreqHttp {
    agent: ureq::Agent,
}

impl UreqHttp {
    pub fn new() -> Self {
        let tls = ureq::tls::TlsConfig::builder().provider(ureq::tls::TlsProvider::NativeTls).build();
        let agent = ureq::Agent::config_builder()
            .tls_config(tls)
            .timeout_global(Some(TIMEOUT))
            .user_agent(format!("YT-Launcher/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .into();
        UreqHttp { agent }
    }
}

impl Default for UreqHttp {
    fn default() -> Self {
        Self::new()
    }
}

impl Http for UreqHttp {
    fn get(&self, url: &str, accept: &str) -> Result<Box<dyn Read + Send>, HttpFailure> {
        match self.agent.get(url).header("Accept", accept).call() {
            Ok(response) => Ok(Box::new(response.into_body().into_reader())),
            Err(ureq::Error::StatusCode(code)) => Err(HttpFailure::Status(code)),
            Err(e) => Err(HttpFailure::Transport(e.to_string())),
        }
    }
}

pub fn api_url(repo: &str) -> String {
    format!("https://api.github.com/repos/{repo}/releases/latest")
}

fn is_installer_name(name: &str) -> bool {
    name.strip_prefix(INSTALLER_PREFIX).and_then(|rest| rest.strip_suffix(".exe")).is_some_and(|ver| {
        !ver.is_empty() && ver.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    })
}

pub fn parse_release(data: &Value) -> Option<Release> {
    if data.get("draft").and_then(Value::as_bool).unwrap_or(false)
        || data.get("prerelease").and_then(Value::as_bool).unwrap_or(false)
    {
        return None;
    }
    let tag = data.get("tag_name")?.as_str()?.to_string();
    if tag.is_empty() {
        return None;
    }
    let mut installer = None;
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
        if installer.is_none() && is_installer_name(name) {
            installer = Some(asset);
        } else if name == SUMS_NAME {
            sums = Some(asset);
        }
    }
    Some(Release {
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        tag,
        html_url: data.get("html_url").and_then(Value::as_str).unwrap_or("").to_string(),
        installer,
        sums,
    })
}

fn version_parts(v: &str) -> Option<Vec<u64>> {
    v.trim_start_matches(['v', 'V']).split('.').map(|p| p.parse::<u64>().ok()).collect()
}

/// 숫자 단위로 비교한다 ("0.10.0" > "0.9.0"). 형식이 잘못되면 false.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (version_parts(candidate), version_parts(current)) {
        (Some(mut a), Some(mut b)) => {
            let len = a.len().max(b.len());
            a.resize(len, 0);
            b.resize(len, 0);
            a > b
        }
        _ => false,
    }
}

/// 최신 정식 릴리스. 릴리스가 아직 없으면(404) `Ok(None)`.
pub fn fetch_latest(http: &dyn Http, repo: &str) -> Result<Option<Release>, UpdateError> {
    let mut body = match http.get(&api_url(repo), "application/vnd.github+json") {
        Ok(body) => body,
        Err(HttpFailure::Status(404)) => return Ok(None),
        Err(HttpFailure::Status(code)) => return Err(UpdateError::Network(format!("HTTP {code}"))),
        Err(HttpFailure::Transport(e)) => return Err(UpdateError::Network(e)),
    };
    let mut text = String::new();
    body.read_to_string(&mut text).map_err(|e| UpdateError::Network(e.to_string()))?;
    let value: Value = serde_json::from_str(&text).map_err(|e| UpdateError::BadResponse(e.to_string()))?;
    Ok(parse_release(&value))
}

/// sha256sum 형식(`<hash>  <name>` 또는 `<hash> *<name>`) → {파일명: 소문자 해시}
pub fn parse_sums(text: &str) -> HashMap<String, String> {
    text.trim_start_matches('\u{feff}')
        .lines()
        .filter_map(|line| {
            let (hash, name) = line.trim().split_once(char::is_whitespace)?;
            Some((name.trim().trim_start_matches('*').to_string(), hash.to_ascii_lowercase()))
        })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

/// `.part`로 받으면서 해시를 계산하고, (요청 시) 해시가 맞을 때만 최종 이름으로 바꾼다.
/// 실패하면 `.part`는 항상 지운다.
pub fn download(
    http: &dyn Http,
    asset: &Asset,
    dest: &Path,
    expected_sha256: Option<&str>,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<PathBuf, UpdateError> {
    if let Some(dir) = dest.parent() {
        fs::create_dir_all(dir).map_err(|e| UpdateError::Io(e.to_string()))?;
    }
    let partial =
        dest.with_file_name(format!("{}.part", dest.file_name().and_then(|n| n.to_str()).unwrap_or("download")));
    let result = (|| {
        let mut body = http.get(&asset.url, "application/octet-stream").map_err(|e| match e {
            HttpFailure::Status(code) => UpdateError::Network(format!("HTTP {code}")),
            HttpFailure::Transport(t) => UpdateError::Network(t),
        })?;
        let mut out = File::create(&partial).map_err(|e| UpdateError::Io(e.to_string()))?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; CHUNK];
        let mut done = 0u64;
        loop {
            let n = body.read(&mut buf).map_err(|e| UpdateError::Network(e.to_string()))?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n]).map_err(|e| UpdateError::Io(e.to_string()))?;
            hasher.update(&buf[..n]);
            done += n as u64;
            progress(done, asset.size);
        }
        drop(out);
        if let Some(expected) = expected_sha256
            && hex(&hasher.finalize()) != expected.to_ascii_lowercase()
        {
            return Err(UpdateError::HashMismatch);
        }
        fs::rename(&partial, dest).map_err(|e| UpdateError::Io(e.to_string()))?;
        Ok(dest.to_path_buf())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&partial);
    }
    result
}

pub fn download_and_verify(
    http: &dyn Http,
    release: &Release,
    dir: &Path,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<PathBuf, UpdateError> {
    let (Some(installer), Some(sums)) = (&release.installer, &release.sums) else {
        return Err(UpdateError::MissingAssets);
    };
    let sums_path = download(http, sums, &dir.join(SUMS_NAME), None, &mut |_, _| {})?;
    let bytes = fs::read(&sums_path).map_err(|e| UpdateError::Io(e.to_string()))?;
    let text = String::from_utf8(bytes).map_err(|_| UpdateError::BadResponse("SHA256SUMS 인코딩".into()))?;
    let expected = parse_sums(&text).remove(&installer.name).ok_or(UpdateError::MissingAssets)?;
    let dest = dir.join(&installer.name);
    let _ = fs::remove_file(&dest);
    download(http, installer, &dest, Some(&expected), progress)
}

/// 설치 프로그램을 조용한 모드로 분리 실행한다. 호출한 뒤 앱은 곧바로 종료해야 한다.
pub fn launch_installer(path: &Path) -> io::Result<()> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    std::process::Command::new(path)
        .args(SILENT_ARGS)
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .spawn()
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    const INSTALLER: &[u8] = b"installer-bytes";
    const INSTALLER_SHA: &str = "204676736cea68d6411da9d3aa3fab0a5e70b023ba30cd560cfa9c8e7250f4df";

    enum Reply {
        Body(Vec<u8>),
        Status(u16),
        Fail,
        BreakAfter(Vec<u8>),
    }

    struct FakeHttp {
        replies: HashMap<String, Reply>,
        requested: Mutex<Vec<String>>,
    }

    impl FakeHttp {
        fn new(replies: Vec<(&str, Reply)>) -> Self {
            FakeHttp {
                replies: replies.into_iter().map(|(u, r)| (u.to_string(), r)).collect(),
                requested: Mutex::new(Vec::new()),
            }
        }
    }

    struct Broken(Vec<u8>, bool);
    impl Read for Broken {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.1 {
                return Err(io::Error::other("connection reset"));
            }
            self.1 = true;
            let n = self.0.len().min(buf.len());
            buf[..n].copy_from_slice(&self.0[..n]);
            Ok(n)
        }
    }

    impl Http for FakeHttp {
        fn get(&self, url: &str, _accept: &str) -> Result<Box<dyn Read + Send>, HttpFailure> {
            self.requested.lock().unwrap().push(url.to_string());
            match self.replies.get(url) {
                Some(Reply::Body(b)) => Ok(Box::new(io::Cursor::new(b.clone()))),
                Some(Reply::Status(c)) => Err(HttpFailure::Status(*c)),
                Some(Reply::BreakAfter(b)) => Ok(Box::new(Broken(b.clone(), false))),
                Some(Reply::Fail) | None => Err(HttpFailure::Transport("offline".into())),
            }
        }
    }

    fn release_json() -> Value {
        json!({
            "tag_name": "v0.2.0",
            "html_url": "https://github.com/ssenu/YT-launcher/releases/tag/v0.2.0",
            "draft": false, "prerelease": false,
            "assets": [
                {"name": "YT-Launcher-Setup-0.2.0.exe", "browser_download_url": "https://dl/setup.exe", "size": INSTALLER.len()},
                {"name": "SHA256SUMS.txt", "browser_download_url": "https://dl/sums.txt", "size": 100},
                {"name": "source.zip", "browser_download_url": "https://dl/src.zip", "size": 5}
            ]
        })
    }

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ytl-upd-{}", crate::items::new_id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parses_release_and_assets() {
        let r = parse_release(&release_json()).unwrap();
        assert_eq!(r.version, "0.2.0");
        assert_eq!(r.installer.as_ref().unwrap().name, "YT-Launcher-Setup-0.2.0.exe");
        assert_eq!(r.sums.as_ref().unwrap().url, "https://dl/sums.txt");
        assert!(r.is_installable());
    }

    #[test]
    fn skips_draft_prerelease_and_empty_tag() {
        let mut v = release_json();
        v["draft"] = json!(true);
        assert!(parse_release(&v).is_none());
        let mut v = release_json();
        v["prerelease"] = json!(true);
        assert!(parse_release(&v).is_none());
        let mut v = release_json();
        v["tag_name"] = json!("");
        assert!(parse_release(&v).is_none());
    }

    #[test]
    fn installer_name_rule_matches_v1_updater() {
        assert!(is_installer_name("YT-Launcher-Setup-0.2.0.exe"));
        assert!(is_installer_name("YT-Launcher-Setup-1.0.0-rc_1.exe"));
        assert!(!is_installer_name("YT-Launcher-Setup-.exe"));
        assert!(!is_installer_name("YT-Launcher-Setup-0.2.0.exe.sig"));
        assert!(!is_installer_name("YT-Launcher-Setup-../x.exe"));
    }

    #[test]
    fn compares_versions_numerically() {
        assert!(is_newer("v0.2.0", "0.1.4"));
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(!is_newer("0.1.4", "0.1.4"));
        assert!(!is_newer("0.1.4", "0.2.0"));
        assert!(is_newer("0.2.1", "0.2"));
        assert!(!is_newer("garbage", "0.1.0"));
    }

    #[test]
    fn fetch_latest_handles_ok_404_and_errors() {
        let url = api_url(REPO);
        let ok = FakeHttp::new(vec![(&url, Reply::Body(release_json().to_string().into_bytes()))]);
        assert_eq!(fetch_latest(&ok, REPO).unwrap().unwrap().version, "0.2.0");
        let none = FakeHttp::new(vec![(&url, Reply::Status(404))]);
        assert_eq!(fetch_latest(&none, REPO).unwrap(), None);
        let down = FakeHttp::new(vec![(&url, Reply::Fail)]);
        assert!(matches!(fetch_latest(&down, REPO), Err(UpdateError::Network(_))));
        let bad = FakeHttp::new(vec![(&url, Reply::Body(b"<html>".to_vec()))]);
        assert!(matches!(fetch_latest(&bad, REPO), Err(UpdateError::BadResponse(_))));
    }

    #[test]
    fn parses_sha256sums() {
        let text = format!("\u{feff}{}  YT-Launcher-Setup-0.2.0.exe\nabc *other.exe\n\n", INSTALLER_SHA.to_uppercase());
        let sums = parse_sums(&text);
        assert_eq!(sums["YT-Launcher-Setup-0.2.0.exe"], INSTALLER_SHA);
        assert_eq!(sums["other.exe"], "abc");
    }

    #[test]
    fn download_and_verify_success() {
        let dir = temp_dir();
        let http = FakeHttp::new(vec![
            (
                "https://dl/sums.txt",
                Reply::Body(format!("{INSTALLER_SHA}  YT-Launcher-Setup-0.2.0.exe\n").into_bytes()),
            ),
            ("https://dl/setup.exe", Reply::Body(INSTALLER.to_vec())),
        ]);
        let release = parse_release(&release_json()).unwrap();
        let mut last = (0, 0);
        let path = download_and_verify(&http, &release, &dir, &mut |d, t| last = (d, t)).unwrap();
        assert_eq!(path, dir.join("YT-Launcher-Setup-0.2.0.exe"));
        assert_eq!(fs::read(&path).unwrap(), INSTALLER);
        assert_eq!(sha256_file(&path).unwrap(), INSTALLER_SHA);
        assert_eq!(last, (INSTALLER.len() as u64, INSTALLER.len() as u64));
    }

    #[test]
    fn hash_mismatch_leaves_no_installer_or_part() {
        let dir = temp_dir();
        fs::write(dir.join("YT-Launcher-Setup-0.2.0.exe"), b"stale").unwrap();
        let http = FakeHttp::new(vec![
            (
                "https://dl/sums.txt",
                Reply::Body(format!("{}  YT-Launcher-Setup-0.2.0.exe\n", "0".repeat(64)).into_bytes()),
            ),
            ("https://dl/setup.exe", Reply::Body(INSTALLER.to_vec())),
        ]);
        let release = parse_release(&release_json()).unwrap();
        assert_eq!(download_and_verify(&http, &release, &dir, &mut |_, _| {}), Err(UpdateError::HashMismatch));
        assert!(!dir.join("YT-Launcher-Setup-0.2.0.exe").exists());
        assert!(!dir.join("YT-Launcher-Setup-0.2.0.exe.part").exists());
    }

    #[test]
    fn broken_download_cleans_part() {
        let dir = temp_dir();
        let asset = Asset { name: "x.exe".into(), url: "https://dl/x".into(), size: 10 };
        let http = FakeHttp::new(vec![("https://dl/x", Reply::BreakAfter(b"half".to_vec()))]);
        assert!(matches!(
            download(&http, &asset, &dir.join("x.exe"), None, &mut |_, _| {}),
            Err(UpdateError::Network(_))
        ));
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
    }

    #[test]
    fn missing_assets_or_sums_entry_is_an_error() {
        let dir = temp_dir();
        let mut release = parse_release(&release_json()).unwrap();
        release.sums = None;
        let http = FakeHttp::new(vec![]);
        assert_eq!(download_and_verify(&http, &release, &dir, &mut |_, _| {}), Err(UpdateError::MissingAssets));

        let release = parse_release(&release_json()).unwrap();
        let http = FakeHttp::new(vec![("https://dl/sums.txt", Reply::Body(b"abc  other.exe\n".to_vec()))]);
        assert_eq!(download_and_verify(&http, &release, &dir, &mut |_, _| {}), Err(UpdateError::MissingAssets));
        assert_eq!(*http.requested.lock().unwrap(), vec!["https://dl/sums.txt".to_string()]);
    }
}
```

- [ ] **Step 2: main.rs에 모듈 등록**

```rust
#![cfg_attr(not(test), windows_subsystem = "windows")]
//! YT Launcher 진입점.
// 크롬 실행·항목 모델은 2단계(화면)에서 연결된다. 그때 이 줄을 지운다.
#![allow(dead_code)]

mod autostart;
mod config;
mod items;
mod updater;
mod win;

fn main() {}
```

- [ ] **Step 3: 테스트**

Run: `cargo test updater`
Expected: 10 passed (네트워크 없이 가짜 Http로 실행)

- [ ] **Step 4: 실제 GitHub 조회 확인 (1회, 수동)**

임시로 `src/main.rs`의 `fn main() {}`을 아래로 바꿔 실행한 뒤 원래대로 되돌린다.
```rust
fn main() {
    match updater::fetch_latest(&updater::UreqHttp::new(), updater::REPO) {
        Ok(Some(r)) => println!("{} {}", r.version, r.is_installable()),
        other => println!("{other:?}"),
    }
}
```
Run: `cargo run`
Expected: `0.1.4 true` (현재 최신 릴리스). 일시적 네트워크 오류(`os error 10054`)가 나면 한 번 더 실행한다.

- [ ] **Step 5: 전체 검사**

Run: `cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test`
Expected: clippy 오류 없음, `test result: ok`

- [ ] **Step 6: Commit**

```powershell
git add src/updater.rs src/main.rs
git commit -m "feat: GitHub 릴리스 업데이트 확인·다운로드·해시 검증"
```

---

### Task 6: 창 다루기와 크롬 실행 (win/windows.rs, win/shell.rs, chrome.rs)

**Files:**
- Create: `src/win/windows.rs`, `src/win/shell.rs`, `src/chrome.rs`
- Modify: `src/win/mod.rs`, `src/main.rs`

**Interfaces:**
- Consumes: `win::reg`
- Produces: `win::windows::{TopWindow{hwnd:isize,title,class,exe} + handle()->HWND, list_top_windows()->Vec<TopWindow>, process_image(pid)->String, allow_any_foreground(), maximize_and_focus(HWND)->bool, restore_and_focus(HWND)->bool}`, `win::shell::open(&str)->io::Result<()>`, `chrome::{SEARCH_URL, quote_plus, build_search_url, registry_chrome_path, candidate_paths, find_chrome_with, find_chrome, focus_chrome_window(prefer_title,timeout)->bool, enum Opened{Chrome,DefaultBrowser}, open_url(url,prefer_title)->io::Result<Opened>}`

- [ ] **Step 1: src/win/mod.rs 갱신**

```rust
//! Win32 API를 감싼 얇은 래퍼들.

pub mod reg;
pub mod shell;
pub mod windows;
```

- [ ] **Step 2: src/win/windows.rs (구현 + 테스트)** (같은 단계에서 src/win/reg.rs의 Task 4 임시 allow 두 줄을 지운다)

```rust
//! 최상위 창 목록, 창 최대화·복원·전면화.

use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, TRUE};
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentThreadId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    ASFW_ANY, AllowSetForegroundWindow, BringWindowToTop, EnumWindows, GetClassNameW, GetForegroundWindow,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible, SW_MAXIMIZE, SW_RESTORE,
    SetForegroundWindow, ShowWindow,
};
use windows::core::{BOOL, PWSTR};

#[derive(Debug, Clone)]
pub struct TopWindow {
    pub hwnd: isize,
    pub title: String,
    pub class: String,
    /// 창을 만든 프로세스의 실행 파일 전체 경로 (알 수 없으면 빈 문자열)
    pub exe: String,
}

impl TopWindow {
    pub fn handle(&self) -> HWND {
        HWND(self.hwnd as *mut _)
    }
}

fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    // SAFETY: 버퍼 크기 전달
    let n = unsafe { GetClassNameW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

fn window_text(hwnd: HWND) -> String {
    // SAFETY: 길이를 묻고 그만큼 버퍼를 준비
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; len as usize + 1];
    let n = unsafe { GetWindowTextW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

pub fn process_image(pid: u32) -> String {
    // SAFETY: 핸들은 아래에서 닫는다
    let Ok(handle) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }) else {
        return String::new();
    };
    let mut buf = [0u16; 1024];
    let mut size = buf.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut size) };
    let _ = unsafe { CloseHandle(handle) };
    if ok.is_ok() { String::from_utf16_lossy(&buf[..size as usize]) } else { String::new() }
}

/// 보이는 최상위 창을 Z순서(위가 먼저)로 돌려준다. 제목 없는 창은 뺀다.
pub fn list_top_windows() -> Vec<TopWindow> {
    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: lparam은 아래에서 넘긴 Vec 포인터
        let out = unsafe { &mut *(lparam.0 as *mut Vec<TopWindow>) };
        if unsafe { IsWindowVisible(hwnd) }.as_bool() {
            let title = window_text(hwnd);
            if !title.is_empty() {
                let mut pid = 0u32;
                unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
                out.push(TopWindow { hwnd: hwnd.0 as isize, title, class: class_name(hwnd), exe: process_image(pid) });
            }
        }
        TRUE
    }
    let mut out: Vec<TopWindow> = Vec::new();
    // SAFETY: 콜백은 EnumWindows가 끝나기 전에만 out을 쓴다
    let _ = unsafe { EnumWindows(Some(collect), LPARAM(&mut out as *mut _ as isize)) };
    out
}

/// 다른 프로세스가 전면으로 나오는 것을 허용 (크롬 실행 직전에 호출)
pub fn allow_any_foreground() {
    let _ = unsafe { AllowSetForegroundWindow(ASFW_ANY) };
}

fn force_foreground(hwnd: HWND) -> bool {
    // SAFETY: 모든 호출은 핸들만 사용
    unsafe {
        if SetForegroundWindow(hwnd).as_bool() {
            return true;
        }
        let fg = GetForegroundWindow();
        let fg_thread = if fg.0.is_null() { 0 } else { GetWindowThreadProcessId(fg, None) };
        let current = GetCurrentThreadId();
        let attached = fg_thread != 0 && fg_thread != current && AttachThreadInput(current, fg_thread, true).as_bool();
        let _ = BringWindowToTop(hwnd);
        let ok = SetForegroundWindow(hwnd).as_bool();
        if attached {
            let _ = AttachThreadInput(current, fg_thread, false);
        }
        ok
    }
}

/// 최대화(F11 전체화면 아님) 후 전면으로.
pub fn maximize_and_focus(hwnd: HWND) -> bool {
    let _ = unsafe { ShowWindow(hwnd, SW_MAXIMIZE) };
    force_foreground(hwnd)
}

/// 최소화돼 있으면 복원하고 전면으로 (실행 중인 앱으로 전환할 때).
pub fn restore_and_focus(hwnd: HWND) -> bool {
    if unsafe { IsIconic(hwnd) }.as_bool() {
        let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
    }
    force_foreground(hwnd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_visible_windows_with_titles() {
        for w in list_top_windows() {
            assert!(!w.title.is_empty());
            assert_ne!(w.hwnd, 0);
        }
    }

    #[test]
    fn own_process_image_is_known() {
        let exe = process_image(std::process::id());
        assert!(exe.to_ascii_lowercase().ends_with(".exe"), "{exe}");
    }
}
```

- [ ] **Step 3: src/win/shell.rs**

```rust
//! ShellExecute로 URL·파일 열기 (기본 브라우저 폴백, 릴리스 페이지 열기).

use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, w};

pub fn open(target: &str) -> std::io::Result<()> {
    // SAFETY: 문자열 인자만 넘긴다. 반환값이 32 이하면 실패.
    let result = unsafe { ShellExecuteW(None, w!("open"), &HSTRING::from(target), None, None, SW_SHOWNORMAL) };
    if result.0 as isize > 32 {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("ShellExecute 실패 ({})", result.0 as isize)))
    }
}
```

- [ ] **Step 4: src/chrome.rs (구현 + 테스트)**

```rust
//! 크롬으로 URL 열기 (유튜브 검색·웹 주소 공용). 크롬이 없으면 기본 브라우저.

use crate::win::reg::{self, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use crate::win::windows::{TopWindow, allow_any_foreground, list_top_windows, maximize_and_focus};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const SEARCH_URL: &str = "https://www.youtube.com/results?search_query=";
const APP_PATHS_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe";
const CHROME_SUBPATH: &str = r"Google\Chrome\Application\chrome.exe";
const ENV_ROOTS: [&str; 3] = ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"];
const CHROME_CLASS: &str = "Chrome_WidgetWin_1";

/// 파이썬 `urllib.parse.quote_plus`와 같은 인코딩 (공백 → `+`).
pub fn quote_plus(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for b in text.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn build_search_url(query: &str) -> String {
    format!("{SEARCH_URL}{}", quote_plus(query.trim()))
}

pub fn registry_chrome_path() -> Option<PathBuf> {
    [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE].into_iter().find_map(|root| {
        let value = reg::get_string(root, APP_PATHS_KEY, "")?;
        let path = PathBuf::from(value.trim().trim_matches('"'));
        path.is_file().then_some(path)
    })
}

pub fn candidate_paths(env: &dyn Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    ENV_ROOTS.iter().filter_map(|var| env(var)).map(|root| Path::new(&root).join(CHROME_SUBPATH)).collect()
}

pub fn find_chrome_with(
    registry: &dyn Fn() -> Option<PathBuf>,
    env: &dyn Fn(&str) -> Option<String>,
) -> Option<PathBuf> {
    registry().or_else(|| candidate_paths(env).into_iter().find(|p| p.is_file()))
}

pub fn find_chrome() -> Option<PathBuf> {
    find_chrome_with(&registry_chrome_path, &|var| std::env::var(var).ok())
}

fn is_chrome_window(w: &TopWindow) -> bool {
    w.class == CHROME_CLASS
        && Path::new(&w.exe).file_name().and_then(|n| n.to_str()).is_some_and(|n| n.eq_ignore_ascii_case("chrome.exe"))
}

/// 방금 연 탭의 크롬 창을 찾아 최대화·전면화. `prefer_title`이 제목에 들어간 창을 우선한다.
pub fn focus_chrome_window(prefer_title: Option<&str>, timeout: Duration) -> bool {
    std::thread::sleep(Duration::from_millis(500));
    let deadline = Instant::now() + timeout;
    loop {
        let windows: Vec<TopWindow> = list_top_windows().into_iter().filter(is_chrome_window).collect();
        let preferred = prefer_title.and_then(|t| windows.iter().find(|w| w.title.contains(t)));
        if let Some(w) = preferred {
            return maximize_and_focus(w.handle());
        }
        if Instant::now() >= deadline {
            return windows.first().is_some_and(|w| maximize_and_focus(w.handle()));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opened {
    Chrome,
    DefaultBrowser,
}

/// URL을 크롬 새 탭으로 열고 그 창을 최대화한다. 크롬이 없으면 기본 브라우저.
/// 최대 약 5.5초 기다리므로 UI 스레드가 아닌 곳에서 호출한다.
pub fn open_url(url: &str, prefer_title: Option<&str>) -> std::io::Result<Opened> {
    match find_chrome() {
        Some(chrome) => {
            allow_any_foreground();
            std::process::Command::new(chrome).arg(url).spawn()?;
            focus_chrome_window(prefer_title, Duration::from_secs(5));
            Ok(Opened::Chrome)
        }
        None => {
            crate::win::shell::open(url)?;
            Ok(Opened::DefaultBrowser)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_url_matches_python_quote_plus() {
        assert_eq!(
            build_search_url("로파이 음악"),
            "https://www.youtube.com/results?search_query=%EB%A1%9C%ED%8C%8C%EC%9D%B4+%EC%9D%8C%EC%95%85"
        );
        assert!(build_search_url("lofi & chill #1").ends_with("lofi+%26+chill+%231"));
        assert!(build_search_url("  a b  ").ends_with("=a+b"));
    }

    #[test]
    fn find_chrome_prefers_registry_then_env() {
        let dir = std::env::temp_dir().join(format!("ytl-chrome-{}", crate::items::new_id()));
        let exe = dir.join(CHROME_SUBPATH);
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"").unwrap();
        let reg_hit = PathBuf::from("C:\\from-registry\\chrome.exe");
        assert_eq!(find_chrome_with(&|| Some(reg_hit.clone()), &|_| None), Some(reg_hit.clone()));
        let root = dir.to_string_lossy().to_string();
        let env = move |var: &str| (var == "LOCALAPPDATA").then(|| root.clone());
        assert_eq!(find_chrome_with(&|| None, &env), Some(exe));
        assert_eq!(find_chrome_with(&|| None, &|_| None), None);
    }

    #[test]
    fn registry_lookup_does_not_panic() {
        if let Some(p) = registry_chrome_path() {
            assert!(p.is_file());
        }
    }
}
```

- [ ] **Step 5: main.rs에 모듈 등록**

```rust
#![cfg_attr(not(test), windows_subsystem = "windows")]
//! YT Launcher 진입점.
// 크롬 실행·항목 모델은 2단계(화면)에서 연결된다. 그때 이 줄을 지운다.
#![allow(dead_code)]

mod autostart;
mod chrome;
mod config;
mod items;
mod updater;
mod win;

fn main() {}
```

- [ ] **Step 6: 테스트**

Run: `cargo test chrome; cargo test windows`
Expected: chrome 3 passed, windows 2 passed

- [ ] **Step 7: 크롬 실제 동작 확인 (1회, 수동)**

임시로 `fn main()`을 `fn main() { println!("{:?}", chrome::open_url(&chrome::build_search_url("로파이 음악"), Some("YouTube"))); }`로 바꿔 `cargo run` 후 원래대로 되돌린다.
Expected: `Ok(Chrome)` 출력, 크롬이 최대화된 창(F11 아님)으로 유튜브 "로파이 음악" 검색 결과를 연다. 화면 확인은 사용자 수동 점검 항목으로 넘긴다.

- [ ] **Step 8: 전체 검사**

Run: `cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test`
Expected: clippy 오류 없음, `test result: ok`

- [ ] **Step 9: Commit**

```powershell
git add src/win src/chrome.rs src/main.rs
git commit -m "feat: 최상위 창 다루기와 크롬 실행"
```

---

### Task 7: 로그, 단일 인스턴스, 트레이, 앱 컨트롤러, 진입점

**Files:**
- Create: `src/logging.rs`, `src/win/instance.rs`, `src/win/msg_window.rs`, `src/tray.rs`, `src/app.rs`
- Modify: `src/win/mod.rs`, `src/main.rs`

**Interfaces:**
- Consumes: Task 2~6의 모든 공개 항목
- Produces: `logging::{init(&Path), info, warn, error}`, `win::instance::{MUTEX_NAME, MESSAGE_CLASS, WM_APP_SHOW, acquire_mutex(name)->(Option<HANDLE>,bool), notify_existing()->bool}`, `win::msg_window::{trait Handler{handle(hwnd,msg,wparam,lparam)->Option<LRESULT>}, create(class,Box<dyn Handler>)->Result<HWND>, run_loop()->i32}`, `tray::{WM_APP_TRAY, ICON_RESOURCE_ID, MenuEntry, Tray::new/add/remove/notify/show_menu, app_icon()}`, `app::{APP_NAME, BYLINE, CURRENT_VERSION, RELEASES_URL, WM_APP_EVENT, Event, UpdateDecision, decide_update, startup_notices, App::new(Storage)}` — 2단계에서 화면이 이 App에 붙는다.

- [ ] **Step 1: src/logging.rs (구현 + 테스트)**

```rust
//! `%APPDATA%\YT-Launcher\logs\launcher.log` 파일 로그. 1MB가 넘으면 `.1`~`.3`으로 돌린다.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const MAX_BYTES: u64 = 1_000_000;
const BACKUPS: u32 = 3;

static LOG_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn init(dir: &Path) {
    let _ = fs::create_dir_all(dir);
    *LOG_PATH.lock().unwrap_or_else(|e| e.into_inner()) = Some(dir.join("launcher.log"));
    std::panic::set_hook(Box::new(|info| error(&format!("패닉: {info}"))));
}

fn rotate(path: &Path) {
    if fs::metadata(path).map(|m| m.len()).unwrap_or(0) < MAX_BYTES {
        return;
    }
    for i in (1..BACKUPS).rev() {
        let _ = fs::rename(path.with_extension(format!("log.{i}")), path.with_extension(format!("log.{}", i + 1)));
    }
    let _ = fs::rename(path, path.with_extension("log.1"));
}

fn timestamp() -> String {
    let st = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond)
}

fn write(level: &str, message: &str) {
    let guard = LOG_PATH.lock().unwrap_or_else(|e| e.into_inner());
    let Some(path) = guard.as_ref() else { return };
    rotate(path);
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{} {level} {message}", timestamp());
    }
}

pub fn info(message: &str) {
    write("INFO", message);
}

pub fn warn(message: &str) {
    write("WARN", message);
}

pub fn error(message: &str) {
    write("ERROR", message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_keeps_three_backups() {
        let dir = std::env::temp_dir().join(format!("ytl-log-{}", crate::items::new_id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("launcher.log");
        for round in 0..5 {
            fs::write(&path, vec![b'x'; MAX_BYTES as usize + round]).unwrap();
            rotate(&path);
            assert!(!path.exists());
        }
        assert!(dir.join("launcher.log.1").exists());
        assert!(dir.join("launcher.log.3").exists());
        assert!(!dir.join("launcher.log.4").exists());
    }
}
```

- [ ] **Step 2: src/win/instance.rs (구현 + 테스트)**

```rust
//! 단일 인스턴스: 명명된 뮤텍스로 판정하고, 이미 실행 중이면 기존 인스턴스에 "보이기"를 보낸다.

use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HANDLE, LPARAM, WPARAM};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW, WM_APP};
use windows::core::HSTRING;

pub const MUTEX_NAME: &str = "YTLauncherMutex";
/// 트레이·메시지를 받는 숨은 창의 클래스 이름
pub const MESSAGE_CLASS: &str = "YTLauncherMsg";
/// 두 번째 실행이 기존 인스턴스에 보내는 "런처 보이기" 요청
pub const WM_APP_SHOW: u32 = WM_APP + 1;

/// 뮤텍스를 만든다. (핸들, 다른 프로세스가 이미 가지고 있었는지)
/// 핸들은 프로세스가 끝날 때까지 일부러 닫지 않는다 (설치 프로그램이 실행 중 감지에 쓴다).
pub fn acquire_mutex(name: &str) -> (Option<HANDLE>, bool) {
    // SAFETY: 이름만 넘기는 단순 호출
    match unsafe { CreateMutexW(None, false, &HSTRING::from(name)) } {
        Ok(handle) => {
            let existed = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
            (Some(handle), existed)
        }
        Err(_) => (None, false),
    }
}

/// 기존 인스턴스의 숨은 창에 "보이기"를 보낸다. 창을 찾았으면 true.
pub fn notify_existing() -> bool {
    // SAFETY: 클래스 이름으로 찾고 메시지만 보낸다
    match unsafe { FindWindowW(&HSTRING::from(MESSAGE_CLASS), None) } {
        Ok(hwnd) if !hwnd.0.is_null() => unsafe { PostMessageW(Some(hwnd), WM_APP_SHOW, WPARAM(0), LPARAM(0)) }.is_ok(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_acquire_reports_existing() {
        let name = format!("YTLauncherTest-{}", crate::items::new_id());
        let (first, existed1) = acquire_mutex(&name);
        assert!(first.is_some());
        assert!(!existed1);
        let (second, existed2) = acquire_mutex(&name);
        assert!(second.is_some());
        assert!(existed2);
    }
}
```

- [ ] **Step 3: src/win/msg_window.rs**

```rust
//! 트레이·타이머·앱 메시지를 받는 숨은 창과 메시지 루프.
//!
//! 메뉴나 메시지 상자는 자체 메시지 루프를 돌리므로, 처리 도중 창 프로시저가 다시 불릴 수 있다.
//! 이때 핸들러를 두 번 빌리지 않도록 `RefCell::try_borrow_mut`을 쓰고, 바쁠 때 온 앱 메시지는
//! 다시 큐에 넣어 나중에 처리한다.

use std::cell::RefCell;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CreateWindowExW, DefWindowProcW, DispatchMessageW, GWLP_USERDATA, GetMessageW, GetWindowLongPtrW,
    MSG, PostMessageW, RegisterClassW, SetWindowLongPtrW, TranslateMessage, WINDOW_EX_STYLE, WM_APP, WM_NCCREATE,
    WM_NCDESTROY, WNDCLASSW, WS_OVERLAPPED,
};
use windows::core::{HSTRING, PCWSTR};

pub trait Handler {
    /// 처리했으면 `Some(결과)`, 기본 처리를 원하면 `None`.
    fn handle(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT>;
}

type Slot = RefCell<Box<dyn Handler>>;

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: GWLP_USERDATA에는 WM_NCCREATE에서 넣은 Box<Slot> 포인터만 들어 있다
    unsafe {
        if msg == WM_NCCREATE {
            let create = &*(lparam.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Slot;
        if ptr.is_null() {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        if msg == WM_NCDESTROY {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(Box::from_raw(ptr as *mut Slot));
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        match (*ptr).try_borrow_mut() {
            Ok(mut handler) => match handler.handle(hwnd, msg, wparam, lparam) {
                Some(result) => result,
                None => DefWindowProcW(hwnd, msg, wparam, lparam),
            },
            Err(_) => {
                if (WM_APP..0xC000).contains(&msg) {
                    let _ = PostMessageW(Some(hwnd), msg, wparam, lparam);
                    LRESULT(0)
                } else {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }
        }
    }
}

/// 보이지 않는 최상위 창을 만든다. (메시지 전용 창은 TaskbarCreated 방송을 못 받아서 쓰지 않는다)
pub fn create(class_name: &str, handler: Box<dyn Handler>) -> windows::core::Result<HWND> {
    let class = HSTRING::from(class_name);
    // SAFETY: 클래스 등록과 창 생성, 핸들러 소유권은 창이 가진다
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: PCWSTR(class.as_ptr()),
            ..Default::default()
        };
        RegisterClassW(&wc);
        let slot: Box<Slot> = Box::new(RefCell::new(handler));
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            &class,
            &class,
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            Some(Box::into_raw(slot) as *const _),
        )
    }
}

/// WM_QUIT이 올 때까지 메시지를 처리한다.
pub fn run_loop() -> i32 {
    let mut msg = MSG::default();
    // SAFETY: 표준 메시지 루프
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    msg.wParam.0 as i32
}
```

- [ ] **Step 4: src/win/mod.rs 갱신**

```rust
//! Win32 API를 감싼 얇은 래퍼들.

pub mod instance;
pub mod msg_window;
pub mod reg;
pub mod shell;
pub mod windows;
```

- [ ] **Step 5: src/tray.rs**

```rust
//! 알림 영역(트레이) 아이콘, 풍선 알림, 팝업 메뉴.

use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, GetSystemMetrics, HICON, IMAGE_ICON, LR_DEFAULTCOLOR,
    LoadImageW, MF_CHECKED, MF_GRAYED, MF_SEPARATOR, MF_STRING, SM_CXSMICON, SM_CYSMICON, SetForegroundWindow,
    TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, WM_APP,
};
use windows::core::{HSTRING, PCWSTR};

/// 트레이 아이콘에서 오는 마우스 메시지
pub const WM_APP_TRAY: u32 = WM_APP + 2;
const TRAY_ID: u32 = 1;
/// build.rs가 exe에 넣는 아이콘 리소스 번호
pub const ICON_RESOURCE_ID: u16 = 1;

pub enum MenuEntry {
    Item { id: u32, text: String, checked: bool, enabled: bool },
    Separator,
}

impl MenuEntry {
    pub fn item(id: u32, text: impl Into<String>) -> Self {
        MenuEntry::Item { id, text: text.into(), checked: false, enabled: true }
    }
    pub fn checked(id: u32, text: impl Into<String>, checked: bool) -> Self {
        MenuEntry::Item { id, text: text.into(), checked, enabled: true }
    }
    pub fn label(text: impl Into<String>) -> Self {
        MenuEntry::Item { id: 0, text: text.into(), checked: false, enabled: false }
    }
}

fn copy_wide(dst: &mut [u16], text: &str) {
    let mut i = 0;
    for unit in text.encode_utf16().take(dst.len() - 1) {
        dst[i] = unit;
        i += 1;
    }
    dst[i] = 0;
}

pub fn app_icon() -> HICON {
    // SAFETY: exe 자신의 아이콘 리소스를 작은 아이콘 크기로 읽는다
    unsafe {
        let module = GetModuleHandleW(None).unwrap_or_default();
        LoadImageW(
            Some(module.into()),
            PCWSTR(ICON_RESOURCE_ID as usize as *const u16),
            IMAGE_ICON,
            GetSystemMetrics(SM_CXSMICON),
            GetSystemMetrics(SM_CYSMICON),
            LR_DEFAULTCOLOR,
        )
        .map(|h| HICON(h.0))
        .unwrap_or_default()
    }
}

pub struct Tray {
    hwnd: HWND,
    icon: HICON,
    tooltip: String,
}

impl Tray {
    pub fn new(hwnd: HWND, tooltip: &str) -> Self {
        Tray { hwnd, icon: app_icon(), tooltip: tooltip.to_string() }
    }

    fn data(&self) -> NOTIFYICONDATAW {
        let mut data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: TRAY_ID,
            ..Default::default()
        };
        copy_wide(&mut data.szTip, &self.tooltip);
        data
    }

    /// 탐색기가 다시 시작된 뒤(TaskbarCreated)에도 다시 호출한다.
    pub fn add(&self) -> bool {
        let mut data = self.data();
        data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        data.uCallbackMessage = WM_APP_TRAY;
        data.hIcon = self.icon;
        unsafe { Shell_NotifyIconW(NIM_ADD, &data) }.as_bool()
    }

    pub fn remove(&self) {
        let data = self.data();
        let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &data) };
    }

    /// 윈도우 알림(풍선)을 띄운다.
    pub fn notify(&self, title: &str, message: &str) {
        let mut data = self.data();
        data.uFlags = NIF_INFO;
        data.dwInfoFlags = NIIF_INFO;
        copy_wide(&mut data.szInfoTitle, title);
        copy_wide(&mut data.szInfo, message);
        let _ = unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) };
    }

    /// 마우스 위치에 메뉴를 띄우고 고른 항목 id를 돌려준다 (취소하면 0).
    pub fn show_menu(&self, entries: &[MenuEntry]) -> u32 {
        // SAFETY: 메뉴는 이 함수 안에서 만들고 없앤다
        unsafe {
            let Ok(menu) = CreatePopupMenu() else { return 0 };
            for entry in entries {
                match entry {
                    MenuEntry::Separator => {
                        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
                    }
                    MenuEntry::Item { id, text, checked, enabled } => {
                        let mut flags = MF_STRING;
                        if *checked {
                            flags |= MF_CHECKED;
                        }
                        if !*enabled {
                            flags |= MF_GRAYED;
                        }
                        let _ = AppendMenuW(menu, flags, *id as usize, &HSTRING::from(text.as_str()));
                    }
                }
            }
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            // 메뉴 바깥을 누르면 닫히도록 먼저 전면으로
            let _ = SetForegroundWindow(self.hwnd);
            let chosen = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, pt.x, pt.y, None, self.hwnd, None);
            let _ = DestroyMenu(menu);
            chosen.0 as u32
        }
    }
}
```

- [ ] **Step 6: src/app.rs (구현 + 테스트)**

```rust
//! 앱 컨트롤러: 설정·트레이·업데이트·자동 실행을 연결한다. (화면은 2단계에서 붙는다)

use crate::autostart::{AutoStart, RegistryRun};
use crate::config::{Config, LoadReport, Storage};
use crate::logging;
use crate::tray::{MenuEntry, Tray, WM_APP_TRAY};
use crate::updater::{self, Release, UreqHttp};
use crate::win::instance::WM_APP_SHOW;
use crate::win::msg_window::Handler;
use crate::win::shell;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, IDYES, KillTimer, MB_ICONQUESTION, MB_YESNO, MessageBoxW, PostMessageW, PostQuitMessage,
    RegisterWindowMessageW, SetTimer, WM_APP, WM_CONTEXTMENU, WM_DESTROY, WM_LBUTTONUP, WM_RBUTTONUP, WM_TIMER,
};
use windows::core::{HSTRING, w};

pub const APP_NAME: &str = "YT Launcher";
pub const BYLINE: &str = "by_ssenu";
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const RELEASES_URL: &str = "https://github.com/ssenu/YT-launcher/releases/latest";

/// 백그라운드 작업 결과가 채널에 들어왔음을 알리는 메시지
pub const WM_APP_EVENT: u32 = WM_APP + 3;

const TIMER_FIRST_CHECK: usize = 1;
const TIMER_DAILY_CHECK: usize = 2;
const FIRST_CHECK_MS: u32 = 30_000;
const DAILY_CHECK_MS: u32 = 24 * 60 * 60 * 1000;

const CMD_AUTOSTART: u32 = 10;
const CMD_CHECK_UPDATE: u32 = 11;
const CMD_INSTALL_UPDATE: u32 = 12;
const CMD_RELEASES: u32 = 13;
const CMD_QUIT: u32 = 14;

pub enum Event {
    Checked { manual: bool, result: Result<Option<Release>, String> },
    Downloaded(Result<PathBuf, String>),
}

/// 업데이트 확인 결과를 보고 무엇을 할지 정한다 (순수 함수).
#[derive(Debug, PartialEq, Eq)]
pub enum UpdateDecision {
    /// 새 버전 발견. `notify`면 이번이 이 버전의 첫 알림
    Available {
        version: String,
        notify: bool,
    },
    UpToDate,
    Failed,
}

pub fn decide_update(result: &Result<Option<Release>, String>, current: &str, notified: &str) -> UpdateDecision {
    match result {
        Err(_) => UpdateDecision::Failed,
        Ok(Some(r)) if r.is_installable() && updater::is_newer(&r.version, current) => {
            UpdateDecision::Available { version: r.version.clone(), notify: r.version != notified }
        }
        Ok(_) => UpdateDecision::UpToDate,
    }
}

/// 시작할 때 띄울 알림 문구 (설정 손상 복구, 0.1.x 변환, 업데이트 완료). 순수 함수.
pub fn startup_notices(report: LoadReport, last_run: &str, current: &str) -> Vec<String> {
    let mut notices = Vec::new();
    if report.recovered {
        notices.push("설정 파일이 손상되어 초기화했습니다. 이전 파일은 config.json.bak에 있습니다.".to_string());
    }
    if !last_run.is_empty() && updater::is_newer(current, last_run) {
        notices.push(format!("v{current} 버전으로 업데이트되었습니다."));
    }
    notices
}

pub struct App {
    hwnd: HWND,
    storage: Storage,
    config: Config,
    tray: Option<Tray>,
    autostart: AutoStart<RegistryRun>,
    release: Option<Release>,
    downloading: bool,
    events_tx: Sender<Event>,
    events_rx: Receiver<Event>,
    taskbar_created: u32,
    pending_notices: Vec<String>,
}

impl App {
    pub fn new(storage: Storage) -> Self {
        let (config, report) = storage.load();
        let (events_tx, events_rx) = channel();
        let pending_notices = startup_notices(report, &config.last_run_version, CURRENT_VERSION);
        if report.migrated {
            logging::info("0.1.x 설정을 schema 2로 변환했습니다 (원본: config.v1.json)");
        }
        App {
            hwnd: HWND::default(),
            storage,
            config,
            tray: None,
            autostart: AutoStart { backend: RegistryRun::default() },
            release: None,
            downloading: false,
            events_tx,
            events_rx,
            taskbar_created: unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) },
            pending_notices,
        }
    }

    fn save(&self) {
        if let Err(e) = self.storage.save(&self.config) {
            logging::error(&format!("설정 저장 실패: {e}"));
        }
    }

    fn notify(&self, message: &str) {
        if let Some(tray) = &self.tray {
            tray.notify(APP_NAME, message);
        }
    }

    /// 창이 만들어진 직후 한 번 호출된다.
    fn start(&mut self, hwnd: HWND) {
        self.hwnd = hwnd;
        let tray = Tray::new(hwnd, APP_NAME);
        if !tray.add() {
            logging::warn("트레이 아이콘 추가 실패");
        }
        self.tray = Some(tray);
        // 업데이트 설치가 Run 키를 되살려도 사용자가 고른 값을 지킨다 (개발 빌드에서는 건드리지 않음)
        if !cfg!(debug_assertions)
            && let Some(enabled) = self.config.autostart
            && let Err(e) = self.autostart.set_enabled(enabled)
        {
            logging::error(&format!("자동 실행 설정 실패: {e}"));
        }
        for notice in std::mem::take(&mut self.pending_notices) {
            self.notify(&notice);
        }
        if self.config.last_run_version != CURRENT_VERSION {
            self.config.last_run_version = CURRENT_VERSION.to_string();
            self.save();
        }
        unsafe { SetTimer(Some(hwnd), TIMER_FIRST_CHECK, FIRST_CHECK_MS, None) };
    }

    fn post_event(&self) -> impl Fn(Event) + Send + 'static {
        let tx = self.events_tx.clone();
        let hwnd = self.hwnd.0 as isize;
        move |event| {
            if tx.send(event).is_ok() {
                let _ = unsafe { PostMessageW(Some(HWND(hwnd as *mut _)), WM_APP_EVENT, WPARAM(0), LPARAM(0)) };
            }
        }
    }

    fn check_updates(&self, manual: bool) {
        let send = self.post_event();
        std::thread::spawn(move || {
            let result = updater::fetch_latest(&UreqHttp::new(), updater::REPO).map_err(|e| e.to_string());
            if let Err(e) = &result {
                logging::warn(&format!("업데이트 확인 실패: {e}"));
            }
            send(Event::Checked { manual, result });
        });
    }

    fn on_checked(&mut self, manual: bool, result: Result<Option<Release>, String>) {
        match decide_update(&result, CURRENT_VERSION, &self.config.notified_version) {
            UpdateDecision::Available { version, notify } => {
                self.release = result.ok().flatten();
                if notify {
                    self.config.notified_version = version.clone();
                    self.save();
                }
                if notify || manual {
                    self.notify(&format!("새 버전(v{version})이 나왔습니다. 트레이 메뉴에서 설치하세요."));
                }
            }
            UpdateDecision::UpToDate if manual => self.notify("최신 버전입니다."),
            UpdateDecision::Failed if manual => {
                self.notify("업데이트를 확인하지 못했습니다. 인터넷 연결을 확인하세요.")
            }
            _ => {}
        }
    }

    fn install_update(&mut self) {
        let Some(release) = self.release.clone() else { return };
        if self.downloading {
            return;
        }
        let text =
            HSTRING::from(format!("v{} 버전으로 업데이트할까요?\n런처가 잠시 꺼졌다 켜집니다.", release.version));
        let answer =
            unsafe { MessageBoxW(Some(self.hwnd), &text, &HSTRING::from(APP_NAME), MB_YESNO | MB_ICONQUESTION) };
        if answer != IDYES {
            return;
        }
        self.downloading = true;
        self.notify("업데이트를 내려받는 중입니다…");
        let send = self.post_event();
        std::thread::spawn(move || {
            let dir = std::env::temp_dir().join("YT-Launcher-update");
            let _ = std::fs::remove_dir_all(&dir);
            let result = updater::download_and_verify(&UreqHttp::new(), &release, &dir, &mut |_, _| {})
                .map_err(|e| e.to_string());
            send(Event::Downloaded(result));
        });
    }

    fn on_downloaded(&mut self, result: Result<PathBuf, String>) {
        let launched = result.and_then(|path| updater::launch_installer(&path).map_err(|e| e.to_string()));
        match launched {
            Ok(()) => self.quit(),
            Err(e) => {
                logging::error(&format!("업데이트 실패: {e}"));
                self.downloading = false;
                self.notify(&format!("업데이트에 실패했습니다: {e}"));
            }
        }
    }

    fn set_autostart(&mut self, enabled: bool) {
        self.config.autostart = Some(enabled);
        self.save();
        if let Err(e) = self.autostart.set_enabled(enabled) {
            logging::error(&format!("자동 실행 설정 실패: {e}"));
        }
    }

    fn menu_entries(&self) -> Vec<MenuEntry> {
        let mut entries = vec![
            MenuEntry::label(format!("{APP_NAME}  {BYLINE}  v{CURRENT_VERSION}")),
            MenuEntry::Separator,
            MenuEntry::checked(CMD_AUTOSTART, "윈도우 시작 시 실행", self.autostart.is_enabled()),
            MenuEntry::item(CMD_CHECK_UPDATE, "업데이트 확인"),
        ];
        if let Some(r) = &self.release {
            let text = if self.downloading {
                "업데이트 내려받는 중…".to_string()
            } else {
                format!("업데이트 설치 (v{})", r.version)
            };
            entries.push(MenuEntry::Item { id: CMD_INSTALL_UPDATE, text, checked: false, enabled: !self.downloading });
        }
        entries.extend([
            MenuEntry::item(CMD_RELEASES, "릴리스 페이지 열기"),
            MenuEntry::Separator,
            MenuEntry::item(CMD_QUIT, "종료"),
        ]);
        entries
    }

    fn show_menu(&mut self) {
        let Some(tray) = &self.tray else { return };
        match tray.show_menu(&self.menu_entries()) {
            CMD_AUTOSTART => self.set_autostart(!self.autostart.is_enabled()),
            CMD_CHECK_UPDATE => self.check_updates(true),
            CMD_INSTALL_UPDATE => self.install_update(),
            CMD_RELEASES => {
                let url = self.release.as_ref().map(|r| r.html_url.as_str()).filter(|u| !u.is_empty());
                if let Err(e) = shell::open(url.unwrap_or(RELEASES_URL)) {
                    logging::warn(&format!("릴리스 페이지 열기 실패: {e}"));
                }
            }
            CMD_QUIT => self.quit(),
            _ => {}
        }
    }

    fn quit(&mut self) {
        if let Some(tray) = self.tray.take() {
            tray.remove();
        }
        let _ = unsafe { DestroyWindow(self.hwnd) };
    }
}

impl Handler for App {
    fn handle(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        if self.hwnd.0.is_null() {
            self.start(hwnd);
        }
        match msg {
            WM_APP_TRAY => {
                let event = (lparam.0 & 0xFFFF) as u32;
                if matches!(event, WM_LBUTTONUP | WM_RBUTTONUP | WM_CONTEXTMENU) {
                    self.show_menu();
                }
            }
            WM_APP_EVENT => {
                while let Ok(event) = self.events_rx.try_recv() {
                    match event {
                        Event::Checked { manual, result } => self.on_checked(manual, result),
                        Event::Downloaded(result) => self.on_downloaded(result),
                    }
                }
            }
            WM_APP_SHOW => logging::info("두 번째 실행에서 '보이기' 요청을 받았습니다"),
            WM_TIMER => match wparam.0 {
                TIMER_FIRST_CHECK => {
                    let _ = unsafe { KillTimer(Some(hwnd), TIMER_FIRST_CHECK) };
                    unsafe { SetTimer(Some(hwnd), TIMER_DAILY_CHECK, DAILY_CHECK_MS, None) };
                    self.check_updates(false);
                }
                TIMER_DAILY_CHECK => self.check_updates(false),
                _ => return None,
            },
            WM_DESTROY => {
                if let Some(tray) = self.tray.take() {
                    tray.remove();
                }
                unsafe { PostQuitMessage(0) };
            }
            m if m == self.taskbar_created && m != 0 => {
                if let Some(tray) = &self.tray {
                    tray.add();
                }
            }
            _ => return None,
        }
        Some(LRESULT(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updater::Asset;

    fn release(version: &str, installable: bool) -> Release {
        let asset = Asset { name: "a".into(), url: "u".into(), size: 1 };
        Release {
            tag: format!("v{version}"),
            version: version.into(),
            html_url: String::new(),
            installer: installable.then(|| asset.clone()),
            sums: Some(asset),
        }
    }

    #[test]
    fn newer_installable_release_is_available_and_notified_once() {
        let r = Ok(Some(release("0.3.0", true)));
        assert_eq!(decide_update(&r, "0.2.0", ""), UpdateDecision::Available { version: "0.3.0".into(), notify: true });
        assert_eq!(
            decide_update(&r, "0.2.0", "0.3.0"),
            UpdateDecision::Available { version: "0.3.0".into(), notify: false }
        );
    }

    #[test]
    fn same_older_missing_assets_or_no_release_is_up_to_date() {
        assert_eq!(decide_update(&Ok(Some(release("0.2.0", true))), "0.2.0", ""), UpdateDecision::UpToDate);
        assert_eq!(decide_update(&Ok(Some(release("0.1.4", true))), "0.2.0", ""), UpdateDecision::UpToDate);
        assert_eq!(decide_update(&Ok(Some(release("0.3.0", false))), "0.2.0", ""), UpdateDecision::UpToDate);
        assert_eq!(decide_update(&Ok(None), "0.2.0", ""), UpdateDecision::UpToDate);
        assert_eq!(decide_update(&Err("x".into()), "0.2.0", ""), UpdateDecision::Failed);
    }

    #[test]
    fn startup_notices_cover_recovery_and_upgrade() {
        let recovered = LoadReport { recovered: true, migrated: false };
        assert_eq!(startup_notices(LoadReport::default(), "", "0.2.0"), Vec::<String>::new());
        assert_eq!(startup_notices(LoadReport::default(), "0.2.0", "0.2.0"), Vec::<String>::new());
        assert_eq!(
            startup_notices(LoadReport::default(), "0.1.4", "0.2.0"),
            vec!["v0.2.0 버전으로 업데이트되었습니다.".to_string()]
        );
        assert_eq!(startup_notices(recovered, "", "0.2.0").len(), 1);
    }
}
```

- [ ] **Step 7: src/main.rs 최종**

```rust
#![cfg_attr(not(test), windows_subsystem = "windows")]
//! YT Launcher 진입점.
// 크롬 실행·항목 모델은 2단계(화면)에서 연결된다. 그때 이 줄을 지운다.
#![allow(dead_code)]

mod app;
mod autostart;
mod chrome;
mod config;
mod items;
mod logging;
mod tray;
mod updater;
mod win;

use win::instance::{MESSAGE_CLASS, MUTEX_NAME, acquire_mutex, notify_existing};

fn main() {
    let data_dir = config::default_dir();
    logging::init(&data_dir.join("logs"));
    logging::info(&format!("YT Launcher {} 시작", app::CURRENT_VERSION));

    // 핸들은 프로세스가 끝날 때까지 쥐고 있는다 (설치 프로그램이 실행 중 감지에 씀)
    let (_mutex, existed) = acquire_mutex(MUTEX_NAME);
    if existed {
        notify_existing();
        return;
    }

    let app = app::App::new(config::Storage::new(&data_dir));
    if let Err(e) = win::msg_window::create(MESSAGE_CLASS, Box::new(app)) {
        logging::error(&format!("메시지 창을 만들지 못했습니다: {e}"));
        return;
    }
    std::process::exit(win::msg_window::run_loop());
}
```

- [ ] **Step 8: 테스트**

Run: `cargo test`
Expected: `test result: ok. 35 passed`

- [ ] **Step 9: 릴리스 빌드와 실행 확인 (실제 설정을 건드리지 않도록 임시 APPDATA 사용)**

Run:
```powershell
cargo build --release
"{0:N0} KB" -f ((Get-Item target\release\YTLauncher.exe).Length/1KB)
$tmpApp = Join-Path $env:TEMP "ytl-phase1"; New-Item -ItemType Directory -Force $tmpApp | Out-Null
$psi = New-Object System.Diagnostics.ProcessStartInfo (Resolve-Path target\release\YTLauncher.exe)
$psi.UseShellExecute = $false; $psi.EnvironmentVariables["APPDATA"] = $tmpApp
$p = [System.Diagnostics.Process]::Start($psi); Start-Sleep 3; $p.Refresh()
"alive={0} WS={1:N1}MB" -f (!$p.HasExited), ($p.WorkingSet64/1MB)
$p2 = [System.Diagnostics.Process]::Start($psi); Start-Sleep 2; "second exited={0}" -f $p2.HasExited
Get-Content "$tmpApp\YT-Launcher\logs\launcher.log"
Stop-Process -Id $p.Id
```
Expected: exe 약 1MB 이하, `alive=True`, 작업 집합 약 10~20MB, `second exited=True`, 로그에 `YT Launcher 0.2.0 시작` 두 줄과 `두 번째 실행에서 '보이기' 요청을 받았습니다`.

주의: 설치된 0.1.x 런처가 실행 중이면 같은 뮤텍스 때문에 새 exe가 바로 종료된다. 그 경우 `Get-Process YTLauncher`로 확인하고, 사용자에게 트레이에서 기존 런처를 종료해 달라고 보고한다 (사용자의 프로세스를 직접 종료하지 않는다).

- [ ] **Step 10: 사용자 수동 점검 항목 (보고에 포함)**

화면을 볼 수 없으면 다음을 사용자 확인 항목으로 보고한다: 트레이에 빨간 재생 아이콘이 보이는지, 좌·우클릭 시 메뉴(제목줄, 윈도우 시작 시 실행, 업데이트 확인, 릴리스 페이지 열기, 종료)가 뜨는지, '업데이트 확인'을 누르면 "최신 버전입니다." 알림이 뜨는지(0.2.0 > 0.1.4).

- [ ] **Step 11: 전체 검사**

Run: `cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test`
Expected: clippy 오류 없음, `test result: ok`

- [ ] **Step 12: Commit**

```powershell
git add src
git commit -m "feat: 트레이 런처 기반 (단일 인스턴스, 메뉴, 업데이트 흐름, 로그)"
```

---

