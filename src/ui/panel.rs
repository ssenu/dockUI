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
use crate::ui::scene::{self, DockScene, DragView, SceneState, TileView};
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
            (d.layout.headroom() * s).round() as i32,
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
        // 말풍선은 판보다 넓거나 옆으로 길 수 있으므로 그 사각형을 따로 넣는다 (안 넣으면 잘린다)
        if let Some(r) = self.dock_tooltip_rect(&m) {
            rects.push(px(RectF::new(r.x - 2.0, r.y - 2.0, r.w + 4.0, r.h + 4.0)));
        }
        if let Some(region) = region_from(&rects) {
            let _ = unsafe { SetWindowRgn(self.content, Some(region), true) };
        }
    }

    fn dock_size(&self) -> (f32, f32) {
        (self.dock.content_px.2 as f32 / self.scale, self.dock.content_px.3 as f32 / self.scale)
    }

    fn dock_tooltip_rect(&self, m: &Magnified) -> Option<RectF> {
        let (gfx, i) = (self.gfx.as_ref()?, self.dock_tooltip()?);
        let layout = &self.dock.layout;
        let text = scene::dock_tooltip_text(layout, &self.tiles, i);
        let icon = layout.slot_rect(m, self.dock_center(), i);
        Some(scene::dock_tooltip_rect(gfx, text, icon, layout.edge, self.dock_size()))
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
                    size: self.dock_size(),
                    palette: self.palette(),
                    acrylic: self.acrylic,
                    tiles: &self.tiles,
                    pressed,
                    tooltip: self.dock_tooltip(),
                    drag: self.drag,
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

    /// 테마 선택기는 패널에만 있다 (독은 트레이 메뉴에서 테마를 고른다)
    fn swatch_centers(&self) -> Vec<Pt> {
        match self.mode {
            Mode::Panel => self.layout.swatch_centers(PICKER_ORDER.len(), self.picker.progress()),
            Mode::Dock => Vec::new(),
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
