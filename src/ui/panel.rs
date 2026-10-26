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
