# Rust 독 런처 — 4단계(앱·웹 항목) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 유튜브 말고도 PC 앱과 웹 주소를 독·패널에 올린다. 추가 창에서 종류(유튜브/앱/웹 주소)를 고르고, 앱은 시작 메뉴 "모든 앱" 목록(검색·아이콘)이나 exe/lnk 직접 찾기로 고른다. 앱을 누르면 실행 중이면 그 창을 앞으로, 아니면 실행한다. 실행 중인 앱은 아이콘 아래 점, 찾을 수 없는 앱은 흐리게. 웹은 크롬 새 탭, 아이콘은 사이트 파비콘.

**Architecture:** 셸 기능(`win::shell_items`: AppsFolder 열거, 셸 아이콘, 창의 앱 ID)과 순수 판단(`apps`: 창↔앱 매칭, `icons`: 링크 찾기·형식 판별)을 나눈다. 아이콘 캐시(`%APPDATA%\YT-Launcher\icons\<id>.png`)는 잠깐 뜨는 하위 프로세스(`YTLauncher.exe --fetch-icons`)가 만든다. 실행 중 표시는 `SetWinEventHook`(창 표시·숨김) → 300ms 모아서 재계산(+2초 뒤 한 번 더). 입력 창은 Win32 기본 컨트롤, 앱 목록은 가상 ListView + 보이는 줄만 아이콘 요청.

**Tech Stack:** Rust 1.99, `windows` 0.62 (Shell, PropertiesSystem, WIC, ListView, WinEvent), ureq 3.5.

**Spec:** `docs/superpowers/specs/2026-10-26-dock-launcher-rust-design.md` (4.4 아이콘, 5.1 추가·수정 창, 5.2 앱 선택 창, 5.3 실행, 5.4 아이콘 캐시, 8 오류 처리)

**검증 메모 (프로토타입에서 확인):**
- `cargo test` 93개 통과, clippy 통과. 이 PC에서 앱 235개 열거(142개는 exe 경로까지 확인).
- 실제 화면(임시 APPDATA, 독 맨 위 표시 변형 빌드, 마우스 대신 창 메시지로 클릭):
  - 계산기(Store)·Chrome·Git Bash(데스크톱) 아이콘, naver·github 파비콘이 그려짐. 없는 앱은 글자 아바타.
  - 계산기를 켜면 점이 생기고 끄면 사라짐. 최소화된 계산기 타일을 누르면 복원·전면화. 없는 앱을 누르면 로그에 실패 기록 후 흐리게.
  - ＋ → 종류 버튼 → [앱 선택…] → 목록 첫 앱(Access) 선택 → 저장하면 새 타일에 아이콘이 생김.
- **메모리:** 아이콘을 런처 안에서 만들면 사설 메모리가 13MB→65MB로 늘었다. 원인은 HTTPS 인증서 검사(naver.com 한 번에 +40MB, 프로세스가 끝날 때까지 남음)와 셸 아이콘 추출. 하위 프로세스로 옮겨 사설 약 20MB(작업 집합 약 43~48MB)로 맞췄다. 앱 선택 창을 한 번 열면 셸·ListView 몫으로 사설 약 31MB가 된다. 쉬는 동안 CPU 0ms.
- **2단계 버그 2개 같이 고침:** (1) 입력 창 글꼴이 시스템 DPI 배율을 두 번 곱해 1.5배로 커지고 글자가 잘리던 문제(`SystemParametersInfoForDpi`로 교체), (2) 입력 창 배경이 칠해지지 않던 문제(WM_ERASEBKGND·WM_CTLCOLORSTATIC). 독이 화면 아래에 있을 때 입력 창이 화면 밖으로 나가던 문제도 작업 영역 안으로 맞춤.
- 스펙과 다른 점: 파비콘은 페이지의 `<link rel="apple-touch-icon">`·`<link rel="icon">`을 먼저, `/favicon.ico`를 나중에 시도한다(큰 아이콘이 48px 타일에서 더 선명). SVG 아이콘은 건너뛴다.

## Global Constraints

- 작업 브랜치 `rust-rewrite`. 파이썬 코드는 건드리지 않는다.
- 모든 명령은 저장소 루트에서 PowerShell. 각 태스크 끝에 `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test` 통과 후 커밋. 복사로 바꾼 파일은 수정 시각을 갱신한다.
- 아이콘 캐시는 256px PNG, 파일 이름은 항목 id. 항목을 지우면 아이콘도 지운다. 앱 대상·웹 주소를 바꾸면 다시 만든다. 파비콘 실패는 다음 실행 때 다시 시도.
- 실행 중 판정: 데스크톱 앱은 창 프로세스 exe 경로, Store 앱·크롬 웹앱은 창의 `PKEY_AppUserModel_ID`. `chrome_proxy.exe`는 경로로 판정하지 않는다.
- 앱 실행: 실행 중이면 맨 위 창 복원·전면화, 아니면 `ShellExecuteW("shell:AppsFolder\<target>")` 또는 직접 고른 경로. 대상이 없으면 셸 오류 창 없이 실패 → 트레이 알림 "앱을 찾을 수 없습니다. 수정에서 다시 선택하세요." + 흐리게.
- 수정할 때는 종류를 바꿀 수 없다. 앱 표시 이름을 비우면 앱 이름, 웹은 호스트명.
- UI 문구·주석 한국어.

## File Structure

```
Cargo.toml              (수정) windows 기능 추가: Win32_Storage_EnhancedStorage, Win32_System_Variant, Win32_UI_Accessibility, Win32_UI_Controls_Dialogs, Win32_UI_Shell_Common, Win32_UI_Shell_PropertiesSystem
src/win/shell_items.rs  (신규) 모든 앱 열거, exe 경로 찾기, 셸 아이콘 → PNG, 항목 존재 확인, 창의 앱 ID
src/apps.rs             (신규) 창↔앱 매칭(순수), 실행 중 목록, 실행 또는 전환
src/icons.rs            (신규) 아이콘 캐시 경로, 파비콘 링크 찾기·받기, 이미지 형식 판별, 하위 프로세스 본체
src/ui/gfx.rs           (수정) PNG 그리기(캐시), clear_images
src/ui/scene.rs         (수정) TileView에 icon/running/missing, 앱·웹 아이콘, 실행 중 점, 흐리게
src/ui/panel.rs         (수정) SetTiles: 같은 항목이면 마우스 상태 유지, 이미지 캐시 비우기
src/ui/dialogs.rs       (수정) 종류 선택 입력 창(item_dialog), 공용 도우미, 글꼴·배경·위치 수정
src/ui/app_list.rs      (신규) 앱 선택 창
src/items.rs            (수정) Item::youtube/web 생성 도우미를 테스트 전용으로
src/app.rs              (수정) 입력 창·아이콘 하위 프로세스·실행 중 감시·앱 실행 연결
src/main.rs             (수정) --fetch-icons 하위 프로세스 분기
assets/icon.png         (신규) 테스트용 256px 아이콘 (파이썬 자산에서 복사)
```

---
### Task 1: 셸 항목: 모든 앱 목록·아이콘·창의 앱 ID (shell_items.rs)

**Files:**
- Create: `src/win/shell_items.rs`
- Modify: `Cargo.toml`, `src/win/mod.rs`

**Interfaces:**
- Produces: `shell_items::{APPS_FOLDER, AppEntry{name,target,exe}, expand_known_folder, list_apps() -> Result<Vec<AppEntry>>, file_app(path) -> AppEntry, item_exists(parsing) -> bool, item_image(parsing, px) -> Option<HBITMAP>, apps_folder_path(target), save_bitmap_png, save_item_icon(parsing, &Path, px) -> bool, window_app_id(HWND) -> Option<String>}`. 모두 COM(STA) 초기화된 스레드에서 부른다.

- [ ] **Step 1: Cargo.toml 작성**

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
    "Win32_Graphics_Direct2D",
    "Win32_Graphics_Direct2D_Common",
    "Win32_Graphics_Direct3D",
    "Win32_Graphics_Direct3D11",
    "Win32_Graphics_DirectComposition",
    "Win32_Graphics_DirectWrite",
    "Win32_Graphics_Dwm",
    "Win32_Graphics_Dxgi",
    "Win32_Graphics_Dxgi_Common",
    "Win32_Graphics_Gdi",
    "Win32_Graphics_Imaging",
    "Win32_Security",
    "Win32_Storage_EnhancedStorage",
    "Win32_System_Com",
    "Win32_System_Com_StructuredStorage",
    "Win32_System_LibraryLoader",
    "Win32_System_Registry",
    "Win32_System_SystemInformation",
    "Win32_System_Threading",
    "Win32_System_Variant",
    "Win32_UI_Accessibility",
    "Win32_UI_Controls",
    "Win32_UI_Controls_Dialogs",
    "Win32_UI_HiDpi",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_UI_Shell",
    "Win32_UI_Shell_Common",
    "Win32_UI_Shell_PropertiesSystem",
    "Win32_UI_WindowsAndMessaging",
] }
windows-numerics = "0.3"

[build-dependencies]
embed-resource = "3.0.12"

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

- [ ] **Step 2: src/win/mod.rs 작성**

```rust
//! Win32 API를 감싼 얇은 래퍼들.

pub mod desktop;
pub mod dwm;
pub mod instance;
pub mod menu;
pub mod msg_window;
pub mod reg;
pub mod shell;
#[allow(dead_code)] // 앱·웹 항목 연결(Task 4)에서 지운다
pub mod shell_items;
pub mod windows;
```

- [ ] **Step 3: src/win/shell_items.rs 작성**

```rust
//! 셸 항목: 시작 메뉴 "모든 앱" 목록(shell:AppsFolder), 앱·파일 아이콘 추출, 창의 앱 ID(AUMID).
//!
//! 모두 COM을 쓰므로 호출하는 스레드에서 COM이 초기화돼 있어야 한다.

use std::path::Path;
use windows::Win32::Foundation::GENERIC_WRITE;
use windows::Win32::Foundation::{HWND, SIZE};
use windows::Win32::Graphics::Gdi::{DeleteObject, HBITMAP, HGDIOBJ, HPALETTE};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_ContainerFormatPng, IWICImagingFactory, WICBitmapEncoderNoCache, WICBitmapUseAlpha,
};
use windows::Win32::Storage::EnhancedStorage::{PKEY_AppUserModel_ID, PKEY_Link_TargetParsingPath};
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree};
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow};
use windows::Win32::UI::Shell::{
    BHID_EnumItems, IEnumShellItems, IShellItem, IShellItem2, IShellItemImageFactory, SHCreateItemFromParsingName,
    SIGDN_NORMALDISPLAY, SIGDN_PARENTRELATIVEPARSING, SIIGBF_ICONONLY,
};
use windows::core::{GUID, HSTRING, Interface, PWSTR};

pub const APPS_FOLDER: &str = "shell:AppsFolder";

/// 시작 메뉴 "모든 앱"의 한 줄
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppEntry {
    pub name: String,
    /// shell:AppsFolder 안의 파싱 이름 (Store 앱은 AUMID)
    pub target: String,
    /// 실행 파일 경로 (데스크톱 앱이고 알아낼 수 있을 때만)
    pub exe: Option<String>,
}

fn take_pwstr(p: PWSTR) -> String {
    // SAFETY: 셸이 CoTaskMemAlloc으로 준 문자열을 읽고 해제한다
    unsafe {
        let s = p.to_string().unwrap_or_default();
        CoTaskMemFree(Some(p.0 as *const _));
        s
    }
}

/// `{알려진 폴더 GUID}\나머지` 형식이면 실제 경로로 바꾼다 (예: {Program Files}\Google\Chrome\…\chrome.exe)
pub fn expand_known_folder(target: &str) -> Option<String> {
    let rest = target.strip_prefix('{')?;
    let (guid, tail) = rest.split_once("}\\")?;
    let guid = GUID::try_from(guid).ok()?;
    // SAFETY: 알려진 폴더 경로를 받아 해제한다
    let base = unsafe {
        windows::Win32::UI::Shell::SHGetKnownFolderPath(&guid, windows::Win32::UI::Shell::KF_FLAG_DEFAULT, None)
            .ok()
            .map(take_pwstr)?
    };
    Some(Path::new(&base).join(tail).to_string_lossy().into_owned())
}

fn is_exe(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".exe")
}

fn item_exe(item: &IShellItem, target: &str) -> Option<String> {
    if let Some(path) = expand_known_folder(target).filter(|p| is_exe(p)) {
        return Some(path);
    }
    if is_exe(target) && Path::new(target).is_absolute() {
        return Some(target.to_string());
    }
    let item2: IShellItem2 = item.cast().ok()?;
    // SAFETY: 바로가기 대상 경로 속성을 문자열로 읽는다
    let link = unsafe { item2.GetString(&PKEY_Link_TargetParsingPath) }.ok().map(take_pwstr)?;
    is_exe(&link).then_some(link)
}

/// 시작 메뉴 "모든 앱"을 이름순으로 돌려준다.
pub fn list_apps() -> windows::core::Result<Vec<AppEntry>> {
    // SAFETY: 셸 폴더를 열어 항목을 하나씩 읽는다
    unsafe {
        let folder: IShellItem = SHCreateItemFromParsingName(&HSTRING::from(APPS_FOLDER), None)?;
        let items: IEnumShellItems = folder.BindToHandler(None, &BHID_EnumItems)?;
        let mut apps = Vec::new();
        loop {
            let mut fetched = [None];
            if items.Next(&mut fetched, None).is_err() {
                break;
            }
            let Some(item) = fetched[0].take() else { break };
            let name = item.GetDisplayName(SIGDN_NORMALDISPLAY).map(take_pwstr).unwrap_or_default();
            let target = item.GetDisplayName(SIGDN_PARENTRELATIVEPARSING).map(take_pwstr).unwrap_or_default();
            if name.is_empty() || target.is_empty() {
                continue;
            }
            let exe = item_exe(&item, &target);
            apps.push(AppEntry { name, target, exe });
        }
        apps.sort_by_key(|a| a.name.to_lowercase());
        apps.dedup_by(|a, b| a.target == b.target);
        Ok(apps)
    }
}

/// 직접 고른 exe/lnk 파일을 앱 항목으로: 이름은 파일 이름, 바로가기면 대상 exe를 찾아 둔다.
pub fn file_app(path: &str) -> AppEntry {
    let name = Path::new(path).file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.into());
    // SAFETY: 파일 경로로 셸 항목을 만든다
    let item: Option<IShellItem> = unsafe { SHCreateItemFromParsingName(&HSTRING::from(path), None) }.ok();
    let exe = item.and_then(|item| item_exe(&item, path));
    AppEntry { name, target: path.to_string(), exe }
}

/// 셸 항목(앱 목록 항목 또는 파일 경로)의 아이콘을 `size`×`size`px 비트맵으로 얻는다. 호출자가 DeleteObject 해야 한다.
pub fn item_image(parsing_name: &str, size: i32) -> Option<HBITMAP> {
    // SAFETY: 셸 항목 이미지 팩토리로 비트맵을 얻는다
    unsafe {
        let item: IShellItem = SHCreateItemFromParsingName(&HSTRING::from(parsing_name), None).ok()?;
        let factory: IShellItemImageFactory = item.cast().ok()?;
        factory.GetImage(SIZE { cx: size, cy: size }, SIIGBF_ICONONLY).ok()
    }
}

/// 셸 경로(파일 경로 또는 shell:AppsFolder\…)가 가리키는 항목이 있는가
pub fn item_exists(parsing_name: &str) -> bool {
    // SAFETY: 셸 항목을 만들어 보기만 한다
    unsafe { SHCreateItemFromParsingName::<_, _, IShellItem>(&HSTRING::from(parsing_name), None) }.is_ok()
}

/// 앱 목록 항목의 셸 경로
pub fn apps_folder_path(target: &str) -> String {
    format!("{APPS_FOLDER}\\{target}")
}

/// 비트맵을 PNG로 저장하고 비트맵은 해제한다.
pub fn save_bitmap_png(bitmap: HBITMAP, path: &Path) -> windows::core::Result<()> {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // SAFETY: WIC로 비트맵을 감싸 PNG로 쓴다
    let result = unsafe {
        (|| {
            let wic: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
            let source = wic.CreateBitmapFromHBITMAP(bitmap, HPALETTE::default(), WICBitmapUseAlpha)?;
            let stream = wic.CreateStream()?;
            stream.InitializeFromFilename(&HSTRING::from(path.as_os_str()), GENERIC_WRITE.0)?;
            let encoder = wic.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())?;
            encoder.Initialize(&stream, WICBitmapEncoderNoCache)?;
            let mut frame = None;
            encoder.CreateNewFrame(&mut frame, std::ptr::null_mut())?;
            let frame = frame.ok_or_else(windows::core::Error::empty)?;
            frame.Initialize(None)?;
            frame.WriteSource(&source, std::ptr::null())?;
            frame.Commit()?;
            encoder.Commit()
        })()
    };
    let _ = unsafe { DeleteObject(HGDIOBJ(bitmap.0)) };
    result
}

/// 셸 항목 아이콘을 PNG 파일로 저장한다.
pub fn save_item_icon(parsing_name: &str, path: &Path, size: i32) -> bool {
    match item_image(parsing_name, size) {
        Some(bitmap) => save_bitmap_png(bitmap, path).is_ok(),
        None => false,
    }
}

/// 창에 붙은 앱 ID (Store 앱 창, 작업표시줄 그룹용 ID를 단 데스크톱 앱 창)
pub fn window_app_id(hwnd: HWND) -> Option<String> {
    // SAFETY: 창 속성 저장소에서 문자열 하나를 읽는다
    unsafe {
        let store: IPropertyStore = SHGetPropertyStoreForWindow(hwnd).ok()?;
        let value = store.GetValue(&PKEY_AppUserModel_ID).ok()?;
        let text = PropVariantToStringAlloc(&value).ok().map(take_pwstr)?;
        (!text.is_empty()).then_some(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn com() {
        let _ = unsafe {
            windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)
        };
    }

    #[test]
    fn lists_start_menu_apps_sorted() {
        com();
        let apps = list_apps().unwrap();
        assert!(!apps.is_empty(), "모든 앱 목록이 비어 있다");
        let names: Vec<String> = apps.iter().map(|a| a.name.to_lowercase()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
        assert!(apps.iter().all(|a| !a.target.is_empty()));
    }

    #[test]
    fn expands_known_folder_paths() {
        com();
        // {Windows} 폴더
        let p = expand_known_folder(r"{F38BF404-1D43-42F2-9305-67DE0B28FC23}\System32\notepad.exe").unwrap();
        assert!(p.to_ascii_lowercase().ends_with(r"\windows\system32\notepad.exe"), "{p}");
        assert_eq!(expand_known_folder("Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"), None);
    }

    #[test]
    fn file_app_uses_file_name_and_exe_path() {
        com();
        let notepad = std::env::var("WINDIR").unwrap() + r"\System32\notepad.exe";
        let app = file_app(&notepad);
        assert_eq!(app.name, "notepad");
        assert_eq!(app.target, notepad);
        assert_eq!(app.exe.as_deref(), Some(notepad.as_str()));
    }

    #[test]
    fn checks_item_existence() {
        com();
        let notepad = std::env::var("WINDIR").unwrap() + r"\System32\notepad.exe";
        assert!(item_exists(&notepad));
        assert!(!item_exists(r"C:\없는\폴더\앱.exe"));
        assert!(!item_exists(&apps_folder_path("Nobody.NoSuchApp_0000000000000!App")));
    }

    #[test]
    fn saves_an_app_icon_as_png() {
        com();
        let apps = list_apps().unwrap();
        let path = std::env::temp_dir().join(format!("ytl-icon-{}.png", crate::items::new_id()));
        let ok = apps.iter().take(10).any(|a| save_item_icon(&apps_folder_path(&a.target), &path, 64));
        assert!(ok);
        assert!(std::fs::metadata(&path).unwrap().len() > 100);
    }
}
```

- [ ] **Step 4: 테스트**

Run: `cargo test shell_items`
Expected: 5 passed (목록 이름순, 알려진 폴더 경로, 파일 앱, 항목 존재, 아이콘 PNG 저장)

- [ ] **Step 5: 전체 검사**

Run: `cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test`
Expected: clippy 오류 없음, `test result: ok`

- [ ] **Step 6: Commit**

```powershell
git add Cargo.toml src/win
git commit -m "feat: 모든 앱 목록·셸 아이콘·창 앱 ID 읽기"
```

---

### Task 2: 앱 실행과 실행 중 판정 (apps.rs)

**Files:**
- Create: `src/apps.rs`
- Modify: `src/main.rs` (모듈 등록, 임시 allow)

**Interfaces:**
- Consumes: `shell_items::{apps_folder_path, item_exists, window_app_id}`, `win::windows::{list_top_windows, restore_and_focus, allow_any_foreground}`, `win::shell::open`
- Produces: `apps::{WindowInfo{hwnd,exe,app_id}, AppKey{target,exe}, window_matches, running_flags(&[Option<AppKey>], &[WindowInfo]) -> Vec<bool>, find_window, current_windows(), Launched{Focused,Started}, launch(&AppKey) -> io::Result<Launched>}`

- [ ] **Step 1: src/apps.rs 작성**

```rust
//! 앱 항목: 실행 중인지 판단(순수 함수), 실행 중이면 그 창으로 전환, 아니면 실행.

use crate::win::shell;
use crate::win::shell_items::{apps_folder_path, item_exists, window_app_id};
use crate::win::windows::{TopWindow, list_top_windows, restore_and_focus};
use std::path::Path;

/// 실행 중 판단에 쓰는 창 정보
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInfo {
    pub hwnd: isize,
    pub exe: String,
    pub app_id: Option<String>,
}

/// 앱 항목 하나의 식별 정보
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppKey {
    /// shell:AppsFolder 파싱 이름(AUMID 등) 또는 직접 고른 exe/lnk 경로
    pub target: String,
    pub exe: Option<String>,
}

fn file_name(path: &str) -> String {
    Path::new(path).file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// 크롬 웹앱의 실행 파일은 실제 창을 띄우지 않는 중계 프로그램이라 경로로는 판단하지 않는다.
const LAUNCHER_ONLY_EXES: [&str; 1] = ["chrome_proxy.exe"];

/// 이 창이 이 앱의 창인가: 창의 앱 ID가 대상과 같거나, 실행 파일 경로가 같으면.
pub fn window_matches(app: &AppKey, w: &WindowInfo) -> bool {
    if w.app_id.as_deref().is_some_and(|id| id.eq_ignore_ascii_case(&app.target)) {
        return true;
    }
    match &app.exe {
        Some(exe) if !LAUNCHER_ONLY_EXES.contains(&file_name(exe).as_str()) => {
            !w.exe.is_empty() && w.exe.eq_ignore_ascii_case(exe)
        }
        _ => false,
    }
}

/// 앱마다 실행 중인 창이 있는지
pub fn running_flags(apps: &[Option<AppKey>], windows: &[WindowInfo]) -> Vec<bool> {
    apps.iter().map(|app| app.as_ref().is_some_and(|a| windows.iter().any(|w| window_matches(a, w)))).collect()
}

/// 이 앱의 창 중 맨 위(Z순서 첫 번째) 창
pub fn find_window(app: &AppKey, windows: &[WindowInfo]) -> Option<isize> {
    windows.iter().find(|w| window_matches(app, w)).map(|w| w.hwnd)
}

/// 지금 보이는 최상위 창들 (앱 ID 포함). 우리 프로세스 창은 뺀다.
pub fn current_windows() -> Vec<WindowInfo> {
    let own = std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    list_top_windows()
        .into_iter()
        .filter(|w: &TopWindow| !w.exe.eq_ignore_ascii_case(&own))
        .map(|w| WindowInfo { hwnd: w.hwnd, app_id: window_app_id(w.handle()), exe: w.exe })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Launched {
    /// 이미 실행 중이라 그 창을 앞으로 가져왔다
    Focused,
    Started,
}

/// 실행 중이면 창을 복원해 앞으로, 아니면 실행한다. 실행할 대상을 못 찾으면 Err.
pub fn launch(app: &AppKey) -> std::io::Result<Launched> {
    if let Some(hwnd) = find_window(app, &current_windows()) {
        restore_and_focus(windows::Win32::Foundation::HWND(hwnd as *mut _));
        return Ok(Launched::Focused);
    }
    let target = if Path::new(&app.target).is_absolute() { app.target.clone() } else { apps_folder_path(&app.target) };
    // 지워진 앱은 셸 오류 창을 띄우지 않고 바로 실패로 돌려준다
    if !item_exists(&target) {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, target));
    }
    crate::win::windows::allow_any_foreground();
    shell::open(&target).map(|_| Launched::Started)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(hwnd: isize, exe: &str, id: Option<&str>) -> WindowInfo {
        WindowInfo { hwnd, exe: exe.into(), app_id: id.map(str::to_string) }
    }

    #[test]
    fn matches_store_app_by_app_id() {
        let calc = AppKey { target: "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App".into(), exe: None };
        let w = win(
            1,
            r"C:\Windows\System32\ApplicationFrameHost.exe",
            Some("microsoft.windowscalculator_8wekyb3d8bbwe!app"),
        );
        assert!(window_matches(&calc, &w));
        assert!(!window_matches(&calc, &win(2, r"C:\x.exe", None)));
    }

    #[test]
    fn matches_desktop_app_by_exe_case_insensitively() {
        let kakao = AppKey { target: "Kakao".into(), exe: Some(r"C:\Program Files\Kakao\KakaoTalk.exe".into()) };
        assert!(window_matches(&kakao, &win(1, r"c:\program files\kakao\kakaotalk.exe", None)));
        assert!(!window_matches(&kakao, &win(1, "", None)));
    }

    #[test]
    fn chrome_web_apps_need_app_id_not_proxy_exe() {
        let pwa = AppKey {
            target: "Chrome._crx_abc".into(),
            exe: Some(r"C:\Program Files\Google\Chrome\Application\chrome_proxy.exe".into()),
        };
        let chrome = r"C:\Program Files\Google\Chrome\Application\chrome.exe";
        assert!(!window_matches(&pwa, &win(1, r"C:\Program Files\Google\Chrome\Application\chrome_proxy.exe", None)));
        assert!(!window_matches(&pwa, &win(1, chrome, Some("Chrome"))));
        assert!(window_matches(&pwa, &win(1, chrome, Some("Chrome._crx_abc"))));
    }

    #[test]
    fn running_flags_and_topmost_window() {
        let a = AppKey { target: "A!App".into(), exe: None };
        let b = AppKey { target: "B".into(), exe: Some(r"C:\b.exe".into()) };
        let windows =
            vec![win(10, r"C:\b.exe", None), win(20, r"C:\other.exe", Some("A!App")), win(30, r"C:\b.exe", None)];
        assert_eq!(running_flags(&[Some(a.clone()), Some(b.clone()), None], &windows), vec![true, true, false]);
        assert_eq!(find_window(&b, &windows), Some(10));
        assert_eq!(find_window(&AppKey { target: "none".into(), exe: None }, &windows), None);
    }
}
```

- [ ] **Step 2: src/main.rs 작성**

```rust
#![cfg_attr(not(test), windows_subsystem = "windows")]
//! YT Launcher 진입점.

mod app;
#[allow(dead_code)] // 앱·웹 항목 연결(Task 4)에서 지운다
mod apps;
mod autostart;
mod chrome;
mod config;
mod items;
mod logging;
mod tray;
mod ui;
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

    // 셸·COM을 쓰는 기능(ShellExecute, 4단계의 앱 목록)을 위해 UI 스레드를 STA로 초기화한다
    let _ = unsafe {
        windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)
    };

    let app = app::App::new(config::Storage::new(&data_dir));
    if let Err(e) = win::msg_window::create(MESSAGE_CLASS, Box::new(app)) {
        logging::error(&format!("메시지 창을 만들지 못했습니다: {e}"));
        return;
    }
    std::process::exit(win::msg_window::run_loop());
}
```

- [ ] **Step 3: 테스트**

Run: `cargo test apps::`
Expected: 4 passed

- [ ] **Step 4: 전체 검사**

Run: `cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test`
Expected: clippy 오류 없음, `test result: ok`

- [ ] **Step 5: Commit**

```powershell
git add src/apps.rs src/main.rs
git commit -m "feat: 앱 실행·전환과 실행 중 판정"
```

---

### Task 3: 아이콘 캐시와 타일 그리기 (icons.rs, gfx, scene, panel)

**Files:**
- Create: `src/icons.rs`, `assets/icon.png`
- Modify: `src/ui/gfx.rs`, `src/ui/scene.rs`, `src/ui/panel.rs`, `src/main.rs`, `src/app.rs` (TileView 기본값만)

**Interfaces:**
- Consumes: Task 1 (`save_item_icon`, `apps_folder_path`), `updater::{Http, UreqHttp}`
- Produces: `icons::{FETCH_ARG, ICON_PX=256, icons_dir, icon_path(dir,id), save_app_icon, resolve_url, icon_links, image_format, save_image_png, save_favicon(&dyn Http, url, dest), missing_icons(dir, items), run_fetch_process(args) -> i32}`; `Gfx::{image(path, rect, opacity) -> bool, clear_images()}`; `TileView{.., icon: Option<PathBuf>, running: bool, missing: bool}` (Default 구현).

- [ ] **Step 1: 테스트용 아이콘 복사**

```powershell
Copy-Item src\yt_launcher\assets\icon.png assets\icon.png
```

- [ ] **Step 2: src/icons.rs 작성**

```rust
//! 아이콘 캐시(`%APPDATA%\YT-Launcher\icons\<항목id>.png`): 앱 아이콘 추출, 웹 파비콘 받기.

use crate::items::{Item, host_of};
use crate::updater::{Http, UreqHttp};
use crate::win::shell_items::{apps_folder_path, save_item_icon};
use std::io::Read;
use std::path::{Path, PathBuf};
use windows::Win32::Foundation::{E_INVALIDARG, GENERIC_WRITE};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_ContainerFormatBmp, GUID_ContainerFormatGif, GUID_ContainerFormatIco,
    GUID_ContainerFormatJpeg, GUID_ContainerFormatPng, GUID_WICPixelFormat32bppPBGRA, IWICBitmapSource,
    IWICImagingFactory, WICBitmapDitherTypeNone, WICBitmapEncoderNoCache, WICBitmapInterpolationModeFant,
    WICBitmapPaletteTypeMedianCut, WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx};
use windows::core::{GUID, HSTRING, Interface};

/// 아이콘 만들기 하위 프로세스를 부르는 인자: `YTLauncher.exe --fetch-icons <데이터 폴더> <항목 JSON>`
pub const FETCH_ARG: &str = "--fetch-icons";

/// 저장하는 아이콘 크기 (px)
pub const ICON_PX: u32 = 256;
const MAX_DOWNLOAD: u64 = 1_000_000;
const MAX_HTML: u64 = 512 * 1024;

pub fn icons_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("icons")
}

pub fn icon_path(data_dir: &Path, id: &str) -> PathBuf {
    icons_dir(data_dir).join(format!("{id}.png"))
}

/// 앱 아이콘을 캐시에 저장한다. `target`이 절대 경로면 그 파일, 아니면 시작 메뉴 앱 목록 항목.
pub fn save_app_icon(target: &str, dest: &Path) -> bool {
    let parsing = if Path::new(target).is_absolute() { target.to_string() } else { apps_folder_path(target) };
    save_item_icon(&parsing, dest, ICON_PX as i32)
}

fn attr_value<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(pos) = lower[from..].find(name) {
        let start = from + pos;
        let before_ok = start == 0 || lower.as_bytes()[start - 1].is_ascii_whitespace();
        let rest = lower[start + name.len()..].trim_start();
        if before_ok && rest.starts_with('=') {
            let offset = tag.len() - rest.len() + 1;
            let value = tag[offset..].trim_start();
            let (quote, body) = match value.chars().next() {
                Some(q @ ('"' | '\'')) => (Some(q), &value[1..]),
                _ => (None, value),
            };
            let end = match quote {
                Some(q) => body.find(q).unwrap_or(body.len()),
                None => body.find(|c: char| c.is_whitespace() || c == '>').unwrap_or(body.len()),
            };
            return Some(&body[..end]);
        }
        from = start + name.len();
    }
    None
}

/// `href`를 `page`(페이지 URL) 기준 절대 URL로
pub fn resolve_url(page: &str, href: &str) -> Option<String> {
    let href = href.trim();
    if href.starts_with("http://") || href.starts_with("https://") {
        return Some(href.to_string());
    }
    if href.starts_with("data:") || href.is_empty() {
        return None;
    }
    let scheme = page.split_once("://")?.0;
    if let Some(rest) = href.strip_prefix("//") {
        return Some(format!("{scheme}://{rest}"));
    }
    let host = host_of(page)?;
    let origin = format!("{scheme}://{host}");
    if href.starts_with('/') {
        return Some(format!("{origin}{href}"));
    }
    let path = page.split_once("://")?.1;
    let dir = match path.find('/') {
        Some(i) => {
            let p = &path[i..];
            let p = p.split(['?', '#']).next().unwrap_or("/");
            &p[..p.rfind('/').unwrap_or(0) + 1]
        }
        None => "/",
    };
    Some(format!("{origin}{dir}{href}"))
}

/// HTML에서 아이콘 링크를 찾아 절대 URL로 돌려준다. 큰 아이콘(apple-touch-icon)을 먼저.
pub fn icon_links(html: &str, page: &str) -> Vec<String> {
    let mut touch = Vec::new();
    let mut normal = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut from = 0;
    while let Some(pos) = lower[from..].find("<link") {
        let start = from + pos;
        let end = lower[start..].find('>').map_or(lower.len(), |e| start + e + 1);
        let tag = &html[start..end];
        if let (Some(rel), Some(href)) = (attr_value(tag, "rel"), attr_value(tag, "href")) {
            let rel = rel.to_ascii_lowercase();
            if let Some(url) = resolve_url(page, href) {
                if rel.contains("apple-touch-icon") {
                    touch.push(url);
                } else if rel.split_whitespace().any(|r| r == "icon") {
                    normal.push(url);
                }
            }
        }
        from = end;
    }
    touch.extend(normal);
    touch
}

fn fetch(http: &dyn Http, url: &str, limit: u64) -> Option<Vec<u8>> {
    let reader = http.get(url, "*/*").ok()?;
    let mut bytes = Vec::new();
    reader.take(limit).read_to_end(&mut bytes).ok()?;
    (!bytes.is_empty()).then_some(bytes)
}

/// 파일 앞부분으로 이미지 형식을 알아낸다 (PNG, ICO, JPEG, GIF, BMP). 순수 함수.
pub fn image_format(bytes: &[u8]) -> Option<GUID> {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => Some(GUID_ContainerFormatPng),
        [0, 0, 1, 0, ..] => Some(GUID_ContainerFormatIco),
        [0xFF, 0xD8, ..] => Some(GUID_ContainerFormatJpeg),
        [b'G', b'I', b'F', ..] => Some(GUID_ContainerFormatGif),
        [b'B', b'M', ..] => Some(GUID_ContainerFormatBmp),
        _ => None,
    }
}

/// 이미지 바이트(PNG·ICO·JPEG·GIF·BMP)를 가장 큰 프레임으로 골라 최대 ICON_PX 크기 PNG로 저장한다.
pub fn save_image_png(bytes: &[u8], dest: &Path) -> windows::core::Result<()> {
    if let Some(dir) = dest.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // SAFETY: 메모리 스트림을 WIC로 디코드해 PNG로 다시 쓴다
    unsafe {
        let wic: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let input = wic.CreateStream()?;
        input.InitializeFromMemory(bytes)?;
        // 형식을 직접 골라 디코더를 만든다: 자동 판별은 설치된 모든 코덱(HEIF·RAW 등)을 불러와 메모리를 크게 쓴다
        let format = image_format(bytes).ok_or_else(|| windows::core::Error::from(E_INVALIDARG))?;
        let decoder = wic.CreateDecoder(&format, std::ptr::null())?;
        decoder.Initialize(&input, WICDecodeMetadataCacheOnDemand)?;
        let mut best = (0u32, 0u32);
        for i in 0..decoder.GetFrameCount()? {
            let frame = decoder.GetFrame(i)?;
            let (mut w, mut h) = (0, 0);
            frame.GetSize(&mut w, &mut h)?;
            if w * h > best.1 {
                best = (i, w * h);
            }
        }
        let frame = decoder.GetFrame(best.0)?;
        let (mut w, mut h) = (0, 0);
        frame.GetSize(&mut w, &mut h)?;
        let converter = wic.CreateFormatConverter()?;
        converter.Initialize(
            &frame,
            &GUID_WICPixelFormat32bppPBGRA,
            WICBitmapDitherTypeNone,
            None,
            0.0,
            WICBitmapPaletteTypeMedianCut,
        )?;
        let mut source: IWICBitmapSource = converter.cast()?;
        let side = w.max(h);
        if side > ICON_PX {
            let scaler = wic.CreateBitmapScaler()?;
            let (nw, nh) = ((w * ICON_PX / side).max(1), (h * ICON_PX / side).max(1));
            scaler.Initialize(&source, nw, nh, WICBitmapInterpolationModeFant)?;
            source = scaler.cast()?;
        }
        let stream = wic.CreateStream()?;
        stream.InitializeFromFilename(&HSTRING::from(dest.as_os_str()), GENERIC_WRITE.0)?;
        let encoder = wic.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())?;
        encoder.Initialize(&stream, WICBitmapEncoderNoCache)?;
        let mut out = None;
        encoder.CreateNewFrame(&mut out, std::ptr::null_mut())?;
        let out = out.ok_or_else(windows::core::Error::empty)?;
        out.Initialize(None)?;
        out.WriteSource(&source, std::ptr::null())?;
        out.Commit()?;
        encoder.Commit()
    }
}

/// 사이트 아이콘을 받아 저장한다: 페이지의 `<link rel=icon>`들 → `/favicon.ico` 순으로 시도.
pub fn save_favicon(http: &dyn Http, page_url: &str, dest: &Path) -> bool {
    let mut candidates = Vec::new();
    if let Some(html) = fetch(http, page_url, MAX_HTML) {
        candidates.extend(icon_links(&String::from_utf8_lossy(&html), page_url));
    }
    if let Some(root) = resolve_url(page_url, "/favicon.ico") {
        candidates.push(root);
    }
    candidates.dedup();
    candidates
        .iter()
        .filter(|url| !url.to_ascii_lowercase().ends_with(".svg"))
        .any(|url| fetch(http, url, MAX_DOWNLOAD).is_some_and(|bytes| save_image_png(&bytes, dest).is_ok()))
}

/// 캐시에 아이콘이 없는 앱·웹 항목 (유튜브는 직접 그린다)
pub fn missing_icons(data_dir: &Path, items: &[Item]) -> Vec<Item> {
    items
        .iter()
        .filter(|i| !matches!(i, Item::Youtube { .. }) && !icon_path(data_dir, i.id()).exists())
        .cloned()
        .collect()
}

/// 하위 프로세스 본체. 하나라도 저장하면 0, 아니면 1.
pub fn run_fetch_process(args: &[String]) -> i32 {
    let (Some(dir), Some(json)) = (args.first(), args.get(1)) else { return 2 };
    let Ok(items) = serde_json::from_str::<Vec<Item>>(json) else { return 2 };
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let dir = Path::new(dir);
    let http = UreqHttp::new();
    let mut saved = false;
    for item in missing_icons(dir, &items) {
        let dest = icon_path(dir, item.id());
        saved |= match &item {
            Item::App { target, .. } => save_app_icon(target, &dest),
            Item::Web { url, .. } => save_favicon(&http, url, &dest),
            Item::Youtube { .. } => false,
        };
    }
    if saved { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updater::HttpFailure;
    use std::collections::HashMap;

    #[test]
    fn resolves_relative_icon_urls() {
        let page = "https://example.com/blog/post.html?x=1";
        assert_eq!(resolve_url(page, "/favicon.ico").unwrap(), "https://example.com/favicon.ico");
        assert_eq!(resolve_url(page, "icon.png").unwrap(), "https://example.com/blog/icon.png");
        assert_eq!(resolve_url(page, "//cdn.example.com/i.png").unwrap(), "https://cdn.example.com/i.png");
        assert_eq!(resolve_url(page, "https://a.b/c.ico").unwrap(), "https://a.b/c.ico");
        assert_eq!(resolve_url("https://naver.com", "favicon.ico").unwrap(), "https://naver.com/favicon.ico");
        assert_eq!(resolve_url(page, "data:image/png;base64,xx"), None);
    }

    #[test]
    fn only_app_and_web_items_without_cached_icon_are_missing() {
        let dir = std::env::temp_dir().join(format!("ytl-icons-{}", crate::items::new_id()));
        std::fs::create_dir_all(icons_dir(&dir)).unwrap();
        std::fs::write(icon_path(&dir, "cached"), b"png").unwrap();
        let items = vec![
            Item::Youtube { id: "y".into(), query: "q".into(), name: "".into() },
            Item::App { id: "cached".into(), name: "a".into(), target: "A!App".into(), exe: None },
            Item::App { id: "new".into(), name: "b".into(), target: "B!App".into(), exe: None },
            Item::Web { id: "w".into(), url: "https://a.com".into(), name: "".into() },
        ];
        let ids: Vec<String> = missing_icons(&dir, &items).iter().map(|i| i.id().to_string()).collect();
        assert_eq!(ids, vec!["new", "w"]);
    }

    #[test]
    fn detects_image_formats_by_signature() {
        assert_eq!(image_format(b"\x89PNG\r\n"), Some(GUID_ContainerFormatPng));
        assert_eq!(image_format(&[0, 0, 1, 0, 2]), Some(GUID_ContainerFormatIco));
        assert_eq!(image_format(&[0xFF, 0xD8, 0xFF]), Some(GUID_ContainerFormatJpeg));
        assert_eq!(image_format(b"GIF89a"), Some(GUID_ContainerFormatGif));
        assert_eq!(image_format(b"<!doctype html>"), None);
        assert_eq!(image_format(b"<svg"), None);
    }

    #[test]
    fn finds_icon_links_touch_icon_first() {
        let html = r#"<head><link rel="stylesheet" href="/a.css">
            <LINK REL='icon' type="image/png" href="/fav-32.png">
            <link href="/apple.png" rel="apple-touch-icon" sizes="180x180">
            <link rel="shortcut icon" href=/old.ico></head>"#;
        assert_eq!(
            icon_links(html, "https://site.kr/"),
            vec!["https://site.kr/apple.png", "https://site.kr/fav-32.png", "https://site.kr/old.ico"]
        );
        assert!(icon_links("<html></html>", "https://x.y").is_empty());
    }

    struct FakeHttp(HashMap<String, Vec<u8>>);
    impl Http for FakeHttp {
        fn get(&self, url: &str, _: &str) -> Result<Box<dyn Read + Send>, HttpFailure> {
            match self.0.get(url) {
                Some(b) => Ok(Box::new(std::io::Cursor::new(b.clone()))),
                None => Err(HttpFailure::Status(404)),
            }
        }
    }

    #[test]
    fn saves_favicon_from_page_link_and_scales_down() {
        let _ = unsafe {
            windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)
        };
        // 앱 아이콘(256px PNG)을 사이트 아이콘처럼 내준다
        let png = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icon.png")).unwrap();
        let http = FakeHttp(HashMap::from([
            ("https://site.kr".to_string(), br#"<link rel="icon" href="/i.png">"#.to_vec()),
            ("https://site.kr/i.png".to_string(), png),
        ]));
        let dest = std::env::temp_dir().join(format!("ytl-fav-{}.png", crate::items::new_id()));
        assert!(save_favicon(&http, "https://site.kr", &dest));
        assert!(std::fs::metadata(&dest).unwrap().len() > 100);
        let missing = std::env::temp_dir().join(format!("ytl-fav-{}.png", crate::items::new_id()));
        assert!(!save_favicon(&FakeHttp(HashMap::new()), "https://none.kr", &missing));
    }
}
```

- [ ] **Step 3: src/ui/gfx.rs 작성**

```rust
//! Direct2D + DirectComposition 그리기 장치. 창 하나(콘텐츠 창)에 픽셀 단위 투명도로 GPU 렌더링한다.
//!
//! 좌표는 DIP다. `set_scale`로 DPI 배율을 주면 Direct2D가 실제 픽셀로 바꿔 그린다.

use crate::ui::layout::{Pt, RectF};
use crate::ui::theme::Color;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use windows::Win32::Foundation::{GENERIC_READ, HMODULE, HWND};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED,
    D2D1_GRADIENT_STOP, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_TARGET,
    D2D1_BITMAP_PROPERTIES1, D2D1_BRUSH_PROPERTIES, D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_DRAW_TEXT_OPTIONS_CLIP,
    D2D1_ELLIPSE, D2D1_EXTEND_MODE_CLAMP, D2D1_FACTORY_OPTIONS, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_GAMMA_2_2,
    D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC, D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_ROUNDED_RECT,
    D2D1CreateFactory, ID2D1Bitmap, ID2D1Bitmap1, ID2D1DeviceContext, ID2D1Factory1, ID2D1RenderTarget,
    ID2D1SolidColorBrush,
};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT, DWRITE_TEXT_ALIGNMENT, DWRITE_TEXT_METRICS,
    DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP, DWriteCreateFactory,
    IDWriteFactory5, IDWriteFontCollection1, IDWriteTextFormat,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory2, DXGI_CREATE_FACTORY_FLAGS, DXGI_PRESENT, DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1,
    DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIDevice,
    IDXGIFactory2, IDXGISurface, IDXGISwapChain1,
};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_ContainerFormatPng, GUID_WICPixelFormat32bppPBGRA, IWICBitmap, IWICImagingFactory,
    WICBitmapDitherTypeNone, WICBitmapPaletteTypeMedianCut, WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::core::{HSTRING, Interface, Result, w};
use windows_numerics::{Matrix3x2, Vector2};

static FONT_REGULAR: &[u8] = include_bytes!("../../assets/Pretendard-Regular.otf");
static FONT_SEMIBOLD: &[u8] = include_bytes!("../../assets/Pretendard-SemiBold.otf");
static FONT_BOLD: &[u8] = include_bytes!("../../assets/Pretendard-Bold.otf");
pub const FONT_FAMILY: &str = "Pretendard";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Regular,
    SemiBold,
    Bold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Leading,
    Center,
}

fn color(c: Color) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r: c.r, g: c.g, b: c.b, a: c.a }
}

fn rect(r: RectF) -> D2D_RECT_F {
    D2D_RECT_F { left: r.x, top: r.y, right: r.x + r.w, bottom: r.y + r.h }
}

fn point(p: Pt) -> Vector2 {
    Vector2 { X: p.x, Y: p.y }
}

fn create_d3d_device() -> Result<ID3D11Device> {
    let mut last = Err(windows::core::Error::empty());
    for driver in [D3D_DRIVER_TYPE_WARP, D3D_DRIVER_TYPE_HARDWARE] {
        last = try_device(driver);
        if last.is_ok() {
            break;
        }
    }
    last
}

fn try_device(driver: D3D_DRIVER_TYPE) -> Result<ID3D11Device> {
    let mut device = None;
    // SAFETY: 출력 포인터 하나만 받는 표준 호출
    unsafe {
        D3D11CreateDevice(
            None,
            driver,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )?;
    }
    device.ok_or_else(windows::core::Error::empty)
}

/// exe에 들어 있는 Pretendard를 DirectWrite 메모리 글꼴 모음으로 등록한다.
fn load_fonts(factory: &IDWriteFactory5) -> Result<IDWriteFontCollection1> {
    // SAFETY: 글꼴 데이터는 'static이라 로더가 계속 참조해도 안전하다
    unsafe {
        let loader = factory.CreateInMemoryFontFileLoader()?;
        factory.RegisterFontFileLoader(&loader)?;
        let builder = factory.CreateFontSetBuilder()?;
        for data in [FONT_REGULAR, FONT_SEMIBOLD, FONT_BOLD] {
            let file =
                loader.CreateInMemoryFontFileReference(factory, data.as_ptr().cast(), data.len() as u32, None)?;
            builder.AddFontFile(&file)?;
        }
        let set = builder.CreateFontSet()?;
        factory.CreateFontCollectionFromFontSet(&set)
    }
}

/// 그림이 나가는 곳: 창(DirectComposition 스왑체인) 또는 이미지(PNG 확인용)
enum Surface {
    Window {
        swap: IDXGISwapChain1,
        target_bitmap: Option<ID2D1Bitmap1>,
        // 아래 셋은 화면 합성 연결을 유지하려고 들고 있는다
        _dcomp: IDCompositionDevice,
        _target: IDCompositionTarget,
        _visual: IDCompositionVisual,
    },
    #[cfg_attr(not(test), allow(dead_code))]
    Image { wic: IWICImagingFactory, bitmap: IWICBitmap },
}

pub struct Gfx {
    factory: ID2D1Factory1,
    dwrite: IDWriteFactory5,
    fonts: Option<IDWriteFontCollection1>,
    ctx: ID2D1DeviceContext,
    surface: Surface,
    width: u32,
    height: u32,
    scale: f32,
    /// 아이콘 이미지 캐시 (불러오기 실패도 기억해 매 프레임 다시 읽지 않는다)
    images: RefCell<HashMap<PathBuf, Option<ID2D1Bitmap>>>,
}

fn text_resources() -> Result<(IDWriteFactory5, Option<IDWriteFontCollection1>)> {
    let dwrite: IDWriteFactory5 = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
    let fonts = load_fonts(&dwrite).ok();
    Ok((dwrite, fonts))
}

impl Gfx {
    /// `hwnd`는 `WS_EX_NOREDIRECTIONBITMAP`으로 만든 창이어야 한다.
    pub fn new(hwnd: HWND, width: u32, height: u32, scale: f32) -> Result<Gfx> {
        let (width, height) = (width.max(1), height.max(1));
        // SAFETY: 아래는 모두 COM 객체 생성 순서대로의 표준 호출
        unsafe {
            let d3d = create_d3d_device()?;
            let dxgi_device: IDXGIDevice = d3d.cast()?;
            let dxgi_factory: IDXGIFactory2 = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))?;
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: width,
                Height: height,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
                ..Default::default()
            };
            let swap = dxgi_factory.CreateSwapChainForComposition(&dxgi_device, &desc, None)?;
            let options = D2D1_FACTORY_OPTIONS::default();
            let factory: ID2D1Factory1 = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, Some(&options))?;
            let device = factory.CreateDevice(&dxgi_device)?;
            let ctx = device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
            let dcomp: IDCompositionDevice = DCompositionCreateDevice(&dxgi_device)?;
            let target = dcomp.CreateTargetForHwnd(hwnd, true)?;
            let visual = dcomp.CreateVisual()?;
            visual.SetContent(&swap)?;
            target.SetRoot(&visual)?;
            dcomp.Commit()?;
            let (dwrite, fonts) = text_resources()?;
            let surface =
                Surface::Window { swap, target_bitmap: None, _dcomp: dcomp, _target: target, _visual: visual };
            let mut gfx =
                Gfx { factory, dwrite, fonts, ctx, surface, width, height, scale, images: RefCell::default() };
            gfx.bind_target()?;
            Ok(gfx)
        }
    }

    fn bind_target(&mut self) -> Result<()> {
        let dpi = 96.0 * self.scale;
        let Surface::Window { swap, target_bitmap, .. } = &mut self.surface else { return Ok(()) };
        // SAFETY: 스왑체인 0번 버퍼를 Direct2D 대상 비트맵으로 감싼다
        unsafe {
            let surface: IDXGISurface = swap.GetBuffer(0)?;
            let props = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: dpi,
                dpiY: dpi,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                ..Default::default()
            };
            let bitmap = self.ctx.CreateBitmapFromDxgiSurface(&surface, Some(&props))?;
            self.ctx.SetTarget(&bitmap);
            self.ctx.SetDpi(dpi, dpi);
            *target_bitmap = Some(bitmap);
        }
        Ok(())
    }

    /// 창 크기(실제 픽셀)나 DPI가 바뀌면 호출한다.
    pub fn resize(&mut self, width: u32, height: u32, scale: f32) -> Result<()> {
        let (width, height) = (width.max(1), height.max(1));
        if (width, height, scale) == (self.width, self.height, self.scale) {
            return Ok(());
        }
        let Surface::Window { swap, target_bitmap, .. } = &mut self.surface else { return Ok(()) };
        // SAFETY: 버퍼를 다시 만들기 전에 대상 비트맵을 놓는다
        unsafe {
            self.ctx.SetTarget(None);
            *target_bitmap = None;
            swap.ResizeBuffers(2, width, height, DXGI_FORMAT_UNKNOWN, DXGI_SWAP_CHAIN_FLAG(0))?;
        }
        self.width = width;
        self.height = height;
        self.scale = scale;
        self.bind_target()
    }

    pub fn begin(&self) {
        // SAFETY: 그리기 시작 + 완전 투명으로 지움
        unsafe {
            self.ctx.BeginDraw();
            self.ctx.SetTransform(&Matrix3x2::identity());
            self.ctx.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
            self.ctx.Clear(Some(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }));
        }
    }

    pub fn end(&self) -> Result<()> {
        // SAFETY: 그리기 끝내고 화면에 보냄
        unsafe {
            self.ctx.EndDraw(None, None)?;
            match &self.surface {
                Surface::Window { swap, .. } => swap.Present(1, DXGI_PRESENT(0)).ok(),
                Surface::Image { .. } => Ok(()),
            }
        }
    }

    fn brush(&self, c: Color) -> Option<ID2D1SolidColorBrush> {
        unsafe { self.ctx.CreateSolidColorBrush(&color(c), None) }.ok()
    }

    pub fn fill_round_rect(&self, r: RectF, radius: f32, c: Color) {
        if let Some(b) = self.brush(c) {
            let rr = D2D1_ROUNDED_RECT { rect: rect(r), radiusX: radius, radiusY: radius };
            unsafe { self.ctx.FillRoundedRectangle(&rr, &b) };
        }
    }

    pub fn stroke_round_rect(&self, r: RectF, radius: f32, c: Color, width: f32) {
        if let Some(b) = self.brush(c) {
            let rr = D2D1_ROUNDED_RECT { rect: rect(r), radiusX: radius, radiusY: radius };
            unsafe { self.ctx.DrawRoundedRectangle(&rr, &b, width, None) };
        }
    }

    /// 위→아래 그라데이션으로 채운 둥근 사각형
    pub fn fill_round_rect_gradient(&self, r: RectF, radius: f32, top: Color, bottom: Color) {
        let stops = [
            D2D1_GRADIENT_STOP { position: 0.0, color: color(top) },
            D2D1_GRADIENT_STOP { position: 1.0, color: color(bottom) },
        ];
        // SAFETY: 지역 배열로 그라데이션 정지점을 만든다
        unsafe {
            let Ok(rt) = self.ctx.cast::<ID2D1RenderTarget>() else { return };
            let Ok(collection) = rt.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP) else {
                return;
            };
            let props = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                startPoint: Vector2 { X: r.x, Y: r.y },
                endPoint: Vector2 { X: r.x, Y: r.y + r.h },
            };
            let brush_props = D2D1_BRUSH_PROPERTIES { opacity: 1.0, transform: Matrix3x2::identity() };
            if let Ok(brush) = self.ctx.CreateLinearGradientBrush(&props, Some(&brush_props), &collection) {
                let rr = D2D1_ROUNDED_RECT { rect: rect(r), radiusX: radius, radiusY: radius };
                self.ctx.FillRoundedRectangle(&rr, &brush);
            }
        }
    }

    /// 둥근 사각형 아래로 옅게 번지는 그림자 (겹친 사각형으로 근사)
    pub fn soft_shadow(&self, r: RectF, radius: f32, strength: f32) {
        const STEPS: i32 = 6;
        for i in (1..=STEPS).rev() {
            let spread = i as f32;
            let alpha = strength * (STEPS - i + 1) as f32 / STEPS as f32 * 0.05;
            let shadow = RectF::new(r.x - spread, r.y - spread + 2.0, r.w + 2.0 * spread, r.h + 2.0 * spread);
            self.fill_round_rect(shadow, radius + spread, Color::hex(0x000000, alpha));
        }
    }

    pub fn fill_circle(&self, center: Pt, radius: f32, c: Color) {
        if let Some(b) = self.brush(c) {
            let e = D2D1_ELLIPSE { point: point(center), radiusX: radius, radiusY: radius };
            unsafe { self.ctx.FillEllipse(&e, &b) };
        }
    }

    pub fn stroke_circle(&self, center: Pt, radius: f32, c: Color, width: f32) {
        if let Some(b) = self.brush(c) {
            let e = D2D1_ELLIPSE { point: point(center), radiusX: radius, radiusY: radius };
            unsafe { self.ctx.DrawEllipse(&e, &b, width, None) };
        }
    }

    pub fn line(&self, a: Pt, b: Pt, c: Color, width: f32) {
        if let Some(brush) = self.brush(c) {
            unsafe { self.ctx.DrawLine(point(a), point(b), &brush, width, None) };
        }
    }

    fn load_image(&self, path: &Path) -> Option<ID2D1Bitmap> {
        // SAFETY: WIC로 파일을 읽어 미리 곱한 BGRA로 바꾼 뒤 Direct2D 비트맵으로 올린다
        unsafe {
            let wic: IWICImagingFactory =
                CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok()?;
            // 아이콘 캐시는 늘 PNG: 형식 자동 판별(모든 코덱 불러오기)을 피한다
            let stream = wic.CreateStream().ok()?;
            stream.InitializeFromFilename(&HSTRING::from(path.as_os_str()), GENERIC_READ.0).ok()?;
            let decoder = wic.CreateDecoder(&GUID_ContainerFormatPng, std::ptr::null()).ok()?;
            decoder.Initialize(&stream, WICDecodeMetadataCacheOnDemand).ok()?;
            let frame = decoder.GetFrame(0).ok()?;
            let converter = wic.CreateFormatConverter().ok()?;
            converter
                .Initialize(
                    &frame,
                    &GUID_WICPixelFormat32bppPBGRA,
                    WICBitmapDitherTypeNone,
                    None,
                    0.0,
                    WICBitmapPaletteTypeMedianCut,
                )
                .ok()?;
            self.ctx.CreateBitmapFromWicBitmap(&converter, None).ok().map(|b| b.into())
        }
    }

    /// 아이콘 파일을 사각형에 맞춰 그린다. 파일이 없거나 읽지 못하면 false.
    pub fn image(&self, path: &Path, r: RectF, opacity: f32) -> bool {
        let mut cache = self.images.borrow_mut();
        let bitmap = cache.entry(path.to_path_buf()).or_insert_with(|| self.load_image(path));
        let Some(bitmap) = bitmap else { return false };
        // SAFETY: 캐시된 비트맵을 고품질 보간으로 그린다
        unsafe {
            self.ctx.DrawBitmap(
                &*bitmap,
                Some(&rect(r)),
                opacity,
                D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC,
                None,
                None,
            )
        };
        true
    }

    /// 아이콘 파일이 바뀌었을 수 있을 때 (항목 목록이 바뀔 때) 캐시를 비운다.
    pub fn clear_images(&self) {
        self.images.borrow_mut().clear();
    }

    /// 채운 다각형 (재생 삼각형 등)
    pub fn fill_polygon(&self, points: &[Pt], c: Color) {
        let (Some(first), Some(brush)) = (points.first(), self.brush(c)) else { return };
        // SAFETY: 경로 도형을 만들어 바로 채운다
        unsafe {
            let Ok(geometry) = self.factory.CreatePathGeometry() else { return };
            let Ok(sink) = geometry.Open() else { return };
            sink.BeginFigure(point(*first), D2D1_FIGURE_BEGIN_FILLED);
            for p in &points[1..] {
                sink.AddLine(point(*p));
            }
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            if sink.Close().is_ok() {
                self.ctx.FillGeometry(&geometry, &brush, None);
            }
        }
    }

    fn text_format(&self, size: f32, weight: Weight, align: Align) -> Option<IDWriteTextFormat> {
        let weight = DWRITE_FONT_WEIGHT(match weight {
            Weight::Regular => 400,
            Weight::SemiBold => 600,
            Weight::Bold => 700,
        });
        // SAFETY: 글꼴 모음이 없으면 시스템 글꼴로 대체된다
        unsafe {
            let format = self
                .dwrite
                .CreateTextFormat(
                    &HSTRING::from(FONT_FAMILY),
                    self.fonts.as_ref().and_then(|f| f.cast().ok()).as_ref(),
                    weight,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    size,
                    w!("ko-kr"),
                )
                .ok()?;
            let _ = format.SetTextAlignment(match align {
                Align::Leading => DWRITE_TEXT_ALIGNMENT(0),
                Align::Center => DWRITE_TEXT_ALIGNMENT(2),
            });
            let _ = format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT(2)); // 세로 가운데
            let _ = format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP);
            let trimming =
                DWRITE_TRIMMING { granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER, delimiter: 0, delimiterCount: 0 };
            let _ = format.SetTrimming(&trimming, None);
            Some(format)
        }
    }

    /// 사각형 안에 한 줄 텍스트 (세로 가운데, 넘치면 잘림)
    pub fn text(&self, text: &str, r: RectF, size: f32, weight: Weight, align: Align, c: Color) {
        let (Some(format), Some(brush)) = (self.text_format(size, weight, align), self.brush(c)) else { return };
        let wide: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            self.ctx.DrawText(
                &wide,
                &format,
                &rect(r),
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            )
        };
    }

    /// 한 줄 텍스트의 너비(DIP)
    pub fn measure(&self, text: &str, size: f32, weight: Weight) -> f32 {
        let Some(format) = self.text_format(size, weight, Align::Leading) else { return 0.0 };
        let wide: Vec<u16> = text.encode_utf16().collect();
        // SAFETY: 측정용 레이아웃을 만들어 크기만 읽는다
        unsafe {
            let Ok(layout) = self.dwrite.CreateTextLayout(&wide, &format, 10_000.0, 1_000.0) else { return 0.0 };
            let mut metrics = DWRITE_TEXT_METRICS::default();
            match layout.GetMetrics(&mut metrics) {
                Ok(()) => metrics.widthIncludingTrailingWhitespace,
                Err(_) => 0.0,
            }
        }
    }
}

/// 테스트에서 창 없이 장면을 PNG로 그려 보는 기능
#[cfg(test)]
mod offscreen {
    use super::*;
    use windows::Win32::Foundation::GENERIC_WRITE;
    use windows::Win32::Graphics::Direct2D::D2D1_RENDER_TARGET_PROPERTIES;
    use windows::Win32::Graphics::Imaging::{
        CLSID_WICImagingFactory, GUID_ContainerFormatPng, GUID_WICPixelFormat32bppPBGRA, WICBitmapCacheOnLoad,
        WICBitmapEncoderNoCache,
    };
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};

    impl Gfx {
        /// 창 없이 이미지에 그린다 (화면 확인용 PNG 저장). COM이 초기화돼 있어야 한다.
        pub fn offscreen(width: u32, height: u32, scale: f32) -> Result<Gfx> {
            // SAFETY: WIC 비트맵을 만들고 그 위에 Direct2D 대상을 만든다
            unsafe {
                let wic: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
                let bitmap = wic.CreateBitmap(width, height, &GUID_WICPixelFormat32bppPBGRA, WICBitmapCacheOnLoad)?;
                let options = D2D1_FACTORY_OPTIONS::default();
                let factory: ID2D1Factory1 = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, Some(&options))?;
                let props = D2D1_RENDER_TARGET_PROPERTIES {
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                    },
                    dpiX: 96.0 * scale,
                    dpiY: 96.0 * scale,
                    ..Default::default()
                };
                let rt = factory.CreateWicBitmapRenderTarget(&bitmap, &props)?;
                let ctx: ID2D1DeviceContext = rt.cast()?;
                let (dwrite, fonts) = text_resources()?;
                let surface = Surface::Image { wic, bitmap };
                Ok(Gfx { factory, dwrite, fonts, ctx, surface, width, height, scale, images: RefCell::default() })
            }
        }

        /// 이미지 대상이면 PNG로 저장한다.
        pub fn save_png(&self, path: &std::path::Path) -> Result<()> {
            let Surface::Image { wic, bitmap } = &self.surface else { return Ok(()) };
            // SAFETY: WIC 인코더로 파일에 쓴다
            unsafe {
                let stream = wic.CreateStream()?;
                stream.InitializeFromFilename(&HSTRING::from(path.as_os_str()), GENERIC_WRITE.0)?;
                let encoder = wic.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())?;
                encoder.Initialize(&stream, WICBitmapEncoderNoCache)?;
                let mut frame = None;
                encoder.CreateNewFrame(&mut frame, std::ptr::null_mut())?;
                let frame = frame.ok_or_else(windows::core::Error::empty)?;
                frame.Initialize(None)?;
                frame.WriteSource(bitmap, std::ptr::null())?;
                frame.Commit()?;
                encoder.Commit()
            }
        }
    }
}
```

- [ ] **Step 4: src/ui/scene.rs 작성**

```rust
//! 패널 한 프레임 그리기: 유리 덧칠, 헤더, 아이콘, ＋, 팔레트 버튼, 말풍선, 테마 선택기, 드래그 중인 아이콘.

use crate::config::DockEdge;
use crate::config::Theme;
use crate::ui::dock_layout::{self, DockLayout, Magnified, Slot};
use crate::ui::gfx::{Align, Gfx, Weight};
use crate::ui::layout::{PANEL_HOVER_SCALE, PICKER_RADIUS, PanelLayout, Pt, RectF, SWATCH, fan_points};
use crate::ui::theme::{ACCENT, Color, FALLBACK_ALPHA, PICKER_ORDER, Palette, TOOLTIP_BG, TOOLTIP_TEXT};
use std::path::PathBuf;

pub const WINDOW_RADIUS: f32 = 8.0;
pub const ICON_RADIUS: f32 = 14.0;
pub const TITLE: &str = "YT Launcher";
pub const BYLINE: &str = "by_ssenu";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TileKind {
    #[default]
    Youtube,
    App,
    Web,
}

/// 화면에 그릴 항목 하나
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TileView {
    pub id: String,
    /// 말풍선에 보이는 이름
    pub label: String,
    /// 유튜브 아이콘 아래 두 글자 배지 (다른 종류는 아바타 글자)
    pub badge: String,
    pub kind: TileKind,
    /// 캐시된 아이콘 PNG (앱 아이콘, 사이트 파비콘). 없으면 글자 아바타
    pub icon: Option<PathBuf>,
    /// 실행 중인 앱이면 아이콘 아래 점
    pub running: bool,
    /// 실행 대상을 찾지 못한 앱 (흐리게)
    pub missing: bool,
}

pub struct DragView {
    pub from: usize,
    pub to: usize,
    pub pos: Pt,
}

pub struct SceneState<'a> {
    pub layout: &'a PanelLayout,
    pub palette: Palette,
    /// 아크릴 배경이 켜져 있는지 (꺼져 있으면 덧칠을 거의 불투명하게)
    pub acrylic: bool,
    pub tiles: &'a [TileView],
    /// 칸별 마우스 오버 진행도 0..1 (＋ 칸 포함, 길이 = 칸 수)
    pub hover: &'a [f32],
    pub pressed: Option<usize>,
    pub menu_hover: bool,
    pub palette_hover: bool,
    pub drag: Option<DragView>,
    /// 말풍선을 띄울 칸 번호
    pub tooltip: Option<usize>,
    /// 테마 선택기 펼침 정도 0..1 (0이면 닫힘)
    pub picker: f32,
    pub picker_hover: Option<usize>,
    pub theme: Theme,
}

/// 문자열에서 앞 두 글자 (배지용)
pub fn badge_of(text: &str) -> String {
    text.trim().chars().filter(|c| !c.is_whitespace()).take(2).collect()
}

fn avatar_color(seed: &str) -> Color {
    const COLORS: [u32; 8] = [0x4F6A9E, 0x3F7F94, 0x4E8A78, 0x6E8F57, 0xC07A45, 0xB5544C, 0xB0577A, 0x7E5A9B];
    let hash = seed.bytes().fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
    Color::hex(COLORS[(hash % COLORS.len() as u32) as usize], 1.0)
}

fn draw_youtube_icon(g: &Gfx, r: RectF, badge: &str) {
    g.fill_round_rect_gradient(r, ICON_RADIUS * r.w / 56.0, Color::hex(0xFF4436, 1.0), Color::hex(0xC4161C, 1.0));
    let c = r.center();
    let s = r.w / 56.0;
    let tri = [
        Pt { x: c.x - 7.0 * s, y: c.y - 11.0 * s },
        Pt { x: c.x - 7.0 * s, y: c.y + 5.0 * s },
        Pt { x: c.x + 9.0 * s, y: c.y - 3.0 * s },
    ];
    g.fill_polygon(&tri, Color::hex(0xFFFFFF, 1.0));
    if !badge.is_empty() {
        let size = 10.0 * s;
        let w = g.measure(badge, size, Weight::SemiBold) + 10.0 * s;
        let pill = RectF::new(c.x - w / 2.0, r.bottom() - 18.0 * s, w, 14.0 * s);
        g.fill_round_rect(pill, 7.0 * s, Color::hex(0x000000, 0.32));
        g.text(badge, pill, size, Weight::SemiBold, Align::Center, Color::hex(0xFFFFFF, 0.95));
    }
}

fn draw_letter_icon(g: &Gfx, r: RectF, seed: &str, letter: &str) {
    let base = avatar_color(seed);
    g.fill_round_rect_gradient(r, ICON_RADIUS * r.w / 56.0, base.shade(0.12), base.shade(-0.08));
    g.text(letter, r, 24.0 * r.w / 56.0, Weight::Bold, Align::Center, Color::hex(0xFFFFFF, 0.95));
}

fn draw_tile(g: &Gfx, tile: &TileView, r: RectF, lift: f32) {
    let radius = ICON_RADIUS * r.w / 56.0;
    let opacity = if tile.missing { 0.4 } else { 1.0 };
    // 앱 아이콘은 자체 모양(투명 배경)이 있으므로 그림자 없이 그대로 그린다
    if tile.kind == TileKind::App
        && let Some(path) = &tile.icon
        && g.image(path, r, opacity)
    {
        return;
    }
    g.soft_shadow(r, radius, 1.0 + lift);
    match (tile.kind, &tile.icon) {
        (TileKind::Youtube, _) => draw_youtube_icon(g, r, &tile.badge),
        // 사이트 아이콘은 작아서 흰 둥근 판 위에 올린다
        (TileKind::Web, Some(path)) => {
            g.fill_round_rect(r, radius, Color::hex(0xFFFFFF, 0.96));
            if !g.image(path, r.scaled(0.58), 1.0) {
                draw_letter_icon(g, r, &tile.id, &tile.badge);
            }
        }
        _ => draw_letter_icon(g, r, &tile.id, &tile.badge),
    }
    // 실행 대상을 찾지 못한 항목은 흐리게
    if tile.missing {
        g.fill_round_rect(r, radius, Color::hex(0x808080, 0.55));
    }
}

/// 실행 중 표시 점. `toward`는 점을 놓을 쪽(아이콘 바깥) 방향 단위 벡터.
fn draw_running_dot(g: &Gfx, r: RectF, toward: (f32, f32), c: Color) {
    let center = r.center();
    let gap = 5.0;
    let p = Pt { x: center.x + toward.0 * (r.w / 2.0 + gap), y: center.y + toward.1 * (r.h / 2.0 + gap) };
    g.fill_circle(p, 2.2, c);
}

fn dock_dot_direction(edge: DockEdge) -> (f32, f32) {
    match edge {
        DockEdge::Bottom => (0.0, 1.0),
        DockEdge::Top => (0.0, -1.0),
        DockEdge::Left => (-1.0, 0.0),
        DockEdge::Right => (1.0, 0.0),
    }
}

fn draw_add(g: &Gfx, r: RectF, p: &Palette) {
    g.fill_round_rect(r, ICON_RADIUS * r.w / 56.0, p.add_tile);
    let c = r.center();
    let arm = r.w * 0.16;
    let plus = Color::hex(0xFFFFFF, 0.92);
    g.line(Pt { x: c.x - arm, y: c.y }, Pt { x: c.x + arm, y: c.y }, plus, 2.6);
    g.line(Pt { x: c.x, y: c.y - arm }, Pt { x: c.x, y: c.y + arm }, plus, 2.6);
}

/// 원 안에 선으로 그린 팔레트 (엄지 구멍 + 물감 자리 3개)
fn draw_palette_button(g: &Gfx, r: RectF, p: &Palette, hover: bool) {
    let c = r.center();
    let radius = r.w / 2.0;
    g.soft_shadow(RectF::new(c.x - radius, c.y - radius, radius * 2.0, radius * 2.0), radius, 0.8);
    let face = p.tint.with_alpha(1.0).shade(if p.dark_backdrop { 0.10 } else { -0.03 });
    g.fill_circle(c, radius, if hover { face.shade(0.06) } else { face });
    g.stroke_circle(c, radius - 0.5, p.divider, 1.0);
    g.stroke_circle(c, radius * 0.45, p.text, 1.8);
    g.fill_circle(Pt { x: c.x + radius * 0.22, y: c.y + radius * 0.22 }, radius * 0.13, face);
    for (dx, dy) in [(-0.2f32, -0.06f32), (-0.04, -0.24), (0.16, -0.16)] {
        g.fill_circle(Pt { x: c.x + dx * radius, y: c.y + dy * radius }, radius * 0.07, p.text);
    }
}

fn draw_header(g: &Gfx, s: &SceneState) {
    let h = s.layout.header;
    let p = &s.palette;
    let title_w = g.measure(TITLE, 15.0, Weight::Bold);
    g.text(TITLE, RectF::new(h.x + 4.0, h.y, title_w + 2.0, h.h), 15.0, Weight::Bold, Align::Leading, p.text);
    let by = RectF::new(h.x + 4.0 + title_w + 8.0, h.y, 120.0, h.h);
    g.text(BYLINE, by, 11.0, Weight::Regular, Align::Leading, p.subtext);
    let m = s.layout.menu_button;
    if s.menu_hover {
        g.fill_round_rect(m, 6.0, p.text.with_alpha(0.08));
    }
    let c = m.center();
    for dx in [-7.0, 0.0, 7.0] {
        g.fill_circle(Pt { x: c.x + dx, y: c.y }, 2.4, p.subtext);
    }
    g.line(Pt { x: h.x, y: h.bottom() }, Pt { x: h.right(), y: h.bottom() }, p.divider, 1.0);
}

/// 아이콘 위에 말풍선을 띄운다. 위쪽이 `top_limit`보다 올라가면(헤더를 가리면) 아래에 띄운다.
fn draw_tooltip(g: &Gfx, text: &str, anchor: RectF, bounds: (f32, f32), top_limit: f32) {
    let w = g.measure(text, 12.0, Weight::SemiBold) + 20.0;
    let h = 26.0;
    let mut x = anchor.center().x - w / 2.0;
    x = x.clamp(6.0, (bounds.0 - w - 6.0).max(6.0));
    let mut y = anchor.y - h - 8.0;
    if y < top_limit {
        y = anchor.bottom() + 8.0;
    }
    let r = RectF::new(x, y, w, h);
    g.fill_round_rect(r, 8.0, TOOLTIP_BG);
    g.text(text, r, 12.0, Weight::SemiBold, Align::Center, TOOLTIP_TEXT);
}

/// 테마 색 원들 (패널·독 공용). `progress`는 펼침 정도 0..1.
fn draw_swatches(
    g: &Gfx,
    centers: &[Pt],
    progress: f32,
    palette: &Palette,
    current: Theme,
    hover: Option<usize>,
    bounds: (f32, f32),
) {
    let radius = SWATCH / 2.0 * (0.6 + 0.4 * progress);
    for (i, (theme, c)) in PICKER_ORDER.iter().zip(centers.iter()).enumerate() {
        let swatch = crate::ui::theme::palette(*theme);
        g.fill_circle(*c, radius + 4.0, palette.text.with_alpha(0.25));
        g.fill_circle(*c, radius, swatch.swatch);
        if *theme == current {
            g.stroke_circle(*c, radius, ACCENT, 2.5);
        } else {
            g.stroke_circle(*c, radius, palette.divider.with_alpha(0.6), 1.2);
        }
        if hover == Some(i) && progress >= 1.0 {
            let anchor = RectF::new(c.x - radius, c.y - radius, radius * 2.0, radius * 2.0);
            draw_tooltip(g, crate::ui::theme::label(*theme), anchor, bounds, 6.0);
        }
    }
}

fn draw_picker(g: &Gfx, s: &SceneState) {
    let centers = s.layout.swatch_centers(PICKER_ORDER.len(), s.picker);
    let bounds = (s.layout.width, s.layout.height);
    draw_swatches(g, &centers, s.picker, &s.palette, s.theme, s.picker_hover, bounds);
}

/// 드래그 중이면 놓일 자리 기준으로 항목 순서를 바꿔 보여준다.
fn display_order(s: &SceneState) -> Vec<usize> {
    let order: Vec<usize> = (0..s.tiles.len()).collect();
    match &s.drag {
        Some(d) => crate::ui::layout::reorder(&order, d.from, d.to),
        None => order,
    }
}

pub fn draw(g: &Gfx, s: &SceneState) {
    let (w, h) = (s.layout.width, s.layout.height);
    let p = &s.palette;
    let tint = if s.acrylic { p.tint } else { p.tint.with_alpha(FALLBACK_ALPHA) };
    let bounds = RectF::new(0.5, 0.5, w - 1.0, h - 1.0);
    g.fill_round_rect(bounds, WINDOW_RADIUS, tint);
    g.stroke_round_rect(bounds, WINDOW_RADIUS, p.divider, 1.0);
    draw_header(g, s);

    let order = display_order(s);
    for (slot, &tile_index) in order.iter().enumerate() {
        if s.drag.as_ref().is_some_and(|d| d.from == tile_index) {
            continue;
        }
        let level = s.hover.get(slot).copied().unwrap_or(0.0);
        let press = if s.pressed == Some(slot) && s.drag.is_none() { 0.04 } else { 0.0 };
        let r = s.layout.cells[slot].scaled(1.0 + (PANEL_HOVER_SCALE - 1.0) * level - press);
        draw_tile(g, &s.tiles[tile_index], r, level);
        if s.tiles[tile_index].running {
            draw_running_dot(g, s.layout.cells[slot], (0.0, 1.0), p.text.with_alpha(0.75));
        }
    }
    let add_slot = s.tiles.len();
    let add_level = s.hover.get(add_slot).copied().unwrap_or(0.0);
    draw_add(g, s.layout.add_cell().scaled(1.0 + (PANEL_HOVER_SCALE - 1.0) * add_level), p);
    draw_palette_button(g, s.layout.palette_button, p, s.palette_hover);

    if s.picker > 0.0 {
        draw_picker(g, s);
    }
    if let Some(d) = &s.drag {
        let cell = s.layout.cells[0];
        let r = RectF::new(d.pos.x - cell.w / 2.0, d.pos.y - cell.h / 2.0, cell.w, cell.h).scaled(1.1);
        draw_tile(g, &s.tiles[d.from], r, 2.0);
    } else if let Some(i) = s.tooltip {
        let text = if i < s.tiles.len() { s.tiles[i].label.as_str() } else { "항목 추가" };
        draw_tooltip(g, text, s.layout.cells[i], (w, h), s.layout.header.bottom());
    }
}

/// 독 한 프레임에 필요한 상태
pub struct DockScene<'a> {
    pub layout: &'a DockLayout,
    pub mag: &'a Magnified,
    /// 판 중심 (콘텐츠 창 기준 DIP)
    pub center: (f32, f32),
    /// 콘텐츠 창 크기 (DIP)
    pub size: (f32, f32),
    pub palette: Palette,
    pub acrylic: bool,
    pub tiles: &'a [TileView],
    pub pressed: Option<usize>,
    /// 말풍선을 띄울 칸 번호 (항목·＋·팔레트·메뉴 순서)
    pub tooltip: Option<usize>,
    pub menu_hover: bool,
    pub palette_hover: bool,
    /// 드래그 중인 항목 (번호, 마우스 위치)
    pub drag: Option<(usize, Pt)>,
    pub picker: f32,
    pub picker_hover: Option<usize>,
    pub theme: Theme,
}

/// 독 말풍선: 판 반대쪽(화면 안쪽)에 띄운다.
fn draw_dock_tooltip(g: &Gfx, text: &str, icon: RectF, edge: DockEdge, bounds: (f32, f32)) {
    let w = g.measure(text, 12.0, Weight::SemiBold) + 20.0;
    let h = 26.0;
    let c = icon.center();
    let (x, y) = match edge {
        DockEdge::Bottom => (c.x - w / 2.0, icon.y - h - 8.0),
        DockEdge::Top => (c.x - w / 2.0, icon.bottom() + 8.0),
        DockEdge::Left => (icon.right() + 8.0, c.y - h / 2.0),
        DockEdge::Right => (icon.x - w - 8.0, c.y - h / 2.0),
    };
    let x = x.clamp(4.0, (bounds.0 - w - 4.0).max(4.0));
    let y = y.clamp(4.0, (bounds.1 - h - 4.0).max(4.0));
    let r = RectF::new(x, y, w, h);
    g.fill_round_rect(r, 8.0, TOOLTIP_BG);
    g.text(text, r, 12.0, Weight::SemiBold, Align::Center, TOOLTIP_TEXT);
}

fn draw_dots(g: &Gfx, r: RectF, p: &Palette, hover: bool) {
    let c = r.center();
    g.fill_circle(c, r.w * 0.42, p.text.with_alpha(if hover { 0.16 } else { 0.08 }));
    let step = r.w * 0.15;
    for dx in [-step, 0.0, step] {
        g.fill_circle(Pt { x: c.x + dx, y: c.y }, r.w * 0.05, p.text);
    }
}

/// 테마 선택 원 중심 (독): 팔레트 칸에서 화면 안쪽으로 부채꼴
pub fn dock_swatch_centers(layout: &DockLayout, mag: &Magnified, center: (f32, f32), progress: f32) -> Vec<Pt> {
    let anchor = layout.slot_rect(mag, center, layout.tiles + 1).center();
    fan_points(anchor, layout.picker_angles(), PICKER_ORDER.len(), PICKER_RADIUS * progress)
}

pub fn draw_dock(g: &Gfx, s: &DockScene) {
    let p = &s.palette;
    let tint = if s.acrylic { p.tint } else { p.tint.with_alpha(FALLBACK_ALPHA) };
    let bar = s.layout.bar_rect(s.mag, s.center);
    let bar = RectF::new(bar.x + 0.5, bar.y + 0.5, bar.w - 1.0, bar.h - 1.0);
    g.fill_round_rect(bar, WINDOW_RADIUS, tint);
    g.stroke_round_rect(bar, WINDOW_RADIUS, p.divider, 1.0);

    if !s.mag.separator.is_nan() {
        let along = s.mag.separator;
        let inset = dock_layout::PAD + 4.0;
        let (a, b) = if dock_layout::is_vertical(s.layout.edge) {
            let y = s.center.1 + along;
            (Pt { x: bar.x + inset, y }, Pt { x: bar.right() - inset, y })
        } else {
            let x = s.center.0 + along;
            (Pt { x, y: bar.y + inset }, Pt { x, y: bar.bottom() - inset })
        };
        g.line(a, b, p.divider, 1.0);
    }

    for (i, slot) in s.layout.slots().iter().enumerate() {
        let mut r = s.layout.slot_rect(s.mag, s.center, i);
        match slot {
            Slot::Tile(t) => {
                if s.drag.is_some_and(|(from, _)| from == *t) {
                    continue;
                }
                if s.pressed == Some(*t) && s.drag.is_none() {
                    r = r.scaled(0.94);
                }
                draw_tile(g, &s.tiles[*t], r, s.mag.scales[i] - 1.0);
                if s.tiles[*t].running {
                    draw_running_dot(g, r, dock_dot_direction(s.layout.edge), p.text.with_alpha(0.8));
                }
            }
            Slot::Add => draw_add(g, r, p),
            Slot::Palette => draw_palette_button(g, r.scaled(0.84), p, s.palette_hover),
            Slot::Menu => draw_dots(g, r, p, s.menu_hover),
        }
    }

    if s.picker > 0.0 {
        let centers = dock_swatch_centers(s.layout, s.mag, s.center, s.picker);
        draw_swatches(g, &centers, s.picker, p, s.theme, s.picker_hover, s.size);
    }
    if let Some((from, pos)) = s.drag {
        let size = dock_layout::ICON * 1.1;
        let r = RectF::new(pos.x - size / 2.0, pos.y - size / 2.0, size, size);
        draw_tile(g, &s.tiles[from], r, 2.0);
    } else if let Some(i) = s.tooltip {
        let text = match s.layout.slots()[i] {
            Slot::Tile(t) => s.tiles[t].label.as_str(),
            Slot::Add => "항목 추가",
            Slot::Palette => "테마",
            Slot::Menu => "메뉴",
        };
        let r = s.layout.slot_rect(s.mag, s.center, i);
        draw_dock_tooltip(g, text, r, s.layout.edge, s.size);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_takes_two_visible_chars() {
        assert_eq!(badge_of("  로파이 음악"), "로파");
        assert_eq!(badge_of("a b c"), "ab");
        assert_eq!(badge_of("뉴"), "뉴");
        assert_eq!(badge_of("   "), "");
    }

    #[test]
    fn avatar_color_is_stable() {
        assert_eq!(avatar_color("abc"), avatar_color("abc"));
    }

    /// 실제 Direct2D로 장면을 그려 `target/snapshots/`에 PNG로 남긴다 (눈으로 확인하는 용도).
    #[test]
    fn renders_panel_snapshots_for_every_theme() {
        use crate::ui::gfx::Gfx;
        use crate::ui::layout::PanelLayout;
        let _ = unsafe {
            windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)
        };
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("snapshots");
        std::fs::create_dir_all(&dir).unwrap();
        let tiles = vec![
            TileView {
                id: "1".into(),
                label: "로파이 음악".into(),
                badge: "로파".into(),
                kind: TileKind::Youtube,
                ..Default::default()
            },
            TileView {
                id: "2".into(),
                label: "아침 뉴스".into(),
                badge: "뉴스".into(),
                kind: TileKind::Youtube,
                ..Default::default()
            },
            TileView {
                id: "3".into(),
                label: "github.com".into(),
                badge: "G".into(),
                kind: TileKind::Web,
                ..Default::default()
            },
            TileView {
                id: "4".into(),
                label: "계산기".into(),
                badge: "계".into(),
                kind: TileKind::App,
                icon: Some(std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icon.png"))),
                running: true,
                missing: false,
            },
        ];
        let layout = PanelLayout::compute(360.0, 480.0, tiles.len());
        for (i, theme) in PICKER_ORDER.iter().enumerate() {
            let gfx = Gfx::offscreen(360 * 2, 480 * 2, 2.0).unwrap();
            let mut hover = vec![0.0; tiles.len() + 1];
            hover[1] = 1.0;
            let state = SceneState {
                layout: &layout,
                palette: crate::ui::theme::palette(*theme),
                acrylic: false,
                tiles: &tiles,
                hover: &hover,
                pressed: None,
                menu_hover: false,
                palette_hover: false,
                drag: None,
                tooltip: Some(1),
                picker: if i == 0 { 1.0 } else { 0.0 },
                picker_hover: if i == 0 { Some(2) } else { None },
                theme: *theme,
            };
            gfx.begin();
            draw(&gfx, &state);
            gfx.end().unwrap();
            let path = dir.join(format!("panel-{}.png", crate::ui::theme::label(*theme)));
            gfx.save_png(&path).unwrap();
            assert!(std::fs::metadata(&path).unwrap().len() > 1000);
        }
    }

    #[test]
    fn renders_dock_snapshots_for_bottom_and_left_edges() {
        use crate::ui::gfx::Gfx;
        let _ = unsafe {
            windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)
        };
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("snapshots");
        std::fs::create_dir_all(&dir).unwrap();
        let tiles = vec![
            TileView {
                id: "1".into(),
                label: "로파이 음악".into(),
                badge: "로파".into(),
                kind: TileKind::Youtube,
                ..Default::default()
            },
            TileView {
                id: "2".into(),
                label: "아침 뉴스".into(),
                badge: "뉴스".into(),
                kind: TileKind::Youtube,
                ..Default::default()
            },
            TileView {
                id: "3".into(),
                label: "github.com".into(),
                badge: "G".into(),
                kind: TileKind::Web,
                missing: true,
                ..Default::default()
            },
            TileView {
                id: "4".into(),
                label: "계산기".into(),
                badge: "계".into(),
                kind: TileKind::App,
                icon: Some(std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icon.png"))),
                running: true,
                missing: false,
            },
        ];
        for (edge, name) in [(DockEdge::Bottom, "bottom"), (DockEdge::Left, "left")] {
            let layout = DockLayout::compute(edge, tiles.len());
            let mag = layout.magnified(Some(layout.rest[1]), 1.0);
            let max = layout.max_length();
            let vertical = dock_layout::is_vertical(edge);
            let (w, h) = if vertical {
                (dock_layout::THICK + dock_layout::HEADROOM, max + 2.0 * dock_layout::SIDE_ROOM)
            } else {
                (max + 2.0 * dock_layout::SIDE_ROOM, dock_layout::THICK + dock_layout::HEADROOM)
            };
            let center =
                if vertical { (dock_layout::THICK / 2.0, h / 2.0) } else { (w / 2.0, h - dock_layout::THICK / 2.0) };
            let gfx = Gfx::offscreen((w * 2.0) as u32, (h * 2.0) as u32, 2.0).unwrap();
            let state = DockScene {
                layout: &layout,
                mag: &mag,
                center,
                size: (w, h),
                palette: crate::ui::theme::palette(Theme::Glass),
                acrylic: false,
                tiles: &tiles,
                pressed: None,
                tooltip: Some(1),
                menu_hover: false,
                palette_hover: false,
                drag: None,
                picker: if vertical { 1.0 } else { 0.0 },
                picker_hover: None,
                theme: Theme::Glass,
            };
            gfx.begin();
            draw_dock(&gfx, &state);
            gfx.end().unwrap();
            let path = dir.join(format!("dock-{name}.png"));
            gfx.save_png(&path).unwrap();
            assert!(std::fs::metadata(&path).unwrap().len() > 1000);
        }
    }
}
```

- [ ] **Step 5: src/ui/panel.rs 작성**

```rust
//! 런처 화면 창: 유리 창(아크릴, 입력 받음) + 콘텐츠 창(Direct2D로 그림, 클릭은 유리 창으로 통과).
//!
//! 패널 모드는 두 창이 같은 사각형이다. 독 모드는 유리 창이 판 크기이고, 콘텐츠 창은 아이콘이 커지며
//! 판 밖으로 나갈 수 있도록 화면 안쪽으로 더 크다. 콘텐츠 창은 `SetWindowRgn`으로 지금 그려진 부분만
//! 창 영역으로 두어, 나머지 투명한 곳의 클릭은 뒤 창으로 그대로 간다.
//!
//! 앱과는 메시지로만 주고받는다. 앱 → 화면은 `PanelHandle::send(PanelCmd)`,
//! 화면 → 앱은 생성 때 받은 `notify(UiEvent)` 콜백.

use crate::config::{DockEdge, Mode, Rect, Theme};
use crate::logging;
use crate::ui::anim::{Anim, Tween};
use crate::ui::dock_layout::{self, DockLayout, Magnified};
use crate::ui::gfx::Gfx;
use crate::ui::layout::{
    EDGE_BOTTOM, EDGE_LEFT, EDGE_RIGHT, EDGE_TOP, Hit, MIN_H, MIN_W, PanelLayout, Pt, RectF, SWATCH,
    default_panel_rect, is_visible_on_any, reorder,
};
use crate::ui::scene::{self, DockScene, DragView, SceneState, TileView, dock_swatch_centers};
use crate::ui::theme::{self, PICKER_ORDER, Palette};
use crate::win::desktop::{self, PxRect};
use crate::win::dwm;
use crate::win::menu::{MenuEntry, show_popup};
use crate::win::msg_window::{Handler, WindowSpec, create_window};
use std::sync::mpsc::{Receiver, Sender, channel};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BLACK_BRUSH, BeginPaint, CombineRgn, CreateRectRgn, DeleteObject, EndPaint, FillRect, GetStockObject, HBRUSH,
    HGDIOBJ, HRGN, PAINTSTRUCT, RGN_OR, ScreenToClient, SetWindowRgn,
};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT,
    HTCAPTION, HTCLIENT, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, HTTRANSPARENT, KillTimer, MA_NOACTIVATE,
    MINMAXINFO, PostMessageW, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOZORDER, SetTimer, SetWindowPos, ShowWindow,
    WA_INACTIVE, WM_ACTIVATE, WM_APP, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_NCHITTEST, WM_PAINT, WM_RBUTTONUP, WM_SETTINGCHANGE, WM_TIMER,
    WM_WINDOWPOSCHANGED, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_POPUP,
};

/// 앱 → 화면 명령이 채널에 들어왔음을 알리는 메시지
pub const WM_APP_PANEL: u32 = WM_APP + 10;
const TIMER_ANIM: usize = 1;
const TIMER_SAVE_GEOMETRY: usize = 2;
const FRAME_MS: u32 = 16;
const SAVE_DELAY_MS: u32 = 500;
const DRAG_THRESHOLD: f32 = 6.0;
const PICKER_MS: f32 = 160.0;
const FADE_MS: f32 = 200.0;
/// 독 말풍선이 차지하는 여유 (창 영역 계산용)
const TOOLTIP_ROOM: f32 = 40.0;
const GLASS_CLASS: &str = "YTLauncherGlass";
const CONTENT_CLASS: &str = "YTLauncherContent";
const CMD_EDIT: u32 = 1;
const CMD_DELETE: u32 = 2;

#[derive(Debug, Clone, PartialEq)]
pub enum PanelCmd {
    SetTiles(Vec<TileView>),
    SetTheme(Theme),
    SetLocked(bool),
    /// 독 모드 / 패널 모드와 독 위치를 바꾼다
    SetMode(Mode, DockEdge),
    /// 다른 창 위로 잠깐 불러온다 (트레이 "런처 보이기")
    Show,
    ResetGeometry,
    Close,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UiEvent {
    TileClicked(String),
    AddRequested,
    EditRequested(String),
    DeleteRequested(String),
    Reordered(Vec<String>),
    ThemePicked(Theme),
    /// 패널 모드에서 위치·크기가 바뀜
    GeometryChanged(Rect),
    MenuRequested,
}

/// 처음 화면을 열 때 필요한 값
pub struct OpenSpec {
    pub mode: Mode,
    pub edge: DockEdge,
    /// 패널 모드에서 쓸 창 사각형 (px)
    pub panel_rect: PxRect,
    pub theme: Theme,
    pub locked: bool,
    pub tiles: Vec<TileView>,
}

/// 앱이 화면에 명령을 보내는 손잡이
#[derive(Clone)]
pub struct PanelHandle {
    glass: isize,
    tx: Sender<PanelCmd>,
}

impl PanelHandle {
    pub fn send(&self, cmd: PanelCmd) {
        if self.tx.send(cmd).is_ok() {
            let _ = unsafe { PostMessageW(Some(HWND(self.glass as *mut _)), WM_APP_PANEL, WPARAM(0), LPARAM(0)) };
        }
    }

    /// 대화상자의 소유자로 쓸 창
    pub fn owner(&self) -> HWND {
        HWND(self.glass as *mut _)
    }
}

/// 콘텐츠 창: 그리기만 하고 마우스는 아래 유리 창으로 넘긴다.
struct ContentWindow;

impl Handler for ContentWindow {
    fn handle(&mut self, _hwnd: HWND, msg: u32, _w: WPARAM, _l: LPARAM) -> Option<LRESULT> {
        match msg {
            WM_NCHITTEST => Some(LRESULT(HTTRANSPARENT as isize)),
            WM_MOUSEACTIVATE => Some(LRESULT(MA_NOACTIVATE as isize)),
            _ => None,
        }
    }
}

struct Press {
    hit: Hit,
    at: Pt,
}

/// 독 모드 상태
struct Dock {
    layout: DockLayout,
    edge: DockEdge,
    /// 마우스 위치 (판 중심 기준 along)
    pointer: Option<f32>,
    /// 확대 정도 0..1
    amount: Anim,
    /// 판 중심 (화면 px)
    center_px: (i32, i32),
    /// 콘텐츠 창 사각형 (화면 px)
    content_px: PxRect,
}

pub struct Panel {
    glass: HWND,
    content: HWND,
    gfx: Option<Gfx>,
    acrylic: bool,
    scale: f32,
    mode: Mode,
    layout: PanelLayout,
    dock: Dock,
    panel_rect: PxRect,
    /// 우리가 창을 옮기는 중이면 WM_WINDOWPOSCHANGED를 무시한다
    positioning: bool,
    tiles: Vec<TileView>,
    hover: Vec<Anim>,
    hover_hit: Hit,
    tracking_leave: bool,
    press: Option<Press>,
    drag: Option<(usize, Pt)>,
    theme: Theme,
    fade_from: Palette,
    fade: Tween,
    picker_open: bool,
    picker: Tween,
    picker_hover: Option<usize>,
    locked: bool,
    animating: bool,
    rx: Receiver<PanelCmd>,
    notify: Box<dyn Fn(UiEvent)>,
}

fn screen_rect(hwnd: HWND) -> PxRect {
    let mut r = RECT::default();
    let _ = unsafe { GetWindowRect(hwnd, &mut r) };
    (r.left, r.top, r.right - r.left, r.bottom - r.top)
}

fn lparam_point(l: LPARAM) -> (i32, i32) {
    ((l.0 & 0xFFFF) as i16 as i32, ((l.0 >> 16) & 0xFFFF) as i16 as i32)
}

fn foreground_is_ours() -> bool {
    let fg = unsafe { GetForegroundWindow() };
    if fg.0.is_null() {
        return false;
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(fg, Some(&mut pid)) };
    pid == unsafe { GetCurrentProcessId() }
}

fn move_window(hwnd: HWND, r: PxRect) {
    let _ = unsafe { SetWindowPos(hwnd, None, r.0, r.1, r.2, r.3, SWP_NOACTIVATE | SWP_NOZORDER) };
}

/// 저장된 위치가 화면 안에 있으면 그대로, 아니면 우상단 기본 위치
pub fn initial_rect(saved: Option<Rect>) -> PxRect {
    let works = desktop::work_areas();
    match saved {
        Some(r) if is_visible_on_any((r.x, r.y, r.w, r.h), &works) => (r.x, r.y, r.w, r.h),
        _ => default_panel_rect(desktop::primary_work_area(), desktop::system_scale()),
    }
}

/// 사각형들을 합친 창 영역을 만든다 (px, 창 기준 좌표)
fn region_from(rects: &[PxRect]) -> Option<HRGN> {
    // SAFETY: 새 영역을 만들어 합친다. 최종 영역은 SetWindowRgn이 소유한다.
    unsafe {
        let mut iter = rects.iter();
        let first = iter.next()?;
        let total = CreateRectRgn(first.0, first.1, first.0 + first.2, first.1 + first.3);
        for r in iter {
            let part = CreateRectRgn(r.0, r.1, r.0 + r.2, r.1 + r.3);
            CombineRgn(Some(total), Some(total), Some(part), RGN_OR);
            let _ = DeleteObject(HGDIOBJ(part.0));
        }
        Some(total)
    }
}

impl Panel {
    /// 유리 창과 콘텐츠 창을 만들고 보여준다.
    pub fn open(spec: OpenSpec, notify: Box<dyn Fn(UiEvent)>) -> windows::core::Result<PanelHandle> {
        let (tx, rx) = channel();
        let glass_spec = WindowSpec {
            class: GLASS_CLASS,
            title: "YT Launcher",
            ex_style: WS_EX_TOOLWINDOW,
            style: WS_POPUP,
            rect: spec.panel_rect,
            owner: None,
        };
        let n = spec.tiles.len();
        let panel = Panel {
            glass: HWND::default(),
            content: HWND::default(),
            gfx: None,
            acrylic: false,
            scale: 1.0,
            mode: spec.mode,
            layout: PanelLayout::compute(spec.panel_rect.2 as f32, spec.panel_rect.3 as f32, n),
            dock: Dock {
                layout: DockLayout::compute(spec.edge, n),
                edge: spec.edge,
                pointer: None,
                amount: Anim::at(0.0),
                center_px: (0, 0),
                content_px: spec.panel_rect,
            },
            panel_rect: spec.panel_rect,
            positioning: false,
            hover: vec![Anim::at(0.0); n + 1],
            tiles: spec.tiles,
            hover_hit: Hit::Empty,
            tracking_leave: false,
            press: None,
            drag: None,
            theme: spec.theme,
            fade_from: theme::palette(spec.theme),
            fade: Tween::done(),
            picker_open: false,
            picker: Tween::done(),
            picker_hover: None,
            locked: spec.locked,
            animating: false,
            rx,
            notify,
        };
        let glass = create_window(&glass_spec, Box::new(panel))?;
        let handle = PanelHandle { glass: glass.0 as isize, tx };
        // 유리 창 핸들러가 만들어진 뒤에 콘텐츠 창과 그리기 장치를 붙인다 (WM_APP_PANEL로 초기화)
        let _ = unsafe { PostMessageW(Some(glass), WM_APP_PANEL, WPARAM(1), LPARAM(0)) };
        Ok(handle)
    }

    fn init(&mut self) {
        let rect = screen_rect(self.glass);
        desktop::pin_to_desktop(self.glass);
        self.scale = desktop::scale_for(self.glass);
        self.acrylic = dwm::enable_acrylic(self.glass, theme::palette(self.theme).dark_backdrop);
        let content_spec = WindowSpec {
            class: CONTENT_CLASS,
            title: "",
            ex_style: WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            style: WS_POPUP,
            rect,
            owner: Some(self.glass),
        };
        match create_window(&content_spec, Box::new(ContentWindow)) {
            Ok(content) => {
                self.content = content;
                match Gfx::new(content, rect.2 as u32, rect.3 as u32, self.scale) {
                    Ok(gfx) => self.gfx = Some(gfx),
                    Err(e) => logging::error(&format!("그리기 장치를 만들지 못했습니다: {e}")),
                }
            }
            Err(e) => logging::error(&format!("콘텐츠 창을 만들지 못했습니다: {e}")),
        }
        self.place_windows();
        unsafe {
            let _ = ShowWindow(self.glass, SW_SHOWNOACTIVATE);
            let _ = ShowWindow(self.content, SW_SHOWNOACTIVATE);
        }
        desktop::send_to_bottom(self.glass);
        self.render();
    }

    fn emit(&self, event: UiEvent) {
        (self.notify)(event);
    }

    fn palette(&self) -> Palette {
        theme::blend(&self.fade_from, &theme::palette(self.theme), self.fade.progress())
    }

    fn resize_gfx(&mut self, w: i32, h: i32) {
        if let Some(gfx) = &mut self.gfx
            && let Err(e) = gfx.resize(w as u32, h as u32, self.scale)
        {
            logging::error(&format!("그리기 크기 변경 실패: {e}"));
        }
    }

    /// 지금 모드에 맞게 두 창의 위치·크기를 정한다.
    fn place_windows(&mut self) {
        self.positioning = true;
        match self.mode {
            Mode::Panel => {
                move_window(self.glass, self.panel_rect);
                self.relayout_panel();
                let _ = unsafe { SetWindowRgn(self.content, None, true) };
            }
            Mode::Dock => self.place_dock(),
        }
        self.positioning = false;
    }

    fn relayout_panel(&mut self) {
        let (x, y, w, h) = screen_rect(self.glass);
        self.panel_rect = (x, y, w, h);
        self.layout = PanelLayout::compute(w as f32 / self.scale, h as f32 / self.scale, self.tiles.len());
        self.hover.resize(self.tiles.len() + 1, Anim::at(0.0));
        if !self.content.0.is_null() {
            move_window(self.content, (x, y, w, h));
        }
        self.resize_gfx(w, h);
    }

    fn place_dock(&mut self) {
        let s = self.scale;
        let d = &mut self.dock;
        d.layout = DockLayout::compute(d.edge, self.tiles.len());
        let work = desktop::primary_work_area();
        let thick = (dock_layout::THICK * s).round() as i32;
        let margin = (dock_layout::MARGIN * s).round() as i32;
        let rest = dock_layout::bar_rect(work, d.edge, (d.layout.length * s).round() as i32, thick, margin);
        d.center_px = (rest.0 + rest.2 / 2, rest.1 + rest.3 / 2);
        let widest = dock_layout::bar_rect(work, d.edge, (d.layout.max_length() * s).round() as i32, thick, margin);
        d.content_px = dock_layout::content_rect(
            widest,
            d.edge,
            (dock_layout::HEADROOM * s).round() as i32,
            (dock_layout::SIDE_ROOM * s).round() as i32,
        );
        let content = d.content_px;
        move_window(self.content, content);
        self.resize_gfx(content.2, content.3);
        self.update_dock_glass();
    }

    fn dock_mag(&self) -> Magnified {
        self.dock.layout.magnified(self.dock.pointer, self.dock.amount.value)
    }

    /// 판 중심 (콘텐츠 창 기준 DIP)
    fn dock_center(&self) -> (f32, f32) {
        let d = &self.dock;
        ((d.center_px.0 - d.content_px.0) as f32 / self.scale, (d.center_px.1 - d.content_px.1) as f32 / self.scale)
    }

    /// 확대된 길이에 맞춰 유리 창(판)을 옮기고, 콘텐츠 창 영역을 지금 그려지는 부분으로 줄인다.
    fn update_dock_glass(&mut self) {
        let m = self.dock_mag();
        let bar = self.dock.layout.bar_rect(&m, self.dock_center());
        let s = self.scale;
        let (cx, cy, _, _) = self.dock.content_px;
        let px = |r: RectF| {
            let x = (r.x * s).round() as i32;
            let y = (r.y * s).round() as i32;
            (x, y, (r.right() * s).round() as i32 - x, (r.bottom() * s).round() as i32 - y)
        };
        let local = px(bar);
        self.positioning = true;
        move_window(self.glass, (cx + local.0, cy + local.1, local.2, local.3));
        self.positioning = false;

        // 창 영역: 판 + 커진 아이콘이 나가는 쪽 + 말풍선 + 열린 선택기
        let grow = m.scales.iter().fold(0.0f32, |a, &b| a.max(b - 1.0)) * dock_layout::ICON;
        let extra = if grow > 0.0 || self.dock_tooltip().is_some() { grow + TOOLTIP_ROOM } else { 0.0 };
        let outward = match self.dock.edge {
            DockEdge::Bottom => RectF::new(bar.x, bar.y - extra, bar.w, bar.h + extra),
            DockEdge::Top => RectF::new(bar.x, bar.y, bar.w, bar.h + extra),
            DockEdge::Left => RectF::new(bar.x, bar.y, bar.w + extra, bar.h),
            DockEdge::Right => RectF::new(bar.x - extra, bar.y, bar.w + extra, bar.h),
        };
        let mut rects = vec![px(outward)];
        if self.picker_open {
            let (w, h) = (self.dock.content_px.2, self.dock.content_px.3);
            rects.push((0, 0, w, h));
        }
        if let Some(region) = region_from(&rects) {
            let _ = unsafe { SetWindowRgn(self.content, Some(region), true) };
        }
    }

    fn dock_tooltip(&self) -> Option<usize> {
        if self.drag.is_some() || self.dock.amount.value < 0.5 {
            return None;
        }
        let tiles = self.tiles.len();
        match self.hover_hit {
            Hit::Tile(i) => Some(i),
            Hit::Add => Some(tiles),
            _ => None,
        }
    }

    fn render(&mut self) {
        let Some(gfx) = &self.gfx else { return };
        let picker = if self.picker_open { self.picker.progress() } else { 0.0 };
        let pressed = match &self.press {
            Some(Press { hit: Hit::Tile(i), .. }) => Some(*i),
            _ => None,
        };
        gfx.begin();
        match self.mode {
            Mode::Panel => {
                let hover: Vec<f32> = self.hover.iter().map(|a| a.value).collect();
                let drag = self.drag.map(|(from, pos)| DragView { from, to: self.layout.drop_index(pos), pos });
                let tooltip = match self.hover_hit {
                    Hit::Tile(i) if self.drag.is_none() && hover.get(i).copied().unwrap_or(0.0) > 0.5 => Some(i),
                    Hit::Add if hover.last().copied().unwrap_or(0.0) > 0.5 => Some(self.tiles.len()),
                    _ => None,
                };
                let state = SceneState {
                    layout: &self.layout,
                    palette: self.palette(),
                    acrylic: self.acrylic,
                    tiles: &self.tiles,
                    hover: &hover,
                    pressed,
                    menu_hover: self.hover_hit == Hit::Menu,
                    palette_hover: self.hover_hit == Hit::Palette,
                    drag,
                    tooltip,
                    picker,
                    picker_hover: self.picker_hover,
                    theme: self.theme,
                };
                scene::draw(gfx, &state);
            }
            Mode::Dock => {
                let m = self.dock_mag();
                let d = &self.dock;
                let state = DockScene {
                    layout: &d.layout,
                    mag: &m,
                    center: self.dock_center(),
                    size: (d.content_px.2 as f32 / self.scale, d.content_px.3 as f32 / self.scale),
                    palette: self.palette(),
                    acrylic: self.acrylic,
                    tiles: &self.tiles,
                    pressed,
                    tooltip: self.dock_tooltip(),
                    menu_hover: self.hover_hit == Hit::Menu,
                    palette_hover: self.hover_hit == Hit::Palette,
                    drag: self.drag,
                    picker,
                    picker_hover: self.picker_hover,
                    theme: self.theme,
                };
                scene::draw_dock(gfx, &state);
            }
        }
        if let Err(e) = gfx.end() {
            logging::warn(&format!("그리기 실패: {e}"));
        }
    }

    fn start_animation(&mut self) {
        if !self.animating {
            self.animating = true;
            unsafe { SetTimer(Some(self.glass), TIMER_ANIM, FRAME_MS, None) };
        }
    }

    fn tick(&mut self) {
        let mut moving = false;
        for anim in &mut self.hover {
            moving |= anim.step();
        }
        moving |= self.fade.advance(FRAME_MS as f32);
        if self.picker_open {
            moving |= self.picker.advance(FRAME_MS as f32);
        }
        if self.mode == Mode::Dock {
            moving |= self.dock.amount.step();
            if self.dock.amount.value == 0.0 && self.dock.amount.target == 0.0 {
                self.dock.pointer = None;
            }
            self.update_dock_glass();
        }
        self.render();
        if !moving {
            self.animating = false;
            let _ = unsafe { KillTimer(Some(self.glass), TIMER_ANIM) };
        }
    }

    /// 유리 창 기준 px 좌표 → 지금 모드의 그리기 좌표(DIP)
    fn local_point(&self, x: i32, y: i32) -> Pt {
        match self.mode {
            Mode::Panel => Pt { x: x as f32 / self.scale, y: y as f32 / self.scale },
            Mode::Dock => {
                let (gx, gy, _, _) = screen_rect(self.glass);
                let (cx, cy, _, _) = self.dock.content_px;
                Pt { x: (gx + x - cx) as f32 / self.scale, y: (gy + y - cy) as f32 / self.scale }
            }
        }
    }

    /// 독에서 그리기 좌표 → (along, across)
    fn dock_axes(&self, p: Pt, m: &Magnified) -> (f32, f32) {
        let center = self.dock_center();
        let bar = self.dock.layout.bar_rect(m, center);
        if dock_layout::is_vertical(self.dock.edge) {
            (p.y - center.1, p.x - bar.x)
        } else {
            (p.x - center.0, p.y - bar.y)
        }
    }

    fn hit(&self, p: Pt) -> Hit {
        match self.mode {
            Mode::Panel => self.layout.hit_test(p, true),
            Mode::Dock => {
                let m = self.dock_mag();
                let (along, across) = self.dock_axes(p, &m);
                self.dock.layout.hit(&m, along, across)
            }
        }
    }

    fn drop_index(&self, p: Pt) -> usize {
        match self.mode {
            Mode::Panel => self.layout.drop_index(p),
            Mode::Dock => {
                let m = self.dock_mag();
                self.dock.layout.drop_index(self.dock_axes(p, &m).0)
            }
        }
    }

    fn swatch_centers(&self) -> Vec<Pt> {
        let progress = self.picker.progress();
        match self.mode {
            Mode::Panel => self.layout.swatch_centers(PICKER_ORDER.len(), progress),
            Mode::Dock => {
                let m = self.dock_mag();
                dock_swatch_centers(&self.dock.layout, &m, self.dock_center(), progress)
            }
        }
    }

    fn client_point_from_screen(&self, l: LPARAM) -> Pt {
        let (x, y) = lparam_point(l);
        let mut pt = POINT { x, y };
        let _ = unsafe { ScreenToClient(self.glass, &mut pt) };
        Pt { x: pt.x as f32 / self.scale, y: pt.y as f32 / self.scale }
    }

    fn set_hover(&mut self, hit: Hit) {
        if hit == self.hover_hit {
            return;
        }
        self.hover_hit = hit;
        let n = self.tiles.len();
        for (i, anim) in self.hover.iter_mut().enumerate() {
            let on = matches!(hit, Hit::Tile(t) if t == i) || (hit == Hit::Add && i == n);
            anim.set(if on { 1.0 } else { 0.0 });
        }
        self.start_animation();
    }

    fn swatch_at(&self, p: Pt) -> Option<usize> {
        if !self.picker_open {
            return None;
        }
        let r = SWATCH / 2.0;
        self.swatch_centers().iter().position(|c| (c.x - p.x).powi(2) + (c.y - p.y).powi(2) <= r * r)
    }

    fn close_picker(&mut self) {
        self.picker_open = false;
        self.picker_hover = None;
        if self.mode == Mode::Dock {
            self.update_dock_glass();
        }
        self.render();
    }

    fn on_mouse_move(&mut self, p: Pt) {
        if !self.tracking_leave {
            let mut tme = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: self.glass,
                dwHoverTime: 0,
            };
            self.tracking_leave = unsafe { TrackMouseEvent(&mut tme) }.is_ok();
        }
        if let Some(press) = &self.press
            && let Hit::Tile(i) = press.hit
            && self.drag.is_none()
            && ((p.x - press.at.x).powi(2) + (p.y - press.at.y).powi(2)).sqrt() > DRAG_THRESHOLD
        {
            self.drag = Some((i, p));
        }
        if let Some((from, _)) = self.drag {
            self.drag = Some((from, p));
            self.render();
            return;
        }
        let swatch = self.swatch_at(p);
        if swatch != self.picker_hover {
            self.picker_hover = swatch;
            self.render();
        }
        if self.mode == Mode::Dock {
            let m = self.dock_mag();
            self.dock.pointer = Some(self.dock_axes(p, &m).0);
            self.dock.amount.set(1.0);
            self.start_animation();
        }
        self.set_hover(self.hit(p));
    }

    fn on_mouse_leave(&mut self) {
        self.tracking_leave = false;
        if self.drag.is_none() {
            self.set_hover(Hit::Empty);
            self.picker_hover = None;
            if self.mode == Mode::Dock {
                self.dock.amount.set(0.0);
                self.start_animation();
            }
        }
    }

    fn on_button_up(&mut self, p: Pt) {
        let _ = unsafe { ReleaseCapture() };
        let press = self.press.take();
        if let Some((from, pos)) = self.drag.take() {
            let to = self.drop_index(pos);
            if from != to {
                self.tiles = reorder(&self.tiles, from, to);
                self.emit(UiEvent::Reordered(self.tiles.iter().map(|t| t.id.clone()).collect()));
            }
            self.render();
            return;
        }
        if self.picker_open {
            if let Some(i) = self.swatch_at(p) {
                let picked = PICKER_ORDER[i];
                self.close_picker();
                if picked != self.theme {
                    self.emit(UiEvent::ThemePicked(picked));
                }
                return;
            }
            let on_button = self.hit(p) == Hit::Palette;
            self.close_picker();
            if on_button {
                return;
            }
        }
        let hit = self.hit(p);
        if press.map(|pr| pr.hit) != Some(hit) {
            self.render();
            return;
        }
        match hit {
            Hit::Tile(i) => self.emit(UiEvent::TileClicked(self.tiles[i].id.clone())),
            Hit::Add => self.emit(UiEvent::AddRequested),
            Hit::Menu => self.emit(UiEvent::MenuRequested),
            Hit::Palette => {
                self.picker_open = true;
                self.picker = Tween::new(PICKER_MS);
                if self.mode == Mode::Dock {
                    self.update_dock_glass();
                }
                self.start_animation();
            }
            _ => {}
        }
        self.render();
    }

    fn on_right_click(&mut self, p: Pt) {
        if let Hit::Tile(i) = self.hit(p) {
            let id = self.tiles[i].id.clone();
            let entries = [MenuEntry::item(CMD_EDIT, "수정"), MenuEntry::item(CMD_DELETE, "삭제")];
            match show_popup(self.glass, &entries) {
                CMD_EDIT => self.emit(UiEvent::EditRequested(id)),
                CMD_DELETE => self.emit(UiEvent::DeleteRequested(id)),
                _ => {}
            }
        }
    }

    fn hit_test_nc(&self, l: LPARAM) -> LRESULT {
        if self.mode == Mode::Dock {
            return LRESULT(HTCLIENT as isize);
        }
        let p = self.client_point_from_screen(l);
        let code = match self.layout.hit_test(p, self.locked) {
            Hit::Edge(e) => match e {
                x if x == EDGE_LEFT | EDGE_TOP => HTTOPLEFT,
                x if x == EDGE_RIGHT | EDGE_TOP => HTTOPRIGHT,
                x if x == EDGE_LEFT | EDGE_BOTTOM => HTBOTTOMLEFT,
                x if x == EDGE_RIGHT | EDGE_BOTTOM => HTBOTTOMRIGHT,
                EDGE_LEFT => HTLEFT,
                EDGE_RIGHT => HTRIGHT,
                EDGE_TOP => HTTOP,
                _ => HTBOTTOM,
            },
            Hit::Header if !self.picker_open => HTCAPTION,
            _ => HTCLIENT,
        };
        LRESULT(code as isize)
    }

    fn apply(&mut self, cmd: PanelCmd) {
        match cmd {
            PanelCmd::SetTiles(tiles) => {
                // 실행 중 점·아이콘만 바뀐 경우(같은 항목 순서)는 마우스 상태를 그대로 둔다
                let same_items =
                    tiles.len() == self.tiles.len() && tiles.iter().zip(&self.tiles).all(|(a, b)| a.id == b.id);
                self.tiles = tiles;
                if let Some(gfx) = &self.gfx {
                    gfx.clear_images();
                }
                if !same_items {
                    self.hover = vec![Anim::at(0.0); self.tiles.len() + 1];
                    self.hover_hit = Hit::Empty;
                }
                self.layout = PanelLayout::compute(self.layout.width, self.layout.height, self.tiles.len());
                if self.mode == Mode::Dock {
                    self.place_windows();
                }
                self.render();
            }
            PanelCmd::SetTheme(theme) => {
                if theme != self.theme {
                    self.fade_from = self.palette();
                    self.theme = theme;
                    self.fade = Tween::new(FADE_MS);
                    if self.acrylic {
                        dwm::set_dark(self.glass, theme::palette(theme).dark_backdrop);
                    }
                    self.start_animation();
                }
            }
            PanelCmd::SetLocked(locked) => self.locked = locked,
            PanelCmd::SetMode(mode, edge) => {
                self.mode = mode;
                self.dock.edge = edge;
                self.dock.pointer = None;
                self.dock.amount = Anim::at(0.0);
                self.picker_open = false;
                self.hover_hit = Hit::Empty;
                self.place_windows();
                self.render();
            }
            PanelCmd::Show => {
                crate::win::windows::restore_and_focus(self.glass);
            }
            PanelCmd::ResetGeometry => {
                if self.mode == Mode::Panel {
                    self.panel_rect = default_panel_rect(desktop::primary_work_area(), self.scale);
                }
                self.place_windows();
                self.render();
            }
            PanelCmd::Close => {
                let _ = unsafe { DestroyWindow(self.content) };
                let _ = unsafe { DestroyWindow(self.glass) };
            }
        }
    }
}

impl Handler for Panel {
    fn handle(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        if self.glass.0.is_null() {
            self.glass = hwnd;
        }
        match msg {
            WM_APP_PANEL => {
                if wparam.0 == 1 && self.gfx.is_none() && self.content.0.is_null() {
                    self.init();
                }
                while let Ok(cmd) = self.rx.try_recv() {
                    self.apply(cmd);
                }
            }
            WM_NCHITTEST => return Some(self.hit_test_nc(lparam)),
            WM_MOUSEMOVE => {
                let (x, y) = lparam_point(lparam);
                self.on_mouse_move(self.local_point(x, y));
            }
            WM_MOUSELEAVE => self.on_mouse_leave(),
            WM_LBUTTONDOWN => {
                let (x, y) = lparam_point(lparam);
                let at = self.local_point(x, y);
                self.press = Some(Press { hit: self.hit(at), at });
                let _ = unsafe { SetCapture(hwnd) };
                self.render();
            }
            WM_LBUTTONUP => {
                let (x, y) = lparam_point(lparam);
                self.on_button_up(self.local_point(x, y));
            }
            WM_RBUTTONUP => {
                let (x, y) = lparam_point(lparam);
                self.on_right_click(self.local_point(x, y));
            }
            WM_ACTIVATE => {
                // 다른 앱으로 포커스가 넘어가면 다시 바탕화면 쪽으로 내린다 (우리 대화상자·메뉴는 제외)
                if (wparam.0 & 0xFFFF) as u32 == WA_INACTIVE && !foreground_is_ours() {
                    desktop::send_to_bottom(hwnd);
                }
                return None;
            }
            WM_WINDOWPOSCHANGED => {
                if !self.positioning && self.mode == Mode::Panel {
                    self.relayout_panel();
                    self.render();
                    unsafe { SetTimer(Some(hwnd), TIMER_SAVE_GEOMETRY, SAVE_DELAY_MS, None) };
                }
                return None;
            }
            WM_DISPLAYCHANGE | WM_SETTINGCHANGE => {
                // 해상도·작업표시줄이 바뀌면 독을 다시 놓는다
                if self.mode == Mode::Dock {
                    self.place_windows();
                    self.render();
                }
                return None;
            }
            WM_DPICHANGED => {
                self.scale = ((wparam.0 & 0xFFFF) as f32) / 96.0;
                if self.mode == Mode::Dock {
                    self.place_windows();
                    self.render();
                } else {
                    // SAFETY: lparam은 시스템이 제안한 새 창 사각형
                    let r = unsafe { &*(lparam.0 as *const RECT) };
                    move_window(hwnd, (r.left, r.top, r.right - r.left, r.bottom - r.top));
                }
            }
            WM_GETMINMAXINFO => {
                if self.mode == Mode::Panel {
                    // SAFETY: lparam은 시스템이 넘긴 MINMAXINFO
                    let info = unsafe { &mut *(lparam.0 as *mut MINMAXINFO) };
                    info.ptMinTrackSize.x = (MIN_W * self.scale) as i32;
                    info.ptMinTrackSize.y = (MIN_H * self.scale) as i32;
                }
            }
            WM_TIMER => match wparam.0 {
                TIMER_ANIM => self.tick(),
                TIMER_SAVE_GEOMETRY => {
                    let _ = unsafe { KillTimer(Some(hwnd), TIMER_SAVE_GEOMETRY) };
                    if self.mode == Mode::Panel {
                        let (x, y, w, h) = screen_rect(hwnd);
                        self.emit(UiEvent::GeometryChanged(Rect { x, y, w, h }));
                    }
                }
                _ => return None,
            },
            WM_ERASEBKGND => {}
            WM_PAINT => {
                // 유리 창은 검게 칠해 둔다: 흐림 효과는 검은(빈) 부분에 비친다
                let mut ps = PAINTSTRUCT::default();
                unsafe {
                    let dc = BeginPaint(hwnd, &mut ps);
                    FillRect(dc, &ps.rcPaint, HBRUSH(GetStockObject(BLACK_BRUSH).0));
                    let _ = EndPaint(hwnd, &ps);
                }
                self.render();
            }
            _ => return None,
        }
        Some(LRESULT(if msg == WM_ERASEBKGND { 1 } else { 0 }))
    }
}
```

- [ ] **Step 6: src/main.rs 작성**

```rust
#![cfg_attr(not(test), windows_subsystem = "windows")]
//! YT Launcher 진입점.

mod app;
#[allow(dead_code)] // 앱·웹 항목 연결(Task 4)에서 지운다
mod apps;
mod autostart;
mod chrome;
mod config;
#[allow(dead_code)] // 앱·웹 항목 연결(Task 4)에서 지운다
mod icons;
mod items;
mod logging;
mod tray;
mod ui;
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

    // 셸·COM을 쓰는 기능(ShellExecute, 4단계의 앱 목록)을 위해 UI 스레드를 STA로 초기화한다
    let _ = unsafe {
        windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)
    };

    let app = app::App::new(config::Storage::new(&data_dir));
    if let Err(e) = win::msg_window::create(MESSAGE_CLASS, Box::new(app)) {
        logging::error(&format!("메시지 창을 만들지 못했습니다: {e}"));
        return;
    }
    std::process::exit(win::msg_window::run_loop());
}
```

- [ ] **Step 7: src/app.rs 작성**

```rust
//! 앱 컨트롤러: 설정·트레이·패널 화면·항목 실행·업데이트·자동 실행을 연결한다.

use crate::autostart::{AutoStart, RegistryRun};
use crate::chrome;
use crate::config::{Config, DockEdge, LoadReport, Mode, Storage, Theme};
use crate::items::Item;
use crate::logging;
use crate::tray::{Tray, WM_APP_TRAY};
use crate::ui::dialogs::{YoutubeInput, youtube_dialog};
use crate::ui::panel::{OpenSpec, Panel, PanelCmd, PanelHandle, UiEvent, initial_rect};
use crate::ui::scene::{TileKind, TileView, badge_of};
use crate::updater::{self, Release, UreqHttp};
use crate::win::instance::WM_APP_SHOW;
use crate::win::menu::{MenuEntry, show_popup};
use crate::win::msg_window::Handler;
use crate::win::shell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};
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
const CLICK_DEBOUNCE: Duration = Duration::from_secs(1);

const CMD_SHOW: u32 = 20;
const CMD_MODE_DOCK: u32 = 30;
const CMD_MODE_PANEL: u32 = 31;
const CMD_EDGE_BOTTOM: u32 = 32;
const CMD_EDGE_TOP: u32 = 33;
const CMD_EDGE_LEFT: u32 = 34;
const CMD_EDGE_RIGHT: u32 = 35;
const CMD_RESET_GEOMETRY: u32 = 21;
const CMD_LOCK: u32 = 22;
const CMD_AUTOSTART: u32 = 10;
const CMD_CHECK_UPDATE: u32 = 11;
const CMD_INSTALL_UPDATE: u32 = 12;
const CMD_RELEASES: u32 = 13;
const CMD_QUIT: u32 = 14;

pub enum Event {
    Checked { manual: bool, result: Result<Option<Release>, String> },
    Downloaded(Result<PathBuf, String>),
    Ui(UiEvent),
    Opened(chrome::Opened),
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

/// 시작할 때 띄울 알림 문구 (설정 손상 복구, 업데이트 완료). 순수 함수.
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

/// 설정의 항목 → 화면에 그릴 타일 (순수 함수)
pub fn tile_views(items: &[Item]) -> Vec<TileView> {
    items
        .iter()
        .map(|item| {
            let label = item.display_name();
            let (kind, badge) = match item {
                Item::Youtube { query, .. } => (TileKind::Youtube, badge_of(query)),
                Item::App { .. } => (TileKind::App, label.chars().take(1).collect()),
                Item::Web { .. } => (TileKind::Web, label.chars().take(1).collect::<String>().to_uppercase()),
            };
            TileView { id: item.id().to_string(), label, badge, kind, ..Default::default() }
        })
        .collect()
}

/// 화면에서 바뀐 순서(id 목록)대로 항목을 다시 늘어놓는다. 목록에 없는 항목은 뒤에 붙인다.
pub fn apply_order(items: &[Item], ids: &[String]) -> Vec<Item> {
    let mut rest: Vec<Item> = items.to_vec();
    let mut ordered = Vec::with_capacity(items.len());
    for id in ids {
        if let Some(pos) = rest.iter().position(|i| i.id() == id) {
            ordered.push(rest.remove(pos));
        }
    }
    ordered.extend(rest);
    ordered
}

/// 같은 항목을 짧은 시간 안에 두 번 누르면 무시한다.
#[derive(Default)]
pub struct Debounce {
    last: HashMap<String, Instant>,
}

impl Debounce {
    pub fn allow(&mut self, id: &str, now: Instant) -> bool {
        match self.last.get(id) {
            Some(t) if now.duration_since(*t) < CLICK_DEBOUNCE => false,
            _ => {
                self.last.insert(id.to_string(), now);
                true
            }
        }
    }
}

pub struct App {
    hwnd: HWND,
    storage: Storage,
    config: Config,
    tray: Option<Tray>,
    panel: Option<PanelHandle>,
    autostart: AutoStart<RegistryRun>,
    release: Option<Release>,
    downloading: bool,
    debounce: Debounce,
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
            panel: None,
            autostart: AutoStart { backend: RegistryRun::default() },
            release: None,
            downloading: false,
            debounce: Debounce::default(),
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

    fn panel_send(&self, cmd: PanelCmd) {
        if let Some(panel) = &self.panel {
            panel.send(cmd);
        }
    }

    fn refresh_tiles(&self) {
        self.panel_send(PanelCmd::SetTiles(tile_views(&self.config.items)));
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
        self.open_panel();
        for notice in std::mem::take(&mut self.pending_notices) {
            self.notify(&notice);
        }
        if self.config.last_run_version != CURRENT_VERSION {
            self.config.last_run_version = CURRENT_VERSION.to_string();
            self.save();
        }
        unsafe { SetTimer(Some(hwnd), TIMER_FIRST_CHECK, FIRST_CHECK_MS, None) };
    }

    fn open_panel(&mut self) {
        let send = self.post_event();
        let notify = Box::new(move |e: UiEvent| send(Event::Ui(e)));
        let spec = OpenSpec {
            mode: self.config.mode,
            edge: self.config.dock_edge,
            panel_rect: initial_rect(self.config.panel_window),
            theme: self.config.theme,
            locked: self.config.locked,
            tiles: tile_views(&self.config.items),
        };
        match Panel::open(spec, notify) {
            Ok(handle) => self.panel = Some(handle),
            Err(e) => logging::error(&format!("패널 창을 만들지 못했습니다: {e}")),
        }
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

    fn dialog_owner(&self) -> HWND {
        self.panel.as_ref().map(PanelHandle::owner).unwrap_or(self.hwnd)
    }

    fn on_ui(&mut self, event: UiEvent) {
        match event {
            UiEvent::TileClicked(id) => self.open_item(&id),
            UiEvent::AddRequested => {
                if let Some(input) = youtube_dialog(self.dialog_owner(), "항목 추가", &YoutubeInput::default())
                    && let Ok(item) = Item::youtube(&input.query, &input.name)
                {
                    self.config.items.push(item);
                    self.save();
                    self.refresh_tiles();
                }
            }
            UiEvent::EditRequested(id) => self.edit_item(&id),
            UiEvent::DeleteRequested(id) => self.delete_item(&id),
            UiEvent::Reordered(ids) => {
                self.config.items = apply_order(&self.config.items, &ids);
                self.save();
            }
            UiEvent::ThemePicked(theme) => self.set_theme(theme),
            UiEvent::GeometryChanged(rect) => {
                self.config.panel_window = Some(rect);
                self.save();
            }
            UiEvent::MenuRequested => self.show_menu(),
        }
    }

    fn edit_item(&mut self, id: &str) {
        let Some(index) = self.config.items.iter().position(|i| i.id() == id) else { return };
        if let Item::Youtube { query, name, .. } = &self.config.items[index] {
            let initial = YoutubeInput { query: query.clone(), name: name.clone() };
            if let Some(input) = youtube_dialog(self.dialog_owner(), "항목 수정", &initial) {
                self.config.items[index] = Item::Youtube { id: id.to_string(), query: input.query, name: input.name };
                self.save();
                self.refresh_tiles();
            }
        }
    }

    fn delete_item(&mut self, id: &str) {
        let Some(item) = self.config.items.iter().find(|i| i.id() == id) else { return };
        let text = HSTRING::from(format!("'{}' 항목을 삭제할까요?", item.display_name()));
        let answer = unsafe {
            MessageBoxW(Some(self.dialog_owner()), &text, &HSTRING::from(APP_NAME), MB_YESNO | MB_ICONQUESTION)
        };
        if answer == IDYES {
            self.config.items.retain(|i| i.id() != id);
            self.save();
            self.refresh_tiles();
        }
    }

    fn open_item(&mut self, id: &str) {
        if !self.debounce.allow(id, Instant::now()) {
            return;
        }
        let Some(item) = self.config.items.iter().find(|i| i.id() == id).cloned() else { return };
        let send = self.post_event();
        std::thread::spawn(move || {
            let result = match &item {
                Item::Youtube { query, .. } => chrome::open_url(&chrome::build_search_url(query), Some("YouTube")),
                Item::Web { url, .. } => chrome::open_url(url, None),
                Item::App { .. } => return, // 4단계에서 연결
            };
            match result {
                Ok(opened) => send(Event::Opened(opened)),
                Err(e) => logging::error(&format!("열기 실패: {e}")),
            }
        });
    }

    fn set_mode(&mut self, mode: Mode, edge: DockEdge) {
        self.config.mode = mode;
        self.config.dock_edge = edge;
        self.save();
        self.panel_send(PanelCmd::SetMode(mode, edge));
    }

    fn set_theme(&mut self, theme: Theme) {
        self.config.theme = theme;
        self.save();
        self.panel_send(PanelCmd::SetTheme(theme));
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
        let dock = self.config.mode == Mode::Dock;
        let edge = self.config.dock_edge;
        let mut entries = vec![
            MenuEntry::label(format!("{APP_NAME}  {BYLINE}  v{CURRENT_VERSION}")),
            MenuEntry::Separator,
            MenuEntry::item(CMD_SHOW, "런처 보이기"),
            MenuEntry::Separator,
            MenuEntry::checked(CMD_MODE_DOCK, "독 모드", dock),
            MenuEntry::checked(CMD_MODE_PANEL, "패널 모드", !dock),
        ];
        if dock {
            entries.extend([
                MenuEntry::checked(CMD_EDGE_BOTTOM, "    독 위치: 아래", edge == DockEdge::Bottom),
                MenuEntry::checked(CMD_EDGE_TOP, "    독 위치: 위", edge == DockEdge::Top),
                MenuEntry::checked(CMD_EDGE_LEFT, "    독 위치: 왼쪽", edge == DockEdge::Left),
                MenuEntry::checked(CMD_EDGE_RIGHT, "    독 위치: 오른쪽", edge == DockEdge::Right),
            ]);
        } else {
            entries.extend([
                MenuEntry::item(CMD_RESET_GEOMETRY, "위치·크기 초기화"),
                MenuEntry::checked(CMD_LOCK, "위치 잠금", self.config.locked),
            ]);
        }
        entries.extend([
            MenuEntry::Separator,
            MenuEntry::checked(CMD_AUTOSTART, "윈도우 시작 시 실행", self.autostart.is_enabled()),
            MenuEntry::item(CMD_CHECK_UPDATE, "업데이트 확인"),
        ]);
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
        match show_popup(self.hwnd, &self.menu_entries()) {
            CMD_SHOW => self.panel_send(PanelCmd::Show),
            CMD_MODE_DOCK => self.set_mode(Mode::Dock, self.config.dock_edge),
            CMD_MODE_PANEL => self.set_mode(Mode::Panel, self.config.dock_edge),
            CMD_EDGE_BOTTOM => self.set_mode(Mode::Dock, DockEdge::Bottom),
            CMD_EDGE_TOP => self.set_mode(Mode::Dock, DockEdge::Top),
            CMD_EDGE_LEFT => self.set_mode(Mode::Dock, DockEdge::Left),
            CMD_EDGE_RIGHT => self.set_mode(Mode::Dock, DockEdge::Right),
            CMD_RESET_GEOMETRY => self.panel_send(PanelCmd::ResetGeometry),
            CMD_LOCK => {
                self.config.locked = !self.config.locked;
                self.save();
                self.panel_send(PanelCmd::SetLocked(self.config.locked));
            }
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
        self.panel_send(PanelCmd::Close);
        let _ = unsafe { DestroyWindow(self.hwnd) };
        // 핸들러 안에서 자기 창을 없애면 WM_DESTROY가 핸들러에 오지 않으므로 여기서 루프를 끝낸다
        unsafe { PostQuitMessage(0) };
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
                        Event::Ui(ui) => self.on_ui(ui),
                        Event::Opened(chrome::Opened::DefaultBrowser) if !self.config.chrome_notice_shown => {
                            self.config.chrome_notice_shown = true;
                            self.save();
                            self.notify("크롬을 찾지 못해 기본 브라우저로 열었습니다.");
                        }
                        Event::Opened(_) => {}
                    }
                }
            }
            WM_APP_SHOW => self.panel_send(PanelCmd::Show),
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

    #[test]
    fn tile_views_map_kinds_labels_and_badges() {
        let items = vec![
            Item::Youtube { id: "y".into(), query: "로파이 음악".into(), name: "".into() },
            Item::Web { id: "w".into(), url: "https://github.com".into(), name: "".into() },
            Item::App { id: "a".into(), name: "계산기".into(), target: "Calc!App".into(), exe: None },
        ];
        let views = tile_views(&items);
        assert_eq!(
            views[0],
            TileView {
                id: "y".into(),
                label: "로파이 음악".into(),
                badge: "로파".into(),
                kind: TileKind::Youtube,
                ..Default::default()
            }
        );
        assert_eq!(
            (views[1].label.as_str(), views[1].badge.as_str(), views[1].kind),
            ("github.com", "G", TileKind::Web)
        );
        assert_eq!((views[2].badge.as_str(), views[2].kind), ("계", TileKind::App));
    }

    #[test]
    fn apply_order_follows_ids_and_keeps_unknown() {
        let a = Item::youtube("a", "").unwrap();
        let b = Item::youtube("b", "").unwrap();
        let c = Item::youtube("c", "").unwrap();
        let items = vec![a.clone(), b.clone(), c.clone()];
        let ids = vec![c.id().to_string(), a.id().to_string(), "gone".to_string()];
        assert_eq!(apply_order(&items, &ids), vec![c, a, b]);
    }

    #[test]
    fn debounce_blocks_repeat_within_one_second() {
        let mut d = Debounce::default();
        let t0 = Instant::now();
        assert!(d.allow("x", t0));
        assert!(!d.allow("x", t0 + Duration::from_millis(500)));
        assert!(d.allow("y", t0 + Duration::from_millis(500)));
        assert!(d.allow("x", t0 + Duration::from_millis(1200)));
    }
}
```

- [ ] **Step 8: 테스트와 눈으로 확인**

Run: `cargo test icons; cargo test scene`
Expected: icons 5 passed, scene 4 passed. `target\snapshots\dock-bottom.png`에서 계산기 자리에 앱 아이콘과 아래 점, `dock-left.png`에서 흐린 G 타일과 왼쪽 점을 본다.

- [ ] **Step 9: 전체 검사**

Run: `cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test`
Expected: clippy 오류 없음, `test result: ok`

- [ ] **Step 10: Commit**

```powershell
git add assets/icon.png src
git commit -m "feat: 앱 아이콘·파비콘 캐시와 실행 중 점·흐림 표시"
```

---

### Task 4: 추가·수정 창(종류 선택)·앱 선택 창과 앱 연결 (dialogs, app_list, app.rs)

**Files:**
- Create: `src/ui/app_list.rs`
- Modify: `src/ui/dialogs.rs`, `src/ui/mod.rs`, `src/items.rs`, `src/app.rs`, `src/main.rs`, `src/win/mod.rs` (Task 1~3의 임시 allow 제거)

**Interfaces:**
- Consumes: Task 1~3
- Produces: `dialogs::{Kind, ItemInput{Youtube,App,Web}, Initial{New,Edit}, item_dialog(owner,title,&Initial) -> Option<ItemInput>}` + 공용 `{window_text, warn, message_font, paint_background, Ctl, child, centered_on, clamp_into, run_modal}`; `app_list::{AppChoice(=AppEntry), filter_apps, pick_app(owner) -> Option<AppChoice>}`; `app::{TileStatus, tile_views(items,&TileStatus), item_from_input, input_of, icon_source_changed, app_keys}`; `Event::{IconsReady, LaunchFailed(id)}`.
- Behavior: 시작 때 실행 중 계산 → 화면 열기 → 창 감시 훅 → 아이콘 하위 프로세스. 추가·수정·삭제 때 아이콘 캐시와 실행 중·찾지 못함 상태를 맞춘다.

- [ ] **Step 1: src/ui/dialogs.rs 작성**

```rust
//! 항목 추가·수정 창 (윈도우 기본 컨트롤: 한글 입력·붙여넣기·Tab·Enter·Esc가 그대로 동작).
//!
//! 위쪽 종류 버튼(유튜브 / 앱 / 웹 주소)으로 입력칸이 바뀐다. 수정할 때는 종류를 바꿀 수 없다.
//! 다른 입력 창(앱 선택)도 쓰는 공용 도우미(글꼴, 컨트롤 만들기, 모달 루프)가 여기 있다.

use crate::items::normalize_url;
use crate::ui::app_list::{AppChoice, pick_app};
use crate::win::msg_window::{Handler, WindowSpec, create_window};
use std::cell::RefCell;
use std::rc::Rc;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    COLOR_WINDOW, CreateFontIndirectW, DeleteObject, FillRect, GetMonitorInfoW, GetSysColor, GetSysColorBrush, HDC,
    HFONT, HGDIOBJ, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow, SetBkColor,
};
use windows::Win32::UI::Controls::{BST_CHECKED, BST_UNCHECKED};
use windows::Win32::UI::HiDpi::{GetDpiForWindow, SystemParametersInfoForDpi};
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::{
    BM_SETCHECK, BS_AUTORADIOBUTTON, BS_DEFPUSHBUTTON, BS_PUSHBUTTON, BS_PUSHLIKE, CreateWindowExW, DestroyWindow,
    DispatchMessageW, ES_AUTOHSCROLL, GetClientRect, GetDlgItem, GetMessageW, GetWindowRect, GetWindowTextLengthW,
    GetWindowTextW, HMENU, IDCANCEL, IDOK, IsDialogMessageW, MB_ICONWARNING, MB_OK, MSG, MessageBoxW,
    NONCLIENTMETRICSW, PostQuitMessage, SPI_GETNONCLIENTMETRICS, SW_HIDE, SW_SHOW, SendMessageW, SetForegroundWindow,
    SetWindowTextW, ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND,
    WM_CTLCOLORSTATIC, WM_ERASEBKGND, WM_QUIT, WM_SETFONT, WS_CAPTION, WS_CHILD, WS_EX_CLIENTEDGE, WS_EX_DLGMODALFRAME,
    WS_GROUP, WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};
use windows::core::{HSTRING, w};

const DIALOG_CLASS: &str = "YTLauncherDialog";
/// 정적 글자가 넘치면 … 으로 줄인다 (SystemServices 기능을 켜지 않으려고 값만 둔다)
const SS_ENDELLIPSIS: u32 = 0x4000;
const ID_MAIN: i32 = 101;
const ID_NAME: i32 = 102;
const ID_MAIN_LABEL: i32 = 103;
const ID_HINT: i32 = 104;
const ID_PICK: i32 = 105;
const ID_PICKED: i32 = 106;
const ID_KIND_YOUTUBE: i32 = 111;
const ID_KIND_APP: i32 = 112;
const ID_KIND_WEB: i32 = 113;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    #[default]
    Youtube,
    App,
    Web,
}

impl Kind {
    fn from_id(id: i32) -> Option<Kind> {
        match id {
            ID_KIND_YOUTUBE => Some(Kind::Youtube),
            ID_KIND_APP => Some(Kind::App),
            ID_KIND_WEB => Some(Kind::Web),
            _ => None,
        }
    }

    fn button_id(self) -> i32 {
        match self {
            Kind::Youtube => ID_KIND_YOUTUBE,
            Kind::App => ID_KIND_APP,
            Kind::Web => ID_KIND_WEB,
        }
    }

    fn main_label(self) -> &'static str {
        match self {
            Kind::Youtube => "검색어",
            Kind::App => "앱",
            Kind::Web => "주소",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Kind::Youtube => "비우면 검색어를 그대로 씁니다.",
            Kind::App => "비우면 앱 이름을 씁니다.",
            Kind::Web => "비우면 주소의 사이트 이름을 씁니다.",
        }
    }
}

/// 입력 창에서 고른 값 (저장 조건을 통과한 값만 돌려준다)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemInput {
    Youtube {
        query: String,
        name: String,
    },
    App {
        app: AppChoice,
        name: String,
    },
    /// `url`은 `https://`가 붙은 정규화된 주소
    Web {
        url: String,
        name: String,
    },
}

impl ItemInput {
    pub fn kind(&self) -> Kind {
        match self {
            ItemInput::Youtube { .. } => Kind::Youtube,
            ItemInput::App { .. } => Kind::App,
            ItemInput::Web { .. } => Kind::Web,
        }
    }
}

/// 입력 창을 여는 방식
pub enum Initial {
    /// 새 항목 (유튜브 칸으로 시작)
    New,
    /// 기존 항목 수정 (종류 고정)
    Edit(ItemInput),
}

struct DialogState {
    hwnd: HWND,
    kind: Kind,
    /// 종류를 오갈 때 칸에 쓴 글을 기억한다
    youtube_text: String,
    web_text: String,
    app: Option<AppChoice>,
    result: Option<ItemInput>,
    done: bool,
}

struct DialogWindow {
    state: Rc<RefCell<DialogState>>,
}

pub fn window_text(hwnd: HWND) -> String {
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    let mut buf = vec![0u16; len.max(0) as usize + 1];
    let n = unsafe { GetWindowTextW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

fn item(dialog: HWND, id: i32) -> HWND {
    unsafe { GetDlgItem(Some(dialog), id) }.unwrap_or_default()
}

fn set_text(hwnd: HWND, text: &str) {
    let _ = unsafe { SetWindowTextW(hwnd, &HSTRING::from(text)) };
}

fn show(hwnd: HWND, visible: bool) {
    let _ = unsafe { ShowWindow(hwnd, if visible { SW_SHOW } else { SW_HIDE }) };
}

pub fn warn(owner: HWND, text: &str) {
    unsafe { MessageBoxW(Some(owner), &HSTRING::from(text), w!("YT Launcher"), MB_OK | MB_ICONWARNING) };
}

impl DialogState {
    /// 다른 종류로 바꾼다: 지금 칸에 쓴 글을 기억해 두고 입력칸을 바꾼다.
    fn apply_kind(&mut self, kind: Kind) {
        let main = item(self.hwnd, ID_MAIN);
        match self.kind {
            Kind::Youtube => self.youtube_text = window_text(main),
            Kind::Web => self.web_text = window_text(main),
            Kind::App => {}
        }
        self.kind = kind;
        self.show_kind();
    }

    /// 지금 종류에 맞게 버튼·입력칸·안내 문구를 보여 준다.
    fn show_kind(&mut self) {
        let (dialog, kind) = (self.hwnd, self.kind);
        let main = item(dialog, ID_MAIN);
        for k in [Kind::Youtube, Kind::App, Kind::Web] {
            let check = if k == kind { BST_CHECKED } else { BST_UNCHECKED };
            unsafe { SendMessageW(item(dialog, k.button_id()), BM_SETCHECK, Some(WPARAM(check.0 as usize)), None) };
        }
        set_text(item(dialog, ID_MAIN_LABEL), kind.main_label());
        set_text(item(dialog, ID_HINT), kind.hint());
        let app = kind == Kind::App;
        show(main, !app);
        show(item(dialog, ID_PICK), app);
        show(item(dialog, ID_PICKED), app);
        match kind {
            Kind::Youtube => set_text(main, &self.youtube_text),
            Kind::Web => set_text(main, &self.web_text),
            Kind::App => {
                let picked = self.app.as_ref().map_or("선택한 앱이 없습니다", |a| a.name.as_str());
                set_text(item(dialog, ID_PICKED), picked);
            }
        }
    }

    /// 저장 조건을 확인한다. 통과하면 결과, 아니면 (경고 문구, 포커스 줄 칸).
    fn collect(&self) -> Result<ItemInput, (&'static str, i32)> {
        let main = window_text(item(self.hwnd, ID_MAIN)).trim().to_string();
        let name = window_text(item(self.hwnd, ID_NAME)).trim().to_string();
        match self.kind {
            Kind::Youtube if main.is_empty() => Err(("검색어를 입력하세요.", ID_MAIN)),
            Kind::Youtube => Ok(ItemInput::Youtube { query: main, name }),
            Kind::App => match &self.app {
                Some(app) => Ok(ItemInput::App { app: app.clone(), name }),
                None => Err(("앱을 선택하세요.", ID_PICK)),
            },
            Kind::Web => match normalize_url(&main) {
                Ok(url) => Ok(ItemInput::Web { url, name }),
                Err(_) => Err(("올바른 주소를 입력하세요. 예: naver.com", ID_MAIN)),
            },
        }
    }
}

impl DialogWindow {
    /// 끝났다고 먼저 표시한 뒤 창을 없앤다 (핸들러 안의 DestroyWindow는 WM_DESTROY를 받지 못한다).
    fn close(&self, hwnd: HWND) {
        self.state.borrow_mut().done = true;
        let _ = unsafe { DestroyWindow(hwnd) };
    }
}

impl Handler for DialogWindow {
    fn handle(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, _lparam: LPARAM) -> Option<LRESULT> {
        match msg {
            WM_COMMAND => {
                let id = (wparam.0 & 0xFFFF) as i32;
                if id == IDOK.0 {
                    let collected = self.state.borrow().collect();
                    match collected {
                        Ok(input) => {
                            self.state.borrow_mut().result = Some(input);
                            self.close(hwnd);
                        }
                        Err((text, focus)) => {
                            warn(hwnd, text);
                            let _ = unsafe { SetFocus(Some(item(hwnd, focus))) };
                        }
                    }
                } else if id == IDCANCEL.0 {
                    self.close(hwnd);
                } else if id == ID_PICK {
                    if let Some(app) = pick_app(hwnd) {
                        let mut st = self.state.borrow_mut();
                        st.app = Some(app);
                        st.apply_kind(Kind::App);
                    }
                } else if let Some(kind) = Kind::from_id(id) {
                    self.state.borrow_mut().apply_kind(kind);
                }
                Some(LRESULT(0))
            }
            WM_ERASEBKGND | WM_CTLCOLORSTATIC => paint_background(hwnd, msg, wparam),
            WM_CLOSE => {
                self.close(hwnd);
                Some(LRESULT(0))
            }
            _ => None,
        }
    }
}

/// 이 DPI에 맞춘 시스템 메시지 글꼴. (SystemParametersInfoW는 이미 시스템 DPI로 키운 값을 주므로
/// 거기에 다시 배율을 곱하면 글자가 너무 커진다.)
pub fn message_font(dpi: u32) -> HFONT {
    let mut metrics =
        NONCLIENTMETRICSW { cbSize: std::mem::size_of::<NONCLIENTMETRICSW>() as u32, ..Default::default() };
    // SAFETY: 구조체 크기를 넘겨 이 DPI의 시스템 메시지 글꼴을 읽는다
    unsafe {
        let _ = SystemParametersInfoForDpi(
            SPI_GETNONCLIENTMETRICS.0,
            metrics.cbSize,
            Some((&mut metrics as *mut NONCLIENTMETRICSW).cast()),
            0,
            dpi,
        );
        CreateFontIndirectW(&metrics.lfMessageFont)
    }
}

/// 입력 창 배경을 창 색으로 칠하고, 정적 글자 배경도 같은 색으로 맞춘다 (창 클래스에 배경 브러시가 없다).
pub fn paint_background(hwnd: HWND, msg: u32, wparam: WPARAM) -> Option<LRESULT> {
    // SAFETY: 받은 DC와 시스템 브러시만 쓴다
    unsafe {
        match msg {
            WM_ERASEBKGND => {
                let mut r = RECT::default();
                let _ = GetClientRect(hwnd, &mut r);
                FillRect(HDC(wparam.0 as *mut _), &r, GetSysColorBrush(COLOR_WINDOW));
                Some(LRESULT(1))
            }
            WM_CTLCOLORSTATIC => {
                let dc = HDC(wparam.0 as *mut _);
                SetBkColor(dc, COLORREF(GetSysColor(COLOR_WINDOW)));
                Some(LRESULT(GetSysColorBrush(COLOR_WINDOW).0 as isize))
            }
            _ => None,
        }
    }
}

pub struct Ctl<'a> {
    pub class: &'a str,
    pub text: &'a str,
    pub style: WINDOW_STYLE,
    pub ex: WINDOW_EX_STYLE,
    pub rect: (i32, i32, i32, i32),
    pub id: i32,
}

impl<'a> Ctl<'a> {
    pub fn label(text: &'a str, rect: (i32, i32, i32, i32), id: i32) -> Self {
        Ctl { class: "STATIC", text, style: WINDOW_STYLE(SS_ENDELLIPSIS), ex: WINDOW_EX_STYLE(0), rect, id }
    }

    pub fn edit(text: &'a str, rect: (i32, i32, i32, i32), id: i32) -> Self {
        Ctl {
            class: "EDIT",
            text,
            style: WINDOW_STYLE(ES_AUTOHSCROLL as u32) | WS_TABSTOP,
            ex: WS_EX_CLIENTEDGE,
            rect,
            id,
        }
    }

    pub fn button(text: &'a str, rect: (i32, i32, i32, i32), id: i32, default: bool) -> Self {
        let kind = if default { BS_DEFPUSHBUTTON } else { BS_PUSHBUTTON };
        Ctl { class: "BUTTON", text, style: WINDOW_STYLE(kind as u32) | WS_TABSTOP, ex: WINDOW_EX_STYLE(0), rect, id }
    }
}

pub fn child(parent: HWND, font: HFONT, c: Ctl) -> HWND {
    let (class, text, style, ex, id) = (c.class, c.text, c.style, c.ex, c.id);
    let (x, y, w, h) = c.rect;
    // SAFETY: 부모 창 안에 기본 컨트롤을 만든다
    unsafe {
        let hwnd = CreateWindowExW(
            ex,
            &HSTRING::from(class),
            &HSTRING::from(text),
            WS_CHILD | WS_VISIBLE | style,
            x,
            y,
            w,
            h,
            Some(parent),
            Some(HMENU(id as isize as *mut _)),
            None,
            None,
        )
        .unwrap_or_default();
        SendMessageW(hwnd, WM_SETFONT, Some(WPARAM(font.0 as usize)), Some(LPARAM(1)));
        hwnd
    }
}

/// 소유 창 가운데에 놓을 (x, y). 화면 아래 독처럼 소유 창이 가장자리에 있어도 작업 영역 안에 들어오게 한다.
pub fn centered_on(owner: HWND, w: i32, h: i32) -> (i32, i32) {
    let mut r = RECT::default();
    let _ = unsafe { GetWindowRect(owner, &mut r) };
    let (x, y) = ((r.left + r.right) / 2 - w / 2, (r.top + r.bottom) / 2 - h / 2);
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    // SAFETY: 소유 창이 있는 모니터의 작업 영역을 읽는다
    if !unsafe { GetMonitorInfoW(MonitorFromWindow(owner, MONITOR_DEFAULTTONEAREST), &mut info) }.as_bool() {
        return (x.max(0), y.max(0));
    }
    clamp_into(x, y, w, h, info.rcWork)
}

/// (x, y, w, h) 창을 `area` 안으로 옮긴다 (순수 함수)
pub fn clamp_into(x: i32, y: i32, w: i32, h: i32, area: RECT) -> (i32, i32) {
    let x = x.min(area.right - w).max(area.left);
    let y = y.min(area.bottom - h).max(area.top);
    (x, y)
}

/// 모달: 소유 창을 막고 `done()`이 참이 될 때까지 메시지를 돌린다.
pub fn run_modal(owner: HWND, dialog: HWND, focus: HWND, done: &dyn Fn() -> bool) {
    unsafe {
        let _ = EnableWindow(owner, false);
        let _ = ShowWindow(dialog, SW_SHOW);
        let _ = SetForegroundWindow(dialog);
        let _ = SetFocus(Some(focus));
        let mut msg = MSG::default();
        while !done() {
            if !GetMessageW(&mut msg, None, 0, 0).as_bool() {
                // WM_QUIT은 바깥 루프가 받도록 되돌려 놓는다
                PostQuitMessage(msg.wParam.0 as i32);
                break;
            }
            if msg.message == WM_QUIT || IsDialogMessageW(dialog, &msg).as_bool() {
                continue;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = EnableWindow(owner, true);
        let _ = SetForegroundWindow(owner);
    }
}

/// 항목 입력 창을 띄우고 닫힐 때까지 기다린다. 저장하면 `Some`, 취소하면 `None`.
pub fn item_dialog(owner: HWND, title: &str, initial: &Initial) -> Option<ItemInput> {
    let dpi = unsafe { GetDpiForWindow(owner) }.max(96);
    let s = |v: i32| v * dpi as i32 / 96;
    let (w, h) = (s(412), s(262));
    let (x, y) = centered_on(owner, w, h);
    let (kind, fixed) = match initial {
        Initial::New => (Kind::Youtube, false),
        Initial::Edit(input) => (input.kind(), true),
    };
    let (mut youtube_text, mut web_text, mut app, mut name) = (String::new(), String::new(), None, String::new());
    if let Initial::Edit(input) = initial {
        match input.clone() {
            ItemInput::Youtube { query, name: n } => (youtube_text, name) = (query, n),
            ItemInput::App { app: a, name: n } => (app, name) = (Some(a), n),
            ItemInput::Web { url, name: n } => (web_text, name) = (url, n),
        }
    }

    let spec = WindowSpec {
        class: DIALOG_CLASS,
        title,
        ex_style: WS_EX_DLGMODALFRAME,
        style: WS_POPUP | WS_CAPTION | WS_SYSMENU,
        rect: (x, y, w, h),
        owner: Some(owner),
    };
    let state = Rc::new(RefCell::new(DialogState {
        hwnd: HWND::default(),
        kind: Kind::Youtube,
        youtube_text,
        web_text,
        app,
        result: None,
        done: false,
    }));
    let dialog = create_window(&spec, Box::new(DialogWindow { state: state.clone() })).ok()?;
    let font = message_font(dpi);
    let kind_style = WINDOW_STYLE((BS_AUTORADIOBUTTON | BS_PUSHLIKE) as u32) | WS_TABSTOP;
    for (i, k) in [Kind::Youtube, Kind::App, Kind::Web].into_iter().enumerate() {
        let text = ["유튜브", "앱", "웹 주소"][i];
        let group = if i == 0 { WS_GROUP } else { WINDOW_STYLE(0) };
        let button = child(
            dialog,
            font,
            Ctl {
                class: "BUTTON",
                text,
                style: kind_style | group,
                ex: WINDOW_EX_STYLE(0),
                rect: (s(16 + 128 * i as i32), s(14), s(120), s(30)),
                id: k.button_id(),
            },
        );
        let _ = unsafe { EnableWindow(button, !fixed || k == kind) };
    }
    child(dialog, font, Ctl::label("", (s(16), s(66), s(80), s(20)), ID_MAIN_LABEL));
    let main = child(dialog, font, Ctl::edit("", (s(100), s(62), s(288), s(26)), ID_MAIN));
    child(dialog, font, Ctl::button("앱 선택…", (s(100), s(60), s(96), s(30)), ID_PICK, false));
    child(dialog, font, Ctl::label("", (s(206), s(66), s(182), s(20)), ID_PICKED));
    child(dialog, font, Ctl::label("표시 이름", (s(16), s(110), s(80), s(20)), -1));
    child(dialog, font, Ctl::edit(&name, (s(100), s(106), s(288), s(26)), ID_NAME));
    child(dialog, font, Ctl::label("", (s(100), s(136), s(288), s(20)), ID_HINT));
    child(dialog, font, Ctl::button("저장", (s(208), s(176), s(86), s(30)), IDOK.0, true));
    child(dialog, font, Ctl::button("취소", (s(302), s(176), s(86), s(30)), IDCANCEL.0, false));
    {
        let mut st = state.borrow_mut();
        st.hwnd = dialog;
        st.kind = kind;
        st.show_kind();
    }
    let focus = if kind == Kind::App { item(dialog, ID_PICK) } else { main };
    run_modal(owner, dialog, focus, &|| state.borrow().done);
    let _ = unsafe { DeleteObject(HGDIOBJ(font.0)) };
    state.borrow_mut().result.take()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::win::msg_window::create;
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW, WM_COMMAND};

    struct Owner;
    impl Handler for Owner {
        fn handle(&mut self, _: HWND, _: u32, _: WPARAM, _: LPARAM) -> Option<LRESULT> {
            None
        }
    }

    /// 다른 스레드에서 입력 창을 찾아 버튼 명령들을 보낸다.
    fn press_later(ids: &[i32]) {
        let ids = ids.to_vec();
        std::thread::spawn(move || {
            for _ in 0..100 {
                std::thread::sleep(std::time::Duration::from_millis(30));
                if let Ok(h) = unsafe { FindWindowW(&HSTRING::from(DIALOG_CLASS), None) }
                    && !h.0.is_null()
                {
                    for id in ids {
                        let _ = unsafe { PostMessageW(Some(h), WM_COMMAND, WPARAM(id as usize), LPARAM(0)) };
                    }
                    return;
                }
            }
        });
    }

    /// 입력 창 테스트는 같은 창 클래스를 찾으므로 한 번에 하나씩 돌린다
    pub static UI_TEST: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        UI_TEST.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn dialog_is_kept_inside_work_area() {
        let area = RECT { left: 0, top: 0, right: 1920, bottom: 1040 };
        assert_eq!(clamp_into(800, 900, 400, 300, area), (800, 740));
        assert_eq!(clamp_into(-50, -20, 400, 300, area), (0, 0));
        assert_eq!(clamp_into(1700, 100, 400, 300, area), (1520, 100));
    }

    fn edit(input: ItemInput) -> Initial {
        Initial::Edit(input)
    }

    #[test]
    fn save_returns_trimmed_input_and_cancel_returns_none() {
        let _guard = lock();
        let owner = create("YTLauncherTestDialogOwner", Box::new(Owner)).unwrap();
        press_later(&[IDOK.0]);
        let initial = edit(ItemInput::Youtube { query: "  로파이 음악 ".into(), name: " 공부 ".into() });
        let saved = item_dialog(owner, "항목 수정", &initial);
        assert_eq!(saved, Some(ItemInput::Youtube { query: "로파이 음악".into(), name: "공부".into() }));
        press_later(&[IDCANCEL.0]);
        assert_eq!(item_dialog(owner, "항목 수정", &initial), None);
    }

    #[test]
    fn web_address_is_normalized_and_app_edit_keeps_choice() {
        let _guard = lock();
        let owner = create("YTLauncherTestDialogOwner2", Box::new(Owner)).unwrap();
        press_later(&[IDOK.0]);
        let web = edit(ItemInput::Web { url: " naver.com ".into(), name: "".into() });
        assert_eq!(
            item_dialog(owner, "항목 수정", &web),
            Some(ItemInput::Web { url: "https://naver.com".into(), name: "".into() })
        );
        let app = AppChoice { name: "계산기".into(), target: "Calc!App".into(), exe: None };
        press_later(&[IDOK.0]);
        let saved = item_dialog(owner, "항목 수정", &edit(ItemInput::App { app: app.clone(), name: "".into() }));
        assert_eq!(saved, Some(ItemInput::App { app, name: "".into() }));
    }

    #[test]
    fn switching_kind_keeps_typed_text_per_kind() {
        let _guard = lock();
        let owner = create("YTLauncherTestDialogOwner3", Box::new(Owner)).unwrap();
        // 새 항목: 유튜브 칸(비어 있음) → 웹으로 바꿔도 빈 주소라 저장 안 됨 → 취소
        press_later(&[ID_KIND_WEB, IDCANCEL.0]);
        assert_eq!(item_dialog(owner, "항목 추가", &Initial::New), None);
    }
}
```

- [ ] **Step 2: src/ui/app_list.rs 작성**

```rust
//! 앱 선택 창: 시작 메뉴 "모든 앱" 목록(검색, 아이콘) + exe/lnk 직접 찾기.
//!
//! 목록은 백그라운드 스레드가 읽고, 아이콘은 화면에 보이는 줄이 요청할 때만 같은 스레드가 하나씩 만든다.

use crate::ui::dialogs::{Ctl, centered_on, child, message_font, paint_background, run_modal, warn, window_text};
use crate::win::msg_window::{Handler, WindowSpec, create_window};
use crate::win::shell_items::{AppEntry, apps_folder_path, file_app, item_image, list_apps};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{DeleteObject, HBITMAP, HGDIOBJ, InvalidateRect};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_NODEREFERENCELINKS, OPENFILENAMEW,
};
use windows::Win32::UI::Controls::{
    HIMAGELIST, I_IMAGENONE, ICC_LISTVIEW_CLASSES, ILC_COLOR32, INITCOMMONCONTROLSEX, ImageList_Add, ImageList_Create,
    ImageList_Destroy, InitCommonControlsEx, LIST_VIEW_ITEM_STATE_FLAGS, LVCF_WIDTH, LVCOLUMNW, LVIF_IMAGE, LVIF_TEXT,
    LVIS_FOCUSED, LVIS_SELECTED, LVITEMW, LVM_ENSUREVISIBLE, LVM_GETNEXTITEM, LVM_INSERTCOLUMNW,
    LVM_SETEXTENDEDLISTVIEWSTYLE, LVM_SETIMAGELIST, LVM_SETITEMCOUNT, LVM_SETITEMSTATE, LVN_GETDISPINFOW,
    LVNI_SELECTED, LVS_EX_DOUBLEBUFFER, LVS_EX_FULLROWSELECT, LVS_NOCOLUMNHEADER, LVS_OWNERDATA, LVS_REPORT,
    LVS_SHOWSELALWAYS, LVS_SINGLESEL, LVSIL_SMALL, NM_DBLCLK, NMHDR, NMLVDISPINFOW,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, EN_CHANGE, GetDlgItem, IDCANCEL, IDOK, PostMessageW, SW_HIDE, SW_SHOW, SendMessageW, SetWindowTextW,
    ShowWindow, WINDOW_STYLE, WM_APP, WM_CLOSE, WM_COMMAND, WM_CTLCOLORSTATIC, WM_DESTROY, WM_ERASEBKGND, WM_NOTIFY,
    WS_BORDER, WS_CAPTION, WS_EX_CLIENTEDGE, WS_EX_DLGMODALFRAME, WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_VSCROLL,
};
use windows::core::{HSTRING, PWSTR};

pub use crate::win::shell_items::AppEntry as AppChoice;

const PICKER_CLASS: &str = "YTLauncherAppPicker";
const ID_SEARCH: i32 = 201;
const ID_LIST: i32 = 202;
const ID_STATUS: i32 = 203;
const ID_BROWSE: i32 = 204;
/// 백그라운드 스레드 결과가 채널에 들어왔음
const WM_APP_LOADED: u32 = WM_APP + 21;

/// 검색어에 맞는 앱의 인덱스 (이름 부분 일치, 대소문자 무시). 순수 함수.
pub fn filter_apps(apps: &[AppEntry], query: &str) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    apps.iter()
        .enumerate()
        .filter(|(_, a)| q.is_empty() || a.name.to_lowercase().contains(&q))
        .map(|(i, _)| i)
        .collect()
}

enum Loaded {
    Apps(Result<Vec<AppEntry>, String>),
    /// (앱 인덱스, 아이콘 비트맵 핸들)
    Icon(usize, isize),
}

struct PickerState {
    hwnd: HWND,
    list: HWND,
    apps: Vec<AppEntry>,
    visible: Vec<usize>,
    /// 앱마다 이미지 목록 안 번호 (None = 아직 없음)
    icons: Vec<Option<i32>>,
    requested: Vec<bool>,
    images: HIMAGELIST,
    requests: Option<Sender<(usize, String)>>,
    loaded: Receiver<Loaded>,
    text: Vec<u16>,
    result: Option<AppChoice>,
    done: bool,
}

impl PickerState {
    fn status(&self, text: Option<&str>) {
        let status = unsafe { GetDlgItem(Some(self.hwnd), ID_STATUS) }.unwrap_or_default();
        if let Some(text) = text {
            let _ = unsafe { SetWindowTextW(status, &HSTRING::from(text)) };
        }
        let _ = unsafe { ShowWindow(status, if text.is_some() { SW_SHOW } else { SW_HIDE }) };
    }

    fn refilter(&mut self) {
        let search = unsafe { GetDlgItem(Some(self.hwnd), ID_SEARCH) }.unwrap_or_default();
        self.visible = filter_apps(&self.apps, &window_text(search));
        unsafe {
            SendMessageW(self.list, LVM_SETITEMCOUNT, Some(WPARAM(self.visible.len())), Some(LPARAM(0)));
            let _ = InvalidateRect(Some(self.list), None, true);
        }
        if !self.visible.is_empty() {
            self.select_row(0);
        }
        self.status(self.visible.is_empty().then_some("찾는 앱이 없습니다."));
    }

    fn select_row(&self, row: usize) {
        let state = LIST_VIEW_ITEM_STATE_FLAGS(LVIS_SELECTED.0 | LVIS_FOCUSED.0);
        let mut item = LVITEMW { stateMask: state, state, ..Default::default() };
        unsafe {
            SendMessageW(self.list, LVM_SETITEMSTATE, Some(WPARAM(row)), Some(LPARAM(&mut item as *mut _ as isize)));
            SendMessageW(self.list, LVM_ENSUREVISIBLE, Some(WPARAM(row)), Some(LPARAM(0)));
        }
    }

    fn selected(&self) -> Option<AppEntry> {
        let row = unsafe {
            SendMessageW(self.list, LVM_GETNEXTITEM, Some(WPARAM(usize::MAX)), Some(LPARAM(LVNI_SELECTED as isize)))
        };
        let row = usize::try_from(row.0).ok()?;
        self.visible.get(row).map(|&i| self.apps[i].clone())
    }

    fn on_loaded(&mut self) {
        while let Ok(msg) = self.loaded.try_recv() {
            match msg {
                Loaded::Apps(Ok(apps)) => {
                    self.icons = vec![None; apps.len()];
                    self.requested = vec![false; apps.len()];
                    self.apps = apps;
                    self.refilter();
                }
                Loaded::Apps(Err(e)) => {
                    self.status(Some(&format!("앱 목록을 불러오지 못했습니다. [직접 찾기]를 쓰세요.\n({e})")));
                }
                Loaded::Icon(index, bitmap) => {
                    let bitmap = HBITMAP(bitmap as *mut _);
                    // SAFETY: 받은 비트맵을 이미지 목록에 복사하고 원본은 지운다
                    let slot = unsafe {
                        let slot = ImageList_Add(self.images, bitmap, None);
                        let _ = DeleteObject(HGDIOBJ(bitmap.0));
                        slot
                    };
                    if slot >= 0 && index < self.icons.len() {
                        self.icons[index] = Some(slot);
                        let _ = unsafe { InvalidateRect(Some(self.list), None, false) };
                    }
                }
            }
        }
    }

    /// 목록이 한 줄을 그리려 할 때: 이름을 주고, 아이콘이 없으면 만들어 달라고 요청한다.
    fn display_info(&mut self, info: &mut NMLVDISPINFOW) {
        let row = info.item.iItem as usize;
        let Some(&index) = self.visible.get(row) else { return };
        if info.item.mask.contains(LVIF_TEXT) && !info.item.pszText.is_null() && info.item.cchTextMax > 0 {
            self.text = self.apps[index].name.encode_utf16().take(info.item.cchTextMax as usize - 1).collect();
            self.text.push(0);
            // SAFETY: 목록이 준 버퍼 크기 안에서 복사한다
            unsafe { std::ptr::copy_nonoverlapping(self.text.as_ptr(), info.item.pszText.0, self.text.len()) };
        }
        if info.item.mask.contains(LVIF_IMAGE) {
            info.item.iImage = self.icons[index].unwrap_or(I_IMAGENONE);
            if self.icons[index].is_none() && !self.requested[index] {
                self.requested[index] = true;
                if let Some(tx) = &self.requests {
                    let _ = tx.send((index, self.apps[index].target.clone()));
                }
            }
        }
    }
}

struct PickerWindow {
    state: Rc<RefCell<PickerState>>,
}

impl PickerWindow {
    fn finish(&self, hwnd: HWND, result: Option<AppChoice>) {
        {
            let mut st = self.state.borrow_mut();
            st.result = result;
            st.done = true;
            st.requests = None;
        }
        let _ = unsafe { DestroyWindow(hwnd) };
    }
}

fn browse(owner: HWND) -> Option<AppChoice> {
    let filter: Vec<u16> = "프로그램 (*.exe, *.lnk)\0*.exe;*.lnk\0모든 파일\0*.*\0\0".encode_utf16().collect();
    let mut file = vec![0u16; 1024];
    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner,
        lpstrFilter: windows::core::PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(file.as_mut_ptr()),
        nMaxFile: file.len() as u32,
        // 바로가기(.lnk)를 고르면 대상이 아니라 바로가기 자체를 받는다
        Flags: OFN_FILEMUSTEXIST | OFN_NODEREFERENCELINKS,
        ..Default::default()
    };
    // SAFETY: 버퍼와 필터는 호출이 끝날 때까지 살아 있다
    if !unsafe { GetOpenFileNameW(&mut ofn) }.as_bool() {
        return None;
    }
    let len = file.iter().position(|&c| c == 0).unwrap_or(file.len());
    Some(file_app(&String::from_utf16_lossy(&file[..len])))
}

impl Handler for PickerWindow {
    fn handle(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        match msg {
            WM_COMMAND => {
                let id = (wparam.0 & 0xFFFF) as i32;
                let code = (wparam.0 >> 16) as u32;
                if id == IDOK.0 {
                    let (selected, only) = {
                        let st = self.state.borrow();
                        let only = (st.visible.len() == 1).then(|| st.apps[st.visible[0]].clone());
                        (st.selected(), only)
                    };
                    match selected.or(only) {
                        Some(app) => self.finish(hwnd, Some(app)),
                        None => warn(hwnd, "목록에서 앱을 고르세요."),
                    }
                } else if id == IDCANCEL.0 {
                    self.finish(hwnd, None);
                } else if id == ID_BROWSE {
                    if let Some(app) = browse(hwnd) {
                        self.finish(hwnd, Some(app));
                    }
                } else if id == ID_SEARCH && code == EN_CHANGE {
                    self.state.borrow_mut().refilter();
                }
                Some(LRESULT(0))
            }
            WM_NOTIFY => {
                // SAFETY: WM_NOTIFY의 lparam은 NMHDR로 시작하는 알림 구조체
                let header = unsafe { &*(lparam.0 as *const NMHDR) };
                if header.idFrom != ID_LIST as usize {
                    return None;
                }
                if header.code == LVN_GETDISPINFOW {
                    let info = unsafe { &mut *(lparam.0 as *mut NMLVDISPINFOW) };
                    self.state.borrow_mut().display_info(info);
                } else if header.code == NM_DBLCLK {
                    let selected = self.state.borrow().selected();
                    if selected.is_some() {
                        self.finish(hwnd, selected);
                    }
                }
                Some(LRESULT(0))
            }
            WM_APP_LOADED => {
                self.state.borrow_mut().on_loaded();
                Some(LRESULT(0))
            }
            WM_ERASEBKGND | WM_CTLCOLORSTATIC => paint_background(hwnd, msg, wparam),
            WM_CLOSE => {
                self.finish(hwnd, None);
                Some(LRESULT(0))
            }
            WM_DESTROY => None,
            _ => None,
        }
    }
}

/// 백그라운드 스레드: 앱 목록을 읽어 보내고, 요청받은 아이콘을 하나씩 만든다. 요청 채널이 닫히면 끝난다.
fn spawn_loader(hwnd: HWND, px: i32) -> (Sender<(usize, String)>, Receiver<Loaded>) {
    let (req_tx, req_rx) = channel::<(usize, String)>();
    let (out_tx, out_rx) = channel();
    let hwnd = hwnd.0 as isize;
    std::thread::spawn(move || {
        let post = |m: Loaded| {
            if out_tx.send(m).is_ok() {
                let _ = unsafe { PostMessageW(Some(HWND(hwnd as *mut _)), WM_APP_LOADED, WPARAM(0), LPARAM(0)) };
            }
        };
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        post(Loaded::Apps(list_apps().map_err(|e| e.message())));
        while let Ok((index, target)) = req_rx.recv() {
            if let Some(bitmap) = item_image(&apps_folder_path(&target), px) {
                post(Loaded::Icon(index, bitmap.0 as isize));
            }
        }
        unsafe { CoUninitialize() };
    });
    (req_tx, out_rx)
}

/// 앱 선택 창을 띄우고 닫힐 때까지 기다린다. 고르면 `Some`.
pub fn pick_app(owner: HWND) -> Option<AppChoice> {
    let dpi = unsafe { GetDpiForWindow(owner) }.max(96);
    let s = |v: i32| v * dpi as i32 / 96;
    let (w, h) = (s(436), s(540));
    let (x, y) = centered_on(owner, w, h);
    let icc = INITCOMMONCONTROLSEX {
        dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
        dwICC: ICC_LISTVIEW_CLASSES,
    };
    let _ = unsafe { InitCommonControlsEx(&icc) };
    let icon_px = s(32);
    let images = unsafe { ImageList_Create(icon_px, icon_px, ILC_COLOR32, 64, 64) };
    let (_, placeholder) = channel();
    let state = Rc::new(RefCell::new(PickerState {
        hwnd: HWND::default(),
        list: HWND::default(),
        apps: Vec::new(),
        visible: Vec::new(),
        icons: Vec::new(),
        requested: Vec::new(),
        images,
        requests: None,
        loaded: placeholder,
        text: Vec::new(),
        result: None,
        done: false,
    }));
    let spec = WindowSpec {
        class: PICKER_CLASS,
        title: "앱 선택",
        ex_style: WS_EX_DLGMODALFRAME,
        style: WS_POPUP | WS_CAPTION | WS_SYSMENU,
        rect: (x, y, w, h),
        owner: Some(owner),
    };
    let picker = create_window(&spec, Box::new(PickerWindow { state: state.clone() })).ok()?;
    let font = message_font(dpi);
    let search = child(picker, font, Ctl::edit("", (s(16), s(14), s(388), s(26)), ID_SEARCH));
    let list_style = WINDOW_STYLE(LVS_REPORT | LVS_OWNERDATA | LVS_SINGLESEL | LVS_NOCOLUMNHEADER | LVS_SHOWSELALWAYS)
        | WS_TABSTOP
        | WS_VSCROLL
        | WS_BORDER;
    let list = child(
        picker,
        font,
        Ctl {
            class: "SysListView32",
            text: "",
            style: list_style,
            ex: WS_EX_CLIENTEDGE,
            rect: (s(16), s(50), s(388), s(388)),
            id: ID_LIST,
        },
    );
    child(picker, font, Ctl::label("불러오는 중…", (s(28), s(62), s(364), s(40)), ID_STATUS));
    child(picker, font, Ctl::button("직접 찾기…", (s(16), s(452), s(110), s(30)), ID_BROWSE, false));
    child(picker, font, Ctl::button("선택", (s(224), s(452), s(86), s(30)), IDOK.0, true));
    child(picker, font, Ctl::button("취소", (s(318), s(452), s(86), s(30)), IDCANCEL.0, false));
    unsafe {
        let ex = (LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER) as isize;
        SendMessageW(list, LVM_SETEXTENDEDLISTVIEWSTYLE, Some(WPARAM(ex as usize)), Some(LPARAM(ex)));
        SendMessageW(list, LVM_SETIMAGELIST, Some(WPARAM(LVSIL_SMALL as usize)), Some(LPARAM(images.0)));
        let column = LVCOLUMNW { mask: LVCF_WIDTH, cx: s(360), ..Default::default() };
        SendMessageW(list, LVM_INSERTCOLUMNW, Some(WPARAM(0)), Some(LPARAM(&column as *const _ as isize)));
    }
    let (requests, loaded) = spawn_loader(picker, icon_px);
    {
        let mut st = state.borrow_mut();
        st.hwnd = picker;
        st.list = list;
        st.requests = Some(requests);
        st.loaded = loaded;
    }
    run_modal(owner, picker, search, &|| state.borrow().done);
    let mut st = state.borrow_mut();
    st.requests = None;
    // 아직 오지 않은 아이콘 비트맵은 정리한다
    while let Ok(msg) = st.loaded.try_recv() {
        if let Loaded::Icon(_, bitmap) = msg {
            let _ = unsafe { DeleteObject(HGDIOBJ(bitmap as *mut _)) };
        }
    }
    let _ = unsafe { ImageList_Destroy(Some(st.images)) };
    let _ = unsafe { DeleteObject(HGDIOBJ(font.0)) };
    st.result.take()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::win::msg_window::create;
    use windows::Win32::UI::Controls::LVM_GETITEMCOUNT;
    use windows::Win32::UI::WindowsAndMessaging::FindWindowW;

    fn app(name: &str) -> AppEntry {
        AppEntry { name: name.into(), target: name.into(), exe: None }
    }

    #[test]
    fn filters_by_name_case_insensitively() {
        let apps = vec![app("Google Chrome"), app("계산기"), app("KakaoTalk"), app("Chrome Remote Desktop")];
        assert_eq!(filter_apps(&apps, ""), vec![0, 1, 2, 3]);
        assert_eq!(filter_apps(&apps, "  chrome "), vec![0, 3]);
        assert_eq!(filter_apps(&apps, "계산"), vec![1]);
        assert!(filter_apps(&apps, "없는앱").is_empty());
    }

    struct Owner;
    impl Handler for Owner {
        fn handle(&mut self, _: HWND, _: u32, _: WPARAM, _: LPARAM) -> Option<LRESULT> {
            None
        }
    }

    #[test]
    fn loads_list_and_picks_selected_app() {
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let owner = create("YTLauncherTestPickerOwner", Box::new(Owner)).unwrap();
        // 목록이 채워지면 [선택]을 누른다: 첫 줄이 자동으로 선택돼 있다
        std::thread::spawn(|| {
            for _ in 0..400 {
                std::thread::sleep(std::time::Duration::from_millis(50));
                let Ok(h) = (unsafe { FindWindowW(&HSTRING::from(PICKER_CLASS), None) }) else { continue };
                let list = unsafe { GetDlgItem(Some(h), ID_LIST) }.unwrap_or_default();
                let count = unsafe { SendMessageW(list, LVM_GETITEMCOUNT, None, None) }.0;
                if !h.0.is_null() && count > 0 {
                    let _ = unsafe { PostMessageW(Some(h), WM_COMMAND, WPARAM(IDOK.0 as usize), LPARAM(0)) };
                    return;
                }
            }
        });
        let picked = pick_app(owner).expect("첫 앱이 선택돼야 한다");
        let first = list_apps().unwrap().into_iter().next().unwrap();
        assert_eq!(picked, first);
    }
}
```

- [ ] **Step 3: src/ui/mod.rs 작성**

```rust
//! 화면: 테마, 배치, 애니메이션, 그리기 장치, 장면, 패널 창, 입력 창.

pub mod anim;
pub mod app_list;
pub mod dialogs;
pub mod dock_layout;
pub mod gfx;
pub mod layout;
pub mod panel;
pub mod scene;
pub mod theme;
```

- [ ] **Step 4: src/items.rs 작성**

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
    #[cfg(test)]
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

    /// 테스트용 생성 도우미 (앱에서는 입력 창이 값을 검사한 뒤 `app::item_from_input`으로 만든다)
    #[cfg(test)]
    pub fn youtube(query: &str, name: &str) -> Result<Item, ItemError> {
        let query = query.trim();
        if query.is_empty() {
            return Err(ItemError::EmptyQuery);
        }
        Ok(Item::Youtube { id: new_id(), query: query.to_string(), name: name.trim().to_string() })
    }

    #[cfg(test)]
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

- [ ] **Step 5: src/app.rs 작성**

```rust
//! 앱 컨트롤러: 설정·트레이·패널 화면·항목 실행·업데이트·자동 실행을 연결한다.

use crate::apps::{self, AppKey};
use crate::autostart::{AutoStart, RegistryRun};
use crate::chrome;
use crate::config::{Config, DockEdge, LoadReport, Mode, Storage, Theme};
use crate::icons::{FETCH_ARG, icon_path, missing_icons};
use crate::items::{Item, new_id};
use crate::logging;
use crate::tray::{Tray, WM_APP_TRAY};
use crate::ui::app_list::AppChoice;
use crate::ui::dialogs::{Initial, ItemInput, item_dialog};
use crate::ui::panel::{OpenSpec, Panel, PanelCmd, PanelHandle, UiEvent, initial_rect};
use crate::ui::scene::{TileKind, TileView, badge_of};
use crate::updater::{self, Release, UreqHttp};
use crate::win::instance::WM_APP_SHOW;
use crate::win::menu::{MenuEntry, show_popup};
use crate::win::msg_window::Handler;
use crate::win::shell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    CHILDID_SELF, DestroyWindow, EVENT_OBJECT_HIDE, EVENT_OBJECT_SHOW, GA_ROOT, GetAncestor, IDYES, KillTimer,
    MB_ICONQUESTION, MB_YESNO, MessageBoxW, OBJID_WINDOW, PostMessageW, PostQuitMessage, RegisterWindowMessageW,
    SetTimer, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_APP, WM_CONTEXTMENU, WM_DESTROY, WM_LBUTTONUP,
    WM_RBUTTONUP, WM_TIMER,
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
/// 창이 생기거나 없어진 뒤 실행 중 점을 다시 계산 (짧게 모아서 한 번, 제목이 늦게 붙는 창을 위해 한 번 더)
const TIMER_RUNNING: usize = 3;
const TIMER_RUNNING_LATE: usize = 4;
const RUNNING_DEBOUNCE_MS: u32 = 300;
const RUNNING_LATE_MS: u32 = 2000;
const FIRST_CHECK_MS: u32 = 30_000;
const DAILY_CHECK_MS: u32 = 24 * 60 * 60 * 1000;
const CLICK_DEBOUNCE: Duration = Duration::from_secs(1);

const CMD_SHOW: u32 = 20;
const CMD_MODE_DOCK: u32 = 30;
const CMD_MODE_PANEL: u32 = 31;
const CMD_EDGE_BOTTOM: u32 = 32;
const CMD_EDGE_TOP: u32 = 33;
const CMD_EDGE_LEFT: u32 = 34;
const CMD_EDGE_RIGHT: u32 = 35;
const CMD_RESET_GEOMETRY: u32 = 21;
const CMD_LOCK: u32 = 22;
const CMD_AUTOSTART: u32 = 10;
const CMD_CHECK_UPDATE: u32 = 11;
const CMD_INSTALL_UPDATE: u32 = 12;
const CMD_RELEASES: u32 = 13;
const CMD_QUIT: u32 = 14;

pub enum Event {
    Checked {
        manual: bool,
        result: Result<Option<Release>, String>,
    },
    Downloaded(Result<PathBuf, String>),
    Ui(UiEvent),
    Opened(chrome::Opened),
    /// 아이콘 캐시에 새 파일이 생김
    IconsReady,
    /// 앱 실행 대상을 찾지 못함 (항목 id)
    LaunchFailed(String),
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

/// 시작할 때 띄울 알림 문구 (설정 손상 복구, 업데이트 완료). 순수 함수.
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

/// 타일에 덧붙일 상태: 캐시된 아이콘 찾기, 실행 중인 앱, 실행 대상을 찾지 못한 앱 (항목 id)
pub struct TileStatus<'a> {
    pub icon_of: &'a dyn Fn(&str) -> Option<PathBuf>,
    pub running: &'a HashSet<String>,
    pub missing: &'a HashSet<String>,
}

/// 설정의 항목 → 화면에 그릴 타일 (순수 함수)
pub fn tile_views(items: &[Item], status: &TileStatus) -> Vec<TileView> {
    items
        .iter()
        .map(|item| {
            let id = item.id();
            let label = item.display_name();
            let (kind, badge) = match item {
                Item::Youtube { query, .. } => (TileKind::Youtube, badge_of(query)),
                Item::App { .. } => (TileKind::App, label.chars().take(1).collect()),
                Item::Web { .. } => (TileKind::Web, label.chars().take(1).collect::<String>().to_uppercase()),
            };
            TileView {
                id: id.to_string(),
                badge,
                kind,
                icon: if kind == TileKind::Youtube { None } else { (status.icon_of)(id) },
                running: status.running.contains(id),
                missing: status.missing.contains(id),
                label,
            }
        })
        .collect()
}

/// 입력 창 결과 → 항목 (순수 함수). 앱의 표시 이름을 비우면 앱 이름을 쓴다.
pub fn item_from_input(id: String, input: ItemInput) -> Item {
    match input {
        ItemInput::Youtube { query, name } => Item::Youtube { id, query, name },
        ItemInput::App { app, name } => {
            Item::App { id, name: if name.is_empty() { app.name } else { name }, target: app.target, exe: app.exe }
        }
        ItemInput::Web { url, name } => Item::Web { id, url, name },
    }
}

/// 항목 → 수정 창의 처음 값 (순수 함수)
pub fn input_of(item: &Item) -> ItemInput {
    match item.clone() {
        Item::Youtube { query, name, .. } => ItemInput::Youtube { query, name },
        Item::App { name, target, exe, .. } => {
            ItemInput::App { app: AppChoice { name: name.clone(), target, exe }, name }
        }
        Item::Web { url, name, .. } => ItemInput::Web { url, name },
    }
}

/// 실행 대상이 바뀌어 아이콘을 다시 받아야 하는가 (순수 함수)
pub fn icon_source_changed(before: &Item, after: &Item) -> bool {
    match (before, after) {
        (Item::App { target: a, .. }, Item::App { target: b, .. }) => a != b,
        (Item::Web { url: a, .. }, Item::Web { url: b, .. }) => a != b,
        _ => false,
    }
}

/// 실행 중 판정에 쓰는 앱 식별 정보 (앱이 아닌 항목은 None)
pub fn app_keys(items: &[Item]) -> Vec<Option<AppKey>> {
    items
        .iter()
        .map(|item| match item {
            Item::App { target, exe, .. } => Some(AppKey { target: target.clone(), exe: exe.clone() }),
            _ => None,
        })
        .collect()
}

/// 창 변화 알림을 받을 앱 창 (WinEvent 콜백은 인자를 받지 못한다)
static APP_HWND: AtomicIsize = AtomicIsize::new(0);

/// 최상위 창이 보이거나 숨으면 실행 중 점 재계산 타이머를 (다시) 건다. 앱 스레드에서 불린다.
unsafe extern "system" fn on_window_event(
    _: HWINEVENTHOOK,
    _: u32,
    hwnd: HWND,
    object: i32,
    child: i32,
    _: u32,
    _: u32,
) {
    if object != OBJID_WINDOW.0 || child != CHILDID_SELF as i32 || hwnd.0.is_null() {
        return;
    }
    // SAFETY: 핸들 값만 쓰는 호출
    unsafe {
        if GetAncestor(hwnd, GA_ROOT) != hwnd {
            return;
        }
        let app = HWND(APP_HWND.load(Ordering::Relaxed) as *mut _);
        if !app.0.is_null() {
            SetTimer(Some(app), TIMER_RUNNING, RUNNING_DEBOUNCE_MS, None);
            SetTimer(Some(app), TIMER_RUNNING_LATE, RUNNING_LATE_MS, None);
        }
    }
}

/// 화면에서 바뀐 순서(id 목록)대로 항목을 다시 늘어놓는다. 목록에 없는 항목은 뒤에 붙인다.
pub fn apply_order(items: &[Item], ids: &[String]) -> Vec<Item> {
    let mut rest: Vec<Item> = items.to_vec();
    let mut ordered = Vec::with_capacity(items.len());
    for id in ids {
        if let Some(pos) = rest.iter().position(|i| i.id() == id) {
            ordered.push(rest.remove(pos));
        }
    }
    ordered.extend(rest);
    ordered
}

/// 같은 항목을 짧은 시간 안에 두 번 누르면 무시한다.
#[derive(Default)]
pub struct Debounce {
    last: HashMap<String, Instant>,
}

impl Debounce {
    pub fn allow(&mut self, id: &str, now: Instant) -> bool {
        match self.last.get(id) {
            Some(t) if now.duration_since(*t) < CLICK_DEBOUNCE => false,
            _ => {
                self.last.insert(id.to_string(), now);
                true
            }
        }
    }
}

pub struct App {
    hwnd: HWND,
    storage: Storage,
    config: Config,
    tray: Option<Tray>,
    panel: Option<PanelHandle>,
    autostart: AutoStart<RegistryRun>,
    release: Option<Release>,
    downloading: bool,
    debounce: Debounce,
    events_tx: Sender<Event>,
    events_rx: Receiver<Event>,
    taskbar_created: u32,
    pending_notices: Vec<String>,
    running: HashSet<String>,
    missing: HashSet<String>,
    window_hook: Option<HWINEVENTHOOK>,
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
            panel: None,
            autostart: AutoStart { backend: RegistryRun::default() },
            release: None,
            downloading: false,
            debounce: Debounce::default(),
            events_tx,
            events_rx,
            taskbar_created: unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) },
            pending_notices,
            running: HashSet::new(),
            missing: HashSet::new(),
            window_hook: None,
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

    fn panel_send(&self, cmd: PanelCmd) {
        if let Some(panel) = &self.panel {
            panel.send(cmd);
        }
    }

    fn tiles(&self) -> Vec<TileView> {
        let dir = self.storage.dir.clone();
        let icon_of = move |id: &str| Some(icon_path(&dir, id)).filter(|p| p.exists());
        let status = TileStatus { icon_of: &icon_of, running: &self.running, missing: &self.missing };
        tile_views(&self.config.items, &status)
    }

    fn refresh_tiles(&self) {
        self.panel_send(PanelCmd::SetTiles(self.tiles()));
    }

    /// 캐시에 아이콘이 없는 앱·웹 항목의 아이콘을 만든다 (파비콘 실패는 다음 실행 때 다시).
    /// 셸 아이콘 추출과 HTTPS 인증서 검사는 수십 MB를 남기므로 잠깐 뜨는 하위 프로세스에 맡긴다.
    fn fetch_icons(&self, items: Vec<Item>) {
        let dir = self.storage.dir.clone();
        let todo = missing_icons(&dir, &items);
        if todo.is_empty() {
            return;
        }
        let Ok(json) = serde_json::to_string(&todo) else { return };
        let send = self.post_event();
        std::thread::spawn(move || {
            let status = std::env::current_exe()
                .and_then(|exe| std::process::Command::new(exe).arg(FETCH_ARG).arg(&dir).arg(json).status());
            match status {
                Ok(s) if s.success() => send(Event::IconsReady),
                Ok(_) => logging::info("일부 아이콘을 만들지 못했습니다 (다음 실행 때 다시 시도)"),
                Err(e) => logging::warn(&format!("아이콘 프로세스 실행 실패: {e}")),
            }
        });
    }

    fn remove_icon(&self, id: &str) {
        let _ = std::fs::remove_file(icon_path(&self.storage.dir, id));
    }

    /// 지금 열린 창으로 실행 중인 앱을 다시 계산하고, 바뀌었으면 화면에 알린다.
    fn update_running(&mut self) {
        let keys = app_keys(&self.config.items);
        let running: HashSet<String> = if keys.iter().all(Option::is_none) {
            HashSet::new()
        } else {
            let flags = apps::running_flags(&keys, &apps::current_windows());
            self.config.items.iter().zip(flags).filter(|(_, on)| *on).map(|(i, _)| i.id().to_string()).collect()
        };
        if running != self.running {
            // 실행 중인 앱은 찾지 못함 표시를 지운다
            self.missing.retain(|id| !running.contains(id));
            self.running = running;
            self.refresh_tiles();
        }
    }

    fn watch_windows(&mut self, hwnd: HWND) {
        APP_HWND.store(hwnd.0 as isize, Ordering::Relaxed);
        // SAFETY: 우리 스레드 메시지 루프로 불리는 창 표시·숨김 알림
        let hook = unsafe {
            SetWinEventHook(
                EVENT_OBJECT_SHOW,
                EVENT_OBJECT_HIDE,
                None,
                Some(on_window_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            )
        };
        if hook.0.is_null() {
            logging::warn("창 변화 알림을 등록하지 못했습니다 (실행 중 표시 없음)");
        } else {
            self.window_hook = Some(hook);
        }
    }

    fn unwatch_windows(&mut self) {
        if let Some(hook) = self.window_hook.take() {
            let _ = unsafe { UnhookWinEvent(hook) };
        }
        APP_HWND.store(0, Ordering::Relaxed);
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
        self.update_running_silently();
        self.open_panel();
        self.watch_windows(hwnd);
        self.fetch_icons(self.config.items.clone());
        for notice in std::mem::take(&mut self.pending_notices) {
            self.notify(&notice);
        }
        if self.config.last_run_version != CURRENT_VERSION {
            self.config.last_run_version = CURRENT_VERSION.to_string();
            self.save();
        }
        unsafe { SetTimer(Some(hwnd), TIMER_FIRST_CHECK, FIRST_CHECK_MS, None) };
    }

    /// 화면을 열기 전 첫 계산 (화면에 보낼 곳이 아직 없다)
    fn update_running_silently(&mut self) {
        let keys = app_keys(&self.config.items);
        if keys.iter().any(Option::is_some) {
            let flags = apps::running_flags(&keys, &apps::current_windows());
            self.running =
                self.config.items.iter().zip(flags).filter(|(_, on)| *on).map(|(i, _)| i.id().to_string()).collect();
        }
    }

    fn open_panel(&mut self) {
        let send = self.post_event();
        let notify = Box::new(move |e: UiEvent| send(Event::Ui(e)));
        let spec = OpenSpec {
            mode: self.config.mode,
            edge: self.config.dock_edge,
            panel_rect: initial_rect(self.config.panel_window),
            theme: self.config.theme,
            locked: self.config.locked,
            tiles: self.tiles(),
        };
        match Panel::open(spec, notify) {
            Ok(handle) => self.panel = Some(handle),
            Err(e) => logging::error(&format!("패널 창을 만들지 못했습니다: {e}")),
        }
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

    fn dialog_owner(&self) -> HWND {
        self.panel.as_ref().map(PanelHandle::owner).unwrap_or(self.hwnd)
    }

    fn on_ui(&mut self, event: UiEvent) {
        match event {
            UiEvent::TileClicked(id) => self.open_item(&id),
            UiEvent::AddRequested => {
                if let Some(input) = item_dialog(self.dialog_owner(), "항목 추가", &Initial::New) {
                    let item = item_from_input(new_id(), input);
                    self.config.items.push(item.clone());
                    self.save();
                    self.update_running();
                    self.refresh_tiles();
                    self.fetch_icons(vec![item]);
                }
            }
            UiEvent::EditRequested(id) => self.edit_item(&id),
            UiEvent::DeleteRequested(id) => self.delete_item(&id),
            UiEvent::Reordered(ids) => {
                self.config.items = apply_order(&self.config.items, &ids);
                self.save();
            }
            UiEvent::ThemePicked(theme) => self.set_theme(theme),
            UiEvent::GeometryChanged(rect) => {
                self.config.panel_window = Some(rect);
                self.save();
            }
            UiEvent::MenuRequested => self.show_menu(),
        }
    }

    fn edit_item(&mut self, id: &str) {
        let Some(index) = self.config.items.iter().position(|i| i.id() == id) else { return };
        let initial = Initial::Edit(input_of(&self.config.items[index]));
        let Some(input) = item_dialog(self.dialog_owner(), "항목 수정", &initial) else { return };
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

    fn delete_item(&mut self, id: &str) {
        let Some(item) = self.config.items.iter().find(|i| i.id() == id) else { return };
        let text = HSTRING::from(format!("'{}' 항목을 삭제할까요?", item.display_name()));
        let answer = unsafe {
            MessageBoxW(Some(self.dialog_owner()), &text, &HSTRING::from(APP_NAME), MB_YESNO | MB_ICONQUESTION)
        };
        if answer == IDYES {
            self.config.items.retain(|i| i.id() != id);
            self.save();
            self.remove_icon(id);
            self.running.remove(id);
            self.missing.remove(id);
            self.refresh_tiles();
        }
    }

    fn open_item(&mut self, id: &str) {
        if !self.debounce.allow(id, Instant::now()) {
            return;
        }
        let Some(item) = self.config.items.iter().find(|i| i.id() == id).cloned() else { return };
        let send = self.post_event();
        std::thread::spawn(move || {
            let result = match &item {
                Item::Youtube { query, .. } => chrome::open_url(&chrome::build_search_url(query), Some("YouTube")),
                Item::Web { url, .. } => chrome::open_url(url, None),
                Item::App { id, target, exe, .. } => {
                    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
                    let key = AppKey { target: target.clone(), exe: exe.clone() };
                    if let Err(e) = apps::launch(&key) {
                        logging::warn(&format!("앱 실행 실패 ({target}): {e}"));
                        send(Event::LaunchFailed(id.clone()));
                    }
                    unsafe { CoUninitialize() };
                    return;
                }
            };
            match result {
                Ok(opened) => send(Event::Opened(opened)),
                Err(e) => logging::error(&format!("열기 실패: {e}")),
            }
        });
    }

    fn set_mode(&mut self, mode: Mode, edge: DockEdge) {
        self.config.mode = mode;
        self.config.dock_edge = edge;
        self.save();
        self.panel_send(PanelCmd::SetMode(mode, edge));
    }

    fn set_theme(&mut self, theme: Theme) {
        self.config.theme = theme;
        self.save();
        self.panel_send(PanelCmd::SetTheme(theme));
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
        let dock = self.config.mode == Mode::Dock;
        let edge = self.config.dock_edge;
        let mut entries = vec![
            MenuEntry::label(format!("{APP_NAME}  {BYLINE}  v{CURRENT_VERSION}")),
            MenuEntry::Separator,
            MenuEntry::item(CMD_SHOW, "런처 보이기"),
            MenuEntry::Separator,
            MenuEntry::checked(CMD_MODE_DOCK, "독 모드", dock),
            MenuEntry::checked(CMD_MODE_PANEL, "패널 모드", !dock),
        ];
        if dock {
            entries.extend([
                MenuEntry::checked(CMD_EDGE_BOTTOM, "    독 위치: 아래", edge == DockEdge::Bottom),
                MenuEntry::checked(CMD_EDGE_TOP, "    독 위치: 위", edge == DockEdge::Top),
                MenuEntry::checked(CMD_EDGE_LEFT, "    독 위치: 왼쪽", edge == DockEdge::Left),
                MenuEntry::checked(CMD_EDGE_RIGHT, "    독 위치: 오른쪽", edge == DockEdge::Right),
            ]);
        } else {
            entries.extend([
                MenuEntry::item(CMD_RESET_GEOMETRY, "위치·크기 초기화"),
                MenuEntry::checked(CMD_LOCK, "위치 잠금", self.config.locked),
            ]);
        }
        entries.extend([
            MenuEntry::Separator,
            MenuEntry::checked(CMD_AUTOSTART, "윈도우 시작 시 실행", self.autostart.is_enabled()),
            MenuEntry::item(CMD_CHECK_UPDATE, "업데이트 확인"),
        ]);
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
        match show_popup(self.hwnd, &self.menu_entries()) {
            CMD_SHOW => self.panel_send(PanelCmd::Show),
            CMD_MODE_DOCK => self.set_mode(Mode::Dock, self.config.dock_edge),
            CMD_MODE_PANEL => self.set_mode(Mode::Panel, self.config.dock_edge),
            CMD_EDGE_BOTTOM => self.set_mode(Mode::Dock, DockEdge::Bottom),
            CMD_EDGE_TOP => self.set_mode(Mode::Dock, DockEdge::Top),
            CMD_EDGE_LEFT => self.set_mode(Mode::Dock, DockEdge::Left),
            CMD_EDGE_RIGHT => self.set_mode(Mode::Dock, DockEdge::Right),
            CMD_RESET_GEOMETRY => self.panel_send(PanelCmd::ResetGeometry),
            CMD_LOCK => {
                self.config.locked = !self.config.locked;
                self.save();
                self.panel_send(PanelCmd::SetLocked(self.config.locked));
            }
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
        self.unwatch_windows();
        if let Some(tray) = self.tray.take() {
            tray.remove();
        }
        self.panel_send(PanelCmd::Close);
        let _ = unsafe { DestroyWindow(self.hwnd) };
        // 핸들러 안에서 자기 창을 없애면 WM_DESTROY가 핸들러에 오지 않으므로 여기서 루프를 끝낸다
        unsafe { PostQuitMessage(0) };
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
                        Event::Ui(ui) => self.on_ui(ui),
                        Event::Opened(chrome::Opened::DefaultBrowser) if !self.config.chrome_notice_shown => {
                            self.config.chrome_notice_shown = true;
                            self.save();
                            self.notify("크롬을 찾지 못해 기본 브라우저로 열었습니다.");
                        }
                        Event::Opened(_) => {}
                        Event::IconsReady => self.refresh_tiles(),
                        Event::LaunchFailed(id) => {
                            self.missing.insert(id);
                            self.refresh_tiles();
                            self.notify("앱을 찾을 수 없습니다. 수정에서 다시 선택하세요.");
                        }
                    }
                }
            }
            WM_APP_SHOW => self.panel_send(PanelCmd::Show),
            WM_TIMER => match wparam.0 {
                TIMER_FIRST_CHECK => {
                    let _ = unsafe { KillTimer(Some(hwnd), TIMER_FIRST_CHECK) };
                    unsafe { SetTimer(Some(hwnd), TIMER_DAILY_CHECK, DAILY_CHECK_MS, None) };
                    self.check_updates(false);
                }
                TIMER_DAILY_CHECK => self.check_updates(false),
                TIMER_RUNNING | TIMER_RUNNING_LATE => {
                    let _ = unsafe { KillTimer(Some(hwnd), wparam.0) };
                    self.update_running();
                }
                _ => return None,
            },
            WM_DESTROY => {
                self.unwatch_windows();
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

    #[test]
    fn tile_views_map_kinds_labels_and_badges() {
        let items = vec![
            Item::Youtube { id: "y".into(), query: "로파이 음악".into(), name: "".into() },
            Item::Web { id: "w".into(), url: "https://github.com".into(), name: "".into() },
            Item::App { id: "a".into(), name: "계산기".into(), target: "Calc!App".into(), exe: None },
        ];
        let none = HashSet::new();
        let views = tile_views(&items, &TileStatus { icon_of: &|_| None, running: &none, missing: &none });
        assert_eq!(
            views[0],
            TileView {
                id: "y".into(),
                label: "로파이 음악".into(),
                badge: "로파".into(),
                kind: TileKind::Youtube,
                ..Default::default()
            }
        );
        assert_eq!(
            (views[1].label.as_str(), views[1].badge.as_str(), views[1].kind),
            ("github.com", "G", TileKind::Web)
        );
        assert_eq!((views[2].badge.as_str(), views[2].kind), ("계", TileKind::App));
    }

    #[test]
    fn tile_views_add_icons_running_and_missing_state() {
        let items = vec![
            Item::Youtube { id: "y".into(), query: "음악".into(), name: "".into() },
            Item::App { id: "a".into(), name: "계산기".into(), target: "Calc!App".into(), exe: None },
            Item::Web { id: "w".into(), url: "https://github.com".into(), name: "".into() },
        ];
        let running = HashSet::from(["a".to_string()]);
        let missing = HashSet::from(["w".to_string()]);
        let icon_of = |id: &str| Some(PathBuf::from(format!("{id}.png")));
        let views = tile_views(&items, &TileStatus { icon_of: &icon_of, running: &running, missing: &missing });
        // 유튜브는 직접 그리므로 아이콘 파일을 쓰지 않는다
        assert_eq!(views[0].icon, None);
        assert_eq!(views[1].icon, Some(PathBuf::from("a.png")));
        assert_eq!((views[1].running, views[1].missing), (true, false));
        assert_eq!((views[2].running, views[2].missing), (false, true));
    }

    #[test]
    fn dialog_input_round_trips_to_items() {
        let calc = AppChoice { name: "계산기".into(), target: "Calc!App".into(), exe: None };
        let app = item_from_input("1".into(), ItemInput::App { app: calc.clone(), name: "".into() });
        assert_eq!(app, Item::App { id: "1".into(), name: "계산기".into(), target: "Calc!App".into(), exe: None });
        let named = item_from_input("2".into(), ItemInput::App { app: calc, name: "계산".into() });
        assert_eq!(named.display_name(), "계산");
        let web = item_from_input("3".into(), ItemInput::Web { url: "https://naver.com".into(), name: "".into() });
        assert_eq!(input_of(&web), ItemInput::Web { url: "https://naver.com".into(), name: "".into() });
        let yt = Item::Youtube { id: "4".into(), query: "로파이".into(), name: "공부".into() };
        assert_eq!(item_from_input("4".into(), input_of(&yt)), yt);
        assert_eq!(item_from_input("1".into(), input_of(&app)), app);
    }

    #[test]
    fn icon_is_refetched_only_when_target_changes() {
        let app = |t: &str| Item::App { id: "a".into(), name: "x".into(), target: t.into(), exe: None };
        let web = |u: &str, n: &str| Item::Web { id: "w".into(), url: u.into(), name: n.into() };
        assert!(icon_source_changed(&app("A!App"), &app("B!App")));
        assert!(!icon_source_changed(&app("A!App"), &app("A!App")));
        assert!(!icon_source_changed(&web("https://a.com", ""), &web("https://a.com", "에이")));
        assert!(icon_source_changed(&web("https://a.com", ""), &web("https://b.com", "")));
    }

    #[test]
    fn app_keys_only_for_app_items() {
        let items = vec![
            Item::Youtube { id: "y".into(), query: "음악".into(), name: "".into() },
            Item::App { id: "a".into(), name: "x".into(), target: "T".into(), exe: Some(r"C:\x.exe".into()) },
        ];
        assert_eq!(app_keys(&items), vec![None, Some(AppKey { target: "T".into(), exe: Some(r"C:\x.exe".into()) })]);
    }

    #[test]
    fn apply_order_follows_ids_and_keeps_unknown() {
        let a = Item::youtube("a", "").unwrap();
        let b = Item::youtube("b", "").unwrap();
        let c = Item::youtube("c", "").unwrap();
        let items = vec![a.clone(), b.clone(), c.clone()];
        let ids = vec![c.id().to_string(), a.id().to_string(), "gone".to_string()];
        assert_eq!(apply_order(&items, &ids), vec![c, a, b]);
    }

    #[test]
    fn debounce_blocks_repeat_within_one_second() {
        let mut d = Debounce::default();
        let t0 = Instant::now();
        assert!(d.allow("x", t0));
        assert!(!d.allow("x", t0 + Duration::from_millis(500)));
        assert!(d.allow("y", t0 + Duration::from_millis(500)));
        assert!(d.allow("x", t0 + Duration::from_millis(1200)));
    }
}
```

- [ ] **Step 6: src/main.rs 작성**

```rust
#![cfg_attr(not(test), windows_subsystem = "windows")]
//! YT Launcher 진입점.

mod app;
mod apps;
mod autostart;
mod chrome;
mod config;
mod icons;
mod items;
mod logging;
mod tray;
mod ui;
mod updater;
mod win;

use win::instance::{MESSAGE_CLASS, MUTEX_NAME, acquire_mutex, notify_existing};

fn main() {
    // 아이콘 만들기 전용 하위 프로세스: 셸 아이콘·인증서 검사가 남기는 메모리를 런처에 남기지 않는다
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some(icons::FETCH_ARG) {
        std::process::exit(icons::run_fetch_process(&args[2..]));
    }

    let data_dir = config::default_dir();
    logging::init(&data_dir.join("logs"));
    logging::info(&format!("YT Launcher {} 시작", app::CURRENT_VERSION));

    // 핸들은 프로세스가 끝날 때까지 쥐고 있는다 (설치 프로그램이 실행 중 감지에 씀)
    let (_mutex, existed) = acquire_mutex(MUTEX_NAME);
    if existed {
        notify_existing();
        return;
    }

    // 셸·COM을 쓰는 기능(ShellExecute, 4단계의 앱 목록)을 위해 UI 스레드를 STA로 초기화한다
    let _ = unsafe {
        windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)
    };

    let app = app::App::new(config::Storage::new(&data_dir));
    if let Err(e) = win::msg_window::create(MESSAGE_CLASS, Box::new(app)) {
        logging::error(&format!("메시지 창을 만들지 못했습니다: {e}"));
        return;
    }
    std::process::exit(win::msg_window::run_loop());
}
```

- [ ] **Step 7: src/win/mod.rs 작성**

```rust
//! Win32 API를 감싼 얇은 래퍼들.

pub mod desktop;
pub mod dwm;
pub mod instance;
pub mod menu;
pub mod msg_window;
pub mod reg;
pub mod shell;
pub mod shell_items;
pub mod windows;
```

- [ ] **Step 8: 테스트**

Run: `cargo test`
Expected: `test result: ok. 93 passed`

- [ ] **Step 9: 릴리스 실행·아이콘·메모리 (임시 APPDATA)**

Run:
```powershell
cargo build --release
$tmpApp = Join-Path $env:TEMP "ytl-phase4"; Remove-Item -Recurse -Force $tmpApp -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force "$tmpApp\YT-Launcher" | Out-Null
'{"schema":2,"mode":"dock","dock_edge":"bottom","theme":"glass","items":[{"kind":"youtube","id":"y","query":"로파이","name":""},{"kind":"app","id":"calc","name":"계산기","target":"Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"},{"kind":"web","id":"naver","url":"https://naver.com","name":""}]}' | Set-Content "$tmpApp\YT-Launcher\config.json" -Encoding utf8NoBOM
$psi = New-Object System.Diagnostics.ProcessStartInfo (Resolve-Path target\release\YTLauncher.exe)
$psi.UseShellExecute = $false; $psi.EnvironmentVariables["APPDATA"] = $tmpApp
$p = [System.Diagnostics.Process]::Start($psi); Start-Sleep 12; $p.Refresh(); $c0 = $p.TotalProcessorTime.TotalMilliseconds
Start-Sleep 10; $p.Refresh()
"WS={0:N1}MB Private={1:N1}MB idleCPU10s={2:N0}ms" -f ($p.WorkingSet64/1MB), ($p.PrivateMemorySize64/1MB), ($p.TotalProcessorTime.TotalMilliseconds - $c0)
Get-ChildItem "$tmpApp\YT-Launcher\icons" | ForEach-Object Name
Stop-Process -Id $p.Id
```
Expected: `calc.png`, `naver.png`가 생기고, 작업 집합 약 40~50MB, 사설 약 20MB, 쉬는 동안 CPU 0ms. (설치된 0.1.x 런처가 실행 중이면 바로 종료된다 — 사용자에게 알리고 직접 종료하지 않는다.)

- [ ] **Step 10: 사용자 수동 점검 항목 (보고에 포함)**

＋ → 앱 → [앱 선택…] 목록·검색·아이콘, [직접 찾기…]로 exe/lnk 고르기, 저장 후 아이콘, 앱 타일 클릭 시 실행·이미 켜져 있으면 전환(최소화 복원), 실행 중 점이 켜고 끌 때 따라오는지, 웹 주소 추가 시 파비콘, 수정 창에서 종류 버튼이 잠기는지, 지워진 앱을 누를 때 알림과 흐림.

- [ ] **Step 11: 전체 검사**

Run: `cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test`
Expected: clippy 오류 없음, `test result: ok`

- [ ] **Step 12: Commit**

```powershell
git add src
git commit -m "feat: 앱·웹 항목 (종류 선택 창, 앱 선택 창, 실행·전환, 실행 중 점)"
```

---

