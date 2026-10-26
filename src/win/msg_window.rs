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
