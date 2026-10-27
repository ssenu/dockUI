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
use crate::ui::dock_layout::{self, DockLayout, Magnified, Slot};
use crate::ui::drop::{self, Dropped};
use crate::ui::gfx::Gfx;
use crate::ui::layout::{
    EDGE_BOTTOM, EDGE_LEFT, EDGE_RIGHT, EDGE_TOP, Hit, MIN_W, PanelLayout, Pt, RectF, default_panel_rect,
    is_visible_on_any, reorder,
};
use crate::ui::scene::{self, DockScene, DragView, SceneState, TileKind, TileView, WINDOW_RADIUS};
use crate::ui::theme::{self, Palette};
use crate::win::desktop::{self, PxRect};
use crate::win::dwm;
use crate::win::menu::{MenuEntry, show_popup};
use crate::win::msg_window::{Handler, WindowSpec, create_window};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Instant;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::DwmFlush;
use windows::Win32::Graphics::Gdi::{
    BLACK_BRUSH, BeginPaint, CombineRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject, EndPaint, FillRect,
    GetStockObject, HBRUSH, HGDIOBJ, HRGN, PAINTSTRUCT, RGN_OR, ScreenToClient, SetWindowRgn,
};
use windows::Win32::Media::{timeBeginPeriod, timeEndPeriod};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, GetCursorPos, GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId, HTBOTTOM, HTBOTTOMLEFT,
    HTBOTTOMRIGHT, HTCAPTION, HTCLIENT, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, HTTRANSPARENT, IDC_SIZEALL,
    KillTimer, LoadCursorW, MA_NOACTIVATE, MINMAXINFO, PostMessageW, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, SetCursor, SetTimer, SetWindowPos, ShowWindow, WA_INACTIVE, WINDOWPOS, WM_ACTIVATE,
    WM_APP, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_NCHITTEST, WM_PAINT, WM_RBUTTONUP, WM_SETCURSOR, WM_SETTINGCHANGE, WM_SIZING,
    WM_TIMER, WM_WINDOWPOSCHANGED, WM_WINDOWPOSCHANGING, WMSZ_TOP, WMSZ_TOPLEFT, WMSZ_TOPRIGHT, WS_EX_NOACTIVATE,
    WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_POPUP,
};

/// 앱 → 화면 명령이 채널에 들어왔음을 알리는 메시지
pub const WM_APP_PANEL: u32 = WM_APP + 10;
const TIMER_ANIM: usize = 1;
const TIMER_SAVE_GEOMETRY: usize = 2;
/// 애니메이션 타이머 간격. 실제 프레임은 화면 합성 주기(DwmFlush)에 맞춘다.
const FRAME_MS: u32 = 10;
/// 프레임 사이 시간이 이보다 길면(창이 잠깐 멈춤 등) 이만큼만 진행한 것으로 본다
const MAX_FRAME_DT: f32 = 50.0;
const SAVE_DELAY_MS: u32 = 500;
const DRAG_THRESHOLD: f32 = 6.0;
const FADE_MS: f32 = 200.0;
/// 독 말풍선이 차지하는 여유 (창 영역 계산용)
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
    /// 끌어 놓기 대상 위에 받을 수 있는 것이 올라옴 / 떠남 (강조 표시)
    DropHover(bool),
    /// 끌어다 놓은 파일·주소 (앱에 그대로 넘긴다)
    Dropped(Vec<Dropped>),
    /// 독 수정 모드 켜기/끄기
    SetEditing(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum UiEvent {
    TileClicked(String),
    AddRequested,
    EditRequested(String),
    DeleteRequested(String),
    Reordered(Vec<String>),
    /// 패널 모드에서 위치·크기가 바뀜
    GeometryChanged(Rect),
    MenuRequested,
    /// 독·패널에 파일·주소를 끌어다 놓음
    Dropped(Vec<Dropped>),
    /// 독 수정 모드에서 위치·가장자리·크기가 바뀜 (저장)
    DockChanged {
        edge: DockEdge,
        offset: f32,
        size: u32,
    },
    /// 독 수정 모드가 켜지거나 꺼짐 (트레이 메뉴 체크 표시)
    EditingChanged(bool),
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
    /// 독 위치 (가장자리를 따라 0..1, 0.5가 가운데)
    pub dock_offset: f32,
    /// 독 아이콘 크기 (DIP)
    pub dock_size: u32,
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
    /// 가장자리를 따라 놓인 위치 0..1
    offset: f32,
    /// 아이콘 크기 (DIP, 48이 기본)
    size: u32,
    /// 수정 모드: 판 끌어 옮기기, 크기 단추, 아이콘 순서 바꾸기
    editing: bool,
    /// 판을 끌어 옮기는 중: 누른 곳과 판 중심의 along 거리 (px)
    moving: Option<i32>,
    /// 마우스가 판 위에 있음 (표시 영역을 넓혀 둔다)
    inside: bool,
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
    locked: bool,
    animating: bool,
    /// 지난 애니메이션 프레임 시각 (경과 시간으로 진행)
    last_frame: Instant,
    /// 마지막으로 놓은 유리 창·창 영역 (같으면 다시 하지 않는다)
    last_glass: Option<PxRect>,
    /// 독 흐림(유리) 창의 화면 사각형 (가장 커졌을 때 판 크기로 고정)
    glass_px: PxRect,
    last_region: Vec<PxRect>,
    /// 패널 콘텐츠 창이 패널 좌우로 더 차지하는 폭 (px, 긴 말풍선용)
    panel_side: i32,
    /// 말풍선 불투명도 (페이드)와 마지막으로 보인 칸 (사라질 때도 그 자리에서 사라지게). 독·패널 공용
    tip: Anim,
    tip_slot: Option<usize>,
    rx: Receiver<PanelCmd>,
    /// 끌어 놓기 대상이 화면에 명령을 보낼 채널
    tx: Sender<PanelCmd>,
    /// 끌어 놓을 수 있는 것이 위에 있음
    drop_hover: bool,
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

/// 패널 콘텐츠 창이 패널 좌우로 더 차지하는 폭 (px): 가장 긴 말풍선이 패널보다 넓을 때만
fn panel_side_px(longest_tip: f32, panel_w: f32, scale: f32) -> i32 {
    (((longest_tip + 16.0 - panel_w) / 2.0).max(0.0) * scale).ceil() as i32
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
            title: crate::app::APP_NAME,
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
                layout: DockLayout::compute(spec.edge, n, false),
                edge: spec.edge,
                pointer: None,
                amount: Anim::at(0.0),
                center_px: (0, 0),
                content_px: spec.panel_rect,
                offset: spec.dock_offset,
                size: spec.dock_size,
                editing: false,
                moving: None,
                inside: false,
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
            locked: spec.locked,
            animating: false,
            last_frame: Instant::now(),
            last_glass: None,
            glass_px: (0, 0, 0, 0),
            last_region: Vec::new(),
            panel_side: 0,
            tip: Anim::at(0.0),
            tip_slot: None,
            rx,
            tx: tx.clone(),
            drop_hover: false,
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
        // 콘텐츠 창이 유리 창을 덮고 있어 다른 프로세스에서 끌어 오면 콘텐츠 창이 놓을 곳이 된다: 둘 다 등록
        drop::register(&[self.glass, self.content], self.glass, self.tx.clone());
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

    /// 독 그리기 배율: DPI 배율 × 아이콘 크기 배율 (독은 48px 기준으로 배치하고 통째로 키운다)
    fn dock_scale(&self) -> f32 {
        self.scale * dock_layout::zoom_for(self.dock.size)
    }

    /// 지금 모드의 그리기 배율 (DIP → px)
    fn draw_scale(&self) -> f32 {
        match self.mode {
            Mode::Panel => self.scale,
            Mode::Dock => self.dock_scale(),
        }
    }

    fn resize_gfx(&mut self, w: i32, h: i32) {
        let scale = self.draw_scale();
        if let Some(gfx) = &mut self.gfx
            && let Err(e) = gfx.resize(w as u32, h as u32, scale)
        {
            logging::error(&format!("그리기 크기 변경 실패: {e}"));
        }
    }

    /// 지금 모드에 맞게 두 창의 위치·크기를 정한다.
    fn place_windows(&mut self) {
        // 창을 새로 놓으므로 기억해 둔 독 유리 창·영역은 무효
        self.last_glass = None;
        self.last_region.clear();
        self.positioning = true;
        match self.mode {
            Mode::Panel => {
                // 독에서 걸어 둔 흐림 모양을 풀고 패널 크기로
                let _ = unsafe { SetWindowRgn(self.glass, None, true) };
                move_window(self.glass, self.panel_rect);
                self.relayout_panel();
            }
            Mode::Dock => self.place_dock(),
        }
        self.positioning = false;
    }

    fn relayout_panel(&mut self) {
        let rect = screen_rect(self.glass);
        self.relayout_panel_to(rect);
    }

    /// 패널이 `rect`(화면 px)에 있다고 보고 레이아웃·콘텐츠 창·그리기 버퍼를 맞춘다
    fn relayout_panel_to(&mut self, rect: PxRect) {
        let (x, y, w, h) = rect;
        self.panel_rect = (x, y, w, h);
        self.layout = PanelLayout::compute(w as f32 / self.scale, h as f32 / self.scale, self.tiles.len());
        self.hover.resize(self.tiles.len() + 1, Anim::at(0.0));
        // 콘텐츠 창은 패널보다 좌우로 넓게: 패널보다 긴 이름 말풍선이 패널 밖까지 한 줄로 나가게
        let side = panel_side_px(self.longest_tip(), w as f32 / self.scale, self.scale);
        self.panel_side = side;
        if !self.content.0.is_null() {
            move_window(self.content, (x - side, y, w + 2 * side, h));
        }
        self.resize_gfx(w + 2 * side, h);
        self.last_region.clear();
        self.update_panel_region();
    }

    /// 패널 말풍선 사각형 (콘텐츠 창 기준 DIP)
    fn panel_tooltip_rect(&self, cell: usize) -> Option<RectF> {
        let gfx = self.gfx.as_ref()?;
        let anchor = *self.layout.cells.get(cell)?;
        let side = self.panel_side as f32 / self.scale;
        let anchor = RectF::new(anchor.x + side, anchor.y, anchor.w, anchor.h);
        let text = scene::panel_tooltip_text(&self.tiles, cell);
        let right = self.layout.width + 2.0 * side;
        Some(scene::panel_tooltip_rect(gfx, text, anchor, 0.0, right, self.layout.header.bottom()))
    }

    /// 패널 콘텐츠 창 표시 영역: 평소엔 패널만, 말풍선이 보이는 동안만 패널 + 말풍선 (바뀔 때만 설정)
    fn update_panel_region(&mut self) {
        if self.mode != Mode::Panel || self.content.0.is_null() {
            return;
        }
        let (_, _, w, h) = self.panel_rect;
        let mut rects = vec![(self.panel_side, 0, w, h)];
        if self.tip.value > 0.0
            && let Some(r) = self.tip_slot.and_then(|i| self.panel_tooltip_rect(i))
        {
            let s = self.scale;
            let x = ((r.x - 2.0) * s).floor() as i32;
            let y = ((r.y - 2.0) * s).floor() as i32;
            rects.push((x, y, ((r.right() + 2.0) * s).ceil() as i32 - x, ((r.bottom() + 2.0) * s).ceil() as i32 - y));
        }
        if rects != self.last_region
            && let Some(region) = region_from(&rects)
        {
            self.last_region = rects;
            let _ = unsafe { SetWindowRgn(self.content, Some(region), true) };
        }
    }

    /// 패널에서 말풍선을 띄울 칸 (마우스가 올라간 항목·＋)
    fn panel_tooltip(&self) -> Option<usize> {
        if self.drag.is_some() {
            return None;
        }
        match self.hover_hit {
            Hit::Tile(i) => Some(i),
            Hit::Add => Some(self.tiles.len()),
            _ => None,
        }
    }

    /// 지금 항목들 중 가장 긴 말풍선 폭 (DIP, 작업 영역 폭을 넘지 않게)
    fn longest_tip(&self) -> f32 {
        let cap = desktop::primary_work_area().2 as f32 / self.scale;
        let Some(gfx) = &self.gfx else { return 200.0 };
        let mut labels: Vec<&str> = self.tiles.iter().map(|t| t.label.as_str()).collect();
        labels.push("항목 추가");
        scene::longest_tip(gfx, &labels, cap)
    }

    fn place_dock(&mut self) {
        // 독 그리기 배율로 잰 폭이 아니라 DIP 폭을 쓰므로 배율로 나눠 맞춘다
        let tip = self.longest_tip() / dock_layout::zoom_for(self.dock.size);
        let s = self.dock_scale();
        let d = &mut self.dock;
        d.layout = DockLayout::compute(d.edge, self.tiles.len(), d.editing);
        let work = desktop::primary_work_area();
        let px = |v: f32| (v * s).round() as i32;
        let thick = px(dock_layout::THICK);
        let widest = px(d.layout.max_length());
        d.center_px =
            dock_layout::bar_center(work, d.edge, widest, thick, px(dock_layout::edge_margin(d.edge)), d.offset);
        let widest_rect = dock_layout::rect_around(d.center_px, d.edge, widest, thick);
        d.content_px =
            dock_layout::content_rect(widest_rect, d.edge, px(d.layout.headroom(tip)), px(d.layout.side_room(tip)));
        let content = d.content_px;
        move_window(self.content, content);
        // 흐림(유리) 창은 가장 커졌을 때 판 크기로 한 번만 놓고, 확대 중에는 흐림 모양만 바꾼다
        self.glass_px = widest_rect;
        self.positioning = true;
        move_window(self.glass, widest_rect);
        self.positioning = false;
        self.resize_gfx(content.2, content.3);
        self.update_dock_glass();
    }

    fn dock_mag(&self) -> Magnified {
        self.dock.layout.magnified(self.dock.pointer, self.dock.amount.value)
    }

    /// 판 중심 (콘텐츠 창 기준 DIP)
    fn dock_center(&self) -> (f32, f32) {
        let d = &self.dock;
        let s = self.dock_scale();
        ((d.center_px.0 - d.content_px.0) as f32 / s, (d.center_px.1 - d.content_px.1) as f32 / s)
    }

    /// 확대된 길이에 맞춰 유리 창(판)을 옮기고, 콘텐츠 창 영역을 지금 그려지는 부분으로 줄인다.
    fn update_dock_glass(&mut self) {
        let m = self.dock_mag();
        let bar = self.dock.layout.bar_rect(&m, self.dock_center());
        let s = self.dock_scale();
        let (cx, cy, _, _) = self.dock.content_px;
        let px = |r: RectF| {
            let x = (r.x * s).round() as i32;
            let y = (r.y * s).round() as i32;
            (x, y, (r.right() * s).round() as i32 - x, (r.bottom() * s).round() as i32 - y)
        };
        // 흐림 창을 매 프레임 옮기거나 크기를 바꾸지 않고(창 이동·다시 칠하기가 끊김의 원인),
        // 흐림이 보이는 모양만 지금 판(둥근 사각형)으로 바꾼다. 모양이 같으면 건너뛴다.
        let local = px(bar);
        let (gx, gy, _, _) = self.glass_px;
        let shape = (cx + local.0 - gx, cy + local.1 - gy, local.2, local.3);
        if self.last_glass != Some(shape) {
            self.last_glass = Some(shape);
            let d = (WINDOW_RADIUS * 2.0 * s).round() as i32;
            // SAFETY: 새 영역을 만들어 넘긴다 (영역은 SetWindowRgn이 가진다)
            unsafe {
                let rgn = CreateRoundRectRgn(shape.0, shape.1, shape.0 + shape.2 + 1, shape.1 + shape.3 + 1, d, d);
                let _ = SetWindowRgn(self.glass, Some(rgn), false);
            }
        }

        // 창 영역은 두 상태로만 바꾼다: 쉼(판 + 수정 안내) / 활성(콘텐츠 창 전체).
        // 확대 애니메이션 매 프레임 영역을 바꾸면 새 그림과 어긋나 말풍선이 잘리고 잔상이 남는다.
        let busy = self.drag.is_some() || self.dock.moving.is_some();
        let active = dock_layout::region_active(self.dock.inside, busy, self.dock.amount.value, self.tip.value);
        let rects = if active {
            let (_, _, w, h) = self.dock.content_px;
            vec![(0, 0, w, h)]
        } else {
            let mut v = vec![px(bar)];
            if self.dock.editing
                && let Some(gfx) = &self.gfx
            {
                let r = scene::edit_hint_rect(gfx, bar, self.dock.edge, self.dock_size());
                v.push(px(RectF::new(r.x - 2.0, r.y - 2.0, r.w + 4.0, r.h + 4.0)));
            }
            v
        };
        if rects != self.last_region
            && let Some(region) = region_from(&rects)
        {
            self.last_region = rects;
            let _ = unsafe { SetWindowRgn(self.content, Some(region), true) };
        }
    }

    fn dock_size(&self) -> (f32, f32) {
        let s = self.dock_scale();
        (self.dock.content_px.2 as f32 / s, self.dock.content_px.3 as f32 / s)
    }

    /// 마우스가 올라간 독 칸 번호
    fn dock_hovered(&self) -> Option<usize> {
        let slots = self.dock.layout.slots();
        let find = |s: Slot| slots.iter().position(|x| *x == s);
        match self.hover_hit {
            Hit::Tile(i) => find(Slot::Tile(i)),
            Hit::Add => find(Slot::Add),
            Hit::Shrink => find(Slot::Shrink),
            Hit::Grow => find(Slot::Grow),
            Hit::Done => find(Slot::Done),
            _ => None,
        }
    }

    fn dock_tooltip(&self) -> Option<usize> {
        if self.drag.is_some() || self.dock.moving.is_some() {
            return None;
        }
        // 수정 모드는 확대가 없으므로 수정 단추에만 말풍선
        if self.dock.editing {
            return self.dock_hovered().filter(|_| matches!(self.hover_hit, Hit::Shrink | Hit::Grow | Hit::Done));
        }
        if self.dock.amount.value < 0.5 {
            return None;
        }
        self.dock_hovered()
    }

    /// 판을 끌어 옮기는 중: 커서에서 가장 가까운 가장자리로 붙이고 그 가장자리를 따라 옮긴다.
    fn move_dock(&mut self, grab: i32) {
        let mut pt = POINT::default();
        let _ = unsafe { GetCursorPos(&mut pt) };
        let work = desktop::primary_work_area();
        let edge = dock_layout::nearest_edge(work, (pt.x, pt.y));
        let grab = if edge == self.dock.edge { grab } else { 0 };
        self.dock.edge = edge;
        self.dock.moving = Some(grab);
        let along = if dock_layout::is_vertical(edge) { pt.y } else { pt.x } - grab;
        let layout = DockLayout::compute(edge, self.tiles.len(), self.dock.editing);
        let widest = (layout.max_length() * self.dock_scale()).round() as i32;
        self.dock.offset = dock_layout::offset_for(work, edge, widest, along);
        self.place_windows();
        self.render();
    }

    fn emit_dock_changed(&self) {
        let d = &self.dock;
        self.emit(UiEvent::DockChanged { edge: d.edge, offset: d.offset, size: d.size });
    }

    fn set_editing(&mut self, on: bool) {
        if self.dock.editing == on {
            return;
        }
        self.dock.editing = on;
        self.dock.pointer = None;
        self.dock.amount = Anim::at(0.0);
        self.dock.moving = None;
        self.hover_hit = Hit::Empty;
        if self.mode == Mode::Dock {
            self.place_windows();
        }
        self.render();
        self.emit(UiEvent::EditingChanged(on));
    }

    /// 한 프레임 그리기. 그래픽 장치를 잃었으면(절전 복귀, 드라이버 재시작 등) 장치를 다시 만들고 한 번 더 그린다.
    fn render(&mut self) {
        if let Err(e) = self.draw_frame() {
            logging::warn(&format!("그리기 실패, 그리기 장치를 다시 만듭니다: {e}"));
            self.recreate_gfx();
            if let Err(e) = self.draw_frame() {
                logging::error(&format!("다시 만든 장치로도 그리지 못했습니다: {e}"));
            }
        }
    }

    fn recreate_gfx(&mut self) {
        self.gfx = None;
        let (_, _, w, h) = screen_rect(self.content);
        match Gfx::new(self.content, w as u32, h as u32, self.draw_scale()) {
            Ok(gfx) => self.gfx = Some(gfx),
            Err(e) => logging::error(&format!("그리기 장치를 다시 만들지 못했습니다: {e}")),
        }
    }

    fn draw_frame(&self) -> windows::core::Result<()> {
        let Some(gfx) = &self.gfx else { return Ok(()) };
        let pressed = match &self.press {
            Some(Press { hit: Hit::Tile(i), .. }) => Some(*i),
            _ => None,
        };
        gfx.begin();
        match self.mode {
            Mode::Panel => {
                let hover: Vec<f32> = self.hover.iter().map(|a| a.value).collect();
                let drag = self.drag.map(|(from, pos)| DragView { from, to: self.layout.drop_index(pos), pos });
                let state = SceneState {
                    layout: &self.layout,
                    palette: self.palette(),
                    acrylic: self.acrylic,
                    tiles: &self.tiles,
                    hover: &hover,
                    pressed,
                    menu_hover: self.hover_hit == Hit::Menu,
                    drag,
                    drop_hover: self.drop_hover,
                };
                let side = self.panel_side as f32 / self.scale;
                gfx.set_offset(side, 0.0);
                scene::draw(gfx, &state);
                gfx.set_offset(0.0, 0.0);
                if self.tip.value > 0.0
                    && let Some(i) = self.tip_slot
                    && let Some(rect) = self.panel_tooltip_rect(i)
                {
                    let text = scene::panel_tooltip_text(&self.tiles, i);
                    scene::draw_panel_tooltip(gfx, text, rect, self.tip.value, self.scale);
                }
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
                    tooltip: self.tip_slot.filter(|&i| i < self.dock.layout.slots().len() && self.tip.value > 0.0),
                    drag: self.drag,
                    drop_hover: self.drop_hover,
                    hovered: self.dock_hovered(),
                    tooltip_alpha: self.tip.value,
                    px_scale: self.dock_scale(),
                };
                scene::draw_dock(gfx, &state);
            }
        }
        gfx.end()
    }

    fn start_animation(&mut self) {
        if !self.animating {
            self.animating = true;
            self.last_frame = Instant::now();
            // 애니메이션 동안만 타이머 해상도를 1ms로 (기본 15.6ms면 프레임 간격이 들쭉날쭉하다)
            unsafe {
                timeBeginPeriod(1);
                SetTimer(Some(self.glass), TIMER_ANIM, FRAME_MS, None);
            }
        }
    }

    fn stop_animation(&mut self) {
        if self.animating {
            self.animating = false;
            unsafe {
                let _ = KillTimer(Some(self.glass), TIMER_ANIM);
                timeEndPeriod(1);
            }
        }
    }

    fn tick(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32() * 1000.0;
        self.last_frame = now;
        let dt = dt.clamp(1.0, MAX_FRAME_DT);
        let mut moving = false;
        for anim in &mut self.hover {
            moving |= anim.step(dt);
        }
        moving |= self.fade.advance(dt);
        if self.mode == Mode::Dock {
            moving |= self.dock.amount.step(dt);
            if self.dock.amount.value == 0.0 && self.dock.amount.target == 0.0 {
                self.dock.pointer = None;
            }
        }
        let target = match self.mode {
            Mode::Dock => self.dock_tooltip(),
            Mode::Panel => self.panel_tooltip(),
        };
        if target.is_some() {
            self.tip_slot = target;
        }
        self.tip.set(if target.is_some() { 1.0 } else { 0.0 });
        moving |= self.tip.step(dt);
        if self.tip.value == 0.0 && self.tip.target == 0.0 {
            self.tip_slot = None;
        }
        match self.mode {
            Mode::Dock => self.update_dock_glass(),
            Mode::Panel => self.update_panel_region(),
        }
        self.render();
        if moving {
            // 다음 화면 합성까지 기다려 프레임을 주사율에 맞춘다 (타이머는 우선순위가 낮아 그 사이 입력이 먼저 처리된다)
            let _ = unsafe { DwmFlush() };
        } else {
            self.stop_animation();
        }
    }

    /// 유리 창 기준 px 좌표 → 지금 모드의 그리기 좌표(DIP)
    fn local_point(&self, x: i32, y: i32) -> Pt {
        match self.mode {
            Mode::Panel => Pt { x: x as f32 / self.scale, y: y as f32 / self.scale },
            Mode::Dock => {
                let (gx, gy, _, _) = screen_rect(self.glass);
                let (cx, cy, _, _) = self.dock.content_px;
                let s = self.dock_scale();
                Pt { x: (gx + x - cx) as f32 / s, y: (gy + y - cy) as f32 / s }
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

    /// 끌어서 옮길 수 있는 항목 수 (끝에 붙는 업데이트 단추는 빼고)
    fn movable(&self) -> usize {
        self.tiles.iter().filter(|t| t.kind != TileKind::Update).count()
    }

    fn drop_index(&self, p: Pt) -> usize {
        let index = match self.mode {
            Mode::Panel => self.layout.drop_index(p),
            Mode::Dock => {
                let m = self.dock_mag();
                self.dock.layout.drop_index(self.dock_axes(p, &m).0)
            }
        };
        index.min(self.movable().saturating_sub(1))
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
        if let Some(grab) = self.dock.moving {
            self.move_dock(grab);
            return;
        }
        // 독은 수정 모드에서만 아이콘을 끌어 순서를 바꾼다 (패널은 늘)
        let can_reorder = self.mode == Mode::Panel || self.dock.editing;
        if let Some(press) = &self.press
            && let Hit::Tile(i) = press.hit
            && can_reorder
            && self.tiles[i].kind != TileKind::Update
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
        if self.mode == Mode::Dock {
            if !self.dock.inside {
                self.dock.inside = true;
                self.start_animation();
            }
            if !self.dock.editing {
                let m = self.dock_mag();
                self.dock.pointer = Some(self.dock_axes(p, &m).0);
                self.dock.amount.set(1.0);
                self.start_animation();
            }
        }
        self.set_hover(self.hit(p));
    }

    fn on_mouse_leave(&mut self) {
        self.tracking_leave = false;
        if self.drag.is_none() {
            self.set_hover(Hit::Empty);
            if self.mode == Mode::Dock {
                self.dock.inside = false;
                self.dock.amount.set(0.0);
                self.start_animation();
            }
        }
    }

    fn on_button_up(&mut self, p: Pt) {
        let _ = unsafe { ReleaseCapture() };
        let press = self.press.take();
        if self.dock.moving.take().is_some() {
            self.emit_dock_changed();
            self.render();
            return;
        }
        if let Some((from, pos)) = self.drag.take() {
            let to = self.drop_index(pos);
            if from != to {
                self.tiles = reorder(&self.tiles, from, to);
                let ids = self.tiles.iter().filter(|t| t.kind != TileKind::Update).map(|t| t.id.clone()).collect();
                self.emit(UiEvent::Reordered(ids));
            }
            self.render();
            return;
        }
        let hit = self.hit(p);
        if press.map(|pr| pr.hit) != Some(hit) {
            self.render();
            return;
        }
        match hit {
            // 수정 모드에서는 아이콘을 눌러도 열지 않는다
            Hit::Tile(_) if self.mode == Mode::Dock && self.dock.editing => {}
            Hit::Tile(i) => self.emit(UiEvent::TileClicked(self.tiles[i].id.clone())),
            Hit::Add => self.emit(UiEvent::AddRequested),
            Hit::Shrink | Hit::Grow => {
                let dir = if hit == Hit::Grow { 1 } else { -1 };
                let size = dock_layout::step_size(self.dock.size, dir);
                if size != self.dock.size {
                    self.dock.size = size;
                    self.place_windows();
                    self.emit_dock_changed();
                }
            }
            Hit::Done => self.set_editing(false),
            Hit::Menu => self.emit(UiEvent::MenuRequested),
            _ => {}
        }
        self.render();
    }

    fn on_right_click(&mut self, p: Pt) {
        if let Hit::Tile(i) = self.hit(p)
            && self.tiles[i].kind != TileKind::Update
        {
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
            Hit::Header => HTCAPTION,
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
                // 아이콘 캐시는 항목·아이콘 파일이 바뀔 때만 비운다 (실행 중 점만 바뀐 경우는 그대로)
                let same_icons = same_items && tiles.iter().zip(&self.tiles).all(|(a, b)| a.icon == b.icon);
                self.tiles = tiles;
                if !same_icons && let Some(gfx) = &self.gfx {
                    gfx.clear_images();
                }
                if !same_items {
                    self.hover = vec![Anim::at(0.0); self.tiles.len() + 1];
                    self.hover_hit = Hit::Empty;
                }
                self.layout = PanelLayout::compute(self.layout.width, self.layout.height, self.tiles.len());
                match self.mode {
                    Mode::Dock => self.place_windows(),
                    Mode::Panel => {
                        // 항목이 늘어 아래 줄이 가려지면 패널을 그만큼 늘린다
                        let min =
                            (PanelLayout::min_height(self.layout.width, self.tiles.len()) * self.scale).ceil() as i32;
                        if self.panel_rect.3 < min {
                            self.panel_rect.3 = min;
                            self.place_windows();
                        }
                    }
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
                if self.dock.editing && mode == Mode::Panel {
                    self.dock.editing = false;
                    self.emit(UiEvent::EditingChanged(false));
                }
                self.mode = mode;
                self.dock.edge = edge;
                self.dock.pointer = None;
                self.dock.amount = Anim::at(0.0);
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
            PanelCmd::DropHover(on) => {
                if on != self.drop_hover {
                    self.drop_hover = on;
                    self.render();
                }
            }
            PanelCmd::Dropped(items) => self.emit(UiEvent::Dropped(items)),
            PanelCmd::SetEditing(on) => self.set_editing(on && self.mode == Mode::Dock),
            PanelCmd::Close => {
                drop::revoke(&[self.glass, self.content]);
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
                let hit = self.hit(at);
                self.press = Some(Press { hit, at });
                // 수정 모드에서 판의 빈 곳을 누르면 판 옮기기
                if self.mode == Mode::Dock && self.dock.editing && hit == Hit::Empty {
                    let mut pt = POINT::default();
                    let _ = unsafe { GetCursorPos(&mut pt) };
                    let c = self.dock.center_px;
                    let grab = if dock_layout::is_vertical(self.dock.edge) { pt.y - c.1 } else { pt.x - c.0 };
                    self.dock.moving = Some(grab);
                }
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
            WM_WINDOWPOSCHANGING => {
                // 패널을 끌어 옮기거나 크기를 바꿀 때, 유리 창이 움직이기 직전에 콘텐츠 창도 같은 자리로 옮기고
                // 다시 그린다 (움직인 뒤에 따라가면 내용이 테두리보다 한 박자 늦는다)
                if !self.positioning && self.mode == Mode::Panel {
                    // SAFETY: lparam은 시스템이 넘긴 WINDOWPOS
                    let wp = unsafe { &*(lparam.0 as *const WINDOWPOS) };
                    let (mut x, mut y, mut w, mut h) = self.panel_rect;
                    if !wp.flags.contains(SWP_NOMOVE) {
                        (x, y) = (wp.x, wp.y);
                    }
                    if !wp.flags.contains(SWP_NOSIZE) {
                        (w, h) = (wp.cx, wp.cy);
                    }
                    if (x, y, w, h) != self.panel_rect {
                        self.relayout_panel_to((x, y, w, h));
                        self.render();
                    }
                }
                return None;
            }
            WM_WINDOWPOSCHANGED => {
                if !self.positioning && self.mode == Mode::Panel {
                    if screen_rect(hwnd) != self.panel_rect {
                        self.relayout_panel();
                        self.render();
                    }
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
                    // 지금 폭에서 모든 아이콘이 보이는 높이 아래로는 줄지 않게
                    let w = self.panel_rect.2 as f32 / self.scale;
                    info.ptMinTrackSize.y = (PanelLayout::min_height(w, self.tiles.len()) * self.scale).ceil() as i32;
                }
            }
            WM_SIZING if self.mode == Mode::Panel => {
                // 폭을 줄여 줄이 늘면 최소 높이도 따라 커진다: 끌고 있는 쪽 가장자리를 늘려 맞춘다
                // SAFETY: lparam은 시스템이 넘긴 끌기 중 사각형
                let r = unsafe { &mut *(lparam.0 as *mut RECT) };
                let w = (r.right - r.left) as f32 / self.scale;
                let min = (PanelLayout::min_height(w, self.tiles.len()) * self.scale).ceil() as i32;
                if r.bottom - r.top < min {
                    if matches!(wparam.0 as u32, WMSZ_TOP | WMSZ_TOPLEFT | WMSZ_TOPRIGHT) {
                        r.top = r.bottom - min;
                    } else {
                        r.bottom = r.top + min;
                    }
                }
                return Some(LRESULT(1));
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
            WM_SETCURSOR
                if self.mode == Mode::Dock
                    && self.dock.editing
                    && (self.dock.moving.is_some() || self.hover_hit == Hit::Empty)
                    && (lparam.0 & 0xFFFF) as u32 == HTCLIENT =>
            {
                // 판을 옮길 수 있는 곳은 이동 커서
                unsafe { SetCursor(LoadCursorW(None, IDC_SIZEALL).ok()) };
                return Some(LRESULT(1));
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_side_room_only_when_tooltip_is_wider_than_the_panel() {
        assert_eq!(panel_side_px(200.0, 360.0, 1.0), 0);
        assert_eq!(panel_side_px(500.0, 360.0, 1.0), 78); // (500 + 16 - 360) / 2
        assert_eq!(panel_side_px(500.0, 360.0, 2.0), 156);
    }
}
