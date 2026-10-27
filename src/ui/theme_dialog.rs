//! 사용자 지정 테마 설정 창. 그림은 `theme_form`, HEX 글자 입력만 기본 EDIT 컨트롤.
//! 값이 바뀔 때마다 `on_change`로 알려 독·패널에 바로 보이게 하고(미리보기), 저장하면 `Some`, 취소하면 `None`.

use crate::config::CustomTheme;
use crate::ui::dialogs::{
    Ctl, EN_CHANGE, WM_MOUSELEAVE, centered_on, child, invalidate, item, run_modal, set_cue, set_text, window_text,
};
use crate::ui::gfx::{Gfx, gdi_font};
use crate::ui::layout::{Pt, RectF};
use crate::ui::theme::{PRESETS, parse_hex, to_hex};
use crate::ui::theme_form::{self, FormView, Target};
use crate::win::dwm;
use crate::win::msg_window::{Handler, WindowSpec, create_window};
use std::cell::RefCell;
use std::rc::Rc;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, DeleteObject, EndPaint, FillRect, GetStockObject, HBRUSH, HDC, HGDIOBJ, PAINTSTRUCT, ScreenToClient,
    SetBkColor, SetTextColor, WHITE_BRUSH,
};
use windows::Win32::UI::Controls::Dialogs::{CC_FULLOPEN, CC_RGBINIT, CHOOSECOLORW, ChooseColorW};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetFocus, ReleaseCapture, SetCapture, SetFocus, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, ES_AUTOHSCROLL, GetClientRect, HTCAPTION, HTCLIENT, IDC_HAND, IDCANCEL, IDOK, LoadCursorW,
    PostMessageW, SetCursor, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_CTLCOLOREDIT, WM_ERASEBKGND,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCCALCSIZE, WM_NCHITTEST, WM_PAINT, WM_SETCURSOR, WS_CAPTION,
    WS_CLIPCHILDREN, WS_POPUP, WS_SYSMENU, WS_TABSTOP,
};

const DIALOG_CLASS: &str = "DockUIThemeDialog";
const ID_HEX: i32 = 301;

/// 0xRRGGBB ↔ COLORREF(0x00BBGGRR)
fn to_colorref(rgb: u32) -> COLORREF {
    COLORREF(((rgb & 0xFF) << 16) | (rgb & 0xFF00) | ((rgb >> 16) & 0xFF))
}

fn from_colorref(c: COLORREF) -> u32 {
    ((c.0 & 0xFF) << 16) | (c.0 & 0xFF00) | ((c.0 >> 16) & 0xFF)
}

struct State {
    hwnd: HWND,
    theme: CustomTheme,
    result: Option<CustomTheme>,
    done: bool,
    scale: f32,
    gfx: Option<Gfx>,
    hover: Option<Target>,
    pressed: Option<Target>,
    hex_error: bool,
    tracking_leave: bool,
    /// "더 많은 색" 창의 사용자 색 칸 (창을 다시 열어도 남게)
    custom_colors: [COLORREF; 16],
    on_change: Rc<dyn Fn(CustomTheme)>,
}

impl State {
    fn point(&self, l: LPARAM) -> Pt {
        let (x, y) = ((l.0 & 0xFFFF) as i16 as f32, ((l.0 >> 16) & 0xFFFF) as i16 as f32);
        Pt { x: x / self.scale, y: y / self.scale }
    }

    /// 값을 바꾸고 미리보기로 알린 뒤 다시 그린다
    fn set(&mut self, theme: CustomTheme) {
        if theme != self.theme {
            self.theme = theme;
            (self.on_change)(theme);
        }
        invalidate(self.hwnd);
    }

    /// 색을 바꾸고 HEX 칸 글자도 맞춘다 (추천 색·더 많은 색)
    fn set_color(&mut self, rgb: u32) {
        self.hex_error = false;
        self.set(CustomTheme { color: rgb, ..self.theme });
        set_text(item(self.hwnd, ID_HEX), &to_hex(rgb));
    }

    fn paint(&self) {
        let mut ps = PAINTSTRUCT::default();
        // SAFETY: WM_PAINT 안에서 BeginPaint/EndPaint 짝을 맞춘다
        unsafe {
            let hdc = BeginPaint(self.hwnd, &mut ps);
            let drawn = self.gfx.as_ref().is_some_and(|gfx| {
                gfx.begin();
                theme_form::draw(
                    gfx,
                    &FormView {
                        theme: self.theme,
                        hover: self.hover,
                        pressed: self.pressed,
                        hex_focus: GetFocus() == item(self.hwnd, ID_HEX),
                        hex_error: self.hex_error,
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
}

struct Window {
    state: Rc<RefCell<State>>,
}

impl Window {
    fn close(&self, hwnd: HWND) {
        self.state.borrow_mut().done = true;
        let _ = unsafe { DestroyWindow(hwnd) };
    }

    /// 윈도우 색 선택 창 (지금 색으로 시작, 사용자 색 칸 유지)
    fn choose_more(&self, hwnd: HWND) {
        let (initial, mut custom) = {
            let st = self.state.borrow();
            (st.theme.color, st.custom_colors)
        };
        let mut cc = CHOOSECOLORW {
            lStructSize: std::mem::size_of::<CHOOSECOLORW>() as u32,
            hwndOwner: hwnd,
            rgbResult: to_colorref(initial),
            lpCustColors: custom.as_mut_ptr(),
            Flags: CC_FULLOPEN | CC_RGBINIT,
            ..Default::default()
        };
        // SAFETY: 구조체와 사용자 색 배열은 호출이 끝날 때까지 살아 있다
        let picked = unsafe { ChooseColorW(&mut cc) }.as_bool();
        let mut st = self.state.borrow_mut();
        st.custom_colors = custom;
        if picked {
            st.set_color(from_colorref(cc.rgbResult));
        }
    }

    fn act(&self, hwnd: HWND, target: Target) {
        match target {
            Target::Close | Target::Cancel => self.close(hwnd),
            Target::Ok => {
                let _ = unsafe { PostMessageW(Some(hwnd), WM_COMMAND, WPARAM(IDOK.0 as usize), LPARAM(0)) };
            }
            Target::Swatch(i) => self.state.borrow_mut().set_color(PRESETS[i]),
            Target::More => self.choose_more(hwnd),
            Target::Blur => {
                let mut st = self.state.borrow_mut();
                let theme = CustomTheme { blur: !st.theme.blur, ..st.theme };
                st.set(theme);
            }
            Target::Slider => {}
        }
    }
}

impl Handler for Window {
    fn handle(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        match msg {
            WM_NCCALCSIZE => Some(LRESULT(0)),
            WM_NCHITTEST => {
                let mut pt =
                    POINT { x: (lparam.0 & 0xFFFF) as i16 as i32, y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32 };
                let _ = unsafe { ScreenToClient(hwnd, &mut pt) };
                let scale = self.state.borrow().scale;
                let p = Pt { x: pt.x as f32 / scale, y: pt.y as f32 / scale };
                Some(LRESULT(if theme_form::is_drag_area(p) { HTCAPTION } else { HTCLIENT } as isize))
            }
            WM_COMMAND => {
                let (id, code) = ((wparam.0 & 0xFFFF) as i32, ((wparam.0 >> 16) & 0xFFFF) as u32);
                if id == ID_HEX {
                    // 글자를 우리가 바꾼 직후(상태를 빌린 중)에는 건너뛴다
                    if code == EN_CHANGE
                        && let Ok(mut st) = self.state.try_borrow_mut()
                    {
                        let text = window_text(item(hwnd, ID_HEX));
                        match parse_hex(&text) {
                            Some(rgb) => {
                                st.hex_error = false;
                                let theme = CustomTheme { color: rgb, ..st.theme };
                                st.set(theme);
                            }
                            None => {
                                st.hex_error = !text.trim().is_empty();
                                invalidate(hwnd);
                            }
                        }
                    } else {
                        invalidate(hwnd);
                    }
                } else if id == IDOK.0 {
                    let theme = self.state.borrow().theme;
                    self.state.borrow_mut().result = Some(theme);
                    self.close(hwnd);
                } else if id == IDCANCEL.0 {
                    self.close(hwnd);
                }
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
                if st.pressed == Some(Target::Slider) {
                    let theme = CustomTheme { opacity: theme_form::opacity_at(p.x), ..st.theme };
                    st.set(theme);
                } else {
                    let hover = theme_form::hit(p);
                    if hover != st.hover {
                        st.hover = hover;
                        invalidate(hwnd);
                    }
                }
                Some(LRESULT(0))
            }
            WM_MOUSELEAVE => {
                let mut st = self.state.borrow_mut();
                st.tracking_leave = false;
                st.hover = None;
                invalidate(hwnd);
                Some(LRESULT(0))
            }
            WM_LBUTTONDOWN => {
                let mut st = self.state.borrow_mut();
                let p = st.point(lparam);
                st.pressed = theme_form::hit(p);
                if st.pressed == Some(Target::Slider) {
                    let theme = CustomTheme { opacity: theme_form::opacity_at(p.x), ..st.theme };
                    st.set(theme);
                }
                if st.pressed.is_some() {
                    unsafe { SetCapture(hwnd) };
                } else if theme_form::hex_field().contains(p) {
                    let _ = unsafe { SetFocus(Some(item(hwnd, ID_HEX))) };
                }
                invalidate(hwnd);
                Some(LRESULT(0))
            }
            WM_LBUTTONUP => {
                let _ = unsafe { ReleaseCapture() };
                let target = {
                    let mut st = self.state.borrow_mut();
                    let p = st.point(lparam);
                    let pressed = st.pressed.take();
                    invalidate(hwnd);
                    theme_form::hit(p).filter(|t| pressed == Some(*t) && *t != Target::Slider)
                };
                if let Some(t) = target {
                    self.act(hwnd, t);
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

/// 사용자 지정 테마 설정 창을 띄우고 닫힐 때까지 기다린다. 저장하면 `Some`, 취소하면 `None`.
/// 값이 바뀔 때마다 `on_change`를 부른다 (미리보기). 취소했을 때 되돌리는 것은 부른 쪽이 한다.
pub fn custom_theme_dialog(
    owner: HWND,
    initial: CustomTheme,
    on_change: Rc<dyn Fn(CustomTheme)>,
) -> Option<CustomTheme> {
    let dpi = unsafe { GetDpiForWindow(owner) }.max(96);
    let scale = dpi as f32 / 96.0;
    let s = |v: f32| (v * scale).round() as i32;
    let (w, h) = (s(theme_form::WIDTH), s(theme_form::HEIGHT));
    let (x, y) = centered_on(owner, w, h);
    let spec = WindowSpec {
        class: DIALOG_CLASS,
        title: "사용자 지정 테마",
        ex_style: WINDOW_EX_STYLE(0),
        style: WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN,
        rect: (x, y, w, h),
        owner: Some(owner),
    };
    let state = Rc::new(RefCell::new(State {
        hwnd: HWND::default(),
        theme: initial,
        result: None,
        done: false,
        scale,
        gfx: Gfx::offscreen(w as u32, h as u32, scale)
            .inspect_err(|e| crate::logging::warn(&format!("테마 창 그림판을 만들지 못했습니다: {e}")))
            .ok(),
        hover: None,
        pressed: None,
        hex_error: false,
        tracking_leave: false,
        custom_colors: [COLORREF(0x00FFFFFF); 16],
        on_change,
    }));
    let dialog = create_window(&spec, Box::new(Window { state: state.clone() })).ok()?;
    dwm::round_corners(dialog);
    let font = gdi_font(s(theme_form::EDIT_FONT), false);
    let r: RectF = theme_form::hex_edit_rect();
    let style = WINDOW_STYLE(ES_AUTOHSCROLL as u32) | WS_TABSTOP;
    let rect = (s(r.x), s(r.y), s(r.w), s(r.h));
    let hex = child(
        dialog,
        font,
        Ctl { class: "EDIT", text: &to_hex(initial.color), style, ex: WINDOW_EX_STYLE(0), rect, id: ID_HEX },
    );
    set_cue(hex, "#1E1E1E");
    state.borrow_mut().hwnd = dialog;
    run_modal(owner, dialog, hex, &|| state.borrow().done);
    let _ = unsafe { DeleteObject(HGDIOBJ(font.0)) };
    state.borrow_mut().result.take()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colorref_round_trips() {
        assert_eq!(to_colorref(0x112233), COLORREF(0x00332211));
        assert_eq!(from_colorref(to_colorref(0x0F4C5C)), 0x0F4C5C);
    }
}
