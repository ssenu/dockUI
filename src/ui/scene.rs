//! 패널 한 프레임 그리기: 유리 덧칠, 헤더, 아이콘, ＋, 팔레트 버튼, 말풍선, 테마 선택기, 드래그 중인 아이콘.

use crate::config::DockEdge;
use crate::config::Theme;
use crate::ui::dock_layout::{self, DockLayout, Magnified, Slot};
use crate::ui::gfx::{Align, Gfx, Weight};
use crate::ui::layout::{PANEL_HOVER_SCALE, PICKER_RADIUS, PanelLayout, Pt, RectF, SWATCH, fan_points};
use crate::ui::theme::{ACCENT, Color, FALLBACK_ALPHA, PICKER_ORDER, Palette, TOOLTIP_BG, TOOLTIP_TEXT};

pub const WINDOW_RADIUS: f32 = 8.0;
pub const ICON_RADIUS: f32 = 14.0;
pub const TITLE: &str = "YT Launcher";
pub const BYLINE: &str = "by_ssenu";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileKind {
    Youtube,
    App,
    Web,
}

/// 화면에 그릴 항목 하나
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileView {
    pub id: String,
    /// 말풍선에 보이는 이름
    pub label: String,
    /// 유튜브 아이콘 아래 두 글자 배지 (다른 종류는 아바타 글자)
    pub badge: String,
    pub kind: TileKind,
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

fn draw_letter_icon(g: &Gfx, r: RectF, seed: &str, letter: &str) {
    let base = avatar_color(seed);
    g.fill_round_rect_gradient(r, ICON_RADIUS * r.w / 56.0, base.shade(0.12), base.shade(-0.08));
    g.text(letter, r, 24.0 * r.w / 56.0, Weight::Bold, Align::Center, Color::hex(0xFFFFFF, 0.95));
}

fn draw_tile(g: &Gfx, tile: &TileView, r: RectF, lift: f32) {
    g.soft_shadow(r, ICON_RADIUS * r.w / 56.0, 1.0 + lift);
    match tile.kind {
        TileKind::Youtube => draw_youtube_icon(g, r, &tile.badge),
        TileKind::App | TileKind::Web => draw_letter_icon(g, r, &tile.id, &tile.badge),
    }
}

fn draw_add(g: &Gfx, r: RectF, p: &Palette) {
    g.fill_round_rect(r, ICON_RADIUS * r.w / 56.0, p.add_tile);
    let c = r.center();
    let arm = r.w * 0.16;
    let plus = Color::hex(0xFFFFFF, 0.92);
    g.line(Pt { x: c.x - arm, y: c.y }, Pt { x: c.x + arm, y: c.y }, plus, 2.6);
    g.line(Pt { x: c.x, y: c.y - arm }, Pt { x: c.x, y: c.y + arm }, plus, 2.6);
}

/// 원 안에 선으로 그린 팔레트 (엄지 구멍 + 물감 자리 3개)
fn draw_palette_button(g: &Gfx, r: RectF, p: &Palette, hover: bool) {
    let c = r.center();
    let radius = r.w / 2.0;
    g.soft_shadow(RectF::new(c.x - radius, c.y - radius, radius * 2.0, radius * 2.0), radius, 0.8);
    let face = p.tint.with_alpha(1.0).shade(if p.dark_backdrop { 0.10 } else { -0.03 });
    g.fill_circle(c, radius, if hover { face.shade(0.06) } else { face });
    g.stroke_circle(c, radius - 0.5, p.divider, 1.0);
    g.stroke_circle(c, radius * 0.45, p.text, 1.8);
    g.fill_circle(Pt { x: c.x + radius * 0.22, y: c.y + radius * 0.22 }, radius * 0.13, face);
    for (dx, dy) in [(-0.2f32, -0.06f32), (-0.04, -0.24), (0.16, -0.16)] {
        g.fill_circle(Pt { x: c.x + dx * radius, y: c.y + dy * radius }, radius * 0.07, p.text);
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

/// 아이콘 위에 말풍선을 띄운다. 위쪽이 `top_limit`보다 올라가면(헤더를 가리면) 아래에 띄운다.
fn draw_tooltip(g: &Gfx, text: &str, anchor: RectF, bounds: (f32, f32), top_limit: f32) {
    let w = g.measure(text, 12.0, Weight::SemiBold) + 20.0;
    let h = 26.0;
    let mut x = anchor.center().x - w / 2.0;
    x = x.clamp(6.0, (bounds.0 - w - 6.0).max(6.0));
    let mut y = anchor.y - h - 8.0;
    if y < top_limit {
        y = anchor.bottom() + 8.0;
    }
    let r = RectF::new(x, y, w, h);
    g.fill_round_rect(r, 8.0, TOOLTIP_BG);
    g.text(text, r, 12.0, Weight::SemiBold, Align::Center, TOOLTIP_TEXT);
}

/// 테마 색 원들 (패널·독 공용). `progress`는 펼침 정도 0..1.
fn draw_swatches(
    g: &Gfx,
    centers: &[Pt],
    progress: f32,
    palette: &Palette,
    current: Theme,
    hover: Option<usize>,
    bounds: (f32, f32),
) {
    let radius = SWATCH / 2.0 * (0.6 + 0.4 * progress);
    for (i, (theme, c)) in PICKER_ORDER.iter().zip(centers.iter()).enumerate() {
        let swatch = crate::ui::theme::palette(*theme);
        g.fill_circle(*c, radius + 4.0, palette.text.with_alpha(0.25));
        g.fill_circle(*c, radius, swatch.swatch);
        if *theme == current {
            g.stroke_circle(*c, radius, ACCENT, 2.5);
        } else {
            g.stroke_circle(*c, radius, palette.divider.with_alpha(0.6), 1.2);
        }
        if hover == Some(i) && progress >= 1.0 {
            let anchor = RectF::new(c.x - radius, c.y - radius, radius * 2.0, radius * 2.0);
            draw_tooltip(g, crate::ui::theme::label(*theme), anchor, bounds, 6.0);
        }
    }
}

fn draw_picker(g: &Gfx, s: &SceneState) {
    let centers = s.layout.swatch_centers(PICKER_ORDER.len(), s.picker);
    let bounds = (s.layout.width, s.layout.height);
    draw_swatches(g, &centers, s.picker, &s.palette, s.theme, s.picker_hover, bounds);
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
    /// 말풍선을 띄울 칸 번호 (항목·＋·팔레트·메뉴 순서)
    pub tooltip: Option<usize>,
    pub menu_hover: bool,
    pub palette_hover: bool,
    /// 드래그 중인 항목 (번호, 마우스 위치)
    pub drag: Option<(usize, Pt)>,
    pub picker: f32,
    pub picker_hover: Option<usize>,
    pub theme: Theme,
}

/// 독 말풍선: 판 반대쪽(화면 안쪽)에 띄운다.
fn draw_dock_tooltip(g: &Gfx, text: &str, icon: RectF, edge: DockEdge, bounds: (f32, f32)) {
    let w = g.measure(text, 12.0, Weight::SemiBold) + 20.0;
    let h = 26.0;
    let c = icon.center();
    let (x, y) = match edge {
        DockEdge::Bottom => (c.x - w / 2.0, icon.y - h - 8.0),
        DockEdge::Top => (c.x - w / 2.0, icon.bottom() + 8.0),
        DockEdge::Left => (icon.right() + 8.0, c.y - h / 2.0),
        DockEdge::Right => (icon.x - w - 8.0, c.y - h / 2.0),
    };
    let x = x.clamp(4.0, (bounds.0 - w - 4.0).max(4.0));
    let y = y.clamp(4.0, (bounds.1 - h - 4.0).max(4.0));
    let r = RectF::new(x, y, w, h);
    g.fill_round_rect(r, 8.0, TOOLTIP_BG);
    g.text(text, r, 12.0, Weight::SemiBold, Align::Center, TOOLTIP_TEXT);
}

fn draw_dots(g: &Gfx, r: RectF, p: &Palette, hover: bool) {
    let c = r.center();
    g.fill_circle(c, r.w * 0.42, p.text.with_alpha(if hover { 0.16 } else { 0.08 }));
    let step = r.w * 0.15;
    for dx in [-step, 0.0, step] {
        g.fill_circle(Pt { x: c.x + dx, y: c.y }, r.w * 0.05, p.text);
    }
}

/// 테마 선택 원 중심 (독): 팔레트 칸에서 화면 안쪽으로 부채꼴
pub fn dock_swatch_centers(layout: &DockLayout, mag: &Magnified, center: (f32, f32), progress: f32) -> Vec<Pt> {
    let anchor = layout.slot_rect(mag, center, layout.tiles + 1).center();
    fan_points(anchor, layout.picker_angles(), PICKER_ORDER.len(), PICKER_RADIUS * progress)
}

pub fn draw_dock(g: &Gfx, s: &DockScene) {
    let p = &s.palette;
    let tint = if s.acrylic { p.tint } else { p.tint.with_alpha(FALLBACK_ALPHA) };
    let bar = s.layout.bar_rect(s.mag, s.center);
    let bar = RectF::new(bar.x + 0.5, bar.y + 0.5, bar.w - 1.0, bar.h - 1.0);
    g.fill_round_rect(bar, WINDOW_RADIUS, tint);
    g.stroke_round_rect(bar, WINDOW_RADIUS, p.divider, 1.0);

    if !s.mag.separator.is_nan() {
        let along = s.mag.separator;
        let inset = dock_layout::PAD + 4.0;
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
            }
            Slot::Add => draw_add(g, r, p),
            Slot::Palette => draw_palette_button(g, r.scaled(0.84), p, s.palette_hover),
            Slot::Menu => draw_dots(g, r, p, s.menu_hover),
        }
    }

    if s.picker > 0.0 {
        let centers = dock_swatch_centers(s.layout, s.mag, s.center, s.picker);
        draw_swatches(g, &centers, s.picker, p, s.theme, s.picker_hover, s.size);
    }
    if let Some((from, pos)) = s.drag {
        let size = dock_layout::ICON * 1.1;
        let r = RectF::new(pos.x - size / 2.0, pos.y - size / 2.0, size, size);
        draw_tile(g, &s.tiles[from], r, 2.0);
    } else if let Some(i) = s.tooltip {
        let text = match s.layout.slots()[i] {
            Slot::Tile(t) => s.tiles[t].label.as_str(),
            Slot::Add => "항목 추가",
            Slot::Palette => "테마",
            Slot::Menu => "메뉴",
        };
        let r = s.layout.slot_rect(s.mag, s.center, i);
        draw_dock_tooltip(g, text, r, s.layout.edge, s.size);
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
                id: "1".into(), label: "로파이 음악".into(), badge: "로파".into(), kind: TileKind::Youtube
            },
            TileView { id: "2".into(), label: "아침 뉴스".into(), badge: "뉴스".into(), kind: TileKind::Youtube },
            TileView { id: "3".into(), label: "github.com".into(), badge: "G".into(), kind: TileKind::Web },
            TileView { id: "4".into(), label: "계산기".into(), badge: "계".into(), kind: TileKind::App },
        ];
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
                id: "1".into(), label: "로파이 음악".into(), badge: "로파".into(), kind: TileKind::Youtube
            },
            TileView { id: "2".into(), label: "아침 뉴스".into(), badge: "뉴스".into(), kind: TileKind::Youtube },
            TileView { id: "3".into(), label: "github.com".into(), badge: "G".into(), kind: TileKind::Web },
            TileView { id: "4".into(), label: "계산기".into(), badge: "계".into(), kind: TileKind::App },
        ];
        for (edge, name) in [(DockEdge::Bottom, "bottom"), (DockEdge::Left, "left")] {
            let layout = DockLayout::compute(edge, tiles.len());
            let mag = layout.magnified(Some(layout.rest[1]), 1.0);
            let max = layout.max_length();
            let vertical = dock_layout::is_vertical(edge);
            let (w, h) = if vertical {
                (dock_layout::THICK + dock_layout::HEADROOM, max + 2.0 * dock_layout::SIDE_ROOM)
            } else {
                (max + 2.0 * dock_layout::SIDE_ROOM, dock_layout::THICK + dock_layout::HEADROOM)
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
                menu_hover: false,
                palette_hover: false,
                drag: None,
                picker: if vertical { 1.0 } else { 0.0 },
                picker_hover: None,
                theme: Theme::Glass,
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
