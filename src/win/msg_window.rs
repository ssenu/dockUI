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
