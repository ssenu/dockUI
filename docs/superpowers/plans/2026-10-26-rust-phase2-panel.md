# Rust 독 런처 — 2단계(화면 엔진 + 패널 모드) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 1단계 트레이 런처에 패널 모드 화면을 붙인다. 유리 창(아크릴) + 콘텐츠 창(Direct2D) 두 장 구조, 테마 4개와 부채꼴 선택기, 유튜브 아이콘·배지·말풍선, 마우스 확대, 드래그 정렬, 유튜브 항목 추가·수정·삭제, 위치·크기 저장과 잠금까지. 0.1.4 기능을 Rust로 재현한다.

**Architecture:** 판단·배치·색은 순수 모듈(`ui::theme`, `ui::anim`, `ui::layout`)로 두고 단위 테스트한다. `ui::gfx`가 DirectComposition + Direct2D + DirectWrite(exe에 넣은 Pretendard)로 그리고, `ui::scene`이 한 프레임을 그린다. `ui::panel`은 유리 창(입력·아크릴)과 콘텐츠 창(그리기, 클릭 통과)을 만들고 앱과는 채널 메시지(`PanelCmd`/`UiEvent`)로만 주고받는다. 입력 창은 윈도우 기본 컨트롤(`ui::dialogs`).

**Tech Stack:** Rust 1.99, `windows` 0.62(Direct2D, DirectComposition, DirectWrite, DXGI, D3D11 WARP, DWM, WIC), `windows-numerics` 0.3.

**Spec:** `docs/superpowers/specs/2026-10-26-dock-launcher-rust-design.md` (4장 화면, 5.1 추가·수정 창, 10장의 2단계)

**검증 메모 (프로토타입에서 확인):**
- `cargo test` 59개 통과, `cargo clippy --all-targets -- -D warnings` 통과.
- 테마 4개 장면을 실제 Direct2D로 PNG 렌더링해 확인(`target/snapshots/`).
- 릴리스 exe 약 5.6MB (Pretendard 3종 4.7MB 포함), 실행 중 작업 집합 약 41MB / 전용 약 18MB, 쉬는 동안 CPU 0ms(10초 측정).
- GPU 하드웨어 장치는 드라이버 때문에 작업 집합이 86MB까지 늘어 **WARP(소프트웨어 래스터라이저)를 먼저** 쓰고 실패하면 하드웨어로 넘어간다.
- 1단계 버그 발견·수정: 핸들러 안에서 자기 창을 `DestroyWindow`하면 WM_DESTROY가 핸들러에 오지 않아 트레이 "종료" 후 프로세스가 남고, WM_NCDESTROY에서 빌린 핸들러를 해제하는 문제가 있었다 → Task 1.

## Global Constraints

- 작업 브랜치 `rust-rewrite`. 파이썬 코드는 건드리지 않는다.
- 모든 명령은 저장소 루트에서 PowerShell. cargo가 PATH에 없으면 `$env:Path += ";$env:USERPROFILE\.cargo\bin"`.
- 각 태스크 끝에 `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test` 통과 후 커밋.
- 파일을 복사로 옮기는 경우 수정 시각이 예전이면 cargo가 다시 빌드하지 않는다. 내용을 바꾼 파일은 저장(또는 `(Get-Item f).LastWriteTime = Get-Date`) 후 테스트한다.
- 테마: light / dark / sepia / glass(라벨 "투명"). 덧칠: light 흰색 55%, dark `#181A1F` 60%, sepia `#F4ECD8` 60%, glass 검정 30%. 아크릴 미지원 시 덧칠 불투명도 92%.
- 패널: 기본 우상단 여백 16, 360×480 DIP, 최소 240×200, 위치 저장 500ms 디바운스, 아이콘 56 DIP·간격 16, 마우스 오버 1.15배, 테마 페이드 200ms, 선택기 펼침 160ms, 반지름 70, 위(-90°)→왼쪽(-180°) 부채꼴.
- 유리 창 클래스 `YTLauncherGlass`, 콘텐츠 창 `YTLauncherContent`, 입력 창 `YTLauncherDialog`.
- UI 문구·주석 한국어.

## File Structure

```
src/win/msg_window.rs  (수정) 공용 창 생성 WindowSpec, 재진입 안전 해제
src/win/menu.rs        (신규) 팝업 메뉴 (트레이·⋯·우클릭 공용)
src/tray.rs            (수정) 메뉴를 win::menu로 옮김
src/win/dwm.rs         (신규) 아크릴, 어두운 톤, 둥근 모서리
src/win/desktop.rs     (신규) 바탕화면에 붙이기, 맨 아래로, 작업 영역, DPI
src/win/instance.rs    (수정) 두 번째 실행이 전면 표시 권한을 넘김
src/ui/theme.rs        (신규) 색표·블렌드
src/ui/anim.rs         (신규) Anim(접근), Tween(시간)
src/ui/layout.rs       (신규) 패널 배치·클릭 판정·드롭 위치·부채꼴·기본 위치·화면 안 판정
src/ui/gfx.rs          (신규) Direct2D/DirectComposition/DirectWrite 장치, 도형·글자, PNG(테스트)
src/ui/scene.rs        (신규) 한 프레임 그리기 + PNG 스냅샷 테스트
src/ui/dialogs.rs      (신규) 유튜브 항목 입력 창
src/ui/panel.rs        (신규) 유리+콘텐츠 창, 입력·애니메이션·명령/이벤트
src/app.rs             (수정) 패널 연결, 항목 추가·수정·삭제·정렬·실행, 메뉴 확장
src/main.rs            (수정) COM 초기화, dead_code 허용 제거
Cargo.toml             (수정) 그래픽 기능, windows-numerics
assets/Pretendard-*.otf, assets/Pretendard-LICENSE.txt (신규, 파이썬 assets에서 복사)
```

---
### Task 1: 창 프로시저 재진입 수정과 팝업 메뉴 분리

**Files:**
- Modify: `src/win/msg_window.rs`, `src/tray.rs`, `src/app.rs`, `src/win/mod.rs`
- Create: `src/win/menu.rs`

**Interfaces:**
- Produces: `win::msg_window::{WindowSpec{class,title,ex_style,style,rect,owner}, create_window(&WindowSpec, Box<dyn Handler>)->Result<HWND>}` (기존 `create`, `run_loop` 유지), `win::menu::{MenuEntry, show_popup(owner: HWND, &[MenuEntry])->u32}`. `tray`에서 `MenuEntry`/`show_menu` 제거.
- Fixes: 핸들러 안 `DestroyWindow` 시 해제를 바깥 호출 뒤로 미룸, 바쁠 때 온 WM_APP 메시지는 핸들러가 끝난 뒤 다시 게시(바쁜 반복 없음), `App::quit`에서 `PostQuitMessage`.

- [ ] **Step 1: src/win/msg_window.rs 작성**

```rust
//! 창 프로시저를 Rust 핸들러에 연결하는 공용 틀과 메시지 루프.
//!
//! 메뉴·메시지 상자·입력 창은 자체 메시지 루프를 돌리므로, 처리 도중 같은 창의 프로시저가 다시 불릴 수 있다.
//! 이때 핸들러를 두 번 빌리지 않도록 `RefCell::try_borrow_mut`을 쓰고, 바쁠 때 온 앱 메시지(WM_APP 이상)는
//! 모아 두었다가 핸들러가 끝난 뒤 다시 큐에 넣는다.
//!
//! 핸들러 안에서 자기 창을 `DestroyWindow`하면 WM_DESTROY·WM_NCDESTROY가 핸들러가 빌려진 채로 들어온다.
//! 이때 WM_DESTROY는 핸들러에 닿지 못하므로, 창을 닫는 쪽에서 필요한 마무리(예: PostQuitMessage)를 직접 한다.
//! 핸들러 메모리는 바깥 호출이 끝난 뒤에 해제한다.

use std::cell::{Cell, RefCell};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DispatchMessageW, GWLP_USERDATA,
    GetMessageW, GetWindowLongPtrW, IDC_ARROW, LoadCursorW, MSG, PostMessageW, RegisterClassW, SetWindowLongPtrW,
    TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_NCCREATE, WM_NCDESTROY, WNDCLASSW, WS_OVERLAPPED,
};
use windows::core::{HSTRING, PCWSTR};

pub trait Handler {
    /// 처리했으면 `Some(결과)`, 기본 처리를 원하면 `None`.
    fn handle(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT>;
}

struct Slot {
    handler: RefCell<Box<dyn Handler>>,
    deferred: RefCell<Vec<(u32, WPARAM, LPARAM)>>,
    /// 핸들러가 빌려진 동안 창이 없어졌으면 true (바깥 호출이 끝난 뒤 해제)
    destroyed: Cell<bool>,
}

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
        let slot = &*ptr;
        if msg == WM_NCDESTROY {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            if slot.handler.try_borrow_mut().is_ok() {
                drop(Box::from_raw(ptr as *mut Slot));
            } else {
                slot.destroyed.set(true);
            }
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        let Ok(mut handler) = slot.handler.try_borrow_mut() else {
            if (WM_APP..0xC000).contains(&msg) {
                slot.deferred.borrow_mut().push((msg, wparam, lparam));
                return LRESULT(0);
            }
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        };
        let result = handler.handle(hwnd, msg, wparam, lparam);
        drop(handler);
        if slot.destroyed.get() {
            // 핸들러 처리 중에 창이 없어졌다: 이제 안전하게 해제한다
            drop(Box::from_raw(ptr as *mut Slot));
            return result.unwrap_or(LRESULT(0));
        }
        for (m, w, l) in slot.deferred.borrow_mut().drain(..) {
            let _ = PostMessageW(Some(hwnd), m, w, l);
        }
        result.unwrap_or_else(|| DefWindowProcW(hwnd, msg, wparam, lparam))
    }
}

pub struct WindowSpec<'a> {
    pub class: &'a str,
    pub title: &'a str,
    pub ex_style: WINDOW_EX_STYLE,
    pub style: WINDOW_STYLE,
    /// (x, y, w, h) 실제 픽셀
    pub rect: (i32, i32, i32, i32),
    pub owner: Option<HWND>,
}

/// 창 클래스를 (처음이면) 등록하고 창을 만든다. 핸들러는 창이 없어질 때 함께 해제된다.
pub fn create_window(spec: &WindowSpec, handler: Box<dyn Handler>) -> windows::core::Result<HWND> {
    let class = HSTRING::from(spec.class);
    // SAFETY: 클래스 등록(이미 있으면 실패해도 무시)과 창 생성
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: PCWSTR(class.as_ptr()),
            ..Default::default()
        };
        RegisterClassW(&wc);
        let slot = Box::new(Slot {
            handler: RefCell::new(handler),
            deferred: RefCell::new(Vec::new()),
            destroyed: Cell::new(false),
        });
        let (x, y, w, h) = spec.rect;
        CreateWindowExW(
            spec.ex_style,
            &class,
            &HSTRING::from(spec.title),
            spec.style,
            x,
            y,
            w,
            h,
            spec.owner,
            None,
            Some(instance.into()),
            Some(Box::into_raw(slot) as *const _),
        )
    }
}

/// 보이지 않는 최상위 창을 만든다. (메시지 전용 창은 TaskbarCreated 방송을 못 받아서 쓰지 않는다)
pub fn create(class_name: &str, handler: Box<dyn Handler>) -> windows::core::Result<HWND> {
    let spec = WindowSpec {
        class: class_name,
        title: class_name,
        ex_style: WINDOW_EX_STYLE::default(),
        style: WS_OVERLAPPED,
        rect: (0, 0, 0, 0),
        owner: None,
    };
    create_window(&spec, handler)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use windows::Win32::UI::WindowsAndMessaging::{DestroyWindow, PM_REMOVE, PeekMessageW};

    struct SelfDestroying {
        drops: Rc<Cell<u32>>,
        handled: Rc<Cell<u32>>,
    }

    impl Handler for SelfDestroying {
        fn handle(&mut self, hwnd: HWND, msg: u32, _w: WPARAM, _l: LPARAM) -> Option<LRESULT> {
            if msg == WM_APP + 50 {
                self.handled.set(self.handled.get() + 1);
                // 핸들러 안에서 자기 창을 없앤다 (WM_NCDESTROY가 빌린 상태로 들어온다)
                let _ = unsafe { DestroyWindow(hwnd) };
                return Some(LRESULT(0));
            }
            None
        }
    }

    impl Drop for SelfDestroying {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }

    fn pump() {
        let mut msg = MSG::default();
        // SAFETY: 이 스레드의 대기 메시지를 처리한다
        unsafe {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    #[test]
    fn handler_can_destroy_its_own_window_and_is_freed_once() {
        let drops = Rc::new(Cell::new(0));
        let handled = Rc::new(Cell::new(0));
        let handler = SelfDestroying { drops: drops.clone(), handled: handled.clone() };
        let hwnd = create("YTLauncherTestSelfDestroy", Box::new(handler)).unwrap();
        unsafe { PostMessageW(Some(hwnd), WM_APP + 50, WPARAM(0), LPARAM(0)).unwrap() };
        pump();
        assert_eq!(handled.get(), 1);
        assert_eq!(drops.get(), 1);
    }
}
```

- [ ] **Step 2: src/win/menu.rs 작성**

```rust
//! 마우스 위치에 뜨는 팝업 메뉴 (트레이 메뉴, ⋯ 메뉴, 항목 우클릭 메뉴 공용).

use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, MF_CHECKED, MF_GRAYED, MF_SEPARATOR, MF_STRING,
    SetForegroundWindow, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu,
};
use windows::core::HSTRING;

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

/// 마우스 위치에 메뉴를 띄우고 고른 항목 id를 돌려준다 (취소하면 0).
pub fn show_popup(owner: HWND, entries: &[MenuEntry]) -> u32 {
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
        let _ = SetForegroundWindow(owner);
        let chosen = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, pt.x, pt.y, None, owner, None);
        let _ = DestroyMenu(menu);
        chosen.0 as u32
    }
}
```

- [ ] **Step 3: src/win/mod.rs 작성**

```rust
//! Win32 API를 감싼 얇은 래퍼들.

pub mod instance;
pub mod menu;
pub mod msg_window;
pub mod reg;
pub mod shell;
pub mod windows;
```

- [ ] **Step 4: src/tray.rs 작성**

```rust
//! 알림 영역(트레이) 아이콘과 풍선 알림.

use windows::Win32::Foundation::HWND;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, HICON, IMAGE_ICON, LR_DEFAULTCOLOR, LoadImageW, SM_CXSMICON, SM_CYSMICON, WM_APP,
};
use windows::core::PCWSTR;

/// 트레이 아이콘에서 오는 마우스 메시지
pub const WM_APP_TRAY: u32 = WM_APP + 2;
const TRAY_ID: u32 = 1;
/// build.rs가 exe에 넣는 아이콘 리소스 번호
pub const ICON_RESOURCE_ID: u16 = 1;

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
}
```

- [ ] **Step 5: src/app.rs 작성**

```rust
//! 앱 컨트롤러: 설정·트레이·업데이트·자동 실행을 연결한다. (화면은 2단계에서 붙는다)

use crate::autostart::{AutoStart, RegistryRun};
use crate::config::{Config, LoadReport, Storage};
use crate::logging;
use crate::tray::{Tray, WM_APP_TRAY};
use crate::updater::{self, Release, UreqHttp};
use crate::win::instance::WM_APP_SHOW;
use crate::win::menu::{MenuEntry, show_popup};
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
        match show_popup(self.hwnd, &self.menu_entries()) {
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

- [ ] **Step 6: 테스트**

Run: `cargo test msg_window`
Expected: `handler_can_destroy_its_own_window_and_is_freed_once` 통과

- [ ] **Step 7: 전체 검사**

Run: `cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test`
Expected: clippy 오류 없음, `test result: ok`

- [ ] **Step 8: Commit**

```powershell
git add src/win src/tray.rs src/app.rs
git commit -m "fix: 창 프로시저 재진입 해제·종료 처리, 팝업 메뉴 분리"
```

---

### Task 2: 색표·애니메이션·패널 배치 (순수 모듈)

**Files:**
- Create: `src/ui/theme.rs`, `src/ui/anim.rs`, `src/ui/layout.rs`, `src/ui/mod.rs`
- Modify: `src/main.rs` (`mod ui;`)

**Interfaces:**
- Consumes: `config::Theme`
- Produces: `theme::{Color{r,g,b,a}+hex/rgba/with_alpha/lerp/shade, Palette{tint,text,subtext,divider,add_tile,swatch,dark_backdrop}, palette(Theme), blend(&Palette,&Palette,t), PICKER_ORDER, label(Theme), FALLBACK_ALPHA, TOOLTIP_BG, TOOLTIP_TEXT, ACCENT}`, `anim::{Anim{value,target}+at/set/step, Tween+new/done/advance/progress}`, `layout::{Pt, RectF+contains/center/right/bottom/scaled, Hit{Tile,Add,Palette,Menu,Header,Edge,Empty}, EDGE_* 비트, PanelLayout::compute(w,h,items)+tile_count/add_cell/hit_test(p,locked)/drop_index(p)/swatch_centers(count,progress), reorder, default_panel_rect(work,scale), is_visible_on_any(rect,works), 상수(PADDING, HEADER_H, ICON, GAP, EDGE, MENU_BUTTON, PALETTE_BUTTON, PANEL_HOVER_SCALE, PICKER_RADIUS, SWATCH, MIN_W, MIN_H, DEFAULT_W, DEFAULT_H, MARGIN)}`

- [ ] **Step 1: src/ui/theme.rs 작성**

```rust
//! 테마 4가지(라이트·다크·세피아·투명)의 색표. 유리(아크릴) 위에 덧칠하는 색과 글자색.

use crate::config::Theme;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color {
        Color { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a }
    }

    pub const fn hex(rgb: u32, a: f32) -> Color {
        Color::rgba((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, a)
    }

    pub fn with_alpha(self, a: f32) -> Color {
        Color { a, ..self }
    }

    pub fn lerp(self, other: Color, t: f32) -> Color {
        if t <= 0.0 {
            return self;
        }
        if t >= 1.0 {
            return other;
        }
        Color {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }

    /// 밝게(+) / 어둡게(-) 섞기. amount는 -1..1
    pub fn shade(self, amount: f32) -> Color {
        let target = if amount >= 0.0 { Color::hex(0xFFFFFF, self.a) } else { Color::hex(0x000000, self.a) };
        self.lerp(target, amount.abs())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// 유리 위에 덧칠하는 색 (아크릴을 쓸 때)
    pub tint: Color,
    pub text: Color,
    pub subtext: Color,
    pub divider: Color,
    pub add_tile: Color,
    /// 테마 선택 원에 보이는 색
    pub swatch: Color,
    /// 블러 배경을 어두운 톤으로 할지
    pub dark_backdrop: bool,
}

/// 아크릴을 못 쓰는 환경(Windows 10 등)에서 덧칠 불투명도
pub const FALLBACK_ALPHA: f32 = 0.92;
pub const TOOLTIP_BG: Color = Color::hex(0x14161A, 0.92);
pub const TOOLTIP_TEXT: Color = Color::hex(0xFFFFFF, 1.0);
pub const ACCENT: Color = Color::hex(0xE53935, 1.0);

pub fn palette(theme: Theme) -> Palette {
    match theme {
        Theme::Light => Palette {
            tint: Color::hex(0xFFFFFF, 0.55),
            text: Color::hex(0x1F232B, 1.0),
            subtext: Color::hex(0x7A818C, 1.0),
            divider: Color::hex(0x1F232B, 0.12),
            add_tile: Color::hex(0xA9AFB8, 0.9),
            swatch: Color::hex(0xF7F8FA, 1.0),
            dark_backdrop: false,
        },
        Theme::Dark => Palette {
            tint: Color::hex(0x181A1F, 0.60),
            text: Color::hex(0xF1F3F5, 1.0),
            subtext: Color::hex(0x8A919C, 1.0),
            divider: Color::hex(0xFFFFFF, 0.10),
            add_tile: Color::hex(0x2B2F3A, 0.95),
            swatch: Color::hex(0x181A1F, 1.0),
            dark_backdrop: true,
        },
        Theme::Sepia => Palette {
            tint: Color::hex(0xF4ECD8, 0.60),
            text: Color::hex(0x4B3A26, 1.0),
            subtext: Color::hex(0x8C7656, 1.0),
            divider: Color::hex(0x4B3A26, 0.15),
            add_tile: Color::hex(0xA8916B, 0.9),
            swatch: Color::hex(0xF4ECD8, 1.0),
            dark_backdrop: false,
        },
        Theme::Glass => Palette {
            tint: Color::hex(0x000000, 0.30),
            text: Color::hex(0xFFFFFF, 1.0),
            subtext: Color::hex(0xFFFFFF, 0.70),
            divider: Color::hex(0xFFFFFF, 0.18),
            add_tile: Color::hex(0xFFFFFF, 0.22),
            swatch: Color::hex(0x3A3D44, 1.0),
            dark_backdrop: true,
        },
    }
}

/// 테마 선택기에 나오는 순서
pub const PICKER_ORDER: [Theme; 4] = [Theme::Light, Theme::Dark, Theme::Sepia, Theme::Glass];

pub fn label(theme: Theme) -> &'static str {
    match theme {
        Theme::Light => "라이트",
        Theme::Dark => "다크",
        Theme::Sepia => "세피아",
        Theme::Glass => "투명",
    }
}

/// 테마 전환 페이드: from → to 사이를 t(0..1)로 섞은 색표
pub fn blend(from: &Palette, to: &Palette, t: f32) -> Palette {
    Palette {
        tint: from.tint.lerp(to.tint, t),
        text: from.text.lerp(to.text, t),
        subtext: from.subtext.lerp(to.subtext, t),
        divider: from.divider.lerp(to.divider, t),
        add_tile: from.add_tile.lerp(to.add_tile, t),
        swatch: to.swatch,
        dark_backdrop: to.dark_backdrop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parses_channels() {
        let c = Color::hex(0xFF8000, 0.5);
        assert_eq!((c.r, c.g, c.a), (1.0, 128.0 / 255.0, 0.5));
        assert_eq!(c.b, 0.0);
    }

    #[test]
    fn lerp_clamps_and_interpolates() {
        let a = Color::hex(0x000000, 0.0);
        let b = Color::hex(0xFFFFFF, 1.0);
        assert_eq!(a.lerp(b, 0.5).r, 0.5);
        assert_eq!(a.lerp(b, 2.0), b);
        assert_eq!(a.lerp(b, -1.0), a);
    }

    #[test]
    fn glass_is_smoky_black_30_percent() {
        let p = palette(Theme::Glass);
        assert_eq!(p.tint, Color::hex(0x000000, 0.30));
        assert!(p.dark_backdrop);
        assert!(!palette(Theme::Light).dark_backdrop);
    }

    #[test]
    fn blend_ends_at_target() {
        let from = palette(Theme::Light);
        let to = palette(Theme::Dark);
        assert_eq!(blend(&from, &to, 1.0).tint, to.tint);
        assert_eq!(blend(&from, &to, 0.0).text, from.text);
    }

    #[test]
    fn picker_lists_four_themes_with_korean_labels() {
        let labels: Vec<_> = PICKER_ORDER.iter().map(|t| label(*t)).collect();
        assert_eq!(labels, ["라이트", "다크", "세피아", "투명"]);
    }
}
```

- [ ] **Step 2: src/ui/anim.rs 작성**

```rust
//! 프레임마다 목표값으로 다가가는 간단한 애니메이션 값.

/// 한 프레임(16ms)에 남은 거리의 이 비율만큼 다가간다.
pub const RATE: f32 = 0.25;
/// 이보다 가까우면 도착한 것으로 본다.
pub const EPSILON: f32 = 0.002;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anim {
    pub value: f32,
    pub target: f32,
}

impl Anim {
    pub const fn at(value: f32) -> Anim {
        Anim { value, target: value }
    }

    pub fn set(&mut self, target: f32) {
        self.target = target;
    }

    /// 한 프레임 진행. 아직 움직이는 중이면 true.
    pub fn step(&mut self) -> bool {
        let diff = self.target - self.value;
        if diff.abs() <= EPSILON {
            self.value = self.target;
            return false;
        }
        self.value += diff * RATE;
        true
    }
}

/// 0→1로 일정 시간 동안 진행하는 값 (테마 페이드, 선택기 펼치기). 부드럽게 감속한다.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tween {
    pub elapsed_ms: f32,
    pub duration_ms: f32,
}

impl Tween {
    pub const fn new(duration_ms: f32) -> Tween {
        Tween { elapsed_ms: 0.0, duration_ms }
    }

    pub fn done() -> Tween {
        Tween { elapsed_ms: 1.0, duration_ms: 1.0 }
    }

    pub fn advance(&mut self, ms: f32) -> bool {
        self.elapsed_ms = (self.elapsed_ms + ms).min(self.duration_ms);
        self.elapsed_ms < self.duration_ms
    }

    /// 감속 곡선(ease-out cubic)을 적용한 0..1
    pub fn progress(&self) -> f32 {
        let t = (self.elapsed_ms / self.duration_ms).clamp(0.0, 1.0);
        1.0 - (1.0 - t).powi(3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anim_converges_and_stops() {
        let mut a = Anim::at(1.0);
        a.set(1.15);
        let mut frames = 0;
        while a.step() {
            frames += 1;
            assert!(frames < 100);
        }
        assert_eq!(a.value, 1.15);
        assert!(frames > 5);
        assert!(!a.step());
    }

    #[test]
    fn tween_eases_out_and_finishes() {
        let mut t = Tween::new(160.0);
        assert_eq!(t.progress(), 0.0);
        assert!(t.advance(80.0));
        assert!(t.progress() > 0.5); // 감속: 절반 시간에 절반보다 많이 진행
        assert!(!t.advance(100.0));
        assert_eq!(t.progress(), 1.0);
        assert_eq!(Tween::done().progress(), 1.0);
    }
}
```

- [ ] **Step 3: src/ui/layout.rs 작성**

```rust
//! 패널 모드 배치와 클릭 판정 (순수 함수). 모든 값은 DIP(96dpi 기준 논리 픽셀).

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Pt {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RectF {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl RectF {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> RectF {
        RectF { x, y, w, h }
    }
    pub fn contains(&self, p: Pt) -> bool {
        p.x >= self.x && p.x < self.x + self.w && p.y >= self.y && p.y < self.y + self.h
    }
    pub fn center(&self) -> Pt {
        Pt { x: self.x + self.w / 2.0, y: self.y + self.h / 2.0 }
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    /// 가운데를 기준으로 배율만큼 키운 사각형
    pub fn scaled(&self, s: f32) -> RectF {
        let c = self.center();
        RectF::new(c.x - self.w * s / 2.0, c.y - self.h * s / 2.0, self.w * s, self.h * s)
    }
}

pub const PADDING: f32 = 14.0;
pub const HEADER_H: f32 = 40.0;
pub const ICON: f32 = 56.0;
pub const GAP: f32 = 16.0;
pub const EDGE: f32 = 6.0;
pub const MENU_BUTTON: f32 = 28.0;
pub const PALETTE_BUTTON: f32 = 40.0;
pub const PANEL_HOVER_SCALE: f32 = 1.15;
pub const PICKER_RADIUS: f32 = 70.0;
pub const SWATCH: f32 = 34.0;
pub const MIN_W: f32 = 240.0;
pub const MIN_H: f32 = 200.0;
pub const DEFAULT_W: f32 = 360.0;
pub const DEFAULT_H: f32 = 480.0;
pub const MARGIN: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    /// 항목 칸 (0부터, ＋ 칸 제외)
    Tile(usize),
    Add,
    Palette,
    Menu,
    /// 헤더 빈 곳 (드래그하면 창 이동)
    Header,
    /// 창 가장자리 (크기 조절). 비트: 1=왼쪽 2=오른쪽 4=위 8=아래
    Edge(u8),
    Empty,
}

pub const EDGE_LEFT: u8 = 1;
pub const EDGE_RIGHT: u8 = 2;
pub const EDGE_TOP: u8 = 4;
pub const EDGE_BOTTOM: u8 = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct PanelLayout {
    pub width: f32,
    pub height: f32,
    pub header: RectF,
    pub menu_button: RectF,
    pub palette_button: RectF,
    /// 항목 칸들, 마지막은 ＋ 칸
    pub cells: Vec<RectF>,
}

impl PanelLayout {
    pub fn compute(width: f32, height: f32, items: usize) -> PanelLayout {
        let header = RectF::new(PADDING, PADDING - 4.0, width - 2.0 * PADDING, HEADER_H);
        let menu_button = RectF::new(
            header.right() - MENU_BUTTON,
            header.y + (HEADER_H - MENU_BUTTON) / 2.0,
            MENU_BUTTON,
            MENU_BUTTON,
        );
        let palette_button = RectF::new(
            width - PADDING - PALETTE_BUTTON,
            height - PADDING - PALETTE_BUTTON,
            PALETTE_BUTTON,
            PALETTE_BUTTON,
        );
        let top = header.bottom() + 12.0;
        let usable = (width - 2.0 * PADDING + GAP).max(ICON + GAP);
        let columns = ((usable / (ICON + GAP)).floor() as usize).max(1);
        let cells = (0..=items)
            .map(|i| {
                let (row, col) = (i / columns, i % columns);
                RectF::new(PADDING + col as f32 * (ICON + GAP), top + row as f32 * (ICON + GAP), ICON, ICON)
            })
            .collect();
        PanelLayout { width, height, header, menu_button, palette_button, cells }
    }

    pub fn tile_count(&self) -> usize {
        self.cells.len() - 1
    }

    pub fn add_cell(&self) -> RectF {
        *self.cells.last().expect("＋ 칸은 항상 있다")
    }

    fn edge_at(&self, p: Pt) -> u8 {
        let mut edge = 0;
        if p.x < EDGE {
            edge |= EDGE_LEFT;
        } else if p.x >= self.width - EDGE {
            edge |= EDGE_RIGHT;
        }
        if p.y < EDGE {
            edge |= EDGE_TOP;
        } else if p.y >= self.height - EDGE {
            edge |= EDGE_BOTTOM;
        }
        edge
    }

    /// `locked`면 가장자리·헤더 판정을 하지 않는다 (이동·크기 조절 금지).
    pub fn hit_test(&self, p: Pt, locked: bool) -> Hit {
        if !locked {
            let edge = self.edge_at(p);
            if edge != 0 {
                return Hit::Edge(edge);
            }
        }
        if self.menu_button.contains(p) {
            return Hit::Menu;
        }
        if self.palette_button.contains(p) {
            return Hit::Palette;
        }
        if let Some(i) = self.cells.iter().position(|c| c.contains(p)) {
            return if i == self.tile_count() { Hit::Add } else { Hit::Tile(i) };
        }
        if self.header.contains(p) && !locked {
            return Hit::Header;
        }
        Hit::Empty
    }

    /// 드래그 중인 항목을 놓을 자리: 마우스와 가장 가까운 항목 칸의 번호.
    pub fn drop_index(&self, p: Pt) -> usize {
        let n = self.tile_count();
        if n == 0 {
            return 0;
        }
        (0..n)
            .min_by(|&a, &b| {
                let da = dist2(self.cells[a].center(), p);
                let db = dist2(self.cells[b].center(), p);
                da.total_cmp(&db)
            })
            .unwrap_or(0)
    }

    /// 테마 선택 원의 중심: 팔레트 버튼을 중심으로 위쪽(-90°)부터 왼쪽(-180°)까지 부채꼴.
    pub fn swatch_centers(&self, count: usize, progress: f32) -> Vec<Pt> {
        let anchor = self.palette_button.center();
        let (start, end) = (-90.0f32, -180.0f32);
        let step = if count > 1 { (end - start) / (count - 1) as f32 } else { 0.0 };
        (0..count)
            .map(|i| {
                let a = (start + step * i as f32).to_radians();
                let r = PICKER_RADIUS * progress;
                Pt { x: anchor.x + r * a.cos(), y: anchor.y + r * a.sin() }
            })
            .collect()
    }
}

fn dist2(a: Pt, b: Pt) -> f32 {
    (a.x - b.x).powi(2) + (a.y - b.y).powi(2)
}

/// 기존 순서에서 `from`을 빼서 `to` 자리에 넣은 결과 (드래그 정렬)
pub fn reorder<T: Clone>(items: &[T], from: usize, to: usize) -> Vec<T> {
    let mut v = items.to_vec();
    if from < v.len() {
        let item = v.remove(from);
        v.insert(to.min(v.len()), item);
    }
    v
}

/// 작업 영역 우상단에 여백을 두고 기본 크기로 놓는다. (x, y, w, h)
pub fn default_panel_rect(work: (i32, i32, i32, i32), scale: f32) -> (i32, i32, i32, i32) {
    let (wx, wy, ww, wh) = work;
    let m = (MARGIN * scale) as i32;
    let w = ((DEFAULT_W * scale) as i32).min(ww - 2 * m);
    let h = ((DEFAULT_H * scale) as i32).min(wh - 2 * m);
    (wx + ww - w - m, wy + m, w, h)
}

/// 창 면적의 30% 이상이 어느 한 작업 영역과 겹치면 true (모니터를 뺀 뒤 화면 밖에 남는 것 방지)
pub fn is_visible_on_any(rect: (i32, i32, i32, i32), works: &[(i32, i32, i32, i32)]) -> bool {
    let (x, y, w, h) = rect;
    let area = w as i64 * h as i64;
    if area <= 0 {
        return false;
    }
    works.iter().any(|&(wx, wy, ww, wh)| {
        let iw = (x + w).min(wx + ww) - x.max(wx);
        let ih = (y + h).min(wy + wh) - y.max(wy);
        let overlap = iw.max(0) as i64 * ih.max(0) as i64;
        overlap * 10 >= area * 3
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_wraps_by_width_and_add_is_last() {
        let l = PanelLayout::compute(360.0, 480.0, 5);
        // (360 - 28 + 16) / 72 = 4.83 → 4열
        assert_eq!(l.cells.len(), 6);
        assert_eq!(l.cells[0].x, PADDING);
        assert_eq!(l.cells[3].x, PADDING + 3.0 * 72.0);
        assert_eq!(l.cells[4].y, l.cells[0].y + 72.0);
        assert_eq!(l.add_cell(), l.cells[5]);
        let narrow = PanelLayout::compute(100.0, 300.0, 2);
        assert_eq!(narrow.cells[1].x, PADDING);
    }

    #[test]
    fn hit_test_finds_buttons_tiles_and_edges() {
        let l = PanelLayout::compute(360.0, 480.0, 2);
        assert_eq!(l.hit_test(l.cells[1].center(), false), Hit::Tile(1));
        assert_eq!(l.hit_test(l.cells[2].center(), false), Hit::Add);
        assert_eq!(l.hit_test(l.menu_button.center(), false), Hit::Menu);
        assert_eq!(l.hit_test(l.palette_button.center(), false), Hit::Palette);
        assert_eq!(l.hit_test(Pt { x: 60.0, y: 25.0 }, false), Hit::Header);
        assert_eq!(l.hit_test(Pt { x: 1.0, y: 1.0 }, false), Hit::Edge(EDGE_LEFT | EDGE_TOP));
        assert_eq!(l.hit_test(Pt { x: 359.0, y: 240.0 }, false), Hit::Edge(EDGE_RIGHT));
        assert_eq!(l.hit_test(Pt { x: 200.0, y: 300.0 }, false), Hit::Empty);
    }

    #[test]
    fn locked_disables_move_and_resize() {
        let l = PanelLayout::compute(360.0, 480.0, 0);
        assert_eq!(l.hit_test(Pt { x: 1.0, y: 1.0 }, true), Hit::Empty);
        assert_eq!(l.hit_test(Pt { x: 60.0, y: 25.0 }, true), Hit::Empty);
        assert_eq!(l.hit_test(l.add_cell().center(), true), Hit::Add);
    }

    #[test]
    fn drop_index_picks_nearest_tile_cell() {
        let l = PanelLayout::compute(360.0, 480.0, 3);
        assert_eq!(l.drop_index(l.cells[2].center()), 2);
        assert_eq!(l.drop_index(Pt { x: 0.0, y: 0.0 }), 0);
        // ＋ 칸 위로 끌어도 마지막 항목 자리
        assert_eq!(l.drop_index(l.add_cell().center()), 2);
        assert_eq!(PanelLayout::compute(360.0, 480.0, 0).drop_index(Pt::default()), 0);
    }

    #[test]
    fn reorder_moves_one_item() {
        assert_eq!(reorder(&['a', 'b', 'c', 'd'], 0, 2), vec!['b', 'c', 'a', 'd']);
        assert_eq!(reorder(&['a', 'b', 'c'], 2, 0), vec!['c', 'a', 'b']);
        assert_eq!(reorder(&['a', 'b'], 1, 9), vec!['a', 'b']);
        assert_eq!(reorder(&['a'], 5, 0), vec!['a']);
    }

    #[test]
    fn swatches_fan_up_and_left_of_palette_button() {
        let l = PanelLayout::compute(360.0, 480.0, 0);
        let a = l.palette_button.center();
        let s = l.swatch_centers(4, 1.0);
        assert!((s[0].x - a.x).abs() < 0.01 && (s[0].y - (a.y - PICKER_RADIUS)).abs() < 0.01);
        assert!((s[3].y - a.y).abs() < 0.01 && (s[3].x - (a.x - PICKER_RADIUS)).abs() < 0.01);
        assert!(s.iter().all(|p| p.x <= a.x + 0.01 && p.y <= a.y + 0.01));
        assert!(l.swatch_centers(4, 0.0).iter().all(|p| *p == a));
    }

    #[test]
    fn default_rect_is_top_right_and_scaled() {
        assert_eq!(default_panel_rect((0, 0, 1920, 1040), 1.0), (1544, 16, 360, 480));
        assert_eq!(default_panel_rect((0, 0, 3840, 2080), 2.0), (3840 - 720 - 32, 32, 720, 960));
        assert_eq!(default_panel_rect((0, 0, 300, 400), 1.0), (16, 16, 268, 368));
    }

    #[test]
    fn visibility_requires_30_percent_overlap() {
        let works = [(0, 0, 1920, 1080), (1920, 0, 1280, 1024)];
        assert!(is_visible_on_any((100, 100, 360, 480), &works));
        assert!(is_visible_on_any((1800, 0, 200, 100), &works[..1])); // 60%
        assert!(!is_visible_on_any((1880, 0, 200, 100), &works[..1])); // 20%
        assert!(is_visible_on_any((2500, 100, 360, 480), &works));
        assert!(!is_visible_on_any((5000, 0, 360, 480), &works));
        assert!(!is_visible_on_any((0, 0, 0, 0), &works));
    }
}
```

- [ ] **Step 4: src/ui/mod.rs 작성**

```rust
//! 화면: 테마, 배치, 애니메이션, 그리기 장치, 장면, 패널 창, 입력 창.

pub mod anim;
pub mod layout;
pub mod theme;
```

- [ ] **Step 5: src/main.rs 작성**

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

    let app = app::App::new(config::Storage::new(&data_dir));
    if let Err(e) = win::msg_window::create(MESSAGE_CLASS, Box::new(app)) {
        logging::error(&format!("메시지 창을 만들지 못했습니다: {e}"));
        return;
    }
    std::process::exit(win::msg_window::run_loop());
}
```

- [ ] **Step 6: 테스트**

Run: `cargo test ui::`
Expected: theme 5, anim 2, layout 8 passed

- [ ] **Step 7: 전체 검사**

Run: `cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test`
Expected: clippy 오류 없음, `test result: ok`

- [ ] **Step 8: Commit**

```powershell
git add src/ui src/main.rs
git commit -m "feat: 테마 색표, 애니메이션 값, 패널 배치와 클릭 판정"
```

---

### Task 3: 그리기 장치 (gfx.rs): Direct2D + DirectComposition + DirectWrite

**Files:**
- Modify: `Cargo.toml`, `src/ui/mod.rs`
- Create: `src/ui/gfx.rs`, `assets/Pretendard-{Regular,SemiBold,Bold}.otf`, `assets/Pretendard-LICENSE.txt`

**Interfaces:**
- Consumes: `layout::{Pt, RectF}`, `theme::Color`
- Produces: `Gfx::new(hwnd, w_px, h_px, scale)`, `resize(w,h,scale)`, `begin()`, `end()`, `fill_round_rect`, `stroke_round_rect`, `fill_round_rect_gradient`, `soft_shadow`, `fill_circle`, `stroke_circle`, `line`, `fill_polygon`, `text(text, rect, size, Weight, Align, Color)`, `measure(text,size,Weight)->f32`, `enum Weight{Regular,SemiBold,Bold}`, `enum Align{Leading,Center}`, `FONT_FAMILY`. 테스트 전용: `Gfx::offscreen(w,h,scale)`, `save_png(path)`.

- [ ] **Step 1: 글꼴 복사**

Run:
```powershell
Copy-Item src\yt_launcher\assets\Pretendard-Regular.otf, src\yt_launcher\assets\Pretendard-SemiBold.otf, src\yt_launcher\assets\Pretendard-Bold.otf, src\yt_launcher\assets\Pretendard-LICENSE.txt assets\
```
Expected: `assets`에 otf 3개와 라이선스(SIL OFL 1.1, 재배포 허용) 파일.

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
    "Win32_System_Com",
    "Win32_System_Com_StructuredStorage",
    "Win32_System_LibraryLoader",
    "Win32_System_Registry",
    "Win32_System_SystemInformation",
    "Win32_System_Threading",
    "Win32_UI_Controls",
    "Win32_UI_HiDpi",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_UI_Shell",
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

- [ ] **Step 3: src/ui/gfx.rs 작성**

```rust
//! Direct2D + DirectComposition 그리기 장치. 창 하나(콘텐츠 창)에 픽셀 단위 투명도로 GPU 렌더링한다.
//!
//! 좌표는 DIP다. `set_scale`로 DPI 배율을 주면 Direct2D가 실제 픽셀로 바꿔 그린다.

use crate::ui::layout::{Pt, RectF};
use crate::ui::theme::Color;
use windows::Win32::Foundation::{HMODULE, HWND};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED,
    D2D1_GRADIENT_STOP, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_TARGET,
    D2D1_BITMAP_PROPERTIES1, D2D1_BRUSH_PROPERTIES, D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_DRAW_TEXT_OPTIONS_CLIP,
    D2D1_ELLIPSE, D2D1_EXTEND_MODE_CLAMP, D2D1_FACTORY_OPTIONS, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_GAMMA_2_2,
    D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_ROUNDED_RECT, D2D1CreateFactory, ID2D1Bitmap1, ID2D1DeviceContext,
    ID2D1Factory1, ID2D1RenderTarget, ID2D1SolidColorBrush,
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
use windows::Win32::Graphics::Imaging::{IWICBitmap, IWICImagingFactory};
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
            let mut gfx = Gfx { factory, dwrite, fonts, ctx, surface, width, height, scale };
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
                Ok(Gfx { factory, dwrite, fonts, ctx, surface: Surface::Image { wic, bitmap }, width, height, scale })
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

- [ ] **Step 4: src/ui/mod.rs 작성**

```rust
//! 화면: 테마, 배치, 애니메이션, 그리기 장치, 장면, 패널 창, 입력 창.

pub mod anim;
pub mod gfx;
pub mod layout;
pub mod theme;
```

- [ ] **Step 5: 빌드**

Run: `cargo build`
Expected: 경고 외 오류 없음 (gfx는 아직 쓰는 곳이 없어도 main의 `#![allow(dead_code)]`로 통과)

- [ ] **Step 6: 전체 검사**

Run: `cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test`
Expected: clippy 오류 없음, `test result: ok`

- [ ] **Step 7: Commit**

```powershell
git add Cargo.toml Cargo.lock assets src/ui
git commit -m "feat: Direct2D·DirectComposition 그리기 장치와 Pretendard 글꼴"
```

---

### Task 4: 바탕화면 동작과 아크릴 (dwm.rs, desktop.rs)

**Files:**
- Create: `src/win/dwm.rs`, `src/win/desktop.rs`
- Modify: `src/win/instance.rs`, `src/win/mod.rs`

**Interfaces:**
- Produces: `dwm::{enable_acrylic(hwnd,dark)->bool, set_dark(hwnd,dark)->bool}`, `desktop::{PxRect, pin_to_desktop, send_to_bottom, primary_work_area, work_areas, system_scale, scale_for(hwnd)}`. `instance::notify_existing`가 `AllowSetForegroundWindow(ASFW_ANY)`를 먼저 호출.

- [ ] **Step 1: src/win/dwm.rs 작성**

```rust
//! DWM(데스크톱 창 관리자) 효과: 아크릴 배경, 어두운 톤, 둥근 모서리.

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DWM_SYSTEMBACKDROP_TYPE, DWM_WINDOW_CORNER_PREFERENCE, DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE,
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmExtendFrameIntoClientArea,
    DwmSetWindowAttribute,
};
use windows::Win32::UI::Controls::MARGINS;

fn set_attr<T>(hwnd: HWND, attr: windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE, value: &T) -> bool {
    // SAFETY: value의 크기를 함께 넘긴다
    unsafe { DwmSetWindowAttribute(hwnd, attr, (value as *const T).cast(), std::mem::size_of::<T>() as u32) }.is_ok()
}

/// 블러 배경의 밝기 톤 (어두운 테마면 true)
pub fn set_dark(hwnd: HWND, dark: bool) -> bool {
    let value: i32 = dark.into();
    set_attr(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &value)
}

/// Windows 11 아크릴 배경을 켠다. 지원하지 않으면(Windows 10 등) false.
pub fn enable_acrylic(hwnd: HWND, dark: bool) -> bool {
    set_dark(hwnd, dark);
    let corner: DWM_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND;
    set_attr(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &corner);
    let margins = MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 };
    // SAFETY: 창 전체를 프레임 영역으로 넓혀 배경 효과가 보이게 한다
    let _ = unsafe { DwmExtendFrameIntoClientArea(hwnd, &margins) };
    let backdrop: DWM_SYSTEMBACKDROP_TYPE = DWMSBT_TRANSIENTWINDOW;
    set_attr(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, &backdrop)
}
```

- [ ] **Step 2: src/win/desktop.rs 작성**

```rust
//! 바탕화면 위젯 동작: 바탕화면에 붙이기, 맨 아래로 보내기, 모니터 작업 영역, DPI.

use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint,
};
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GWLP_HWNDPARENT, HWND_BOTTOM, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowLongPtrW, SetWindowPos,
};
use windows::core::{BOOL, w};

/// (x, y, w, h) 실제 픽셀
pub type PxRect = (i32, i32, i32, i32);

/// 창의 소유자를 바탕화면(Progman)으로 지정해 Win+D에도 남게 한다.
pub fn pin_to_desktop(hwnd: HWND) -> bool {
    // SAFETY: 소유자 핸들만 바꾼다
    unsafe {
        match FindWindowW(w!("Progman"), None) {
            Ok(progman) if !progman.0.is_null() => {
                SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, progman.0 as isize);
                true
            }
            _ => false,
        }
    }
}

/// 다른 창들 뒤로 보낸다 (활성화하지 않음).
pub fn send_to_bottom(hwnd: HWND) {
    let _ = unsafe { SetWindowPos(hwnd, Some(HWND_BOTTOM), 0, 0, 0, 0, SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE) };
}

fn rect_tuple(r: RECT) -> PxRect {
    (r.left, r.top, r.right - r.left, r.bottom - r.top)
}

/// 주 모니터의 작업 영역 (작업표시줄 제외)
pub fn primary_work_area() -> PxRect {
    // SAFETY: (0,0)이 있는 모니터 = 주 모니터
    unsafe {
        let monitor = MonitorFromPoint(Default::default(), MONITOR_DEFAULTTOPRIMARY);
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if GetMonitorInfoW(monitor, &mut info).as_bool() { rect_tuple(info.rcWork) } else { (0, 0, 1280, 720) }
    }
}

/// 모든 모니터의 작업 영역
pub fn work_areas() -> Vec<PxRect> {
    unsafe extern "system" fn collect(monitor: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        // SAFETY: data는 아래에서 넘긴 Vec 포인터
        let out = unsafe { &mut *(data.0 as *mut Vec<PxRect>) };
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
            out.push(rect_tuple(info.rcWork));
        }
        true.into()
    }
    let mut out: Vec<PxRect> = Vec::new();
    let _ = unsafe { EnumDisplayMonitors(None, None, Some(collect), LPARAM(&mut out as *mut _ as isize)) };
    out
}

/// 시스템(주 모니터) 배율. 창을 만들기 전 기본 크기 계산에 쓴다.
pub fn system_scale() -> f32 {
    let dpi = unsafe { GetDpiForSystem() };
    if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 }
}

/// 창이 있는 모니터의 배율 (100% = 1.0)
pub fn scale_for(hwnd: HWND) -> f32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_areas_include_primary() {
        let primary = primary_work_area();
        assert!(primary.2 > 0 && primary.3 > 0);
        assert!(work_areas().contains(&primary));
    }
}
```

- [ ] **Step 3: src/win/instance.rs 작성**

```rust
//! 단일 인스턴스: 명명된 뮤텍스로 판정하고, 이미 실행 중이면 기존 인스턴스에 "보이기"를 보낸다.

use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HANDLE, LPARAM, WPARAM};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{ASFW_ANY, AllowSetForegroundWindow, FindWindowW, PostMessageW, WM_APP};
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
    // 사용자가 방금 실행한 이쪽 프로세스가 기존 인스턴스에 전면 표시 권한을 넘겨준다
    let _ = unsafe { AllowSetForegroundWindow(ASFW_ANY) };
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

- [ ] **Step 4: src/win/mod.rs 작성**

```rust
//! Win32 API를 감싼 얇은 래퍼들.

pub mod desktop;
pub mod dwm;
pub mod instance;
pub mod menu;
pub mod msg_window;
pub mod reg;
pub mod shell;
pub mod windows;
```

- [ ] **Step 5: 테스트**

Run: `cargo test desktop`
Expected: 1 passed

- [ ] **Step 6: 전체 검사**

Run: `cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test`
Expected: clippy 오류 없음, `test result: ok`

- [ ] **Step 7: Commit**

```powershell
git add src/win
git commit -m "feat: 아크릴 배경, 바탕화면 레이어, 작업 영역·DPI"
```

---

### Task 5: 장면 그리기 (scene.rs) + PNG 스냅샷

**Files:**
- Create: `src/ui/scene.rs`
- Modify: `src/ui/mod.rs`

**Interfaces:**
- Consumes: `gfx`, `layout`, `theme`, `config::Theme`
- Produces: `TileKind{Youtube,App,Web}`, `TileView{id,label,badge,kind}`, `DragView{from,to,pos}`, `SceneState{...}`, `draw(&Gfx,&SceneState)`, `badge_of(&str)->String`, 상수 `TITLE`, `BYLINE`.

- [ ] **Step 1: src/ui/scene.rs 작성**

```rust
//! 패널 한 프레임 그리기: 유리 덧칠, 헤더, 아이콘, ＋, 팔레트 버튼, 말풍선, 테마 선택기, 드래그 중인 아이콘.

use crate::config::Theme;
use crate::ui::gfx::{Align, Gfx, Weight};
use crate::ui::layout::{PANEL_HOVER_SCALE, PanelLayout, Pt, RectF, SWATCH};
use crate::ui::theme::{ACCENT, Color, FALLBACK_ALPHA, PICKER_ORDER, Palette, TOOLTIP_BG, TOOLTIP_TEXT};

pub const WINDOW_RADIUS: f32 = 8.0;
pub const ICON_RADIUS: f32 = 14.0;
pub const TITLE: &str = "YT Launcher";
pub const BYLINE: &str = "by_ssenu";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileKind {
    Youtube,
    App,
    Web,
}

/// 화면에 그릴 항목 하나
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileView {
    pub id: String,
    /// 말풍선에 보이는 이름
    pub label: String,
    /// 유튜브 아이콘 아래 두 글자 배지 (다른 종류는 아바타 글자)
    pub badge: String,
    pub kind: TileKind,
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
    g.soft_shadow(r, ICON_RADIUS * r.w / 56.0, 1.0 + lift);
    match tile.kind {
        TileKind::Youtube => draw_youtube_icon(g, r, &tile.badge),
        TileKind::App | TileKind::Web => draw_letter_icon(g, r, &tile.id, &tile.badge),
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

fn draw_picker(g: &Gfx, s: &SceneState) {
    let centers = s.layout.swatch_centers(PICKER_ORDER.len(), s.picker);
    let radius = SWATCH / 2.0 * (0.6 + 0.4 * s.picker);
    for (i, (theme, c)) in PICKER_ORDER.iter().zip(centers.iter()).enumerate() {
        let swatch = crate::ui::theme::palette(*theme);
        g.fill_circle(*c, radius + 4.0, s.palette.text.with_alpha(0.25));
        g.fill_circle(*c, radius, swatch.swatch);
        if *theme == s.theme {
            g.stroke_circle(*c, radius, ACCENT, 2.5);
        } else {
            g.stroke_circle(*c, radius, s.palette.divider.with_alpha(0.6), 1.2);
        }
        if s.picker_hover == Some(i) && s.picker >= 1.0 {
            let anchor = RectF::new(c.x - radius, c.y - radius, radius * 2.0, radius * 2.0);
            draw_tooltip(g, crate::ui::theme::label(*theme), anchor, (s.layout.width, s.layout.height), 6.0);
        }
    }
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
                id: "1".into(), label: "로파이 음악".into(), badge: "로파".into(), kind: TileKind::Youtube
            },
            TileView { id: "2".into(), label: "아침 뉴스".into(), badge: "뉴스".into(), kind: TileKind::Youtube },
            TileView { id: "3".into(), label: "github.com".into(), badge: "G".into(), kind: TileKind::Web },
            TileView { id: "4".into(), label: "계산기".into(), badge: "계".into(), kind: TileKind::App },
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
}
```

- [ ] **Step 2: src/ui/mod.rs 작성**

```rust
//! 화면: 테마, 배치, 애니메이션, 그리기 장치, 장면, 패널 창, 입력 창.

pub mod anim;
pub mod gfx;
pub mod layout;
pub mod scene;
pub mod theme;
```

- [ ] **Step 3: 테스트와 눈으로 확인**

Run: `cargo test scene`
Expected: 3 passed. `target\snapshots\panel-라이트.png` 등 4장이 생긴다. 열어서 다음을 확인: 헤더(YT Launcher + by_ssenu + 점 3개), 빨간 유튜브 아이콘과 두 글자 배지, 글자 아바타(G, 계), ＋, 오른쪽 아래 팔레트 버튼, 두 번째 아이콘 확대와 말풍선(헤더를 가리지 않게 아래쪽), 라이트 장면의 부채꼴 테마 원 4개와 "세피아" 말풍선.

- [ ] **Step 4: 전체 검사**

Run: `cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test`
Expected: clippy 오류 없음, `test result: ok`

- [ ] **Step 5: Commit**

```powershell
git add src/ui
git commit -m "feat: 패널 장면 그리기와 테마별 PNG 스냅샷 테스트"
```

---

### Task 6: 유튜브 항목 입력 창 (dialogs.rs)

**Files:**
- Create: `src/ui/dialogs.rs`
- Modify: `src/ui/mod.rs`

**Interfaces:**
- Consumes: `win::msg_window::{Handler, WindowSpec, create_window}`
- Produces: `YoutubeInput{query,name}`, `youtube_dialog(owner: HWND, title, &YoutubeInput)->Option<YoutubeInput>` (모달, 저장 시 앞뒤 공백 제거, 검색어 비면 경고).

- [ ] **Step 1: src/ui/dialogs.rs 작성**

```rust
//! 항목 추가·수정 창 (윈도우 기본 컨트롤: 한글 입력·붙여넣기·Tab·Enter·Esc가 그대로 동작).
//!
//! 2단계는 유튜브 항목만 다룬다. 앱·웹 종류 선택은 4단계에서 이 창에 붙는다.

use crate::win::msg_window::{Handler, WindowSpec, create_window};
use std::cell::RefCell;
use std::rc::Rc;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateFontIndirectW, DeleteObject, HFONT, HGDIOBJ};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, BS_PUSHBUTTON, CreateWindowExW, DestroyWindow, DispatchMessageW, ES_AUTOHSCROLL, GetMessageW,
    GetWindowRect, GetWindowTextLengthW, GetWindowTextW, HMENU, IDCANCEL, IDOK, IsDialogMessageW, MB_ICONWARNING,
    MB_OK, MSG, MessageBoxW, NONCLIENTMETRICSW, PostQuitMessage, SPI_GETNONCLIENTMETRICS, SW_SHOW,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SendMessageW, SetForegroundWindow, SetWindowTextW, ShowWindow,
    SystemParametersInfoW, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_QUIT, WM_SETFONT,
    WS_CAPTION, WS_CHILD, WS_EX_CLIENTEDGE, WS_EX_DLGMODALFRAME, WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};
use windows::core::{HSTRING, w};

const DIALOG_CLASS: &str = "YTLauncherDialog";
const ID_QUERY: i32 = 101;
const ID_NAME: i32 = 102;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct YoutubeInput {
    pub query: String,
    pub name: String,
}

struct DialogState {
    query_edit: HWND,
    name_edit: HWND,
    result: Option<YoutubeInput>,
    done: bool,
}

struct DialogWindow {
    state: Rc<RefCell<DialogState>>,
}

fn window_text(hwnd: HWND) -> String {
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    let mut buf = vec![0u16; len.max(0) as usize + 1];
    let n = unsafe { GetWindowTextW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
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
                    let (query_edit, name_edit) = {
                        let s = self.state.borrow();
                        (s.query_edit, s.name_edit)
                    };
                    let query = window_text(query_edit).trim().to_string();
                    if query.is_empty() {
                        unsafe {
                            MessageBoxW(
                                Some(hwnd),
                                w!("검색어를 입력하세요."),
                                w!("YT Launcher"),
                                MB_OK | MB_ICONWARNING,
                            );
                            let _ = SetFocus(Some(query_edit));
                        }
                        return Some(LRESULT(0));
                    }
                    let name = window_text(name_edit).trim().to_string();
                    self.state.borrow_mut().result = Some(YoutubeInput { query, name });
                    self.close(hwnd);
                } else if id == IDCANCEL.0 {
                    self.close(hwnd);
                }
                Some(LRESULT(0))
            }
            WM_CLOSE => {
                self.close(hwnd);
                Some(LRESULT(0))
            }
            _ => None,
        }
    }
}

fn message_font(dpi: u32) -> HFONT {
    let mut metrics =
        NONCLIENTMETRICSW { cbSize: std::mem::size_of::<NONCLIENTMETRICSW>() as u32, ..Default::default() };
    // SAFETY: 구조체 크기를 넘겨 시스템 메시지 글꼴을 읽는다
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS,
            metrics.cbSize,
            Some((&mut metrics as *mut NONCLIENTMETRICSW).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        let mut font = metrics.lfMessageFont;
        font.lfHeight = font.lfHeight * dpi as i32 / 96;
        CreateFontIndirectW(&font)
    }
}

struct Ctl<'a> {
    class: &'a str,
    text: &'a str,
    style: WINDOW_STYLE,
    ex: WINDOW_EX_STYLE,
    rect: (i32, i32, i32, i32),
    id: i32,
}

fn child(parent: HWND, font: HFONT, c: Ctl) -> HWND {
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

/// 유튜브 항목 입력 창을 띄우고 닫힐 때까지 기다린다. 저장하면 `Some`, 취소하면 `None`.
pub fn youtube_dialog(owner: HWND, title: &str, initial: &YoutubeInput) -> Option<YoutubeInput> {
    let dpi = unsafe { GetDpiForWindow(owner) }.max(96);
    let s = |v: i32| v * dpi as i32 / 96;
    let (w, h) = (s(380), s(212));
    // 소유 창 가운데에 띄운다
    let mut owner_rect = RECT::default();
    let _ = unsafe { GetWindowRect(owner, &mut owner_rect) };
    let x = (owner_rect.left + owner_rect.right) / 2 - w / 2;
    let y = (owner_rect.top + owner_rect.bottom) / 2 - h / 2;

    let state = Rc::new(RefCell::new(DialogState {
        query_edit: HWND::default(),
        name_edit: HWND::default(),
        result: None,
        done: false,
    }));
    let spec = WindowSpec {
        class: DIALOG_CLASS,
        title,
        ex_style: WS_EX_DLGMODALFRAME,
        style: WS_POPUP | WS_CAPTION | WS_SYSMENU,
        rect: (x.max(0), y.max(0), w, h),
        owner: Some(owner),
    };
    let dialog = create_window(&spec, Box::new(DialogWindow { state: state.clone() })).ok()?;
    let font = message_font(dpi);
    let none = WINDOW_EX_STYLE(0);
    let edit_style = WINDOW_STYLE(ES_AUTOHSCROLL as u32) | WS_TABSTOP;
    let label = |text, rect| Ctl { class: "STATIC", text, style: WINDOW_STYLE(0), ex: none, rect, id: -1 };
    child(dialog, font, label("검색어", (s(16), s(18), s(80), s(20))));
    let query_edit = child(
        dialog,
        font,
        Ctl {
            class: "EDIT",
            text: &initial.query,
            style: edit_style,
            ex: WS_EX_CLIENTEDGE,
            rect: (s(100), s(14), s(256), s(26)),
            id: ID_QUERY,
        },
    );
    child(dialog, font, label("표시 이름", (s(16), s(58), s(80), s(20))));
    let name_edit = child(
        dialog,
        font,
        Ctl {
            class: "EDIT",
            text: &initial.name,
            style: edit_style,
            ex: WS_EX_CLIENTEDGE,
            rect: (s(100), s(54), s(256), s(26)),
            id: ID_NAME,
        },
    );
    child(dialog, font, label("비우면 검색어를 그대로 씁니다.", (s(100), s(84), s(256), s(20))));
    let button = |text, style: u32, x, id| Ctl {
        class: "BUTTON",
        text,
        style: WINDOW_STYLE(style) | WS_TABSTOP,
        ex: none,
        rect: (x, s(124), s(86), s(30)),
        id,
    };
    child(dialog, font, button("저장", BS_DEFPUSHBUTTON as u32, s(176), IDOK.0));
    child(dialog, font, button("취소", BS_PUSHBUTTON as u32, s(270), IDCANCEL.0));
    {
        let mut st = state.borrow_mut();
        st.query_edit = query_edit;
        st.name_edit = name_edit;
    }

    // 모달: 소유 창을 막고 이 창이 닫힐 때까지 메시지를 돌린다
    unsafe {
        let _ = EnableWindow(owner, false);
        let _ = ShowWindow(dialog, SW_SHOW);
        let _ = SetForegroundWindow(dialog);
        let _ = SetFocus(Some(query_edit));
        let _ = SetWindowTextW(dialog, &HSTRING::from(title));
        let mut msg = MSG::default();
        while !state.borrow().done {
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
        let _ = DeleteObject(HGDIOBJ(font.0));
    }
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

    /// 다른 스레드에서 입력 창을 찾아 버튼 명령을 보낸다.
    fn press_later(id: i32) {
        std::thread::spawn(move || {
            for _ in 0..100 {
                std::thread::sleep(std::time::Duration::from_millis(30));
                if let Ok(h) = unsafe { FindWindowW(&HSTRING::from(DIALOG_CLASS), None) }
                    && !h.0.is_null()
                {
                    let _ = unsafe { PostMessageW(Some(h), WM_COMMAND, WPARAM(id as usize), LPARAM(0)) };
                    return;
                }
            }
        });
    }

    #[test]
    fn save_returns_trimmed_input_and_cancel_returns_none() {
        let owner = create("YTLauncherTestDialogOwner", Box::new(Owner)).unwrap();
        press_later(IDOK.0);
        let initial = YoutubeInput { query: "  로파이 음악 ".into(), name: " 공부 ".into() };
        let saved = youtube_dialog(owner, "항목 추가", &initial);
        assert_eq!(saved, Some(YoutubeInput { query: "로파이 음악".into(), name: "공부".into() }));
        press_later(IDCANCEL.0);
        assert_eq!(youtube_dialog(owner, "항목 수정", &initial), None);
    }
}
```

- [ ] **Step 2: src/ui/mod.rs 작성**

```rust
//! 화면: 테마, 배치, 애니메이션, 그리기 장치, 장면, 패널 창, 입력 창.

pub mod anim;
pub mod dialogs;
pub mod gfx;
pub mod layout;
pub mod scene;
pub mod theme;
```

- [ ] **Step 3: 테스트**

Run: `cargo test dialogs`
Expected: `save_returns_trimmed_input_and_cancel_returns_none` 통과 (테스트 중 입력 창이 잠깐 떴다 닫힌다)

- [ ] **Step 4: 전체 검사**

Run: `cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test`
Expected: clippy 오류 없음, `test result: ok`

- [ ] **Step 5: Commit**

```powershell
git add src/ui
git commit -m "feat: 유튜브 항목 추가·수정 입력 창"
```

---

### Task 7: 패널 창과 앱 연결 (panel.rs, app.rs, main.rs)

**Files:**
- Create: `src/ui/panel.rs`
- Modify: `src/ui/mod.rs`, `src/app.rs`, `src/main.rs`, `src/items.rs`, `src/updater.rs`, `src/win/windows.rs`

**Interfaces:**
- Consumes: Task 1~6 전부
- Produces: `panel::{WM_APP_PANEL, PanelCmd{SetTiles,SetTheme,SetLocked,Show,ResetGeometry,Close}, UiEvent{TileClicked,AddRequested,EditRequested,DeleteRequested,Reordered,ThemePicked,GeometryChanged,MenuRequested}, PanelHandle+send/owner, Panel::open(rect,theme,locked,tiles,notify)->Result<PanelHandle>, initial_rect(Option<Rect>)->PxRect}`, `app::{tile_views, apply_order, Debounce}` 및 `Event::{Ui, Opened}`. 메뉴에 런처 보이기·위치·크기 초기화·위치 잠금 추가.
- Cleanup: `main.rs`의 `#![allow(dead_code)]` 제거, COM(STA) 초기화. `items::Item::web`에만 4단계용 allow, `updater::sha256_file`은 `#[cfg(test)]`, `restore_and_focus`의 allow 제거(런처 보이기에서 사용).

- [ ] **Step 1: src/ui/panel.rs 작성**

```rust
//! 패널 모드 창: 유리 창(아크릴, 입력 받음) + 콘텐츠 창(Direct2D로 그림, 클릭은 유리 창으로 통과).
//!
//! 앱과는 메시지로만 주고받는다. 앱 → 패널은 `PanelHandle::send(PanelCmd)`,
//! 패널 → 앱은 생성 때 받은 `notify(UiEvent)` 콜백.

use crate::config::{Rect, Theme};
use crate::logging;
use crate::ui::anim::{Anim, Tween};
use crate::ui::gfx::Gfx;
use crate::ui::layout::{
    EDGE_BOTTOM, EDGE_LEFT, EDGE_RIGHT, EDGE_TOP, Hit, MIN_H, MIN_W, PanelLayout, Pt, default_panel_rect,
    is_visible_on_any, reorder,
};
use crate::ui::scene::{self, DragView, SceneState, TileView};
use crate::ui::theme::{self, PICKER_ORDER, Palette};
use crate::win::desktop::{self, PxRect};
use crate::win::dwm;
use crate::win::menu::{MenuEntry, show_popup};
use crate::win::msg_window::{Handler, WindowSpec, create_window};
use std::sync::mpsc::{Receiver, Sender, channel};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, PAINTSTRUCT, ScreenToClient};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT,
    HTCAPTION, HTCLIENT, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, HTTRANSPARENT, KillTimer, MA_NOACTIVATE,
    MINMAXINFO, PostMessageW, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOZORDER, SetTimer, SetWindowPos, ShowWindow,
    WA_INACTIVE, WM_ACTIVATE, WM_APP, WM_DPICHANGED, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_NCHITTEST, WM_PAINT, WM_RBUTTONUP, WM_TIMER, WM_WINDOWPOSCHANGED,
    WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_POPUP,
};

/// 앱 → 패널 명령이 채널에 들어왔음을 알리는 메시지
pub const WM_APP_PANEL: u32 = WM_APP + 10;
const TIMER_ANIM: usize = 1;
const TIMER_SAVE_GEOMETRY: usize = 2;
const FRAME_MS: u32 = 16;
const SAVE_DELAY_MS: u32 = 500;
const DRAG_THRESHOLD: f32 = 6.0;
const PICKER_MS: f32 = 160.0;
const FADE_MS: f32 = 200.0;
const GLASS_CLASS: &str = "YTLauncherGlass";
const CONTENT_CLASS: &str = "YTLauncherContent";
const CMD_EDIT: u32 = 1;
const CMD_DELETE: u32 = 2;

#[derive(Debug, Clone, PartialEq)]
pub enum PanelCmd {
    SetTiles(Vec<TileView>),
    SetTheme(Theme),
    SetLocked(bool),
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
    GeometryChanged(Rect),
    MenuRequested,
}

/// 앱이 패널에 명령을 보내는 손잡이
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

pub struct Panel {
    glass: HWND,
    content: HWND,
    gfx: Option<Gfx>,
    acrylic: bool,
    scale: f32,
    layout: PanelLayout,
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

/// 저장된 위치가 화면 안에 있으면 그대로, 아니면 우상단 기본 위치
pub fn initial_rect(saved: Option<Rect>) -> PxRect {
    let works = desktop::work_areas();
    match saved {
        Some(r) if is_visible_on_any((r.x, r.y, r.w, r.h), &works) => (r.x, r.y, r.w, r.h),
        _ => default_panel_rect(desktop::primary_work_area(), desktop::system_scale()),
    }
}

impl Panel {
    /// 유리 창과 콘텐츠 창을 만들고 보여준다.
    pub fn open(
        rect: PxRect,
        theme: Theme,
        locked: bool,
        tiles: Vec<TileView>,
        notify: Box<dyn Fn(UiEvent)>,
    ) -> windows::core::Result<PanelHandle> {
        let (tx, rx) = channel();
        let glass_spec = WindowSpec {
            class: GLASS_CLASS,
            title: "YT Launcher",
            ex_style: WS_EX_TOOLWINDOW,
            style: WS_POPUP,
            rect,
            owner: None,
        };
        let panel = Panel {
            glass: HWND::default(),
            content: HWND::default(),
            gfx: None,
            acrylic: false,
            scale: 1.0,
            layout: PanelLayout::compute(rect.2 as f32, rect.3 as f32, tiles.len()),
            hover: vec![Anim::at(0.0); tiles.len() + 1],
            tiles,
            hover_hit: Hit::Empty,
            tracking_leave: false,
            press: None,
            drag: None,
            theme,
            fade_from: theme::palette(theme),
            fade: Tween::done(),
            picker_open: false,
            picker: Tween::done(),
            picker_hover: None,
            locked,
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
        self.relayout();
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

    fn relayout(&mut self) {
        let (x, y, w, h) = screen_rect(self.glass);
        let (wd, hd) = (w as f32 / self.scale, h as f32 / self.scale);
        self.layout = PanelLayout::compute(wd, hd, self.tiles.len());
        self.hover.resize(self.tiles.len() + 1, Anim::at(0.0));
        if !self.content.0.is_null() {
            let _ = unsafe { SetWindowPos(self.content, None, x, y, w, h, SWP_NOACTIVATE | SWP_NOZORDER) };
        }
        if let Some(gfx) = &mut self.gfx
            && let Err(e) = gfx.resize(w as u32, h as u32, self.scale)
        {
            logging::error(&format!("그리기 크기 변경 실패: {e}"));
        }
    }

    fn render(&mut self) {
        let Some(gfx) = &self.gfx else { return };
        let hover: Vec<f32> = self.hover.iter().map(|a| a.value).collect();
        let pressed = match &self.press {
            Some(Press { hit: Hit::Tile(i), .. }) => Some(*i),
            _ => None,
        };
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
            picker: if self.picker_open { self.picker.progress() } else { 0.0 },
            picker_hover: self.picker_hover,
            theme: self.theme,
        };
        gfx.begin();
        scene::draw(gfx, &state);
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
        self.render();
        if !moving {
            self.animating = false;
            let _ = unsafe { KillTimer(Some(self.glass), TIMER_ANIM) };
        }
    }

    fn to_dip(&self, x: i32, y: i32) -> Pt {
        Pt { x: x as f32 / self.scale, y: y as f32 / self.scale }
    }

    fn client_point_from_screen(&self, l: LPARAM) -> Pt {
        let (x, y) = lparam_point(l);
        let mut pt = POINT { x, y };
        let _ = unsafe { ScreenToClient(self.glass, &mut pt) };
        self.to_dip(pt.x, pt.y)
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
        let r = crate::ui::layout::SWATCH / 2.0;
        self.layout
            .swatch_centers(PICKER_ORDER.len(), self.picker.progress())
            .iter()
            .position(|c| (c.x - p.x).powi(2) + (c.y - p.y).powi(2) <= r * r)
    }

    fn close_picker(&mut self) {
        self.picker_open = false;
        self.picker_hover = None;
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
        self.set_hover(self.layout.hit_test(p, true));
    }

    fn on_button_up(&mut self, p: Pt) {
        let _ = unsafe { ReleaseCapture() };
        let press = self.press.take();
        if let Some((from, pos)) = self.drag.take() {
            let to = self.layout.drop_index(pos);
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
            let on_button = self.layout.hit_test(p, true) == Hit::Palette;
            self.close_picker();
            if on_button {
                return;
            }
        }
        let hit = self.layout.hit_test(p, true);
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
                self.start_animation();
            }
            _ => {}
        }
        self.render();
    }

    fn on_right_click(&mut self, p: Pt) {
        if let Hit::Tile(i) = self.layout.hit_test(p, true) {
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
                self.tiles = tiles;
                self.hover = vec![Anim::at(0.0); self.tiles.len() + 1];
                self.hover_hit = Hit::Empty;
                self.layout = PanelLayout::compute(self.layout.width, self.layout.height, self.tiles.len());
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
            PanelCmd::Show => {
                crate::win::windows::restore_and_focus(self.glass);
            }
            PanelCmd::ResetGeometry => {
                let (x, y, w, h) = default_panel_rect(desktop::primary_work_area(), self.scale);
                let _ = unsafe { SetWindowPos(self.glass, None, x, y, w, h, SWP_NOACTIVATE | SWP_NOZORDER) };
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
                self.on_mouse_move(self.to_dip(x, y));
            }
            WM_MOUSELEAVE => {
                self.tracking_leave = false;
                if self.drag.is_none() {
                    self.set_hover(Hit::Empty);
                    self.picker_hover = None;
                }
            }
            WM_LBUTTONDOWN => {
                let (x, y) = lparam_point(lparam);
                let at = self.to_dip(x, y);
                self.press = Some(Press { hit: self.layout.hit_test(at, true), at });
                let _ = unsafe { SetCapture(hwnd) };
                self.render();
            }
            WM_LBUTTONUP => {
                let (x, y) = lparam_point(lparam);
                self.on_button_up(self.to_dip(x, y));
            }
            WM_RBUTTONUP => {
                let (x, y) = lparam_point(lparam);
                self.on_right_click(self.to_dip(x, y));
            }
            WM_ACTIVATE => {
                // 다른 앱으로 포커스가 넘어가면 다시 바탕화면 쪽으로 내린다 (우리 대화상자·메뉴는 제외)
                if (wparam.0 & 0xFFFF) as u32 == WA_INACTIVE && !foreground_is_ours() {
                    desktop::send_to_bottom(hwnd);
                }
                return None;
            }
            WM_WINDOWPOSCHANGED => {
                self.relayout();
                self.render();
                unsafe { SetTimer(Some(hwnd), TIMER_SAVE_GEOMETRY, SAVE_DELAY_MS, None) };
                return None;
            }
            WM_DPICHANGED => {
                self.scale = ((wparam.0 & 0xFFFF) as f32) / 96.0;
                // SAFETY: lparam은 시스템이 제안한 새 창 사각형
                let r = unsafe { &*(lparam.0 as *const RECT) };
                let _ = unsafe {
                    SetWindowPos(
                        hwnd,
                        None,
                        r.left,
                        r.top,
                        r.right - r.left,
                        r.bottom - r.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    )
                };
            }
            WM_GETMINMAXINFO => {
                // SAFETY: lparam은 시스템이 넘긴 MINMAXINFO
                let info = unsafe { &mut *(lparam.0 as *mut MINMAXINFO) };
                info.ptMinTrackSize.x = (MIN_W * self.scale) as i32;
                info.ptMinTrackSize.y = (MIN_H * self.scale) as i32;
            }
            WM_TIMER => match wparam.0 {
                TIMER_ANIM => self.tick(),
                TIMER_SAVE_GEOMETRY => {
                    let _ = unsafe { KillTimer(Some(hwnd), TIMER_SAVE_GEOMETRY) };
                    let (x, y, w, h) = screen_rect(hwnd);
                    self.emit(UiEvent::GeometryChanged(Rect { x, y, w, h }));
                }
                _ => return None,
            },
            WM_ERASEBKGND => {}
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                unsafe {
                    BeginPaint(hwnd, &mut ps);
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

- [ ] **Step 2: src/ui/mod.rs 작성**

```rust
//! 화면: 테마, 배치, 애니메이션, 그리기 장치, 장면, 패널 창, 입력 창.

pub mod anim;
pub mod dialogs;
pub mod gfx;
pub mod layout;
pub mod panel;
pub mod scene;
pub mod theme;
```

- [ ] **Step 3: src/app.rs 작성**

```rust
//! 앱 컨트롤러: 설정·트레이·패널 화면·항목 실행·업데이트·자동 실행을 연결한다.

use crate::autostart::{AutoStart, RegistryRun};
use crate::chrome;
use crate::config::{Config, LoadReport, Storage, Theme};
use crate::items::Item;
use crate::logging;
use crate::tray::{Tray, WM_APP_TRAY};
use crate::ui::dialogs::{YoutubeInput, youtube_dialog};
use crate::ui::panel::{Panel, PanelCmd, PanelHandle, UiEvent, initial_rect};
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
            TileView { id: item.id().to_string(), label, badge, kind }
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
        let rect = initial_rect(self.config.panel_window);
        match Panel::open(rect, self.config.theme, self.config.locked, tile_views(&self.config.items), notify) {
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
        let mut entries = vec![
            MenuEntry::label(format!("{APP_NAME}  {BYLINE}  v{CURRENT_VERSION}")),
            MenuEntry::Separator,
            MenuEntry::item(CMD_SHOW, "런처 보이기"),
            MenuEntry::item(CMD_RESET_GEOMETRY, "위치·크기 초기화"),
            MenuEntry::checked(CMD_LOCK, "위치 잠금", self.config.locked),
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
        match show_popup(self.hwnd, &self.menu_entries()) {
            CMD_SHOW => self.panel_send(PanelCmd::Show),
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
                id: "y".into(), label: "로파이 음악".into(), badge: "로파".into(), kind: TileKind::Youtube
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

- [ ] **Step 4: src/main.rs 작성**

```rust
#![cfg_attr(not(test), windows_subsystem = "windows")]
//! YT Launcher 진입점.

mod app;
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

- [ ] **Step 5: src/items.rs 작성**

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

    #[allow(dead_code)] // 4단계(웹 주소 항목 추가)에서 사용
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

- [ ] **Step 6: src/updater.rs 작성**

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

#[cfg(test)]
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

- [ ] **Step 7: src/win/windows.rs 작성**

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

- [ ] **Step 8: 테스트**

Run: `cargo test`
Expected: `test result: ok. 59 passed`

- [ ] **Step 9: 릴리스 빌드·메모리·대기 CPU 확인 (임시 APPDATA)**

Run:
```powershell
cargo build --release
"{0:N0} KB" -f ((Get-Item target\release\YTLauncher.exe).Length/1KB)
$tmpApp = Join-Path $env:TEMP "ytl-phase2"; Remove-Item -Recurse -Force $tmpApp -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force "$tmpApp\YT-Launcher" | Out-Null
'{"schema":2,"items":[{"kind":"youtube","id":"a1","query":"로파이 음악","name":""},{"kind":"youtube","id":"a2","query":"뉴스","name":"아침 뉴스"}],"mode":"panel","theme":"dark"}' | Set-Content "$tmpApp\YT-Launcher\config.json" -Encoding utf8NoBOM
$psi = New-Object System.Diagnostics.ProcessStartInfo (Resolve-Path target\release\YTLauncher.exe)
$psi.UseShellExecute = $false; $psi.EnvironmentVariables["APPDATA"] = $tmpApp
$p = [System.Diagnostics.Process]::Start($psi); Start-Sleep 5; $p.Refresh(); $c0 = $p.TotalProcessorTime.TotalMilliseconds
Start-Sleep 10; $p.Refresh()
"WS={0:N1}MB Private={1:N1}MB idleCPU10s={2:N0}ms" -f ($p.WorkingSet64/1MB), ($p.PrivateMemorySize64/1MB), ($p.TotalProcessorTime.TotalMilliseconds - $c0)
Stop-Process -Id $p.Id
```
Expected: exe 약 5~6MB, 작업 집합 약 35~45MB, 전용 약 15~25MB, 쉬는 동안 CPU 0ms. (설치된 0.1.x 런처가 실행 중이면 뮤텍스 때문에 바로 종료된다. 그 경우 사용자에게 알리고 직접 종료하지 않는다.)

- [ ] **Step 10: 사용자 수동 점검 항목 (보고에 포함)**

화면을 볼 수 없으면 다음을 사용자 확인 항목으로 보고한다: 우상단 패널과 유리(아크릴) 배경, 헤더 드래그 이동·가장자리 크기 조절·위치 기억, 아이콘 마우스 확대와 말풍선, 클릭 시 크롬 최대화 검색, ＋로 추가·우클릭 수정/삭제, 드래그 정렬 유지, 팔레트 부채꼴에서 테마 4개 전환(페이드), ⋯ 메뉴와 트레이 메뉴의 런처 보이기·위치 초기화·잠금, 다른 창을 누르면 다시 뒤로 깔리는지, 종료 후 프로세스가 남지 않는지.

- [ ] **Step 11: 전체 검사**

Run: `cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test`
Expected: clippy 오류 없음, `test result: ok`

- [ ] **Step 12: Commit**

```powershell
git add src
git commit -m "feat: 패널 모드 화면 (유리+Direct2D, 테마, 확대, 드래그 정렬, 항목 추가·수정·삭제)"
```

---

