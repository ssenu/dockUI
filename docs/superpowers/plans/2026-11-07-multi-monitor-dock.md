# 모든 모니터 독 표시·안정성 버그 수정 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 먼저 안정성 버그를 고치고, 그다음 독 모드에서 모든 모니터에 같은 독을 띄운다.

**Architecture:** `Panel`을 모니터마다 하나씩 띄우고, `App`이 주 패널과 보조 독 목록을 들고 명령을 나눠 보낸다. 이벤트에는 보낸 독의 id가 붙어 다른 독에만 전달하거나 그 모니터에 대화상자를 띄울 수 있다. 패널 모드는 지금처럼 주 패널 하나다.

**Tech Stack:** Rust 2024, `windows` 0.62 (Win32, Direct2D, DirectComposition), serde_json. 스펙: `docs/superpowers/specs/2026-11-07-multi-monitor-dock-design.md`

## Global Constraints

- 브랜치: `feat/multi-monitor-dock`. 푸시하지 않는다.
- 코드 주석·UI 문구·커밋 메시지는 기존처럼 한국어로 쓴다. 커밋은 `fix:`, `feat:`, `docs:` 머리말을 쓴다.
- 모든 태스크는 끝날 때 `cargo fmt`, `cargo test`를 통과한다. `cargo clippy --all-targets -- -D warnings`도 통과한다(예외는 Task 7, 9에 적었고 Task 10에서 모두 사라진다).
- 새 의존성을 추가하지 않는다. 필요한 `windows` 기능은 이미 `Cargo.toml`에 있다.
- 패널 모드의 동작을 바꾸지 않는다(스펙 2-2의 수정 제외).
- `exe`, 데이터 폴더, 뮤텍스 이름은 바꾸지 않는다(업데이트 호환).
- 창을 띄우는 테스트는 실행 중 사용자 화면에 잠깐 창이 보인다. 정상이다.
- 재진입 주의: `msg_window.rs`는 핸들러가 처리 중이면 중첩 메시지를 핸들러에 넘기지 않는다. 핸들러 안에서 부른 `SetWindowPos`의 `WM_WINDOWPOSCHANGED`, `WM_DPICHANGED`는 핸들러에 오지 않는다.

## 파일 구조

| 파일 | 변경 |
|---|---|
| `src/items.rs` | `normalize_url` 스킴 판정, `authority_of` 공개 |
| `src/icons.rs` | `resolve_url` 포트 유지, 임시 파일 쓰기 |
| `src/win/shell_items.rs` | `write_via_temp`, `save_bitmap_png` 임시 파일 쓰기 |
| `src/chrome.rs` | `pick_window`, 대기 제거 |
| `src/config.rs` | 항목별 읽기, 읽기 재시도, `read_only`, `all_monitors` |
| `src/win/desktop.rs` | `Monitor`, `monitors`, `work_area_of`, `monitor_at_cursor` |
| `src/ui/panel.rs` | 캡처 해제, 최소 높이, 재시도, 모니터 지정, 위치 동기화 명령 |
| `src/app.rs` | 보조 독 관리, 이벤트 전달, 대화상자 소유자, 모달 |
| `src/main.rs` | 시작 로그 위치 |
| `docs/manual-test-checklist.md` | 점검 항목 |

---

# 2부. 버그 수정

### Task 1: 주소 처리 오류 두 가지

**Files:**
- Modify: `src/items.rs` (`normalize_url`, `authority_of`, 테스트)
- Modify: `src/icons.rs` (`resolve_url`, `use` 줄, 테스트)

**Interfaces:**
- Produces: `pub fn authority_of(url: &str) -> Option<&str>` (사용자 정보를 뺀 `호스트:포트`)

- [ ] **Step 1: 기준 상태 확인**

Run: `cargo test`
Expected: 모두 통과. 실패가 있으면 멈추고 보고한다.

- [ ] **Step 2: 실패하는 테스트 작성**

`src/items.rs`의 `normalize_url_adds_https_and_rejects_bad_input` 아래에 추가:

```rust
    #[test]
    fn scheme_is_only_read_before_the_path() {
        assert_eq!(normalize_url("naver.com/a?u=https://b.com").unwrap(), "https://naver.com/a?u=https://b.com");
        assert_eq!(normalize_url("https://naver.com/a?u=ftp://b.com").unwrap(), "https://naver.com/a?u=ftp://b.com");
        assert_eq!(normalize_url("ftp://x.com/a?u=https://b.com"), Err(ItemError::InvalidUrl));
    }
```

`src/icons.rs`의 `resolves_relative_icon_urls` 끝에 추가:

```rust
        let port = "https://user@site.kr:8443/a/b.html";
        assert_eq!(resolve_url(port, "/favicon.ico").unwrap(), "https://site.kr:8443/favicon.ico");
        assert_eq!(resolve_url(port, "i.png").unwrap(), "https://site.kr:8443/a/i.png");
```

- [ ] **Step 3: 실패 확인**

Run: `cargo test scheme_is_only_read_before_the_path` 와 `cargo test resolves_relative_icon_urls`
Expected: 둘 다 FAIL

- [ ] **Step 4: 구현**

`src/items.rs`:

```rust
/// 첫 `/`·`?`·`#` 앞에 `://`가 있으면 스킴이 적힌 주소다 (쿼리 안의 `://`는 세지 않는다)
fn has_scheme(input: &str) -> bool {
    match (input.find("://"), input.find(['/', '?', '#'])) {
        (Some(scheme), Some(delimiter)) => scheme <= delimiter,
        _ => false,
    }
}
```

`normalize_url`의 줄을 바꾼다:

```rust
    let url = if has_scheme(input) { input.to_string() } else { format!("https://{input}") };
```

`authority_of`를 공개하고 주석을 단다:

```rust
/// `https://user@www.github.com:443/a` → `www.github.com:443`
pub fn authority_of(url: &str) -> Option<&str> {
```

`src/icons.rs`: `use crate::items::{Item, authority_of};`로 바꾸고, `resolve_url`의 두 줄을 바꾼다:

```rust
    let authority = authority_of(page).filter(|a| !a.is_empty())?;
    let origin = format!("{scheme}://{authority}");
```

- [ ] **Step 5: 통과 확인**

Run: `cargo test items::` 와 `cargo test icons::`
Expected: 모두 PASS

- [ ] **Step 6: 커밋**

```bash
cargo fmt
git add src/items.rs src/icons.rs
git commit -m "fix: 쿼리에 ://가 있는 주소 거부, 포트 있는 사이트의 아이콘 경로"
```

---

### Task 2: 아이콘을 임시 파일에 쓰고 성공할 때만 옮기기

**Files:**
- Modify: `src/win/shell_items.rs` (`write_via_temp` 추가, `save_bitmap_png`)
- Modify: `src/icons.rs` (`save_image_png`)

**Interfaces:**
- Produces: `pub fn write_via_temp(dest: &Path, write: impl FnOnce(&Path) -> windows::core::Result<()>) -> windows::core::Result<()>`

- [ ] **Step 1: 실패하는 테스트 작성**

`src/win/shell_items.rs` 테스트 모듈에 추가:

```rust
    #[test]
    fn failed_write_leaves_no_file() {
        let dir = std::env::temp_dir().join(format!("ytl-tmp-{}", crate::items::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("a.png");
        let failed = write_via_temp(&dest, |tmp| {
            std::fs::write(tmp, b"half").unwrap();
            Err(windows::core::Error::empty())
        });
        assert!(failed.is_err());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "대상도 임시 파일도 남지 않아야 한다");
        write_via_temp(&dest, |tmp| {
            std::fs::write(tmp, b"whole").unwrap();
            Ok(())
        })
        .unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"whole");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    }
```

`src/icons.rs` 테스트 모듈에 추가:

```rust
    #[test]
    fn broken_image_does_not_leave_a_cached_icon() {
        let _ = unsafe {
            windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)
        };
        let png = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icon.png")).unwrap();
        let cut = &png[..png.len() / 2];
        let dir = std::env::temp_dir().join(format!("ytl-broken-{}", crate::items::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("a.png");
        let saved = save_image_png(cut, &dest).is_ok();
        // 잘린 그림을 디코더가 받아 주더라도 "성공했을 때만 파일이 있다"는 지켜야 한다
        assert_eq!(saved, dest.exists());
        let files = std::fs::read_dir(&dir).unwrap().count();
        assert_eq!(files, usize::from(saved), "임시 파일이 남으면 안 된다");
    }
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test failed_write_leaves_no_file`
Expected: 컴파일 실패 (`write_via_temp` 없음)

- [ ] **Step 3: 구현**

`src/win/shell_items.rs`에 추가:

```rust
/// `dest` 옆 임시 파일에 쓰고, 성공했을 때만 `dest`로 옮긴다. 아이콘 캐시는 파일이 있으면 다시 받지 않으므로
/// 쓰다 실패한 파일을 남기면 안 된다. `write`는 끝날 때 파일 핸들을 모두 놓아야 한다.
pub fn write_via_temp(
    dest: &Path,
    write: impl FnOnce(&Path) -> windows::core::Result<()>,
) -> windows::core::Result<()> {
    let tmp = dest.with_extension("png.tmp");
    let result = write(&tmp).and_then(|()| {
        std::fs::rename(&tmp, dest).map_err(|e| windows::core::Error::new(windows::core::HRESULT(-1), e.to_string()))
    });
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}
```

같은 파일 `save_bitmap_png`의 클로저 안을 감싼다. `let result = unsafe { ... };`를 아래로 바꾼다:

```rust
    // SAFETY: WIC로 비트맵을 감싸 PNG로 쓴다
    let result = write_via_temp(path, |tmp| unsafe {
        let wic: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let source = wic.CreateBitmapFromHBITMAP(bitmap, HPALETTE::default(), WICBitmapUseAlpha)?;
        let stream = wic.CreateStream()?;
        stream.InitializeFromFilename(&HSTRING::from(tmp.as_os_str()), GENERIC_WRITE.0)?;
        let encoder = wic.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())?;
        encoder.Initialize(&stream, WICBitmapEncoderNoCache)?;
        let mut frame = None;
        encoder.CreateNewFrame(&mut frame, std::ptr::null_mut())?;
        let frame = frame.ok_or_else(windows::core::Error::empty)?;
        frame.Initialize(None)?;
        frame.WriteSource(&source, std::ptr::null())?;
        frame.Commit()?;
        encoder.Commit()
    });
```

`src/icons.rs`: `use crate::win::shell_items::{apps_folder_path, save_item_icon, write_via_temp};`로 바꾸고, `save_image_png`의 본문을 나눈다. 기존 `unsafe` 블록 전체를 새 함수 `encode_image_png`로 옮기고, 그 안의 `dest`를 `tmp`로 쓴다:

```rust
pub fn save_image_png(bytes: &[u8], dest: &Path) -> windows::core::Result<()> {
    if let Some(dir) = dest.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    write_via_temp(dest, |tmp| encode_image_png(bytes, tmp))
}

fn encode_image_png(bytes: &[u8], dest: &Path) -> windows::core::Result<()> {
    // SAFETY: 메모리 스트림을 WIC로 디코드해 PNG로 다시 쓴다
    unsafe {
        // (기존 unsafe 블록 내용을 그대로 옮긴다. `dest` 이름은 그대로 두어도 된다)
    }
}
```

- [ ] **Step 4: 통과 확인**

Run: `cargo test failed_write_leaves_no_file` 와 `cargo test broken_image_does_not_leave_a_cached_icon` 와 `cargo test saves_favicon_from_page_link_and_scales_down` 와 `cargo test saves_an_app_icon_as_png`
Expected: 모두 PASS. `rename`이 파일 사용 중으로 실패하면 스트림·인코더가 아직 살아 있는 것이니 쓰기를 별도 함수로 분리했는지 확인한다.

- [ ] **Step 5: 커밋**

```bash
cargo fmt
git add src/win/shell_items.rs src/icons.rs
git commit -m "fix: 만들다 실패한 아이콘 파일이 캐시에 남아 다시 받지 않던 문제"
```

---

### Task 3: 웹 항목을 열 때 5.5초 뒤 크롬이 튀어나오는 문제

**Files:**
- Modify: `src/chrome.rs` (`pick_window` 추가, `focus_chrome_window`)

- [ ] **Step 1: 실패하는 테스트 작성**

`src/chrome.rs` 테스트 모듈에 추가:

```rust
    #[test]
    fn picks_window_without_waiting_when_no_title_is_wanted() {
        // 찾는 제목이 없으면 크롬 창이 보이는 즉시
        assert_eq!(pick_window(&["새 탭", "메일"], None, false), Some(0));
        assert_eq!(pick_window(&[], None, false), None);
        // 찾는 제목이 있으면 그 창을, 없으면 시간이 다 될 때까지 기다린다
        assert_eq!(pick_window(&["메일", "로파이 - YouTube"], Some("YouTube"), false), Some(1));
        assert_eq!(pick_window(&["메일"], Some("YouTube"), false), None);
        assert_eq!(pick_window(&["메일"], Some("YouTube"), true), Some(0));
        assert_eq!(pick_window(&[], Some("YouTube"), true), None);
    }
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test picks_window_without_waiting`
Expected: 컴파일 실패 (`pick_window` 없음)

- [ ] **Step 3: 구현**

`is_chrome_window` 아래에 추가하고 `focus_chrome_window`를 바꾼다:

```rust
/// 앞으로 가져올 창 번호 (순수 함수). 찾는 제목이 없으면 첫 창, 있으면 그 창.
/// 제목을 못 찾았으면 시간이 다 된 뒤에만 첫 창으로 물러난다.
fn pick_window(titles: &[&str], prefer: Option<&str>, expired: bool) -> Option<usize> {
    let first = (!titles.is_empty()).then_some(0);
    match prefer {
        None => first,
        Some(want) => titles.iter().position(|t| t.contains(want)).or(if expired { first } else { None }),
    }
}

/// 방금 연 탭의 크롬 창을 찾아 최대화·전면화. `prefer_title`이 제목에 들어간 창을 우선한다.
pub fn focus_chrome_window(prefer_title: Option<&str>, timeout: Duration) -> bool {
    std::thread::sleep(Duration::from_millis(500));
    let deadline = Instant::now() + timeout;
    loop {
        let windows: Vec<TopWindow> = list_top_windows().into_iter().filter(is_chrome_window).collect();
        let titles: Vec<&str> = windows.iter().map(|w| w.title.as_str()).collect();
        let expired = Instant::now() >= deadline;
        if let Some(i) = pick_window(&titles, prefer_title, expired) {
            return maximize_and_focus(windows[i].handle());
        }
        if expired {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
```

- [ ] **Step 4: 통과 확인**

Run: `cargo test chrome::`
Expected: 모두 PASS

- [ ] **Step 5: 커밋**

```bash
cargo fmt
git add src/chrome.rs
git commit -m "fix: 웹 항목을 열고 5초 뒤 크롬이 앞으로 튀어나오던 문제"
```

---

### Task 4: 설정 파일을 잃지 않게 읽기

**Files:**
- Modify: `src/config.rs` (`LoadReport`, `Storage`, `load`, `recover`, `save`, 테스트)
- Modify: `src/app.rs` (`startup_notices`, 테스트의 `LoadReport` 리터럴)

**Interfaces:**
- Produces: `LoadReport { recovered: bool, migrated: bool, skipped_items: usize, read_failed: bool }`

- [ ] **Step 1: 실패하는 테스트 작성**

`src/config.rs` 테스트 모듈에 추가:

```rust
    #[test]
    fn unknown_item_is_skipped_and_the_rest_are_kept() {
        let dir = temp_dir("skip");
        let text = r#"{"schema":2,"locked":true,"items":[
            {"kind":"web","id":"w","url":"https://a.com"},
            {"kind":"hologram","id":"h"},
            {"kind":"youtube","id":"y","query":"q"}]}"#;
        fs::write(dir.join(CONFIG_NAME), text).unwrap();
        let (config, report) = Storage::new(&dir).load();
        assert_eq!(report.skipped_items, 1);
        assert!(!report.recovered);
        assert!(config.locked, "다른 설정은 그대로");
        let ids: Vec<&str> = config.items.iter().map(Item::id).collect();
        assert_eq!(ids, ["w", "y"]);
        assert_eq!(fs::read_to_string(dir.join(BACKUP_NAME)).unwrap(), text, "원본을 남긴다");
    }

    #[test]
    fn unreadable_file_starts_with_defaults_and_is_never_overwritten() {
        let dir = temp_dir("unreadable");
        // 폴더를 파일처럼 읽으면 NotFound가 아닌 오류가 난다
        fs::create_dir(dir.join(CONFIG_NAME)).unwrap();
        let storage = Storage::new(&dir);
        let (config, report) = storage.load();
        assert_eq!(config, Config::default());
        assert!(report.read_failed);
        assert!(!report.recovered);
        storage.save(&Config { locked: true, ..Config::default() }).unwrap();
        assert!(dir.join(CONFIG_NAME).is_dir(), "이번 실행은 저장하지 않는다");
        assert!(!dir.join("config.json.tmp").exists());
        assert!(!dir.join(BACKUP_NAME).exists());
    }
```

`src/app.rs`의 `startup_notices_cover_recovery_and_upgrade` 안에서 리터럴을 고치고 검사를 더한다:

```rust
        let recovered = LoadReport { recovered: true, ..Default::default() };
```

```rust
        let skipped = LoadReport { skipped_items: 2, ..Default::default() };
        assert!(startup_notices(skipped, "", "0.2.0")[0].contains("2개"));
        let failed = LoadReport { read_failed: true, ..Default::default() };
        assert!(startup_notices(failed, "", "0.2.0")[0].contains("저장되지 않습니다"));
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test config::`
Expected: 컴파일 실패 (`skipped_items`, `read_failed` 없음)

- [ ] **Step 3: 구현**

`src/config.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LoadReport {
    /// 파일이 깨져서 .bak으로 옮기고 기본값으로 시작함
    pub recovered: bool,
    /// 0.1.x 설정을 변환함
    pub migrated: bool,
    /// 읽지 못해 건너뛴 항목 수 (원본은 .bak에 있다)
    pub skipped_items: usize,
    /// 파일을 읽을 수 없어 기본값으로 시작함 (이번 실행은 저장하지 않는다)
    pub read_failed: bool,
}

pub struct Storage {
    pub dir: PathBuf,
    /// 설정을 읽지 못한 실행: 원본을 덮어쓰지 않도록 저장하지 않는다
    read_only: std::cell::Cell<bool>,
}
```

`Storage::new`는 `Storage { dir: dir.into(), read_only: std::cell::Cell::new(false) }`.

`load`:

```rust
    pub fn load(&self) -> (Config, LoadReport) {
        let path = self.path();
        let bytes = match read_with_retry(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return (Config::default(), LoadReport::default()),
            Err(e) => {
                crate::logging::error(&format!("설정 파일을 읽지 못했습니다: {e}"));
                self.read_only.set(true);
                return (Config::default(), LoadReport { read_failed: true, ..Default::default() });
            }
        };
        let parsed: Option<Value> = std::str::from_utf8(&bytes)
            .ok()
            .map(|s| s.trim_start_matches('\u{feff}'))
            .and_then(|s| serde_json::from_str(s).ok());
        let Some(mut value) = parsed else { return self.recover(&path) };
        if value.get("schema").and_then(Value::as_u64) == Some(1) {
            return match migrate_v1(&value) {
                Some(config) => {
                    let _ = fs::copy(&path, self.dir.join(V1_BACKUP_NAME));
                    let _ = self.save(&config);
                    (config, LoadReport { migrated: true, ..Default::default() })
                }
                None => self.recover(&path),
            };
        }
        let skipped_items = drop_bad_items(&mut value);
        if skipped_items > 0 {
            let _ = fs::copy(&path, self.dir.join(BACKUP_NAME));
        }
        match serde_json::from_value::<Config>(value) {
            Ok(mut config) => {
                config.schema = SCHEMA;
                (config, LoadReport { skipped_items, ..Default::default() })
            }
            Err(_) => self.recover(&path),
        }
    }
```

`recover`의 반환을 `LoadReport { recovered: true, ..Default::default() }`로, `save` 첫 줄에 추가:

```rust
        if self.read_only.get() {
            return Ok(());
        }
```

파일 수준 함수 추가:

```rust
const READ_RETRIES: u32 = 3;
const READ_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(100);

/// 백신 등이 잠깐 잡고 있어도 초기화로 넘어가지 않게, 없는 파일이 아닌 읽기 실패는 몇 번 더 시도한다.
fn read_with_retry(path: &Path) -> io::Result<Vec<u8>> {
    let mut result = fs::read(path);
    for _ in 0..READ_RETRIES {
        match &result {
            Err(e) if e.kind() != io::ErrorKind::NotFound => {
                std::thread::sleep(READ_RETRY_DELAY);
                result = fs::read(path);
            }
            _ => break,
        }
    }
    result
}

/// 읽을 수 없는 항목(모르는 종류 등)을 빼고 뺀 수를 돌려준다. 하나 때문에 설정 전체를 잃지 않게 한다.
fn drop_bad_items(value: &mut Value) -> usize {
    let Some(items) = value.get_mut("items").and_then(Value::as_array_mut) else { return 0 };
    let before = items.len();
    items.retain(|item| serde_json::from_value::<Item>(item.clone()).is_ok());
    before - items.len()
}
```

`src/app.rs`의 `startup_notices`에 추가 (`report.recovered` 검사 아래):

```rust
    if report.read_failed {
        notices.push("설정 파일을 읽지 못해 기본 설정으로 시작했습니다. 이번 실행의 변경은 저장되지 않습니다.".to_string());
    }
    if report.skipped_items > 0 {
        notices.push(format!(
            "읽을 수 없는 항목 {}개를 건너뛰었습니다. 이전 파일은 config.json.bak에 있습니다.",
            report.skipped_items
        ));
    }
```

- [ ] **Step 4: 통과 확인**

Run: `cargo test config::` 와 `cargo test startup_notices`
Expected: 모두 PASS

- [ ] **Step 5: 커밋**

```bash
cargo fmt
git add src/config.rs src/app.rs
git commit -m "fix: 항목 하나나 일시적 읽기 실패로 설정 전체를 잃지 않게"
```

---

### Task 5: 패널 창 상태 버그 (캡처 해제, 초기화 저장, 배율, 최소 높이)

**Files:**
- Modify: `src/ui/panel.rs`

**Interfaces:**
- Produces: 테스트 모듈 `window_tests`의 `pump`, `open`, `spec`, `tile`, `close` (Task 6, 7에서 다시 쓴다)
- Produces: `Panel::ensure_panel_height(&mut self) -> bool`, `Panel::emit_panel_rect(&self)`

- [ ] **Step 1: 실패하는 테스트 작성**

`src/ui/panel.rs` 끝에 추가:

```rust
#[cfg(test)]
mod window_tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, EnumThreadWindows, GW_OWNER, GetWindow, MSG, PM_REMOVE, PeekMessageW, SendMessageW,
        TranslateMessage, WM_CAPTURECHANGED,
    };
    use windows::core::BOOL;

    pub fn pump(ms: u64) {
        let end = Instant::now() + std::time::Duration::from_millis(ms);
        let mut msg = MSG::default();
        while Instant::now() < end {
            unsafe {
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    pub fn tile(id: &str) -> TileView {
        TileView { id: id.into(), label: id.into(), badge: "a".into(), kind: TileKind::Web, ..Default::default() }
    }

    pub fn spec(mode: Mode, rect: PxRect, tiles: usize) -> OpenSpec {
        OpenSpec {
            mode,
            edge: DockEdge::Bottom,
            panel_rect: rect,
            theme: Theme::Glass,
            locked: false,
            tiles: (0..tiles).map(|i| tile(&i.to_string())).collect(),
            custom_theme: CustomTheme::default(),
            panel_icon_size: 56,
            dock_offset: 0.5,
            dock_size: 48,
        }
    }

    pub fn open(spec: OpenSpec) -> (PanelHandle, Rc<RefCell<Vec<UiEvent>>>) {
        let _ = unsafe { windows::Win32::System::Ole::OleInitialize(None) };
        let events = Rc::new(RefCell::new(Vec::new()));
        let log = events.clone();
        let handle = Panel::open(spec, Box::new(move |e| log.borrow_mut().push(e))).unwrap();
        pump(100);
        (handle, events)
    }

    pub fn close(handle: &PanelHandle) {
        handle.send(PanelCmd::Close);
        pump(50);
    }

    fn content_of(glass: HWND) -> HWND {
        unsafe extern "system" fn find(h: HWND, l: LPARAM) -> BOOL {
            // SAFETY: l은 아래에서 넘긴 (소유자, 결과) 포인터
            let state = unsafe { &mut *(l.0 as *mut (HWND, HWND)) };
            if unsafe { GetWindow(h, GW_OWNER) }.ok() == Some(state.0) {
                state.1 = h;
                return false.into();
            }
            true.into()
        }
        let mut state = (glass, HWND::default());
        let _ =
            unsafe { EnumThreadWindows(GetCurrentThreadId(), Some(find), LPARAM(&mut state as *mut _ as isize)) };
        state.1
    }

    #[test]
    fn losing_capture_while_moving_saves_and_clears_the_press() {
        let (h, events) = open(spec(Mode::Dock, (0, 0, 200, 200), 1));
        h.send(PanelCmd::SetEditing(true));
        pump(50);
        let glass = h.owner();
        // 판 왼쪽 끝 여백(아이콘 밖)을 누르면 판 옮기기가 시작된다
        unsafe {
            SendMessageW(glass, WM_LBUTTONDOWN, None, Some(LPARAM((20 << 16) | 2)));
            SendMessageW(glass, WM_CAPTURECHANGED, None, Some(LPARAM(0)));
        }
        pump(20);
        assert!(events.borrow().iter().any(|e| matches!(e, UiEvent::DockChanged { .. })), "{:?}", events.borrow());
        let _ = unsafe { ReleaseCapture() };
        close(&h);
    }

    #[test]
    fn reset_geometry_is_saved() {
        let (h, events) = open(spec(Mode::Panel, (100, 100, 360, 480), 1));
        h.send(PanelCmd::ResetGeometry);
        pump(50);
        assert!(events.borrow().iter().any(|e| matches!(e, UiEvent::GeometryChanged(_))), "{:?}", events.borrow());
        close(&h);
    }

    #[test]
    fn panel_relays_out_when_dpi_changes() {
        let (h, _events) = open(spec(Mode::Panel, (100, 100, 360, 480), 1));
        let glass = h.owner();
        let content = content_of(glass);
        assert!(!content.0.is_null());
        let r = RECT { left: 100, top: 100, right: 900, bottom: 900 };
        unsafe {
            SendMessageW(
                glass,
                WM_DPICHANGED,
                Some(WPARAM(192 | (192 << 16))),
                Some(LPARAM(&r as *const RECT as isize)),
            );
        }
        pump(20);
        assert_eq!(screen_rect(content), (100, 100, 800, 800), "콘텐츠 창도 따라와야 한다");
        close(&h);
    }

    #[test]
    fn short_saved_panel_grows_at_start_and_is_saved() {
        let (h, events) = open(spec(Mode::Panel, (100, 100, 300, 210), 40));
        let grown = events.borrow().iter().any(|e| matches!(e, UiEvent::GeometryChanged(r) if r.h > 210));
        assert!(grown, "{:?}", events.borrow());
        assert!(screen_rect(h.owner()).3 > 210);
        close(&h);
    }

    #[test]
    fn switching_to_panel_applies_min_height() {
        let (h, events) = open(spec(Mode::Dock, (100, 100, 300, 210), 40));
        h.send(PanelCmd::SetMode(Mode::Panel, DockEdge::Bottom));
        pump(50);
        let grown = events.borrow().iter().any(|e| matches!(e, UiEvent::GeometryChanged(r) if r.h > 210));
        assert!(grown, "{:?}", events.borrow());
        close(&h);
    }
}
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test window_tests`
Expected: 다섯 테스트 FAIL (패닉 메시지에 이벤트 목록 또는 콘텐츠 창 크기). 컴파일 오류면 `use`를 고친다.

- [ ] **Step 3: 구현**

`use` 목록의 `windows::Win32::UI::WindowsAndMessaging`에 `WM_CAPTURECHANGED`를 추가한다.

`impl Panel`에 추가:

```rust
    /// 저장할 지금 패널 사각형을 앱에 알린다
    fn emit_panel_rect(&self) {
        let (x, y, w, h) = self.panel_rect;
        self.emit(UiEvent::GeometryChanged(Rect { x, y, w, h }));
    }

    /// 패널이 모든 아이콘을 보여 주는 높이보다 작으면 늘리고 저장한다. 늘렸으면 true (창은 부른 쪽이 옮긴다).
    fn ensure_panel_height(&mut self) -> bool {
        let w = self.panel_rect.2 as f32 / self.scale;
        let min = (PanelLayout::min_height(w, self.tiles.len(), self.panel_icon) * self.scale).ceil() as i32;
        if self.panel_rect.3 >= min {
            return false;
        }
        self.panel_rect.3 = min;
        self.emit_panel_rect();
        true
    }

    /// 마우스 캡처를 잃음 (끌던 중 Alt+Tab 등). 눌림 상태가 남으면 마우스 오버가 풀리지 않는다.
    /// 우리가 부른 `ReleaseCapture`의 알림은 재진입 방어에 막혀 여기로 오지 않는다.
    fn on_capture_lost(&mut self) {
        self.press = None;
        self.drag = None;
        if self.dock.moving.take().is_some() {
            self.emit_dock_changed();
        }
        self.render();
    }
```

`init`에서 `self.place_windows();` 바로 앞에 추가:

```rust
        if self.mode == Mode::Panel {
            self.ensure_panel_height();
        }
```

`apply`의 `SetTiles` 패널 분기를 바꾼다:

```rust
                    Mode::Panel => {
                        // 항목이 늘어 아래 줄이 가려지면 패널을 그만큼 늘린다
                        if self.ensure_panel_height() {
                            self.place_windows();
                        }
                    }
```

`SetPanelIcon`의 패널 분기를 바꾼다:

```rust
                if self.mode == Mode::Panel {
                    // 아이콘이 커져 아래 줄이 가려지면 패널을 그만큼 늘린다
                    self.ensure_panel_height();
                    self.place_windows();
                }
```

`SetMode`에서 `self.place_windows();` 바로 앞에 추가:

```rust
                if mode == Mode::Panel {
                    self.ensure_panel_height();
                }
```

`ResetGeometry`를 바꾼다:

```rust
            PanelCmd::ResetGeometry => {
                if self.mode == Mode::Panel {
                    self.panel_rect = default_panel_rect(desktop::primary_work_area(), self.scale);
                    self.ensure_panel_height();
                    self.emit_panel_rect();
                }
                self.place_windows();
                self.render();
            }
```

`handle`의 `WM_DPICHANGED` 패널 분기에 두 줄 추가 (핸들러 안의 `SetWindowPos` 알림은 오지 않으므로 직접 부른다):

```rust
                    move_window(hwnd, (r.left, r.top, r.right - r.left, r.bottom - r.top));
                    self.relayout_panel();
                    self.render();
```

`handle`의 `WM_LBUTTONUP` 분기 아래에 추가:

```rust
            WM_CAPTURECHANGED => self.on_capture_lost(),
```

- [ ] **Step 4: 통과 확인**

Run: `cargo test panel::`
Expected: 모두 PASS

- [ ] **Step 5: 린트**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: 경고 없음

- [ ] **Step 6: 커밋**

```bash
cargo fmt
git add src/ui/panel.rs
git commit -m "fix: 캡처를 잃으면 눌림이 남던 문제, 위치 초기화 저장, 패널 배율 변경과 최소 높이"
```

---

### Task 6: 켜자마자 독이 안 보이는 경로 막기, 진단 로그

**Files:**
- Modify: `src/win/desktop.rs` (`Monitor`, `monitors`)
- Modify: `src/ui/panel.rs` (`place_windows`, `init`, 재시도, `PanelCmd::Repin`)
- Modify: `src/app.rs` (`on_shell_restart`, 로그)
- Modify: `src/main.rs` (시작 로그 위치)

**Interfaces:**
- Consumes: Task 5의 `window_tests::{open, spec, pump, close}`
- Produces: `pub struct Monitor { pub handle: HMONITOR, pub work: PxRect, pub primary: bool }`, `pub fn monitors() -> Vec<Monitor>`, `PanelCmd::Repin`

- [ ] **Step 1: 실패하는 테스트 작성**

`src/win/desktop.rs` 테스트 모듈에 추가:

```rust
    #[test]
    fn exactly_one_primary_monitor_matches_the_work_area() {
        let all = monitors();
        let primary: Vec<&Monitor> = all.iter().filter(|m| m.primary).collect();
        assert_eq!(primary.len(), 1);
        assert_eq!(primary[0].work, primary_work_area());
    }
```

`src/ui/panel.rs`의 `tests` 모듈에 추가:

```rust
    #[test]
    fn retry_runs_until_gfx_exists_and_pin_gives_up() {
        assert!(!retry_continues(true, true, 5), "둘 다 되면 끝");
        assert!(retry_continues(false, true, 5), "소유자 지정은 횟수만큼");
        assert!(!retry_continues(false, true, 0));
        assert!(retry_continues(true, false, 0), "그리기 장치는 될 때까지");
        assert!(retry_continues(false, false, 0));
    }
```

`window_tests` 모듈에 추가:

```rust
    #[test]
    fn repin_keeps_the_window_alive() {
        let (h, _) = open(spec(Mode::Dock, (0, 0, 200, 200), 1));
        h.send(PanelCmd::Repin);
        pump(50);
        assert!(unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindow(Some(h.owner())) }.as_bool());
        close(&h);
    }
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test retry_runs_until`
Expected: 컴파일 실패

- [ ] **Step 3: `desktop.rs` 구현**

`use windows::Win32::UI::WindowsAndMessaging::{...}`에 `MONITORINFOF_PRIMARY`를 더하고 `work_areas`를 아래로 바꾼다:

```rust
/// 모니터 하나
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Monitor {
    pub handle: HMONITOR,
    /// 작업 영역 (작업표시줄 제외)
    pub work: PxRect,
    pub primary: bool,
}

/// 연결된 모든 모니터
pub fn monitors() -> Vec<Monitor> {
    unsafe extern "system" fn collect(monitor: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        // SAFETY: data는 아래에서 넘긴 Vec 포인터
        let out = unsafe { &mut *(data.0 as *mut Vec<Monitor>) };
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
            out.push(Monitor {
                handle: monitor,
                work: rect_tuple(info.rcWork),
                primary: info.dwFlags & MONITORINFOF_PRIMARY != 0,
            });
        }
        true.into()
    }
    let mut out: Vec<Monitor> = Vec::new();
    let _ = unsafe { EnumDisplayMonitors(None, None, Some(collect), LPARAM(&mut out as *mut _ as isize)) };
    out
}

/// 모든 모니터의 작업 영역
pub fn work_areas() -> Vec<PxRect> {
    monitors().into_iter().map(|m| m.work).collect()
}
```

- [ ] **Step 4: `panel.rs` 구현**

상수와 순수 함수 추가:

```rust
/// 소유자 지정·그리기 장치가 준비되지 않았을 때 다시 시도하는 타이머
const TIMER_RETRY: usize = 4;
const RETRY_MS: u32 = 2000;
/// 소유자 지정을 다시 시도하는 횟수 (2초 × 30 = 60초)
const PIN_RETRIES: u32 = 30;

/// 다시 시도를 계속할지: 그리기 장치는 될 때까지, 소유자 지정은 정해진 횟수만큼
fn retry_continues(pinned: bool, has_gfx: bool, retries_left: u32) -> bool {
    !has_gfx || (!pinned && retries_left > 0)
}
```

`PanelCmd`에 추가:

```rust
    /// 탐색기가 다시 시작됨: 바탕화면 소유자를 다시 지정한다
    Repin,
```

`Panel` 구조체와 `Panel::open`의 초기값에 추가:

```rust
    /// 바탕화면(Progman)이 소유자로 지정됨. 아니면 맨 아래로 보내지 않는다 (바탕화면 뒤로 숨는다)
    pinned: bool,
    /// 소유자 지정을 다시 시도할 남은 횟수
    pin_retries: u32,
```

```rust
            pinned: false,
            pin_retries: PIN_RETRIES,
```

`impl Panel`에 추가:

```rust
    /// 바탕화면에 붙이고, 되면 다른 창 뒤로 내린다
    fn try_pin(&mut self) {
        self.pinned = desktop::pin_to_desktop(self.glass);
        if self.pinned {
            desktop::send_to_bottom(self.glass);
        }
        logging::info(&format!("바탕화면 소유자 지정: {}", if self.pinned { "성공" } else { "실패" }));
    }

    /// 아직 준비되지 않은 것이 있으면 다시 시도 타이머를 건다
    fn arm_retry(&self) {
        if retry_continues(self.pinned, self.gfx.is_some(), self.pin_retries) {
            unsafe { SetTimer(Some(self.glass), TIMER_RETRY, RETRY_MS, None) };
        }
    }

    fn on_retry(&mut self) {
        if !self.pinned && self.pin_retries > 0 {
            self.pin_retries -= 1;
            self.try_pin();
        }
        if self.gfx.is_none() && !self.content.0.is_null() {
            self.recreate_gfx();
            if self.gfx.is_some() {
                logging::info("그리기 장치를 다시 만들었습니다");
                self.place_windows();
                self.render();
            }
        }
        if !retry_continues(self.pinned, self.gfx.is_some(), self.pin_retries) {
            let _ = unsafe { KillTimer(Some(self.glass), TIMER_RETRY) };
        }
    }
```

`place_windows`의 이름을 `place_once`로 바꾸고 새 `place_windows`를 둔다:

```rust
    /// 지금 모드에 맞게 두 창의 위치·크기를 정한다. 다른 배율의 모니터로 옮겨졌으면 그 배율로 한 번 더 놓는다
    /// (핸들러 안에서 옮기면 `WM_DPICHANGED`가 재진입 방어에 막혀 오지 않는다).
    fn place_windows(&mut self) {
        self.place_once();
        let scale = desktop::scale_for(self.glass);
        if scale != self.scale {
            logging::info(&format!("배율 변경 {} → {scale}", self.scale));
            self.scale = scale;
            self.place_once();
        }
    }
```

`init` 첫머리의 `desktop::pin_to_desktop(self.glass);`를 아래로 바꾼다 (창을 보이기 전에 소유자를 정해 맨 위로 깜빡이지 않게 한다):

```rust
        self.pinned = desktop::pin_to_desktop(self.glass);
```

뒤쪽의 `desktop::send_to_bottom(self.glass);`를 아래로 바꾼다:

```rust
        if self.pinned {
            desktop::send_to_bottom(self.glass);
        }
        logging::info(&format!("바탕화면 소유자 지정: {}", if self.pinned { "성공" } else { "실패" }));
        logging::info(&format!(
            "화면 준비: 모드 {:?}, 배율 {}, 유리 창 {:?}, 그리기 장치 {}",
            self.mode,
            self.scale,
            screen_rect(self.glass),
            self.gfx.is_some()
        ));
        self.arm_retry();
```

`recreate_gfx`의 오류 분기에 다시 시도를 건다:

```rust
            Err(e) => {
                logging::error(&format!("그리기 장치를 다시 만들지 못했습니다: {e}"));
                self.arm_retry();
            }
```

`apply`에 추가:

```rust
            PanelCmd::Repin => {
                self.pin_retries = PIN_RETRIES;
                self.try_pin();
                self.arm_retry();
            }
```

`handle`의 `WM_ACTIVATE` 조건에 `self.pinned`를 더한다:

```rust
                if self.pinned && (wparam.0 & 0xFFFF) as u32 == WA_INACTIVE && !foreground_is_ours() {
```

`WM_TIMER` 분기에 추가:

```rust
                TIMER_RETRY => self.on_retry(),
```

`WM_DISPLAYCHANGE | WM_SETTINGCHANGE` 분기 첫 줄에 추가:

```rust
                if msg == WM_DISPLAYCHANGE {
                    logging::info("화면 구성 변경 알림");
                }
```

- [ ] **Step 5: `app.rs`, `main.rs` 구현**

`src/app.rs`: `use crate::win::desktop;`와 `IsWindow`를 추가한다. `impl App`에 추가:

```rust
    /// 탐색기가 다시 시작됨: 소유자(Progman)가 사라져 독 창이 없어졌으면 다시 열고, 살아 있으면 소유자를 다시 지정한다.
    fn on_shell_restart(&mut self) {
        let alive = self.panel.as_ref().is_some_and(|p| unsafe { IsWindow(Some(p.owner())) }.as_bool());
        if alive {
            self.panel_send(PanelCmd::Repin);
        } else {
            logging::warn("독 창이 없어져 다시 엽니다");
            self.panel = None;
            self.open_panel();
        }
    }

    fn log_monitors() {
        for m in desktop::monitors() {
            logging::info(&format!("모니터 {:?}: 작업 영역 {:?}, 주 모니터 {}", m.handle.0, m.work, m.primary));
        }
    }
```

`start`에서 `self.update_running_silently();` 위에 `Self::log_monitors();`를 넣는다.

`handle`의 `taskbar_created` 분기를 바꾼다:

```rust
            m if m == self.taskbar_created && m != 0 => {
                logging::info("탐색기 다시 시작 알림");
                if let Some(tray) = &self.tray {
                    tray.add();
                }
                self.on_shell_restart();
            }
```

`src/main.rs`: `logging::info(... 시작)` 줄을 뮤텍스 확인 뒤로 옮긴다:

```rust
    // 핸들은 프로세스가 끝날 때까지 쥐고 있는다 (설치 프로그램이 실행 중 감지에 씀)
    let (_mutex, existed) = acquire_mutex(MUTEX_NAME);
    if existed {
        logging::info("이미 실행 중: 기존 창에 보이기 요청");
        notify_existing();
        return;
    }
    logging::info(&format!("{} {} 시작", app::APP_NAME, app::CURRENT_VERSION));
```

- [ ] **Step 6: 통과 확인**

Run: `cargo test` 와 `cargo clippy --all-targets -- -D warnings`
Expected: 모두 PASS, 경고 없음

- [ ] **Step 7: 커밋**

```bash
cargo fmt
git add src/win/desktop.rs src/ui/panel.rs src/app.rs src/main.rs
git commit -m "fix: 시작 때 독이 숨거나 빈 판으로 남는 경로 막기, 진단 로그 추가"
```

---

# 1부. 모든 모니터에 독 표시

### Task 7: 독을 지정한 모니터에 놓기, 위치 동기화 명령

**Files:**
- Modify: `src/win/desktop.rs` (`work_area_of`)
- Modify: `src/ui/panel.rs` (`OpenSpec`, `Panel`, `place_dock`, `move_dock`, `apply`)
- Modify: `src/app.rs` (`open_panel`의 `OpenSpec`, `on_ui`)

**Interfaces:**
- Consumes: Task 6의 `Monitor`, `monitors()`
- Produces: `OpenSpec.monitor: Option<HMONITOR>`, `OpenSpec.editing: bool`, `UiEvent::DockMoving { edge: DockEdge, offset: f32 }`, `PanelCmd::SetDockPlacement { edge: DockEdge, offset: f32, size: u32 }`, `desktop::work_area_of(HMONITOR) -> PxRect`

이 태스크에서는 `SetDockPlacement`를 테스트만 만들어 `dead_code` 경고가 하나 난다. Task 9에서 사라지므로, 이 태스크의 clippy는 `cargo clippy --all-targets`로 돌려 그 경고 하나만 있는지 본다.

- [ ] **Step 1: 실패하는 테스트 작성**

`window_tests`의 `spec` 함수에 필드 두 개를 더한다:

```rust
            monitor: None,
            editing: false,
```

`window_tests`에 추가:

```rust
    #[test]
    fn dock_is_placed_on_the_given_monitor() {
        let m = *desktop::monitors().last().unwrap();
        let (x, y, ..) = m.work;
        let mut s = spec(Mode::Dock, (x, y, 200, 200), 2);
        s.monitor = Some(m.handle);
        let (h, _) = open(s);
        let (gx, gy, gw, gh) = screen_rect(h.owner());
        let (cx, cy) = (gx + gw / 2, gy + gh / 2);
        let (wx, wy, ww, wh) = m.work;
        assert!(cx >= wx && cx < wx + ww && cy >= wy && cy < wy + wh, "{:?} / {:?}", (gx, gy, gw, gh), m.work);
        close(&h);
    }

    #[test]
    fn placement_command_moves_without_reporting_back() {
        let (h, events) = open(spec(Mode::Dock, (0, 0, 200, 200), 3));
        let (_, _, w, height) = screen_rect(h.owner());
        assert!(w > height, "아래 독은 가로로 길다");
        h.send(PanelCmd::SetDockPlacement { edge: DockEdge::Left, offset: 0.2, size: 48 });
        pump(50);
        let (_, _, w, height) = screen_rect(h.owner());
        assert!(height > w, "왼쪽 독은 세로로 길다");
        assert!(!events.borrow().iter().any(|e| matches!(e, UiEvent::DockChanged { .. } | UiEvent::DockMoving { .. })));
        close(&h);
    }

    #[test]
    fn moving_reports_position_while_dragging() {
        let (h, events) = open(spec(Mode::Dock, (0, 0, 200, 200), 1));
        h.send(PanelCmd::SetEditing(true));
        pump(50);
        let glass = h.owner();
        unsafe {
            SendMessageW(glass, WM_LBUTTONDOWN, None, Some(LPARAM((20 << 16) | 2)));
            SendMessageW(glass, WM_MOUSEMOVE, None, Some(LPARAM((20 << 16) | 3)));
        }
        pump(20);
        assert!(events.borrow().iter().any(|e| matches!(e, UiEvent::DockMoving { .. })), "{:?}", events.borrow());
        unsafe { SendMessageW(glass, WM_LBUTTONUP, None, Some(LPARAM((20 << 16) | 3))) };
        close(&h);
    }

    #[test]
    fn opens_in_editing_state_when_asked() {
        let mut s = spec(Mode::Dock, (0, 0, 200, 200), 2);
        s.editing = true;
        let (h, _) = open(s);
        let editing_w = screen_rect(h.owner()).2;
        close(&h);
        let (h, _) = open(spec(Mode::Dock, (0, 0, 200, 200), 2));
        assert!(editing_w > screen_rect(h.owner()).2, "수정 모드는 단추만큼 길다");
        close(&h);
    }

    #[test]
    fn close_while_animating_destroys_both_windows() {
        let (h, _) = open(spec(Mode::Dock, (0, 0, 200, 200), 1));
        let glass = h.owner();
        let content = content_of(glass);
        h.send(PanelCmd::SetTheme(Theme::Dark));
        h.send(PanelCmd::Close);
        pump(50);
        use windows::Win32::UI::WindowsAndMessaging::IsWindow;
        assert!(!unsafe { IsWindow(Some(glass)) }.as_bool());
        assert!(!unsafe { IsWindow(Some(content)) }.as_bool());
    }
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test window_tests`
Expected: 컴파일 실패 (`monitor`, `editing`, `SetDockPlacement`, `DockMoving` 없음)

- [ ] **Step 3: 구현**

`src/win/desktop.rs`에 추가:

```rust
/// 이 모니터의 작업 영역. 모니터가 사라졌으면 주 모니터.
pub fn work_area_of(monitor: HMONITOR) -> PxRect {
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    // SAFETY: 구조체 크기를 넘겨 모니터 정보를 읽는다
    if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        rect_tuple(info.rcWork)
    } else {
        primary_work_area()
    }
}
```

`src/ui/panel.rs`:

`use windows::Win32::Graphics::Gdi::{...}`에 `HMONITOR`를 더한다.

`OpenSpec`에 추가:

```rust
    /// 독을 놓을 모니터 (None = 주 모니터)
    pub monitor: Option<HMONITOR>,
    /// 독 수정 모드로 연다 (수정 중에 다시 만든 독)
    pub editing: bool,
```

`Panel` 구조체에 `monitor: Option<HMONITOR>,`를 추가하고 `Panel::open`에서 채운다:

```rust
            monitor: spec.monitor,
```

`Panel::open`의 `Dock` 초기값을 바꾼다:

```rust
                layout: DockLayout::compute(spec.edge, n, spec.editing),
```
```rust
                editing: spec.editing,
```

`UiEvent`에 추가:

```rust
    /// 독 수정 모드에서 판을 끄는 중 (저장하지 않는다, 다른 독이 따라온다)
    DockMoving {
        edge: DockEdge,
        offset: f32,
    },
```

`PanelCmd`에 추가:

```rust
    /// 다른 독에서 바뀐 위치·크기를 맞춘다
    SetDockPlacement {
        edge: DockEdge,
        offset: f32,
        size: u32,
    },
```

`impl Panel`에 추가:

```rust
    /// 독이 놓이는 모니터의 작업 영역
    fn work_area(&self) -> PxRect {
        self.monitor.map_or_else(desktop::primary_work_area, desktop::work_area_of)
    }
```

`place_dock`과 `move_dock`의 `let work = desktop::primary_work_area();`를 `let work = self.work_area();`로 바꾼다. `place_dock`에서는 `let d = &mut self.dock;`보다 앞으로 옮긴다.

`move_dock`에서 `self.dock.offset = ...` 줄 아래에 추가:

```rust
        self.emit(UiEvent::DockMoving { edge, offset: self.dock.offset });
```

`apply`에 추가:

```rust
            PanelCmd::SetDockPlacement { edge, offset, size } => {
                let d = &mut self.dock;
                if (d.edge, d.offset, d.size) != (edge, offset, size) {
                    (d.edge, d.offset, d.size) = (edge, offset, size);
                    if self.mode == Mode::Dock {
                        self.place_windows();
                        self.render();
                    }
                }
            }
```

`PanelCmd::Close` 첫 줄에 추가:

```rust
                // 실행 중에 닫히는 독도 타이머 해상도를 돌려놓는다
                self.stop_animation();
```

`src/app.rs`:

`open_panel`의 `OpenSpec`에 `monitor: None, editing: false,`를 더한다. `on_ui`의 `match`에 임시 분기를 더한다 (Task 9에서 채운다):

```rust
            UiEvent::DockMoving { .. } => {}
```

- [ ] **Step 4: 통과 확인**

Run: `cargo test panel::` 와 `cargo test desktop::` 와 `cargo clippy --all-targets`
Expected: 테스트 PASS. clippy는 `SetDockPlacement`의 `dead_code` 경고 하나만.

- [ ] **Step 5: 커밋**

```bash
cargo fmt
git add src/win/desktop.rs src/ui/panel.rs src/app.rs
git commit -m "feat: 독을 지정한 모니터에 놓고, 끄는 위치를 알리고 맞추는 명령 추가"
```

---

### Task 8: 설정과 메뉴 토글

**Files:**
- Modify: `src/config.rs` (`Config.all_monitors`)
- Modify: `src/app.rs` (`CMD_ALL_MONITORS`, `mode_entries`, `menu_entries`, `show_menu`, 테스트)

**Interfaces:**
- Produces: `Config.all_monitors: bool`, `fn mode_entries(dock: bool, editing: bool, locked: bool, panel_icon: u32, all_monitors: bool) -> Vec<MenuEntry>`

- [ ] **Step 1: 실패하는 테스트 작성**

`src/config.rs` 테스트 모듈에 추가:

```rust
    #[test]
    fn all_monitors_defaults_on_and_round_trips() {
        assert!(Config::default().all_monitors);
        let old: Config = serde_json::from_str("{}").unwrap();
        assert!(old.all_monitors, "필드가 없는 기존 설정도 켜짐");
        let c = Config { all_monitors: false, ..Config::default() };
        let back: Config = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert!(!back.all_monitors);
    }
```

`src/app.rs` 테스트 모듈에서, 기존 `mode_entries(a, b, c, d)` 호출 모두의 끝에 `, true`를 더하고 아래 테스트를 추가한다:

```rust
    #[test]
    fn all_monitors_toggle_is_only_in_dock_mode() {
        let checked = |entries: Vec<MenuEntry>| {
            entries.into_iter().find_map(|e| match e {
                MenuEntry::Item { text, checked, .. } if text.contains("모든 모니터") => Some(checked),
                _ => None,
            })
        };
        assert_eq!(checked(mode_entries(true, false, false, 56, true)), Some(true));
        assert_eq!(checked(mode_entries(true, false, false, 56, false)), Some(false));
        assert_eq!(checked(mode_entries(false, false, false, 56, true)), None);
    }
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test all_monitors`
Expected: 컴파일 실패

- [ ] **Step 3: 구현**

`src/config.rs`의 `Config`에 추가:

```rust
    /// 독을 모든 모니터에 띄운다 (독 모드에서만)
    pub all_monitors: bool,
```

`Config::default`에 `all_monitors: true,`를 더한다.

`src/app.rs`:

```rust
const CMD_ALL_MONITORS: u32 = 37;
```

`mode_entries`를 바꾼다:

```rust
fn mode_entries(dock: bool, editing: bool, locked: bool, panel_icon: u32, all_monitors: bool) -> Vec<MenuEntry> {
    let mut entries = vec![
        MenuEntry::checked(CMD_MODE_DOCK, "독 모드", dock),
        MenuEntry::checked(CMD_DOCK_EDIT, "    독 수정 모드 (옮기기·크기·순서)", dock && editing),
    ];
    if dock {
        entries.push(MenuEntry::checked(CMD_ALL_MONITORS, "    모든 모니터에 표시", all_monitors));
    }
    entries.push(MenuEntry::checked(CMD_MODE_PANEL, "패널 모드", !dock));
    if !dock {
        entries.extend([
            MenuEntry::item(CMD_RESET_GEOMETRY, "위치·크기 초기화"),
            MenuEntry::checked(CMD_LOCK, "위치 잠금", locked),
            MenuEntry::submenu(
                "아이콘 크기",
                PANEL_ICON_SIZES
                    .iter()
                    .zip(["작게", "보통", "크게", "더 크게"])
                    .enumerate()
                    .map(|(i, (size, name))| {
                        MenuEntry::checked(CMD_PANEL_ICON + i as u32, format!("{name} ({size})"), *size == panel_icon)
                    })
                    .collect(),
            ),
        ]);
    }
    entries
}
```

`menu_entries`의 호출을 바꾼다:

```rust
        entries.extend(mode_entries(
            dock,
            self.dock_editing,
            self.config.locked,
            self.config.panel_icon_size,
            self.config.all_monitors,
        ));
```

`show_menu`의 `CMD_DOCK_EDIT` 분기 아래에 추가 (화면 반영은 Task 9):

```rust
            CMD_ALL_MONITORS => {
                self.config.all_monitors = !self.config.all_monitors;
                self.save();
            }
```

- [ ] **Step 4: 통과 확인**

Run: `cargo test all_monitors` 와 `cargo test config::` 와 `cargo test panel_menu_has_icon_sizes_with_check` 와 `cargo test dock_menu_has_edit_mode`
Expected: 모두 PASS

- [ ] **Step 5: 커밋**

```bash
cargo fmt
git add src/config.rs src/app.rs
git commit -m "feat: 모든 모니터에 표시 설정과 트레이 메뉴"
```

---

### Task 9: 앱이 독 여러 개를 띄우고 변경을 나눠 보내기

**Files:**
- Modify: `src/app.rs`

**Interfaces:**
- Consumes: Task 7의 `OpenSpec.monitor/editing`, `DockMoving`, `SetDockPlacement`, `work_area_of`. Task 6의 `monitors()`, `on_shell_restart`. Task 8의 `Config.all_monitors`
- Produces:
  - `pub type PanelId = u32;`, `const MAIN_PANEL: PanelId = 0;`
  - `struct Mirror { id: PanelId, monitor: HMONITOR, handle: PanelHandle }`
  - `pub fn mirror_monitors(mode: Mode, all_monitors: bool, monitors: &[Monitor]) -> Vec<HMONITOR>`
  - `pub fn relay_targets(from: PanelId, ids: &[PanelId]) -> Vec<PanelId>`
  - `Event::Ui { from: PanelId, event: UiEvent }`
  - `App::panels(&self) -> impl Iterator<Item = &PanelHandle>`, `App::handle_of(&self, id: PanelId) -> Option<&PanelHandle>`, `App::sync_mirrors(&mut self)`, `App::relay(&self, from: PanelId, cmd: PanelCmd)`

- [ ] **Step 1: 실패하는 테스트 작성**

`src/app.rs` 테스트 모듈에 추가:

```rust
    use crate::win::desktop::Monitor;
    use windows::Win32::Graphics::Gdi::HMONITOR;

    fn monitor(n: usize, primary: bool) -> Monitor {
        Monitor { handle: HMONITOR(n as *mut _), work: (0, 0, 100, 100), primary }
    }

    #[test]
    fn mirrors_cover_every_other_monitor_in_dock_mode_only() {
        let three = [monitor(1, false), monitor(2, true), monitor(3, false)];
        assert_eq!(
            mirror_monitors(Mode::Dock, true, &three),
            vec![HMONITOR(1 as *mut _), HMONITOR(3 as *mut _)]
        );
        assert!(mirror_monitors(Mode::Dock, false, &three).is_empty(), "꺼져 있으면 없음");
        assert!(mirror_monitors(Mode::Panel, true, &three).is_empty(), "패널 모드는 창 하나");
        assert!(mirror_monitors(Mode::Dock, true, &[monitor(1, true)]).is_empty(), "모니터 하나면 없음");
    }

    #[test]
    fn relay_skips_the_sender() {
        assert_eq!(relay_targets(1, &[0, 1, 2]), vec![0, 2]);
        assert_eq!(relay_targets(0, &[0]), Vec::<PanelId>::new());
        assert_eq!(relay_targets(9, &[0, 1]), vec![0, 1]);
    }
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test mirrors_cover`
Expected: 컴파일 실패

- [ ] **Step 3: 순수 함수와 타입 추가**

`use` 추가:

```rust
use crate::win::desktop::{self, Monitor};
use windows::Win32::Graphics::Gdi::HMONITOR;
```

(Task 6의 `use crate::win::desktop;` 줄을 이것으로 바꾼다.)

```rust
/// 화면(독·패널) 하나를 가리키는 번호. 닫힌 독에서 늦게 온 이벤트도 구분할 수 있게 다시 쓰지 않는다.
pub type PanelId = u32;
/// 주 패널 (주 모니터의 독, 패널 모드의 패널)
const MAIN_PANEL: PanelId = 0;

/// 보조 모니터의 독
struct Mirror {
    id: PanelId,
    monitor: HMONITOR,
    handle: PanelHandle,
}

/// 보조 독을 띄울 모니터 (순수 함수): 독 모드이고 켜져 있으면 주 모니터를 뺀 나머지
pub fn mirror_monitors(mode: Mode, all_monitors: bool, monitors: &[Monitor]) -> Vec<HMONITOR> {
    if mode != Mode::Dock || !all_monitors {
        return Vec::new();
    }
    monitors.iter().filter(|m| !m.primary).map(|m| m.handle).collect()
}

/// 변경을 전달할 독 (순수 함수): 보낸 독을 뺀 나머지
pub fn relay_targets(from: PanelId, ids: &[PanelId]) -> Vec<PanelId> {
    ids.iter().copied().filter(|id| *id != from).collect()
}
```

- [ ] **Step 4: `App` 구조 바꾸기**

`Event`의 `Ui(UiEvent)`를 바꾼다:

```rust
    Ui {
        from: PanelId,
        event: UiEvent,
    },
```

`App`에 필드 추가, `App::new`에 초기값 추가:

```rust
    /// 보조 모니터의 독들
    mirrors: Vec<Mirror>,
    next_panel_id: PanelId,
```
```rust
            mirrors: Vec::new(),
            next_panel_id: MAIN_PANEL + 1,
```

`panel_send`를 아래로 바꾸고 함수를 더한다:

```rust
    fn panels(&self) -> impl Iterator<Item = &PanelHandle> {
        self.panel.iter().chain(self.mirrors.iter().map(|m| &m.handle))
    }

    fn panel_send(&self, cmd: PanelCmd) {
        for panel in self.panels() {
            panel.send(cmd.clone());
        }
    }

    fn handle_of(&self, id: PanelId) -> Option<&PanelHandle> {
        if id == MAIN_PANEL {
            self.panel.as_ref()
        } else {
            self.mirrors.iter().find(|m| m.id == id).map(|m| &m.handle)
        }
    }

    /// 보낸 독을 뺀 나머지에 명령을 보낸다
    fn relay(&self, from: PanelId, cmd: PanelCmd) {
        let mut ids: Vec<PanelId> = self.mirrors.iter().map(|m| m.id).collect();
        ids.push(MAIN_PANEL);
        for id in relay_targets(from, &ids) {
            if let Some(panel) = self.handle_of(id) {
                panel.send(cmd.clone());
            }
        }
    }
```

`open_panel`을 나눈다:

```rust
    fn panel_spec(&self, monitor: Option<HMONITOR>, panel_rect: PxRect) -> OpenSpec {
        OpenSpec {
            mode: self.config.mode,
            edge: self.config.dock_edge,
            panel_rect,
            theme: self.config.theme,
            locked: self.config.locked,
            tiles: self.tiles(),
            dock_offset: self.config.dock_offset,
            dock_size: self.config.dock_size,
            custom_theme: self.config.custom_theme,
            panel_icon_size: self.config.panel_icon_size,
            monitor,
            editing: self.dock_editing,
        }
    }

    fn open_view(&self, id: PanelId, spec: OpenSpec) -> windows::core::Result<PanelHandle> {
        let send = self.post_event();
        Panel::open(spec, Box::new(move |event: UiEvent| send(Event::Ui { from: id, event })))
    }

    fn open_panel(&mut self) {
        let spec = self.panel_spec(None, initial_rect(self.config.panel_window));
        match self.open_view(MAIN_PANEL, spec) {
            Ok(handle) => self.panel = Some(handle),
            Err(e) => logging::error(&format!("패널 창을 만들지 못했습니다: {e}")),
        }
    }

    /// 보조 독을 닫는다
    fn close_mirrors(&mut self) {
        for mirror in self.mirrors.drain(..) {
            mirror.handle.send(PanelCmd::Close);
        }
    }

    /// 보조 독을 지금 설정·모니터에 맞게 다시 만든다. 실패한 모니터는 건너뛰고 다음 동기화 때 다시 시도한다.
    fn sync_mirrors(&mut self) {
        self.close_mirrors();
        for monitor in mirror_monitors(self.config.mode, self.config.all_monitors, &desktop::monitors()) {
            // 창이 처음부터 그 모니터에서 만들어져야 올바른 배율을 읽는다. 크기는 독을 놓을 때 정해진다
            let (x, y, ..) = desktop::work_area_of(monitor);
            let spec = self.panel_spec(Some(monitor), (x, y, 200, 200));
            let id = self.next_panel_id;
            self.next_panel_id += 1;
            match self.open_view(id, spec) {
                Ok(handle) => self.mirrors.push(Mirror { id, monitor, handle }),
                Err(e) => logging::warn(&format!("보조 독을 만들지 못했습니다: {e}")),
            }
        }
    }
```

`use crate::ui::panel::{...}` 옆에 `use crate::win::desktop::PxRect;`가 필요하면 `use crate::win::desktop::{self, Monitor, PxRect};`로 합친다.

`Mirror.monitor`는 Task 10에서 읽는다. 이 태스크에서 `dead_code` 경고가 나면 clippy는 그 경고 하나만 허용하고 Task 10에서 사라지는지 확인한다.

- [ ] **Step 5: 이벤트 처리 바꾸기**

`on_ui`를 바꾼다 (`from` 인자). Task 10에서 대화상자 소유자로도 쓴다:

```rust
    fn on_ui(&mut self, from: PanelId, event: UiEvent) {
        match event {
            UiEvent::TileClicked(id) if id == UPDATE_TILE_ID => self.install_update(),
            UiEvent::TileClicked(id) => self.open_item(&id),
            UiEvent::AddRequested => self.add_item(),
            UiEvent::EditRequested(id) => self.edit_item(&id),
            UiEvent::DeleteRequested(id) => self.delete_item(&id),
            UiEvent::Reordered(ids) => {
                self.config.items = apply_order(&self.config.items, &ids);
                self.save();
                self.refresh_tiles();
            }
            UiEvent::GeometryChanged(rect) => {
                self.config.panel_window = Some(rect);
                self.save();
            }
            UiEvent::MenuRequested => self.show_menu(),
            UiEvent::Dropped(dropped) => self.add_dropped(dropped),
            UiEvent::DockMoving { edge, offset } => {
                let size = self.config.dock_size;
                self.relay(from, PanelCmd::SetDockPlacement { edge, offset, size });
            }
            UiEvent::DockChanged { edge, offset, size } => {
                self.config.dock_edge = edge;
                self.config.dock_offset = offset;
                self.config.dock_size = size;
                self.save();
                self.relay(from, PanelCmd::SetDockPlacement { edge, offset, size });
            }
            UiEvent::EditingChanged(on) => {
                self.dock_editing = on;
                // 같은 값이면 독이 무시하므로 되풀이되지 않는다
                self.relay(from, PanelCmd::SetEditing(on));
            }
        }
    }
```

`handle`의 이벤트 루프를 바꾼다:

```rust
                        Event::Ui { from, event } => self.on_ui(from, event),
```

`set_mode`를 바꾼다:

```rust
    fn set_mode(&mut self, mode: Mode, edge: DockEdge) {
        self.config.mode = mode;
        self.config.dock_edge = edge;
        self.save();
        // 보조 독은 패널로 바뀌면 안 되므로 먼저 닫는다
        self.close_mirrors();
        if let Some(panel) = &self.panel {
            panel.send(PanelCmd::SetMode(mode, edge));
        }
        self.sync_mirrors();
    }
```

`edit_custom_theme`의 미리보기를 모든 독으로 바꾼다:

```rust
        let panels: Vec<PanelHandle> = self.panels().cloned().collect();
        let preview = move |c: CustomTheme| {
            for p in &panels {
                p.send(PanelCmd::SetCustomTheme(c));
                p.send(PanelCmd::SetTheme(Theme::Custom));
            }
        };
```

`show_menu`의 `CMD_ALL_MONITORS` 분기에 `self.sync_mirrors();`를 더한다.

`start`에서 `self.open_panel();` 아래에 `self.sync_mirrors();`를 더한다.

`on_shell_restart` 끝에 `self.sync_mirrors();`를 더한다 (보조 독도 소유자를 잃었을 수 있다).

- [ ] **Step 6: 통과 확인**

Run: `cargo test app::` 와 `cargo test`
Expected: 모두 PASS

- [ ] **Step 7: 실제 실행으로 확인**

Run: `cargo run --release`
Expected: 모든 모니터에 독이 뜬다. 항목·순서·위치가 같고, 트레이의 "모든 모니터에 표시"로 보조 독이 사라지고 다시 나타난다. 모니터가 하나인 환경이면 이 단계를 건너뛰었다고 보고한다. 설치된 앱이 실행 중이면 새 프로세스가 뮤텍스 때문에 바로 끝나므로, 먼저 트레이 → 종료로 끈다.

- [ ] **Step 8: 커밋**

```bash
cargo fmt
git add src/app.rs
git commit -m "feat: 독 모드에서 모든 모니터에 같은 독을 띄우고 변경을 함께 맞추기"
```

---

### Task 10: 대화상자 위치, 모달, 런처 보이기, 모니터 변경

**Files:**
- Modify: `src/win/desktop.rs` (`monitor_at_cursor`)
- Modify: `src/app.rs`

**Interfaces:**
- Consumes: Task 9의 `Mirror`, `PanelId`, `MAIN_PANEL`, `handle_of`, `panels`, `sync_mirrors`
- Produces: `fn with_disabled<R>(windows: &[HWND], f: impl FnOnce() -> R) -> R`, `fn show_target(mirrors: &[(PanelId, HMONITOR)], cursor: HMONITOR) -> PanelId`, `desktop::monitor_at_cursor() -> HMONITOR`

- [ ] **Step 1: 실패하는 테스트 작성**

`src/win/desktop.rs` 테스트 모듈에 추가:

```rust
    #[test]
    fn cursor_is_on_a_known_monitor() {
        let cursor = monitor_at_cursor();
        assert!(monitors().iter().any(|m| m.handle == cursor));
    }
```

`src/app.rs` 테스트 모듈에 추가:

```rust
    #[test]
    fn show_goes_to_the_dock_under_the_cursor() {
        let (a, b) = (HMONITOR(1 as *mut _), HMONITOR(2 as *mut _));
        let mirrors = [(5, a)];
        assert_eq!(show_target(&mirrors, a), 5);
        assert_eq!(show_target(&mirrors, b), MAIN_PANEL, "보조 독이 없는 모니터면 주 패널");
        assert_eq!(show_target(&[], a), MAIN_PANEL);
    }

    struct Quiet;
    impl Handler for Quiet {
        fn handle(&mut self, _: HWND, _: u32, _: WPARAM, _: LPARAM) -> Option<LRESULT> {
            None
        }
    }

    #[test]
    fn windows_are_disabled_only_during_the_call() {
        use windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;
        let a = crate::win::msg_window::create("YTLauncherTestModalA", Box::new(Quiet)).unwrap();
        let b = crate::win::msg_window::create("YTLauncherTestModalB", Box::new(Quiet)).unwrap();
        let during = with_disabled(&[a, b], || unsafe {
            (IsWindowEnabled(a).as_bool(), IsWindowEnabled(b).as_bool())
        });
        assert_eq!(during, (false, false));
        assert!(unsafe { IsWindowEnabled(a) }.as_bool() && unsafe { IsWindowEnabled(b) }.as_bool());
        let _ = unsafe { DestroyWindow(a) };
        let _ = unsafe { DestroyWindow(b) };
    }
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test show_goes_to_the_dock`
Expected: 컴파일 실패

- [ ] **Step 3: 구현**

`src/win/desktop.rs` (`use`에 `POINT`, `GetCursorPos` 추가):

```rust
/// 마우스 커서가 있는 모니터
pub fn monitor_at_cursor() -> HMONITOR {
    let mut pt = POINT::default();
    // SAFETY: 커서 위치를 읽어 모니터를 찾는다
    unsafe {
        let _ = GetCursorPos(&mut pt);
        MonitorFromPoint(pt, MONITOR_DEFAULTTOPRIMARY)
    }
}
```

`src/app.rs` 파일 수준 함수 추가 (`use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;`, `WM_DISPLAYCHANGE` 추가):

```rust
/// 창들을 막아 둔 채 `f`를 부른다. 대화상자는 소유 창만 막으므로, 그대로 두면 다른 독의 클릭이 쌓였다가
/// 대화상자를 닫을 때 한꺼번에 실행된다.
fn with_disabled<R>(windows: &[HWND], f: impl FnOnce() -> R) -> R {
    for hwnd in windows {
        let _ = unsafe { EnableWindow(*hwnd, false) };
    }
    let result = f();
    for hwnd in windows {
        let _ = unsafe { EnableWindow(*hwnd, true) };
    }
    result
}

/// "런처 보이기"를 받을 독 (순수 함수): 커서가 있는 모니터의 독, 없으면 주 패널.
/// 하나만 올린다. 비활성 알림은 마지막으로 활성화된 창에만 와서, 모두 올리면 나머지가 다른 창 위에 남는다.
fn show_target(mirrors: &[(PanelId, HMONITOR)], cursor: HMONITOR) -> PanelId {
    mirrors.iter().find(|(_, m)| *m == cursor).map_or(MAIN_PANEL, |(id, _)| *id)
}
```

상수 추가:

```rust
/// 모니터 구성이 바뀐 뒤 보조 독을 다시 만드는 타이머 (연달아 오는 알림을 모은다)
const TIMER_MONITORS: usize = 5;
const MONITORS_DEBOUNCE_MS: u32 = 500;
```

`impl App`을 바꾼다:

```rust
    fn dialog_owner(&self, from: PanelId) -> HWND {
        self.handle_of(from).or(self.panel.as_ref()).map(PanelHandle::owner).unwrap_or(self.hwnd)
    }

    fn with_modal<R>(&self, f: impl FnOnce() -> R) -> R {
        let windows: Vec<HWND> = self.panels().map(PanelHandle::owner).collect();
        with_disabled(&windows, f)
    }

    fn show_dock(&self) {
        let mirrors: Vec<(PanelId, HMONITOR)> = self.mirrors.iter().map(|m| (m.id, m.monitor)).collect();
        if let Some(panel) = self.handle_of(show_target(&mirrors, desktop::monitor_at_cursor())) {
            panel.send(PanelCmd::Show);
        }
    }
```

`add_item`, `edit_item`, `delete_item`이 `from`을 받게 바꾼다:

```rust
    fn add_item(&mut self, from: PanelId) {
        let owner = self.dialog_owner(from);
        if let Some(input) = self.with_modal(|| item_dialog(owner, "항목 추가", &Initial::New)) {
            let item = item_from_input(new_id(), input);
            self.config.items.push(item.clone());
            self.save();
            self.update_running();
            self.refresh_tiles();
            self.fetch_icons(vec![item]);
        }
    }

    fn edit_item(&mut self, from: PanelId, id: &str) {
        let Some(index) = self.config.items.iter().position(|i| i.id() == id) else { return };
        let initial = Initial::Edit(input_of(&self.config.items[index]));
        let owner = self.dialog_owner(from);
        let Some(input) = self.with_modal(|| item_dialog(owner, "항목 수정", &initial)) else { return };
        let item = item_from_input(id.to_string(), input);
        if icon_source_changed(&self.config.items[index], &item) {
            self.remove_icon(id);
            self.missing.remove(id);
        }
        self.config.items[index] = item.clone();
        self.save();
        self.update_running();
        self.refresh_tiles();
        self.fetch_icons(vec![item]);
    }

    fn delete_item(&mut self, from: PanelId, id: &str) {
        let Some(item) = self.config.items.iter().find(|i| i.id() == id) else { return };
        let text = HSTRING::from(format!("'{}' 항목을 삭제할까요?", item.display_name()));
        let owner = self.dialog_owner(from);
        let answer = self.with_modal(|| unsafe {
            MessageBoxW(Some(owner), &text, &HSTRING::from(APP_NAME), MB_YESNO | MB_ICONQUESTION)
        });
        if answer == IDYES {
            self.config.items.retain(|i| i.id() != id);
            self.save();
            self.remove_icon(id);
            self.running.remove(id);
            self.missing.remove(id);
            self.refresh_tiles();
        }
    }
```

`on_ui`의 호출을 바꾼다:

```rust
            UiEvent::AddRequested => self.add_item(from),
            UiEvent::EditRequested(id) => self.edit_item(from, &id),
            UiEvent::DeleteRequested(id) => self.delete_item(from, &id),
```

`show_menu`를 바꾼다:

```rust
            CMD_SHOW => self.show_dock(),
            CMD_ADD => self.add_item(MAIN_PANEL),
```

`edit_custom_theme`의 대화상자를 바꾼다:

```rust
        let owner = self.dialog_owner(MAIN_PANEL);
        let chosen = self.with_modal(|| custom_theme_dialog(owner, before, std::rc::Rc::new(preview)));
        match chosen {
```

`install_update`의 확인 창을 바꾼다:

```rust
        let answer = self.with_modal(|| unsafe {
            MessageBoxW(Some(self.hwnd), &text, &HSTRING::from(APP_NAME), MB_YESNO | MB_ICONQUESTION)
        });
```

`handle`을 바꾼다:

```rust
            WM_APP_SHOW => self.show_dock(),
            WM_DISPLAYCHANGE => {
                logging::info("모니터 구성 변경 알림");
                unsafe { SetTimer(Some(hwnd), TIMER_MONITORS, MONITORS_DEBOUNCE_MS, None) };
            }
```

`WM_TIMER`에 추가:

```rust
                TIMER_MONITORS => {
                    let _ = unsafe { KillTimer(Some(hwnd), TIMER_MONITORS) };
                    Self::log_monitors();
                    self.sync_mirrors();
                }
```

- [ ] **Step 4: 통과 확인**

Run: `cargo test` 와 `cargo clippy --all-targets -- -D warnings`
Expected: 모두 PASS, 경고 없음 (Task 7, 9의 `dead_code` 경고도 사라짐)

- [ ] **Step 5: 커밋**

```bash
cargo fmt
git add src/win/desktop.rs src/app.rs
git commit -m "feat: 요청한 모니터에 대화상자 표시, 모달 중 독 막기, 모니터 변경 시 보조 독 다시 만들기"
```

---

### Task 11: 문서 정리와 최종 확인

**Files:**
- Modify: `docs/manual-test-checklist.md`
- Modify: `README.md`
- Modify: `src/ui/scene.rs`, `src/ui/theme.rs`, `src/ui/dialogs.rs`, `src/ui/panel.rs` (머리 주석·남은 주석)

- [ ] **Step 1: 오래된 주석 고치기**

- `src/ui/scene.rs` 1번 줄: `//! 패널·독 한 프레임 그리기: 유리 덧칠, 헤더, 아이콘, ＋, 말풍선, 드래그 중인 아이콘.`
- `src/ui/theme.rs` 1번 줄: `//! 테마(라이트·다크·세피아·투명·사용자 지정)의 색표. 유리(아크릴) 위에 덧칠하는 색과 글자색.`
- `src/ui/dialogs.rs` 4번 줄: `//! 종류 카드(유튜브 / 앱 / 웹 주소 / 파일·폴더)로 입력칸이 바뀐다. 수정할 때는 종류를 바꿀 수 없다.`
- `src/ui/panel.rs`: `/// 독 말풍선이 차지하는 여유 (창 영역 계산용)` 줄을 지운다.

- [ ] **Step 2: 점검표 고치기**

`docs/manual-test-checklist.md`:

11번 줄을 바꾼다:

```markdown
- [ ] 독에는 항목 아이콘만 있다(항목이 없으면 ＋ 하나). 긴 이름에 마우스를 올려도 말풍선에 이름 전체가 한 줄로 나온다(아래·왼쪽·오른쪽 독 모두)
```

25번 줄의 `"앱을 찾을 수 없습니다"`를 `"실행할 앱이나 파일을 찾을 수 없습니다"`로 바꾼다.

`## 패널 모드` 앞에 추가:

```markdown
## 모든 모니터 표시
- [ ] 모든 모니터에 같은 항목·위치·크기로 뜨고, 트레이 메뉴 "모든 모니터에 표시"로 켜고 끌 수 있으며 재시작 후 유지된다
- [ ] 배율이 다른 모니터에서도 또렷하고, 확대와 말풍선은 마우스가 있는 독에서만 나타난다
- [ ] 한 독에서 항목을 추가하거나 순서를 바꾸면 다른 독에 바로 반영된다
- [ ] 독 수정 모드에서 끌어 옮기면 다른 독이 함께 움직이고, 수정을 끝내고 재시작해도 유지된다
- [ ] 보조 모니터 독에서 수정을 누르면 그 모니터에 창이 뜨고, 그동안 다른 독을 누를 수 없다
- [ ] 모니터를 빼고 다시 꽂아도 독이 돌아오고 설정이 유지된다
- [ ] 주 모니터를 바꿔도 독이 모니터마다 하나씩만 있다
- [ ] 패널 모드에서는 창이 하나만 뜬다
- [ ] 런처 보이기로 올린 독이, 다른 창을 누르면 다시 뒤로 내려간다
```

`## 안정성` 아래에 추가:

```markdown
- [ ] 탐색기를 다시 시작해도 독과 트레이 아이콘이 돌아온다
- [ ] 재부팅을 여러 번 해도 독이 뜨고, 로그에 모니터·소유자 지정 결과가 남는다
- [ ] 웹 항목을 연 뒤 다른 창으로 옮겨도 몇 초 뒤 크롬이 앞으로 튀어나오지 않는다
- [ ] 판을 끌던 중 Alt+Tab을 눌러도 마우스 오버가 풀린다
- [ ] 패널 모드에서 위치·크기 초기화 후 재시작해도 초기화한 자리에 뜬다
```

`## 기록`의 메모리 줄을 바꾼다:

```markdown
- 메모리(작업 관리자, 독 모드 대기, 기준 34.0MB): 모니터 1대 / 2대 / 3대
```

- [ ] **Step 3: README 고치기**

`README.md`의 `독 수정 모드` 줄 아래에 추가:

```markdown
- **모든 모니터 표시:** 독 모드에서 모니터마다 같은 독이 뜬다(항목·순서·위치·크기 동일). 트레이 메뉴 → 모든 모니터에 표시로 켜고 끈다.
```

- [ ] **Step 4: 전체 확인**

Run: `cargo fmt --check` 와 `cargo clippy --locked --all-targets -- -D warnings` 와 `cargo test --locked`
Expected: 모두 통과

- [ ] **Step 5: 메모리 측정 (여러 모니터 환경)**

`cargo run --release`로 실행하고 독 모드 대기 상태에서 다음을 실행한다:

```powershell
(Get-Process YTLauncher).PrivateMemorySize64 / 1MB
```

트레이 토글을 끈 값과 켠 값을 보고에 적는다. 값이 크게 늘면(보조 독 하나당 20MB 초과) 그리기 장치 공유를 별도 작업으로 제안하고, 이 태스크에서 최적화하지는 않는다.

- [ ] **Step 6: 커밋**

```bash
git add docs/manual-test-checklist.md README.md src/ui/scene.rs src/ui/theme.rs src/ui/dialogs.rs src/ui/panel.rs
git commit -m "docs: 모든 모니터 표시 안내와 점검 항목, 오래된 주석 정리"
```
