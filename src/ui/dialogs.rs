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
