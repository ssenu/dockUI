//! 패널·독 한 프레임 그리기: 유리 덧칠, 헤더, 아이콘, ＋, 팔레트 버튼, 말풍선, 테마 선택기, 드래그 중인 아이콘.

use crate::config::DockEdge;
use crate::ui::dock_layout::{self, DockLayout, Magnified, Slot};
use crate::ui::gfx::{Align, Gfx, Weight};
use crate::ui::layout::{PANEL_HOVER_SCALE, PanelLayout, Pt, RectF};
use crate::ui::theme::{ACCENT, Color, FALLBACK_ALPHA, Palette, TOOLTIP_BG, TOOLTIP_TEXT};
use std::path::PathBuf;

pub const WINDOW_RADIUS: f32 = 8.0;
pub const ICON_RADIUS: f32 = 14.0;
pub const TITLE: &str = crate::app::APP_NAME;
pub const BYLINE: &str = "by_ssenu";
/// 독 수정 모드 안내
pub const EDIT_HINT: &str = "빈 곳을 끌어 옮기기 · 아이콘을 끌어 순서 바꾸기";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TileKind {
    #[default]
    Youtube,
    App,
    Web,
    /// 파일·폴더 (셸 아이콘)
    Path,
    /// 새 버전이 있을 때 목록 끝에 붙는 업데이트 단추 (끌기·수정·삭제 불가)
    Update,
}

/// 화면에 그릴 항목 하나
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TileView {
    pub id: String,
    /// 말풍선에 보이는 이름
    pub label: String,
    /// 유튜브 아이콘 아래 두 글자 배지 (업데이트는 버전, 다른 종류는 아바타 글자)
    pub badge: String,
    pub kind: TileKind,
    /// 캐시된 아이콘 PNG (앱 아이콘, 사이트 파비콘). 없으면 글자 아바타
    pub icon: Option<PathBuf>,
    /// 실행 중인 앱이면 아이콘 아래 점
    pub running: bool,
    /// 실행 대상을 찾지 못한 앱 (흐리게)
    pub missing: bool,
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
    pub drag: Option<DragView>,
    /// 끌어 놓을 수 있는 것이 위에 있음 (강조 테두리)
    pub drop_hover: bool,
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

/// 업데이트 단추: 초록 타일에 아래 화살표 + 받침, 아래 배지(버전 또는 "받는 중")
fn draw_update_icon(g: &Gfx, r: RectF, badge: &str) {
    g.fill_round_rect_gradient(r, ICON_RADIUS * r.w / 56.0, Color::hex(0x3CCB6C, 1.0), Color::hex(0x1A9A4A, 1.0));
    let c = r.center();
    let s = r.w / 56.0;
    let white = Color::hex(0xFFFFFF, 1.0);
    let top = c.y - 14.0 * s;
    let tip = c.y + 3.0 * s;
    g.line(Pt { x: c.x, y: top }, Pt { x: c.x, y: tip - 2.0 * s }, white, 3.2 * s);
    let head = [
        Pt { x: c.x - 7.5 * s, y: tip - 7.0 * s },
        Pt { x: c.x + 7.5 * s, y: tip - 7.0 * s },
        Pt { x: c.x, y: tip + 1.5 * s },
    ];
    g.fill_polygon(&head, white);
    g.line(Pt { x: c.x - 10.0 * s, y: tip + 5.0 * s }, Pt { x: c.x + 10.0 * s, y: tip + 5.0 * s }, white, 2.6 * s);
    if !badge.is_empty() {
        let size = 9.5 * s;
        let w = g.measure(badge, size, Weight::SemiBold) + 10.0 * s;
        let pill = RectF::new(c.x - w / 2.0, r.bottom() - 17.0 * s, w, 13.0 * s);
        g.fill_round_rect(pill, 6.5 * s, Color::hex(0x000000, 0.25));
        g.text(badge, pill, size, Weight::SemiBold, Align::Center, Color::hex(0xFFFFFF, 0.95));
    }
}

fn draw_letter_icon(g: &Gfx, r: RectF, seed: &str, letter: &str) {
    let base = avatar_color(seed);
    g.fill_round_rect_gradient(r, ICON_RADIUS * r.w / 56.0, base.shade(0.12), base.shade(-0.08));
    g.text(letter, r, 24.0 * r.w / 56.0, Weight::Bold, Align::Center, Color::hex(0xFFFFFF, 0.95));
}

fn draw_tile(g: &Gfx, tile: &TileView, r: RectF, lift: f32) {
    let radius = ICON_RADIUS * r.w / 56.0;
    let opacity = if tile.missing { 0.4 } else { 1.0 };
    // 앱·파일 아이콘은 자체 모양(투명 배경)이 있으므로 그림자 없이 그대로 그린다
    if matches!(tile.kind, TileKind::App | TileKind::Path)
        && let Some(path) = &tile.icon
        && g.image(path, r, opacity)
    {
        return;
    }
    g.soft_shadow(r, radius, 1.0 + lift);
    match (tile.kind, &tile.icon) {
        (TileKind::Youtube, _) => draw_youtube_icon(g, r, &tile.badge),
        (TileKind::Update, _) => draw_update_icon(g, r, &tile.badge),
        // 사이트 아이콘은 작아서 흰 둥근 판 위에 올린다
        (TileKind::Web, Some(path)) => {
            g.fill_round_rect(r, radius, Color::hex(0xFFFFFF, 0.96));
            if !g.image(path, r.scaled(0.58), 1.0) {
                draw_letter_icon(g, r, &tile.id, &tile.badge);
            }
        }
        _ => draw_letter_icon(g, r, &tile.id, &tile.badge),
    }
    // 실행 대상을 찾지 못한 항목은 흐리게
    if tile.missing {
        g.fill_round_rect(r, radius, Color::hex(0x808080, 0.55));
    }
}

/// 실행 중 표시 점. `toward`는 점을 놓을 쪽(아이콘 바깥) 방향 단위 벡터.
fn draw_running_dot(g: &Gfx, r: RectF, toward: (f32, f32), c: Color) {
    let center = r.center();
    let gap = 5.0;
    let p = Pt { x: center.x + toward.0 * (r.w / 2.0 + gap), y: center.y + toward.1 * (r.h / 2.0 + gap) };
    g.fill_circle(p, 2.2, c);
}

fn dock_dot_direction(edge: DockEdge) -> (f32, f32) {
    match edge {
        DockEdge::Bottom => (0.0, 1.0),
        DockEdge::Top => (0.0, -1.0),
        DockEdge::Left => (-1.0, 0.0),
        DockEdge::Right => (1.0, 0.0),
    }
}

fn draw_add(g: &Gfx, r: RectF, p: &Palette) {
    let radius = ICON_RADIUS * r.w / 56.0;
    g.fill_round_rect(r, radius, p.add_tile);
    let inner = RectF::new(r.x + 0.5, r.y + 0.5, r.w - 1.0, r.h - 1.0);
    g.stroke_round_rect(inner, radius - 0.5, p.divider, 1.0);
    let c = r.center();
    let arm = r.w * 0.15;
    let width = 2.4 * r.w / 56.0;
    g.line(Pt { x: c.x - arm, y: c.y }, Pt { x: c.x + arm, y: c.y }, p.subtext, width);
    g.line(Pt { x: c.x, y: c.y - arm }, Pt { x: c.x, y: c.y + arm }, p.subtext, width);
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

/// 말풍선 크기 (너비, 높이): 한 줄, 이름 전체 (줄이지 않는다).
/// 여백 10×2에 픽셀 맞춤 반올림 여유 4를 더한다 (여유가 없으면 175% 등에서 글자 칸이 모자라 끝이 … 으로 바뀐다).
pub fn tooltip_size(g: &Gfx, text: &str) -> (f32, f32) {
    ((g.measure(text, 12.0, Weight::SemiBold) + 24.0).ceil(), 26.0)
}

/// 이 이름들 중 가장 긴 말풍선 폭
pub fn longest_tip(g: &Gfx, labels: &[&str]) -> f32 {
    labels.iter().map(|t| tooltip_size(g, t).0).fold(0.0, f32::max)
}

/// 사각형을 실제 화면 픽셀 경계에 맞춘다 (`scale` = DIP → px 배율).
/// 위치는 반올림, 크기는 올림으로 따로 맞춰 위치가 움직여도 크기가 1px씩 흔들리지 않게 한다
/// (크기가 흔들리면 가운데 정렬 글자가 흔들리고 줄바꿈이 바뀐다).
pub fn snap(r: RectF, scale: f32) -> RectF {
    let pos = |v: f32| (v * scale).round() / scale;
    let size = |v: f32| (v * scale - 1e-3).ceil() / scale;
    RectF::new(pos(r.x), pos(r.y), size(r.w), size(r.h))
}

/// 말풍선 상자. 어두운 배경(투명 테마)에서도 경계가 보이게 옅은 테두리를 두른다.
/// `alpha`(0..1)는 페이드, `scale`은 픽셀 맞춤용 배율 (모서리·1px 테두리를 또렷하게).
fn draw_tooltip_box(g: &Gfx, text: &str, r: RectF, alpha: f32, scale: f32) {
    let r = snap(r, scale);
    let half = 0.5 / scale;
    g.fill_round_rect(r, 8.0, TOOLTIP_BG.with_alpha(TOOLTIP_BG.a * alpha));
    let edge = RectF::new(r.x + half, r.y + half, r.w - 2.0 * half, r.h - 2.0 * half);
    g.stroke_round_rect(edge, 8.0 - half, Color::hex(0xFFFFFF, 0.14 * alpha), 1.0 / scale);
    // 글자 영역도 픽셀 경계에서 시작하고 너비가 고정이어야 글자 간격·줄바꿈이 프레임마다 같다
    let pad = (10.0 * scale).round() / scale;
    let inner = RectF::new(r.x + pad, r.y, r.w - 2.0 * pad, r.h);
    g.text_untrimmed(text, inner, 12.0, Weight::SemiBold, Align::Center, TOOLTIP_TEXT.with_alpha(alpha));
}

/// 패널 말풍선 사각형 (콘텐츠 창 기준 DIP): 아이콘 위 가운데, 한 줄. `left..right`는 그릴 수 있는 가로 범위
/// (패널 밖까지 넓힌 콘텐츠 창), 위가 `top_limit`(머리글)보다 올라가면 아이콘 아래에 띄운다.
pub fn panel_tooltip_rect(g: &Gfx, text: &str, anchor: RectF, left: f32, right: f32, top_limit: f32) -> RectF {
    let (w, h) = tooltip_size(g, text);
    let x = (anchor.center().x - w / 2.0).clamp(left + 4.0, (right - w - 4.0).max(left + 4.0));
    let mut y = anchor.y - h - 8.0;
    if y < top_limit {
        y = anchor.bottom() + 8.0;
    }
    RectF::new(x, y, w, h)
}

/// 패널 칸의 말풍선 글자
pub fn panel_tooltip_text(tiles: &[TileView], cell: usize) -> &str {
    tiles.get(cell).map_or("항목 추가", |t| t.label.as_str())
}

/// 패널 말풍선 그리기 (페이드 `alpha`, 픽셀 맞춤 `scale`)
pub fn draw_panel_tooltip(g: &Gfx, text: &str, rect: RectF, alpha: f32, scale: f32) {
    draw_tooltip_box(g, text, rect, alpha, scale);
}

/// 끌어 놓기 강조: 판 테두리를 강조색으로, 안쪽은 옅게
fn draw_drop_highlight(g: &Gfx, r: RectF) {
    g.fill_round_rect(r, WINDOW_RADIUS, ACCENT.with_alpha(0.10));
    let inner = RectF::new(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0);
    g.stroke_round_rect(inner, WINDOW_RADIUS - 1.0, ACCENT, 2.0);
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
    if s.drop_hover {
        draw_drop_highlight(g, bounds);
    }
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
        if s.tiles[tile_index].running {
            draw_running_dot(g, s.layout.cells[slot], (0.0, 1.0), p.text.with_alpha(0.75));
        }
    }
    let add_slot = s.tiles.len();
    let add_level = s.hover.get(add_slot).copied().unwrap_or(0.0);
    draw_add(g, s.layout.add_cell().scaled(1.0 + (PANEL_HOVER_SCALE - 1.0) * add_level), p);
    if let Some(d) = &s.drag {
        let cell = s.layout.cells[0];
        let r = RectF::new(d.pos.x - cell.w / 2.0, d.pos.y - cell.h / 2.0, cell.w, cell.h).scaled(1.1);
        draw_tile(g, &s.tiles[d.from], r, 2.0);
    }
}

/// 독 한 프레임에 필요한 상태
pub struct DockScene<'a> {
    pub layout: &'a DockLayout,
    pub mag: &'a Magnified,
    /// 판 중심 (콘텐츠 창 기준 DIP)
    pub center: (f32, f32),
    /// 콘텐츠 창 크기 (DIP)
    pub size: (f32, f32),
    pub palette: Palette,
    pub acrylic: bool,
    pub tiles: &'a [TileView],
    pub pressed: Option<usize>,
    /// 말풍선을 띄울 칸 번호
    pub tooltip: Option<usize>,
    /// 드래그 중인 항목 (번호, 마우스 위치)
    pub drag: Option<(usize, Pt)>,
    /// 끌어 놓을 수 있는 것이 위에 있음 (강조 테두리)
    pub drop_hover: bool,
    /// 마우스가 올라간 칸 (수정 단추 강조)
    pub hovered: Option<usize>,
    /// 말풍선 불투명도 0..1 (나타나고 사라질 때 페이드)
    pub tooltip_alpha: f32,
    /// DIP → 실제 픽셀 배율 (말풍선 픽셀 맞춤)
    pub px_scale: f32,
}

/// 수정 모드 안내 자리: 판에서 화면 안쪽으로 (콘텐츠 창 기준 DIP)
pub fn edit_hint_rect(g: &Gfx, bar: RectF, edge: DockEdge, bounds: (f32, f32)) -> RectF {
    let (w, h) = tooltip_size(g, EDIT_HINT);
    let c = bar.center();
    let (x, y) = match edge {
        DockEdge::Bottom => (c.x - w / 2.0, bar.y - h - 10.0),
        DockEdge::Top => (c.x - w / 2.0, bar.bottom() + 10.0),
        DockEdge::Left => (bar.right() + 10.0, bar.y),
        DockEdge::Right => (bar.x - w - 10.0, bar.y),
    };
    let x = x.clamp(4.0, (bounds.0 - w - 4.0).max(4.0));
    let y = y.clamp(4.0, (bounds.1 - h - 4.0).max(4.0));
    RectF::new(x, y, w, h)
}

/// 수정 모드 단추: 작게(−) / 크게(＋)는 납작한 원, 완료는 강조색 원에 체크
fn draw_edit_button(g: &Gfx, r: RectF, slot: Slot, p: &Palette, hover: bool) {
    let c = r.center();
    let radius = r.w * 0.40;
    let arm = radius * 0.42;
    match slot {
        Slot::Done => {
            g.fill_circle(c, radius, if hover { ACCENT.shade(-0.1) } else { ACCENT });
            let white = Color::hex(0xFFFFFF, 1.0);
            let w = radius * 0.16;
            g.line(Pt { x: c.x - arm, y: c.y + arm * 0.05 }, Pt { x: c.x - arm * 0.25, y: c.y + arm * 0.75 }, white, w);
            g.line(Pt { x: c.x - arm * 0.25, y: c.y + arm * 0.75 }, Pt { x: c.x + arm, y: c.y - arm * 0.65 }, white, w);
        }
        _ => {
            g.fill_circle(c, radius, p.text.with_alpha(if hover { 0.18 } else { 0.09 }));
            g.stroke_circle(c, radius - 0.5, p.divider, 1.0);
            let w = radius * 0.13;
            g.line(Pt { x: c.x - arm, y: c.y }, Pt { x: c.x + arm, y: c.y }, p.text, w);
            if slot == Slot::Grow {
                g.line(Pt { x: c.x, y: c.y - arm }, Pt { x: c.x, y: c.y + arm }, p.text, w);
            }
        }
    }
}

/// 독 말풍선 사각형: 판 반대쪽(화면 안쪽)에 띄운다. 창 영역 계산에도 쓴다.
pub fn dock_tooltip_rect(g: &Gfx, text: &str, icon: RectF, edge: DockEdge, bounds: (f32, f32)) -> RectF {
    let (w, h) = tooltip_size(g, text);
    let c = icon.center();
    let (x, y) = match edge {
        DockEdge::Bottom => (c.x - w / 2.0, icon.y - h - 8.0),
        DockEdge::Top => (c.x - w / 2.0, icon.bottom() + 8.0),
        DockEdge::Left => (icon.right() + 8.0, c.y - h / 2.0),
        DockEdge::Right => (icon.x - w - 8.0, c.y - h / 2.0),
    };
    let x = x.clamp(4.0, (bounds.0 - w - 4.0).max(4.0));
    let y = y.clamp(4.0, (bounds.1 - h - 4.0).max(4.0));
    RectF::new(x, y, w, h)
}

/// 독 칸의 말풍선 글자
pub fn dock_tooltip_text<'a>(layout: &DockLayout, tiles: &'a [TileView], slot: usize) -> &'a str {
    match layout.slots().get(slot) {
        Some(Slot::Tile(t)) => tiles.get(*t).map_or("", |v| v.label.as_str()),
        Some(Slot::Shrink) => "아이콘 작게",
        Some(Slot::Grow) => "아이콘 크게",
        Some(Slot::Done) => "수정 끝내기",
        _ => "항목 추가",
    }
}

pub fn draw_dock(g: &Gfx, s: &DockScene) {
    let p = &s.palette;
    let tint = if s.acrylic { p.tint } else { p.tint.with_alpha(FALLBACK_ALPHA) };
    let bar = s.layout.bar_rect(s.mag, s.center);
    let bar = RectF::new(bar.x + 0.5, bar.y + 0.5, bar.w - 1.0, bar.h - 1.0);
    g.fill_round_rect(bar, WINDOW_RADIUS, tint);
    g.stroke_round_rect(bar, WINDOW_RADIUS, p.divider, 1.0);
    if s.drop_hover {
        draw_drop_highlight(g, bar);
    } else if s.layout.editing {
        let inner = RectF::new(bar.x + 1.0, bar.y + 1.0, bar.w - 2.0, bar.h - 2.0);
        g.stroke_round_rect(inner, WINDOW_RADIUS - 1.0, ACCENT, 1.6);
    }
    if let Some(along) = s.mag.separator {
        let inset = dock_layout::PAD + 6.0;
        let (a, b) = if dock_layout::is_vertical(s.layout.edge) {
            let y = s.center.1 + along;
            (Pt { x: bar.x + inset, y }, Pt { x: bar.right() - inset, y })
        } else {
            let x = s.center.0 + along;
            (Pt { x, y: bar.y + inset }, Pt { x, y: bar.bottom() - inset })
        };
        g.line(a, b, p.divider, 1.0);
    }

    for (i, slot) in s.layout.slots().iter().enumerate() {
        let mut r = s.layout.slot_rect(s.mag, s.center, i);
        match slot {
            Slot::Tile(t) => {
                if s.drag.is_some_and(|(from, _)| from == *t) {
                    continue;
                }
                if s.pressed == Some(*t) && s.drag.is_none() {
                    r = r.scaled(0.94);
                }
                draw_tile(g, &s.tiles[*t], r, s.mag.scales[i] - 1.0);
                if s.tiles[*t].running {
                    draw_running_dot(g, r, dock_dot_direction(s.layout.edge), p.text.with_alpha(0.8));
                }
            }
            Slot::Add => draw_add(g, r, p),
            Slot::Shrink | Slot::Grow | Slot::Done => draw_edit_button(g, r, *slot, p, s.hovered == Some(i)),
        }
    }
    if s.layout.editing && s.drag.is_none() && s.tooltip.is_none() {
        draw_tooltip_box(g, EDIT_HINT, edit_hint_rect(g, bar, s.layout.edge, s.size), 1.0, s.px_scale);
    }
    if let Some((from, pos)) = s.drag {
        let size = dock_layout::ICON * 1.1;
        let r = RectF::new(pos.x - size / 2.0, pos.y - size / 2.0, size, size);
        draw_tile(g, &s.tiles[from], r, 2.0);
    } else if let Some(i) = s.tooltip {
        let text = dock_tooltip_text(s.layout, s.tiles, i);
        let icon = s.layout.slot_rect(s.mag, s.center, i);
        let rect = dock_tooltip_rect(g, text, icon, s.layout.edge, s.size);
        draw_tooltip_box(g, text, rect, s.tooltip_alpha, s.px_scale);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Theme;

    #[test]
    fn snap_aligns_rect_to_device_pixels() {
        let r = snap(RectF::new(10.3, 20.6, 50.2, 26.1), 1.5);
        for v in [r.x, r.y, r.right(), r.bottom()] {
            assert!(((v * 1.5) - (v * 1.5).round()).abs() < 1e-4, "{v}");
        }
        assert_eq!(snap(RectF::new(1.0, 2.0, 3.0, 4.0), 1.0), RectF::new(1.0, 2.0, 3.0, 4.0));
        // 위치가 조금씩 움직여도 크기는 그대로 (줄바꿈·가운데 정렬이 흔들리지 않게), 줄지도 않는다
        let sizes: Vec<(f32, f32)> = (0..20)
            .map(|i| snap(RectF::new(10.0 + i as f32 * 0.137, 5.3, 80.4, 26.0), 1.25))
            .map(|r| (r.w, r.h))
            .collect();
        assert!(sizes.windows(2).all(|w| w[0] == w[1]), "{sizes:?}");
        assert!(sizes[0].0 >= 80.4 && sizes[0].1 >= 26.0);
    }

    /// 말풍선이 소수점 위치로 조금씩 움직여도 (확대 애니메이션 중) 안쪽 그림이 픽셀 단위로 똑같아야 글자가 흔들리지 않는다
    #[test]
    fn tooltip_pixels_do_not_shift_with_subpixel_moves() {
        let _ = unsafe {
            windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)
        };
        let scale = 1.25;
        let (w, h) = (400u32, 120u32);
        let gfx = Gfx::offscreen(w, h, scale).unwrap();
        let text = "아침 뉴스 라이브 - 오늘의 주요 소식 모아 보기";
        let mut crops = Vec::new();
        for i in 0..8 {
            let dx = i as f32 * 0.13;
            let r = RectF::new(20.0 + dx, 30.0 + dx * 0.5, 240.0, 26.0);
            gfx.begin();
            draw_tooltip_box(&gfx, text, r, 1.0, scale);
            gfx.end().unwrap();
            let px = gfx.pixels();
            // 맞춘 말풍선 왼쪽 위 기준으로 잘라 비교
            let s = snap(r, scale);
            let (ox, oy) = ((s.x * scale).round() as usize, (s.y * scale).round() as usize);
            let (cw, ch) = ((s.w * scale) as usize, (s.h * scale) as usize);
            let mut crop = Vec::with_capacity(cw * ch * 4);
            for y in oy..oy + ch {
                let row = (y * w as usize + ox) * 4;
                crop.extend_from_slice(&px[row..row + cw * 4]);
            }
            crops.push(crop);
        }
        // 둥근 모서리 경계의 부동소수 오차(밝기 1~2단계)만 허용한다. 글자가 0.5px만 밀려도 수십 단계가 바뀐다.
        for c in &crops[1..] {
            let delta = c.iter().zip(&crops[0]).map(|(a, b)| (*a as i32 - *b as i32).abs()).max().unwrap_or(0);
            assert!(delta <= 3, "말풍선 안 픽셀이 위치에 따라 달라짐 (최대 차이 {delta})");
        }
    }

    #[test]
    fn tooltip_is_one_line_as_wide_as_the_name() {
        let _ = unsafe {
            windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)
        };
        let g = Gfx::offscreen(10, 10, 1.0).unwrap();
        let short = tooltip_size(&g, "뉴스");
        let long = tooltip_size(&g, "아침 뉴스 라이브 - 오늘의 주요 소식 모아 보기 (전체 재생목록) 경제·사회·국제");
        assert_eq!((short.1, long.1), (26.0, 26.0));
        assert!(long.0 > 360.0, "{long:?}");
        // 여백 10×2에 반올림 여유가 있어야 175% 등에서 글자 칸이 글자보다 좁아지지 않는다 (… 방지)
        let text_w = g.measure("뉴스", 12.0, Weight::SemiBold);
        assert!(short.0 >= text_w + 2.0 * (10.0 * 1.75f32).round() / 1.75);
        assert!(longest_tip(&g, &["뉴스", "아주 긴 이름 아주 긴 이름"]) > short.0);
    }

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
                id: "1".into(),
                label: "로파이 음악".into(),
                badge: "로파".into(),
                kind: TileKind::Youtube,
                ..Default::default()
            },
            TileView {
                id: "2".into(),
                label: "아침 뉴스".into(),
                badge: "뉴스".into(),
                kind: TileKind::Youtube,
                ..Default::default()
            },
            TileView {
                id: "3".into(),
                label: "github.com".into(),
                badge: "G".into(),
                kind: TileKind::Web,
                ..Default::default()
            },
            TileView {
                id: "4".into(),
                label: "계산기".into(),
                badge: "계".into(),
                kind: TileKind::App,
                icon: Some(std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icon.png"))),
                running: true,
                missing: false,
            },
        ];
        let mut tiles = tiles;
        tiles.push(TileView {
            id: "update".into(),
            label: "새 버전 v0.3.0 설치".into(),
            badge: "v0.3.0".into(),
            kind: TileKind::Update,
            ..Default::default()
        });
        tiles[1].label = "아침 뉴스 라이브 - 오늘의 주요 소식 모아 보기 (전체 재생목록)".into();
        let layout = PanelLayout::compute(360.0, 480.0, tiles.len(), crate::ui::layout::ICON);
        // 패널보다 긴 말풍선이 패널 밖까지 나가는 모습: 콘텐츠 창을 좌우로 SIDE만큼 넓혀 그린다
        const SIDE: f32 = 60.0;
        for theme in crate::ui::theme::PICKER_ORDER.iter() {
            let gfx = Gfx::offscreen(((360.0 + 2.0 * SIDE) * 2.0) as u32, 480 * 2, 2.0).unwrap();
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
                drag: None,
                drop_hover: *theme == Theme::Glass,
            };
            gfx.begin();
            gfx.set_offset(SIDE, 0.0);
            draw(&gfx, &state);
            gfx.set_offset(0.0, 0.0);
            let cell = layout.cells[1];
            let anchor = RectF::new(cell.x + SIDE, cell.y, cell.w, cell.h);
            let text = panel_tooltip_text(&tiles, 1);
            let rect = panel_tooltip_rect(&gfx, text, anchor, 0.0, 360.0 + 2.0 * SIDE, layout.header.bottom());
            draw_panel_tooltip(&gfx, text, rect, 1.0, 2.0);
            gfx.end().unwrap();
            let path = dir.join(format!("panel-{}.png", crate::ui::theme::label(*theme)));
            gfx.save_png(&path).unwrap();
            assert!(std::fs::metadata(&path).unwrap().len() > 1000);
        }
    }

    /// 사용자 지정 테마(밝은 색·어두운 색) 패널: 글자색이 바탕 밝기에 맞춰 바뀌는지 눈으로 확인
    #[test]
    fn renders_custom_theme_panel_snapshots() {
        use crate::config::CustomTheme;
        let _ = unsafe {
            windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)
        };
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("snapshots");
        std::fs::create_dir_all(&dir).unwrap();
        let tiles = vec![TileView {
            id: "1".into(),
            label: "로파이 음악".into(),
            badge: "로파".into(),
            kind: TileKind::Youtube,
            ..Default::default()
        }];
        let layout = PanelLayout::compute(300.0, 220.0, tiles.len(), crate::ui::layout::ICON);
        // 큰 아이콘(72) 패널도 같이
        {
            let big = PanelLayout::compute(300.0, 220.0, tiles.len(), 72.0);
            let gfx = Gfx::offscreen(600, 440, 2.0).unwrap();
            let hover = vec![0.0; 2];
            let state = SceneState {
                layout: &big,
                palette: crate::ui::theme::palette(crate::config::Theme::Dark),
                acrylic: true,
                tiles: &tiles,
                hover: &hover,
                pressed: None,
                menu_hover: false,
                drag: None,
                drop_hover: false,
            };
            gfx.begin();
            draw(&gfx, &state);
            gfx.end().unwrap();
            gfx.save_png(&dir.join("panel-icon-72.png")).unwrap();
        }
        for (name, color) in [("light", 0xF4ECD8), ("dark", 0x0F4C5C)] {
            let gfx = Gfx::offscreen(600, 440, 2.0).unwrap();
            let hover = vec![0.0; 2];
            let state = SceneState {
                layout: &layout,
                palette: crate::ui::theme::custom_palette(&CustomTheme { color, opacity: 85, blur: false }),
                acrylic: true,
                tiles: &tiles,
                hover: &hover,
                pressed: None,
                menu_hover: false,
                drag: None,
                drop_hover: false,
            };
            gfx.begin();
            draw(&gfx, &state);
            gfx.end().unwrap();
            let path = dir.join(format!("panel-custom-{name}.png"));
            gfx.save_png(&path).unwrap();
            assert!(std::fs::metadata(&path).unwrap().len() > 1000);
        }
    }

    #[test]
    fn renders_dock_snapshots_for_bottom_and_left_edges() {
        const TIP: f32 = 560.0;
        use crate::ui::gfx::Gfx;
        let _ = unsafe {
            windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)
        };
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("snapshots");
        std::fs::create_dir_all(&dir).unwrap();
        let tiles = vec![
            TileView {
                id: "1".into(),
                label: "로파이 음악".into(),
                badge: "로파".into(),
                kind: TileKind::Youtube,
                ..Default::default()
            },
            TileView {
                id: "2".into(),
                label: "아침 뉴스 라이브 - 오늘의 주요 소식 모아 보기 (전체 재생목록) 경제·사회·국제 뉴스 한 번에"
                    .into(),
                badge: "뉴스".into(),
                kind: TileKind::Youtube,
                ..Default::default()
            },
            TileView {
                id: "3".into(),
                label: "github.com".into(),
                badge: "G".into(),
                kind: TileKind::Web,
                missing: true,
                ..Default::default()
            },
            TileView {
                id: "4".into(),
                label: "계산기".into(),
                badge: "계".into(),
                kind: TileKind::App,
                icon: Some(std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icon.png"))),
                running: true,
                missing: false,
            },
        ];
        let mut tiles = tiles;
        tiles.push(TileView {
            id: "update".into(),
            label: "업데이트 내려받는 중…".into(),
            badge: "받는 중".into(),
            kind: TileKind::Update,
            ..Default::default()
        });
        for (edge, name) in [(DockEdge::Bottom, "bottom"), (DockEdge::Left, "left"), (DockEdge::Right, "right")] {
            let editing = edge == DockEdge::Left;
            let layout = DockLayout::compute(edge, tiles.len(), editing);
            let mag = if editing { layout.magnified(None, 0.0) } else { layout.magnified(Some(layout.rest[1]), 1.0) };
            let max = layout.max_length();
            let vertical = dock_layout::is_vertical(edge);
            let (w, h) = if vertical {
                (dock_layout::THICK + layout.headroom(TIP), max + 2.0 * layout.side_room(TIP))
            } else {
                (max + 2.0 * layout.side_room(TIP), dock_layout::THICK + layout.headroom(TIP))
            };
            let center = match edge {
                DockEdge::Left => (dock_layout::THICK / 2.0, h / 2.0),
                DockEdge::Right => (w - dock_layout::THICK / 2.0, h / 2.0),
                _ => (w / 2.0, h - dock_layout::THICK / 2.0),
            };
            let gfx = Gfx::offscreen((w * 2.0) as u32, (h * 2.0) as u32, 2.0).unwrap();
            let state = DockScene {
                layout: &layout,
                mag: &mag,
                center,
                size: (w, h),
                palette: crate::ui::theme::palette(Theme::Glass),
                acrylic: false,
                tiles: &tiles,
                pressed: None,
                tooltip: if editing { None } else { Some(1) },
                drag: None,
                drop_hover: false,
                hovered: if editing { Some(layout.slots().len() - 1) } else { None },
                tooltip_alpha: 1.0,
                px_scale: 2.0,
            };
            gfx.begin();
            draw_dock(&gfx, &state);
            gfx.end().unwrap();
            let path = dir.join(format!("dock-{name}.png"));
            gfx.save_png(&path).unwrap();
            assert!(std::fs::metadata(&path).unwrap().len() > 1000);
        }
    }
}
