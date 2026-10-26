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
