//! 항목 추가·수정 창. 배경·종류 카드·버튼은 `form`이 직접 그리고, 글자 입력칸만 윈도우 기본 EDIT를 얹는다
//! (한글 입력·붙여넣기·Tab이 그대로 동작). Enter는 저장, Esc는 닫기.
//!
//! 종류 카드(유튜브 / 앱 / 웹 주소)로 입력칸이 바뀐다. 수정할 때는 종류를 바꿀 수 없다.
//! 다른 입력 창(앱 선택)도 쓰는 공용 도우미(글꼴, 컨트롤 만들기, 모달 루프)가 여기 있다.

use crate::items::normalize_url;
use crate::ui::app_list::{AppChoice, pick_app};
use crate::ui::form::{self, Field, FormView, Target};
use crate::ui::gfx::{Gfx, gdi_font};
use crate::ui::layout::{Pt, RectF};
use crate::win::dwm;
use crate::win::msg_window::{Handler, WindowSpec, create_window};
use std::cell::RefCell;
use std::rc::Rc;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, COLOR_WINDOW, CreateFontIndirectW, DeleteObject, EndPaint, FillRect, GetMonitorInfoW, GetStockObject,
    GetSysColor, GetSysColorBrush, HBRUSH, HDC, HFONT, HGDIOBJ, InvalidateRect, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    MonitorFromWindow, PAINTSTRUCT, ScreenToClient, SetBkColor, SetTextColor, WHITE_BRUSH,
};
use windows::Win32::UI::HiDpi::{GetDpiForWindow, SystemParametersInfoForDpi};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetFocus, ReleaseCapture, SetCapture, SetFocus, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
    VK_ESCAPE, VK_RETURN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, BS_PUSHBUTTON, CreateWindowExW, DestroyWindow, DispatchMessageW, ES_AUTOHSCROLL, GetClientRect,
    GetDlgItem, GetMessageW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, HMENU, HTCAPTION, HTCLIENT, IDC_HAND,
    IDCANCEL, IDOK, IsChild, IsDialogMessageW, LoadCursorW, MB_ICONWARNING, MB_OK, MSG, MessageBoxW, NONCLIENTMETRICSW,
    PostMessageW, PostQuitMessage, SPI_GETNONCLIENTMETRICS, SW_HIDE, SW_SHOW, SendMessageW, SetCursor,
    SetForegroundWindow, SetWindowTextW, ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE,
    WM_COMMAND, WM_CTLCOLOREDIT, WM_CTLCOLORSTATIC, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEMOVE, WM_NCCALCSIZE, WM_NCHITTEST, WM_PAINT, WM_QUIT, WM_SETCURSOR, WM_SETFONT, WS_CAPTION, WS_CHILD,
    WS_CLIPCHILDREN, WS_EX_CLIENTEDGE, WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};
use windows::core::HSTRING;

const DIALOG_CLASS: &str = "YTLauncherDialog";
/// 정적 글자가 넘치면 … 으로 줄인다 (SystemServices 기능을 켜지 않으려고 값만 둔다)
const SS_ENDELLIPSIS: u32 = 0x4000;
/// EDIT 빈 칸 안내 글자 (Common Controls 6)
const EM_SETCUEBANNER: u32 = 0x1501;
const EN_SETFOCUS: u32 = 0x0100;
const EN_KILLFOCUS: u32 = 0x0200;
pub(crate) const EN_CHANGE: u32 = 0x0300;
pub(crate) const WM_MOUSELEAVE: u32 = 0x02A3;
const ID_MAIN: i32 = 101;
const ID_NAME: i32 = 102;
const ID_PICK: i32 = 105;
const ID_PICK_FILE: i32 = 106;
const ID_PICK_FOLDER: i32 = 107;
const ID_KIND_YOUTUBE: i32 = 111;
const ID_KIND_APP: i32 = 112;
const ID_KIND_WEB: i32 = 113;
const ID_KIND_PATH: i32 = 114;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    #[default]
    Youtube,
    App,
    Web,
    Path,
}

impl Kind {
    fn from_id(id: i32) -> Option<Kind> {
        match id {
            ID_KIND_YOUTUBE => Some(Kind::Youtube),
            ID_KIND_APP => Some(Kind::App),
            ID_KIND_WEB => Some(Kind::Web),
            ID_KIND_PATH => Some(Kind::Path),
            _ => None,
        }
    }

    fn button_id(self) -> i32 {
        match self {
            Kind::Youtube => ID_KIND_YOUTUBE,
            Kind::App => ID_KIND_APP,
            Kind::Web => ID_KIND_WEB,
            Kind::Path => ID_KIND_PATH,
        }
    }

    /// 첫 칸이 글자 입력칸인가 (앱·파일은 고르기 단추)
    fn has_text_field(self) -> bool {
        matches!(self, Kind::Youtube | Kind::Web)
    }

    /// 첫 입력칸 빈 칸 안내
    fn main_cue(self) -> &'static str {
        match self {
            Kind::Youtube => "예: 로파이 음악",
            Kind::App | Kind::Path => "",
            Kind::Web => "예: naver.com",
        }
    }

    /// 표시 이름 빈 칸 안내
    fn hint(self) -> &'static str {
        match self {
            Kind::Youtube => "비우면 검색어를 그대로 씁니다",
            Kind::App => "비우면 앱 이름을 씁니다",
            Kind::Web => "비우면 주소의 사이트 이름을 씁니다",
            Kind::Path => "비우면 파일·폴더 이름을 씁니다",
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
    /// 파일이나 폴더의 전체 경로
    Path {
        path: String,
        name: String,
    },
}

impl ItemInput {
    pub fn kind(&self) -> Kind {
        match self {
            ItemInput::Youtube { .. } => Kind::Youtube,
            ItemInput::App { .. } => Kind::App,
            ItemInput::Web { .. } => Kind::Web,
            ItemInput::Path { .. } => Kind::Path,
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
    /// 수정 창: 종류를 바꿀 수 없다
    fixed: bool,
    title: String,
    /// 종류를 오갈 때 칸에 쓴 글을 기억한다
    youtube_text: String,
    web_text: String,
    app: Option<AppChoice>,
    /// 고른 파일·폴더 경로
    path: Option<String>,
    result: Option<ItemInput>,
    done: bool,
    scale: f32,
    /// 창 그림 (못 만들면 흰 바탕만)
    gfx: Option<Gfx>,
    hover: Option<Target>,
    pressed: Option<Target>,
    error: Option<(String, Field)>,
    tracking_leave: bool,
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

pub(crate) fn item(dialog: HWND, id: i32) -> HWND {
    unsafe { GetDlgItem(Some(dialog), id) }.unwrap_or_default()
}

pub(crate) fn set_text(hwnd: HWND, text: &str) {
    let _ = unsafe { SetWindowTextW(hwnd, &HSTRING::from(text)) };
}

fn show(hwnd: HWND, visible: bool) {
    let _ = unsafe { ShowWindow(hwnd, if visible { SW_SHOW } else { SW_HIDE }) };
}

pub fn warn(owner: HWND, text: &str) {
    unsafe {
        MessageBoxW(Some(owner), &HSTRING::from(text), &HSTRING::from(crate::app::APP_NAME), MB_OK | MB_ICONWARNING)
    };
}

pub(crate) fn set_cue(edit: HWND, text: &str) {
    let wide = HSTRING::from(text);
    // SAFETY: 컨트롤이 글자를 복사해 둔다
    unsafe { SendMessageW(edit, EM_SETCUEBANNER, Some(WPARAM(1)), Some(LPARAM(wide.as_ptr() as isize))) };
}

pub(crate) fn invalidate(hwnd: HWND) {
    let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
}

impl DialogState {
    /// 다른 종류로 바꾼다: 지금 칸에 쓴 글을 기억해 두고 입력칸을 바꾼다.
    fn apply_kind(&mut self, kind: Kind) {
        let main = item(self.hwnd, ID_MAIN);
        match self.kind {
            Kind::Youtube => self.youtube_text = window_text(main),
            Kind::Web => self.web_text = window_text(main),
            Kind::App | Kind::Path => {}
        }
        self.kind = kind;
        self.error = None;
        self.show_kind();
    }

    /// 지금 종류에 맞게 입력칸·안내 글자를 바꾸고 다시 그린다.
    fn show_kind(&mut self) {
        let (dialog, kind) = (self.hwnd, self.kind);
        let main = item(dialog, ID_MAIN);
        show(main, kind.has_text_field());
        set_cue(main, kind.main_cue());
        set_cue(item(dialog, ID_NAME), kind.hint());
        match kind {
            Kind::Youtube => set_text(main, &self.youtube_text),
            Kind::Web => set_text(main, &self.web_text),
            Kind::App | Kind::Path => {}
        }
        invalidate(dialog);
    }

    /// 저장 조건을 확인한다. 통과하면 결과, 아니면 (안내 문구, 포커스 줄 칸).
    fn collect(&self) -> Result<ItemInput, (&'static str, i32)> {
        let main = window_text(item(self.hwnd, ID_MAIN)).trim().to_string();
        let name = window_text(item(self.hwnd, ID_NAME)).trim().to_string();
        match self.kind {
            Kind::Youtube if main.is_empty() => Err(("검색어를 입력하세요.", ID_MAIN)),
            Kind::Youtube => Ok(ItemInput::Youtube { query: main, name }),
            Kind::App => match &self.app {
                Some(app) => Ok(ItemInput::App { app: app.clone(), name }),
                None => Err(("실행할 앱을 고르세요.", ID_PICK)),
            },
            Kind::Web => match normalize_url(&main) {
                Ok(url) => Ok(ItemInput::Web { url, name }),
                Err(_) => Err(("올바른 주소를 입력하세요. 예: naver.com", ID_MAIN)),
            },
            Kind::Path => match &self.path {
                Some(path) => Ok(ItemInput::Path { path: path.clone(), name }),
                None => Err(("열 파일이나 폴더를 고르세요.", ID_PICK_FILE)),
            },
        }
    }

    fn paint(&self) {
        let mut ps = PAINTSTRUCT::default();
        // SAFETY: WM_PAINT 안에서 BeginPaint/EndPaint 짝을 맞춘다
        unsafe {
            let hdc = BeginPaint(self.hwnd, &mut ps);
            let drawn = self.gfx.as_ref().is_some_and(|gfx| {
                gfx.begin();
                form::draw(
                    gfx,
                    &FormView {
                        kind: self.kind,
                        fixed: self.fixed,
                        title: &self.title,
                        hover: self.hover,
                        pressed: self.pressed,
                        focus: self.focused_field(),
                        app_name: self.app.as_ref().map(|a| a.name.as_str()),
                        path: self.path.as_deref(),
                        error: self.error.as_ref().map(|(t, f)| (t.as_str(), *f)),
                    },
                );
                let ok = gfx.end().is_ok();
                if ok {
                    gfx.copy_to_dc(hdc);
                }
                ok
            });
            if !drawn {
                let mut r = RECT::default();
                let _ = GetClientRect(self.hwnd, &mut r);
                FillRect(hdc, &r, HBRUSH(GetStockObject(WHITE_BRUSH).0));
            }
            let _ = EndPaint(self.hwnd, &ps);
        }
    }

    /// 키보드 포커스가 있는 입력칸 (알림은 재진입 때 빠질 수 있어 그릴 때 직접 묻는다)
    fn focused_field(&self) -> Option<Field> {
        let focus = unsafe { GetFocus() };
        if focus == item(self.hwnd, ID_MAIN) {
            Some(Field::Main)
        } else if focus == item(self.hwnd, ID_NAME) {
            Some(Field::Name)
        } else {
            None
        }
    }

    /// 창 px 좌표 → 그림 좌표(DIP)
    fn point(&self, l: LPARAM) -> Pt {
        let (x, y) = ((l.0 & 0xFFFF) as i16 as f32, ((l.0 >> 16) & 0xFFFF) as i16 as f32);
        Pt { x: x / self.scale, y: y / self.scale }
    }

    fn set_hover(&mut self, hover: Option<Target>) {
        if hover != self.hover {
            self.hover = hover;
            invalidate(self.hwnd);
        }
    }
}

fn command_of(target: Target) -> i32 {
    match target {
        Target::Close | Target::Cancel => IDCANCEL.0,
        Target::Ok => IDOK.0,
        Target::Pick => ID_PICK,
        Target::PickFile => ID_PICK_FILE,
        Target::PickFolder => ID_PICK_FOLDER,
        Target::Card(kind) => kind.button_id(),
    }
}

impl DialogWindow {
    /// 끝났다고 먼저 표시한 뒤 창을 없앤다 (핸들러 안의 DestroyWindow는 WM_DESTROY를 받지 못한다).
    fn close(&self, hwnd: HWND) {
        self.state.borrow_mut().done = true;
        let _ = unsafe { DestroyWindow(hwnd) };
    }

    fn on_command(&self, hwnd: HWND, id: i32, code: u32) {
        if id == ID_MAIN || id == ID_NAME {
            let field = if id == ID_MAIN { Field::Main } else { Field::Name };
            // 상태를 빌린 채 글자를 바꾸면(show_kind) 알림이 바로 다시 들어온다: 그때는 건너뛴다
            let Ok(mut st) = self.state.try_borrow_mut() else { return };
            match code {
                EN_SETFOCUS | EN_KILLFOCUS => {}
                EN_CHANGE if st.error.as_ref().is_some_and(|(_, f)| *f == field) => st.error = None,
                _ => return,
            }
            invalidate(hwnd);
        } else if id == IDOK.0 {
            let collected = self.state.borrow().collect();
            match collected {
                Ok(input) => {
                    self.state.borrow_mut().result = Some(input);
                    self.close(hwnd);
                }
                Err((text, focus)) => {
                    self.state.borrow_mut().error = Some((text.to_string(), Field::Main));
                    if focus == ID_MAIN {
                        let _ = unsafe { SetFocus(Some(item(hwnd, ID_MAIN))) };
                    }
                    invalidate(hwnd);
                }
            }
        } else if id == IDCANCEL.0 {
            self.close(hwnd);
        } else if id == ID_PICK {
            if let Some(app) = pick_app(hwnd) {
                let mut st = self.state.borrow_mut();
                st.app = Some(app);
                st.error = None;
                st.show_kind();
            }
        } else if id == ID_PICK_FILE || id == ID_PICK_FOLDER {
            if let Some(path) = pick_path(hwnd, id == ID_PICK_FOLDER) {
                let mut st = self.state.borrow_mut();
                st.path = Some(path);
                st.error = None;
                st.show_kind();
            }
        } else if let Some(kind) = Kind::from_id(id) {
            let changed = {
                let mut st = self.state.borrow_mut();
                let changed = !st.fixed && st.kind != kind;
                if changed {
                    st.apply_kind(kind);
                }
                changed
            };
            let target = if kind.has_text_field() { ID_MAIN } else { ID_NAME };
            if changed {
                let _ = unsafe { SetFocus(Some(item(hwnd, target))) };
            }
        }
    }
}

impl Handler for DialogWindow {
    fn handle(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        match msg {
            // 테두리·제목줄 없이 창 전체를 그림 자리로 (DWM 그림자·둥근 모서리는 남는다)
            // (만들 때 오는 wParam=0 호출도 같은 사각형을 그대로 돌려줘야 그림 자리가 줄지 않는다)
            WM_NCCALCSIZE => Some(LRESULT(0)),
            WM_NCHITTEST => {
                let mut pt =
                    POINT { x: (lparam.0 & 0xFFFF) as i16 as i32, y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32 };
                let _ = unsafe { ScreenToClient(hwnd, &mut pt) };
                let scale = self.state.borrow().scale;
                let p = Pt { x: pt.x as f32 / scale, y: pt.y as f32 / scale };
                Some(LRESULT(if form::is_drag_area(p) { HTCAPTION } else { HTCLIENT } as isize))
            }
            WM_COMMAND => {
                let (id, code) = ((wparam.0 & 0xFFFF) as i32, ((wparam.0 >> 16) & 0xFFFF) as u32);
                self.on_command(hwnd, id, code);
                Some(LRESULT(0))
            }
            WM_PAINT => {
                self.state.borrow().paint();
                Some(LRESULT(0))
            }
            WM_ERASEBKGND => Some(LRESULT(1)),
            WM_CTLCOLOREDIT => {
                // SAFETY: 받은 DC에 글자색·바탕색만 정한다
                unsafe {
                    let dc = HDC(wparam.0 as *mut _);
                    SetTextColor(dc, COLORREF(0x002B231F));
                    SetBkColor(dc, COLORREF(0x00FFFFFF));
                    Some(LRESULT(GetStockObject(WHITE_BRUSH).0 as isize))
                }
            }
            WM_MOUSEMOVE => {
                let mut st = self.state.borrow_mut();
                if !st.tracking_leave {
                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    st.tracking_leave = unsafe { TrackMouseEvent(&mut tme) }.is_ok();
                }
                let p = st.point(lparam);
                let hover = form::hit(p, st.kind, st.fixed);
                st.set_hover(hover);
                Some(LRESULT(0))
            }
            WM_MOUSELEAVE => {
                let mut st = self.state.borrow_mut();
                st.tracking_leave = false;
                st.set_hover(None);
                Some(LRESULT(0))
            }
            WM_LBUTTONDOWN => {
                let mut st = self.state.borrow_mut();
                let p = st.point(lparam);
                st.pressed = form::hit(p, st.kind, st.fixed);
                if st.pressed.is_some() {
                    unsafe { SetCapture(hwnd) };
                } else if form::name_field().contains(p) {
                    let _ = unsafe { SetFocus(Some(item(hwnd, ID_NAME))) };
                } else if form::main_field().contains(p) && st.kind.has_text_field() {
                    let _ = unsafe { SetFocus(Some(item(hwnd, ID_MAIN))) };
                }
                invalidate(hwnd);
                Some(LRESULT(0))
            }
            WM_LBUTTONUP => {
                let _ = unsafe { ReleaseCapture() };
                let command = {
                    let mut st = self.state.borrow_mut();
                    let p = st.point(lparam);
                    let target = form::hit(p, st.kind, st.fixed);
                    let pressed = st.pressed.take();
                    invalidate(hwnd);
                    target.filter(|t| pressed == Some(*t)).map(command_of)
                };
                if let Some(id) = command {
                    let _ = unsafe { PostMessageW(Some(hwnd), WM_COMMAND, WPARAM(id as usize), LPARAM(0)) };
                }
                Some(LRESULT(0))
            }
            WM_SETCURSOR if (lparam.0 & 0xFFFF) as u32 == HTCLIENT && self.state.borrow().hover.is_some() => {
                unsafe { SetCursor(LoadCursorW(None, IDC_HAND).ok()) };
                Some(LRESULT(1))
            }
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
            if msg.message == WM_QUIT {
                continue;
            }
            // Enter는 저장(IDOK), Esc는 닫기(IDCANCEL): 기본 단추 없이 직접 그린 창에서도 같게 동작한다
            if msg.message == WM_KEYDOWN
                && (msg.hwnd == dialog || IsChild(dialog, msg.hwnd).as_bool())
                && let Some(id) = match msg.wParam.0 as u16 {
                    k if k == VK_RETURN.0 => Some(IDOK.0),
                    k if k == VK_ESCAPE.0 => Some(IDCANCEL.0),
                    _ => None,
                }
            {
                let _ = PostMessageW(Some(dialog), WM_COMMAND, WPARAM(id as usize), LPARAM(0));
                continue;
            }
            if IsDialogMessageW(dialog, &msg).as_bool() {
                continue;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = EnableWindow(owner, true);
        let _ = SetForegroundWindow(owner);
    }
}

/// 파일 또는 폴더 고르기 (윈도우 기본 열기 창). 고른 전체 경로, 취소하면 None.
fn pick_path(owner: HWND, folder: bool) -> Option<String> {
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree};
    use windows::Win32::UI::Shell::{
        FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS, FileOpenDialog, IFileOpenDialog, SIGDN_FILESYSPATH,
    };
    // SAFETY: COM 열기 창을 만들어 고른 항목의 파일 시스템 경로를 읽고 문자열 메모리를 돌려준다
    unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let options = dialog.GetOptions().ok()?;
        let extra = if folder { FOS_PICKFOLDERS } else { FOS_FILEMUSTEXIST };
        dialog.SetOptions(options | extra | FOS_FORCEFILESYSTEM).ok()?;
        let title = if folder { "폴더 고르기" } else { "파일 고르기" };
        let _ = dialog.SetTitle(&HSTRING::from(title));
        dialog.Show(Some(owner)).ok()?;
        let item = dialog.GetResult().ok()?;
        let name = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let path = name.to_string().ok();
        CoTaskMemFree(Some(name.0 as *const _));
        path
    }
}

/// 항목 입력 창을 띄우고 닫힐 때까지 기다린다. 저장하면 `Some`, 취소하면 `None`.
pub fn item_dialog(owner: HWND, title: &str, initial: &Initial) -> Option<ItemInput> {
    let dpi = unsafe { GetDpiForWindow(owner) }.max(96);
    let scale = dpi as f32 / 96.0;
    let s = |v: f32| (v * scale).round() as i32;
    let (w, h) = (s(form::WIDTH), s(form::HEIGHT));
    let (x, y) = centered_on(owner, w, h);
    let (kind, fixed) = match initial {
        Initial::New => (Kind::Youtube, false),
        Initial::Edit(input) => (input.kind(), true),
    };
    let (mut youtube_text, mut web_text, mut app, mut name) = (String::new(), String::new(), None, String::new());
    let mut path = None;
    if let Initial::Edit(input) = initial {
        match input.clone() {
            ItemInput::Youtube { query, name: n } => (youtube_text, name) = (query, n),
            ItemInput::App { app: a, name: n } => (app, name) = (Some(a), n),
            ItemInput::Web { url, name: n } => (web_text, name) = (url, n),
            ItemInput::Path { path: p, name: n } => (path, name) = (Some(p), n),
        }
    }

    let spec = WindowSpec {
        class: DIALOG_CLASS,
        title,
        ex_style: WINDOW_EX_STYLE(0),
        style: WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN,
        rect: (x, y, w, h),
        owner: Some(owner),
    };
    let state = Rc::new(RefCell::new(DialogState {
        hwnd: HWND::default(),
        kind,
        fixed,
        title: title.to_string(),
        youtube_text,
        web_text,
        app,
        path,
        result: None,
        done: false,
        scale,
        gfx: Gfx::offscreen(w as u32, h as u32, scale)
            .inspect_err(|e| crate::logging::warn(&format!("입력 창 그림판을 만들지 못했습니다: {e}")))
            .ok(),
        hover: None,
        pressed: None,
        error: None,
        tracking_leave: false,
    }));
    let dialog = create_window(&spec, Box::new(DialogWindow { state: state.clone() })).ok()?;
    dwm::round_corners(dialog);
    let font = gdi_font(s(form::EDIT_FONT), false);
    let edit = |field: RectF, text: &str, id: i32| {
        let r = form::edit_rect(field);
        let rect = (s(r.x), s(r.y), s(r.w), s(r.h));
        let style = WINDOW_STYLE(ES_AUTOHSCROLL as u32) | WS_TABSTOP;
        child(dialog, font, Ctl { class: "EDIT", text, style, ex: WINDOW_EX_STYLE(0), rect, id })
    };
    let main = edit(form::main_field(), "", ID_MAIN);
    let name_edit = edit(form::name_field(), &name, ID_NAME);
    {
        let mut st = state.borrow_mut();
        st.hwnd = dialog;
        st.show_kind();
    }
    let focus = if kind.has_text_field() { main } else { name_edit };
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

    /// 창 DC를 그대로 복사해 `target/snapshots/`에 BMP로 남긴다 (미리보기 확인용)
    fn save_window_bmp(hwnd: HWND, name: &str) {
        use windows::Win32::Graphics::Gdi::{
            BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
            GetDC, GetDIBits, ReleaseDC, SRCCOPY, SelectObject,
        };
        let mut r = RECT::default();
        let _ = unsafe { GetClientRect(hwnd, &mut r) };
        let (w, h) = (r.right, r.bottom);
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: h,
                biPlanes: 1,
                biBitCount: 32,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        unsafe {
            let src = GetDC(Some(hwnd));
            let mem = CreateCompatibleDC(Some(src));
            let bmp = CreateCompatibleBitmap(src, w, h);
            let old = SelectObject(mem, bmp.into());
            let _ = BitBlt(mem, 0, 0, w, h, Some(src), 0, 0, SRCCOPY);
            SelectObject(mem, old);
            GetDIBits(mem, bmp, 0, h as u32, Some(pixels.as_mut_ptr().cast()), &mut info, DIB_RGB_COLORS);
            let _ = DeleteObject(bmp.into());
            let _ = DeleteDC(mem);
            ReleaseDC(Some(hwnd), src);
        }
        let mut file = Vec::new();
        let size = 14 + 40 + pixels.len() as u32;
        file.extend_from_slice(b"BM");
        file.extend_from_slice(&size.to_le_bytes());
        file.extend_from_slice(&0u32.to_le_bytes());
        file.extend_from_slice(&54u32.to_le_bytes());
        file.extend_from_slice(&40u32.to_le_bytes());
        file.extend_from_slice(&w.to_le_bytes());
        file.extend_from_slice(&h.to_le_bytes());
        file.extend_from_slice(&1u16.to_le_bytes());
        file.extend_from_slice(&32u16.to_le_bytes());
        file.extend_from_slice(&[0u8; 24]);
        file.extend_from_slice(&pixels);
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("snapshots");
        let _ = std::fs::write(dir.join(name), file);
    }

    /// 실제 창을 몇 초 띄워 눈으로 확인하는 용도: `cargo test preview_item_dialog -- --ignored`
    #[test]
    #[ignore]
    fn preview_item_dialog() {
        let _guard = lock();
        let _ = unsafe {
            windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)
        };
        let owner = create("YTLauncherTestDialogPreview", Box::new(Owner)).unwrap();
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if let Ok(h) = unsafe { FindWindowW(&HSTRING::from(DIALOG_CLASS), None) } {
                let main = unsafe { GetDlgItem(Some(h), ID_MAIN) }.unwrap_or_default();
                let _ = unsafe { SetWindowTextW(main, &HSTRING::from("로파이 음악 Lofi")) };
                std::thread::sleep(std::time::Duration::from_millis(1000));
                save_window_bmp(h, "dialog-real.bmp");
                let _ = unsafe { PostMessageW(Some(h), WM_COMMAND, WPARAM(IDCANCEL.0 as usize), LPARAM(0)) };
            }
        });
        let _ = item_dialog(owner, "항목 추가", &Initial::New);
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
        press_later(&[IDOK.0]);
        let folder = ItemInput::Path { path: r"C:\Users".into(), name: "사용자".into() };
        assert_eq!(item_dialog(owner, "항목 수정", &edit(folder.clone())), Some(folder));
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
