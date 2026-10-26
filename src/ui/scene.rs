//! 패널·독 한 프레임 그리기: 유리 덧칠, 헤더, 아이콘, ＋, 팔레트 버튼, 말풍선, 테마 선택기, 드래그 중인 아이콘.

use crate::config::DockEdge;
use crate::config::Theme;
use crate::ui::dock_layout::{self, DockLayout, Magnified, Slot, TOOLTIP_MAX_W};
use crate::ui::gfx::{Align, Gfx, Weight};
use crate::ui::layout::{PANEL_HOVER_SCALE, PanelLayout, Pt, RectF, SWATCH};
use crate::ui::theme::{ACCENT, Color, FALLBACK_ALPHA, PICKER_ORDER, Palette, TOOLTIP_BG, TOOLTIP_TEXT};
use std::path::PathBuf;

pub const WINDOW_RADIUS: f32 = 8.0;
pub const ICON_RADIUS: f32 = 14.0;
pub const TITLE: &str = "YT Launcher";
pub const BYLINE: &str = "by_ssenu";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TileKind {
    #[default]
    Youtube,
    App,
    Web,
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
    pub palette_hover: bool,
    pub drag: Option<DragView>,
    /// 말풍선을 띄울 칸 번호
    pub tooltip: Option<usize>,
    /// 테마 선택기 펼침 정도 0..1 (0이면 닫힘)
    pub picker: f32,
    pub picker_hover: Option<usize>,
    pub theme: Theme,
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
    // 앱 아이콘은 자체 모양(투명 배경)이 있으므로 그림자 없이 그대로 그린다
    if tile.kind == TileKind::App
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

/// 그림자 있는 떠 있는 둥근 버튼 바탕 (패널 팔레트 버튼)
fn draw_round_button(g: &Gfx, c: Pt, radius: f32, p: &Palette, hover: bool) {
    g.soft_shadow(RectF::new(c.x - radius, c.y - radius, radius * 2.0, radius * 2.0), radius, 0.8);
    let face = p.tint.with_alpha(1.0).shade(if p.dark_backdrop { 0.10 } else { -0.03 });
    g.fill_circle(c, radius, if hover { face.shade(0.06) } else { face });
    g.stroke_circle(c, radius - 0.5, p.divider, 1.0);
}

/// 원 안에 그린 팔레트: 테두리 + 엄지 구멍 + 물감 네 방울
fn draw_palette_button(g: &Gfx, r: RectF, p: &Palette, hover: bool) {
    let c = r.center();
    let radius = r.w / 2.0;
    draw_round_button(g, c, radius, p, hover);
    let body = radius * 0.54;
    g.stroke_circle(c, body, p.text, 1.6);
    g.stroke_circle(Pt { x: c.x + body * 0.42, y: c.y + body * 0.42 }, body * 0.2, p.text, 1.4);
    const PAINTS: [u32; 4] = [0xE53935, 0xFBC02D, 0x43A047, 0x1E88E5];
    for (k, rgb) in PAINTS.iter().enumerate() {
        let a = (110.0 + 55.0 * k as f32).to_radians();
        let at = Pt { x: c.x + body * 0.55 * a.cos(), y: c.y + body * 0.55 * a.sin() };
        g.fill_circle(at, body * 0.17, Color::hex(*rgb, 1.0));
    }
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

/// 말풍선 크기 (너비, 높이): 이름을 자르지 않고, `TOOLTIP_MAX_W`나 `room`보다 길면 줄을 바꾼다
fn tooltip_size(g: &Gfx, text: &str, room: f32) -> (f32, f32) {
    let max_w = TOOLTIP_MAX_W.min(room - 8.0).max(60.0);
    let (w, h) = g.measure_block(text, 12.0, Weight::SemiBold, max_w - 20.0);
    ((w + 20.0).ceil().clamp(40.0, max_w), (h + 10.0).ceil().max(26.0))
}

/// 말풍선 상자. 어두운 배경(투명 테마)에서도 경계가 보이게 옅은 테두리를 두른다.
fn draw_tooltip_box(g: &Gfx, text: &str, r: RectF) {
    g.fill_round_rect(r, 8.0, TOOLTIP_BG);
    g.stroke_round_rect(RectF::new(r.x + 0.5, r.y + 0.5, r.w - 1.0, r.h - 1.0), 7.5, Color::hex(0xFFFFFF, 0.14), 1.0);
    let inner = RectF::new(r.x + 10.0, r.y, r.w - 20.0, r.h);
    g.text_block(text, inner, 12.0, Weight::SemiBold, Align::Center, TOOLTIP_TEXT);
}

/// 아이콘 위에 말풍선을 띄운다. 위쪽이 `top_limit`보다 올라가면(헤더를 가리면) 아래에 띄운다.
fn draw_tooltip(g: &Gfx, text: &str, anchor: RectF, bounds: (f32, f32), top_limit: f32) {
    let (w, h) = tooltip_size(g, text, bounds.0);
    let mut x = anchor.center().x - w / 2.0;
    x = x.clamp(6.0, (bounds.0 - w - 6.0).max(6.0));
    let mut y = anchor.y - h - 8.0;
    if y < top_limit {
        y = anchor.bottom() + 8.0;
    }
    let r = RectF::new(x, y, w, h);
    draw_tooltip_box(g, text, r);
}

/// 원(중심 c, 반지름 r)에서 `dir` 방향으로 떨어진 곳에 말풍선을 띄운다 (이웃 원을 가리지 않게).
fn draw_tooltip_toward(g: &Gfx, text: &str, c: Pt, r: f32, dir: (f32, f32), bounds: (f32, f32)) {
    let w = g.measure(text, 12.0, Weight::SemiBold) + 20.0;
    let h = 26.0;
    // 상자 가운데에서 dir 방향으로 상자 테두리까지 거리
    let half = match (dir.0.abs() > 1e-3, dir.1.abs() > 1e-3) {
        (true, true) => (w / 2.0 / dir.0.abs()).min(h / 2.0 / dir.1.abs()),
        (true, false) => w / 2.0,
        _ => h / 2.0,
    };
    let d = r + 8.0 + half;
    let x = (c.x + dir.0 * d - w / 2.0).clamp(4.0, (bounds.0 - w - 4.0).max(4.0));
    let y = (c.y + dir.1 * d - h / 2.0).clamp(4.0, (bounds.1 - h - 4.0).max(4.0));
    let rect = RectF::new(x, y, w, h);
    draw_tooltip_box(g, text, rect);
}

/// 테마 색 원들 (패널·독 공용). `progress`는 펼침 정도 0..1, `tip_dirs`는 원마다 말풍선을 띄울 방향.
fn draw_swatches(
    g: &Gfx,
    centers: &[Pt],
    tip_dirs: &[(f32, f32)],
    progress: f32,
    current: Theme,
    hover: Option<usize>,
    bounds: (f32, f32),
) {
    let base = SWATCH / 2.0 * (0.6 + 0.4 * progress);
    let hover = hover.filter(|_| progress >= 1.0);
    for (i, (theme, c)) in PICKER_ORDER.iter().zip(centers.iter()).enumerate() {
        let radius = if hover == Some(i) { base * 1.08 } else { base };
        let swatch = crate::ui::theme::palette(*theme);
        g.soft_shadow(RectF::new(c.x - radius, c.y - radius, radius * 2.0, radius * 2.0), radius, 0.7);
        g.fill_circle(*c, radius, swatch.swatch);
        // 밝은 원도 배경과 구분되게 안쪽에 옅은 테두리
        let rim = if swatch.dark_backdrop { Color::hex(0xFFFFFF, 0.28) } else { Color::hex(0x000000, 0.14) };
        g.stroke_circle(*c, radius - 0.5, rim, 1.0);
        if *theme == current {
            g.stroke_circle(*c, radius + 3.5, ACCENT, 2.0);
        }
    }
    if let Some(i) = hover
        && let (Some(theme), Some(c), Some(dir)) = (PICKER_ORDER.get(i), centers.get(i), tip_dirs.get(i))
    {
        draw_tooltip_toward(g, crate::ui::theme::label(*theme), *c, base * 1.08 + 3.5, *dir, bounds);
    }
}

fn draw_picker(g: &Gfx, s: &SceneState) {
    let centers = s.layout.swatch_centers(PICKER_ORDER.len(), s.picker);
    let anchor = s.layout.palette_button.center();
    // 부채꼴 바깥쪽(팔레트 버튼에서 멀어지는 쪽)
    let dirs: Vec<(f32, f32)> = centers
        .iter()
        .map(|c| {
            let (dx, dy) = (c.x - anchor.x, c.y - anchor.y);
            let len = (dx * dx + dy * dy).sqrt().max(1e-3);
            (dx / len, dy / len)
        })
        .collect();
    let bounds = (s.layout.width, s.layout.height);
    draw_swatches(g, &centers, &dirs, s.picker, s.theme, s.picker_hover, bounds);
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
    draw_palette_button(g, s.layout.palette_button, p, s.palette_hover);

    if s.picker > 0.0 {
        draw_picker(g, s);
    }
    if let Some(d) = &s.drag {
        let cell = s.layout.cells[0];
        let r = RectF::new(d.pos.x - cell.w / 2.0, d.pos.y - cell.h / 2.0, cell.w, cell.h).scaled(1.1);
        draw_tile(g, &s.tiles[d.from], r, 2.0);
    } else if let Some(i) = s.tooltip {
        let text = if i < s.tiles.len() { s.tiles[i].label.as_str() } else { "항목 추가" };
        draw_tooltip(g, text, s.layout.cells[i], (w, h), s.layout.header.bottom());
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
}

/// 독 말풍선 사각형: 판 반대쪽(화면 안쪽)에 띄운다. 창 영역 계산에도 쓴다.
pub fn dock_tooltip_rect(g: &Gfx, text: &str, icon: RectF, edge: DockEdge, bounds: (f32, f32)) -> RectF {
    let room = if dock_layout::is_vertical(edge) { bounds.0 - icon.right() - 8.0 } else { bounds.0 };
    let (w, h) = tooltip_size(g, text, room.max(68.0));
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
        }
    }
    if let Some((from, pos)) = s.drag {
        let size = dock_layout::ICON * 1.1;
        let r = RectF::new(pos.x - size / 2.0, pos.y - size / 2.0, size, size);
        draw_tile(g, &s.tiles[from], r, 2.0);
    } else if let Some(i) = s.tooltip {
        let text = dock_tooltip_text(s.layout, s.tiles, i);
        let icon = s.layout.slot_rect(s.mag, s.center, i);
        draw_tooltip_box(g, text, dock_tooltip_rect(g, text, icon, s.layout.edge, s.size));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let layout = PanelLayout::compute(360.0, 480.0, tiles.len());
        for (i, theme) in PICKER_ORDER.iter().enumerate() {
            let gfx = Gfx::offscreen(360 * 2, 480 * 2, 2.0).unwrap();
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
                palette_hover: false,
                drag: None,
                tooltip: Some(1),
                picker: if i == 0 { 1.0 } else { 0.0 },
                picker_hover: if i == 0 { Some(2) } else { None },
                theme: *theme,
                drop_hover: *theme == Theme::Glass,
            };
            gfx.begin();
            draw(&gfx, &state);
            gfx.end().unwrap();
            let path = dir.join(format!("panel-{}.png", crate::ui::theme::label(*theme)));
            gfx.save_png(&path).unwrap();
            assert!(std::fs::metadata(&path).unwrap().len() > 1000);
        }
    }

    #[test]
    fn renders_dock_snapshots_for_bottom_and_left_edges() {
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
        for (edge, name) in [(DockEdge::Bottom, "bottom"), (DockEdge::Left, "left")] {
            let layout = DockLayout::compute(edge, tiles.len());
            let mag = layout.magnified(Some(layout.rest[1]), 1.0);
            let max = layout.max_length();
            let vertical = dock_layout::is_vertical(edge);
            let (w, h) = if vertical {
                (dock_layout::THICK + layout.headroom(), max + 2.0 * layout.side_room())
            } else {
                (max + 2.0 * layout.side_room(), dock_layout::THICK + layout.headroom())
            };
            let center =
                if vertical { (dock_layout::THICK / 2.0, h / 2.0) } else { (w / 2.0, h - dock_layout::THICK / 2.0) };
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
                tooltip: Some(1),
                drag: None,
                drop_hover: false,
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
