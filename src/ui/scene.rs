//! 패널 한 프레임 그리기: 유리 덧칠, 헤더, 아이콘, ＋, 팔레트 버튼, 말풍선, 테마 선택기, 드래그 중인 아이콘.

use crate::config::Theme;
use crate::ui::gfx::{Align, Gfx, Weight};
use crate::ui::layout::{PANEL_HOVER_SCALE, PanelLayout, Pt, RectF, SWATCH};
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

fn draw_picker(g: &Gfx, s: &SceneState) {
    let centers = s.layout.swatch_centers(PICKER_ORDER.len(), s.picker);
    let radius = SWATCH / 2.0 * (0.6 + 0.4 * s.picker);
    for (i, (theme, c)) in PICKER_ORDER.iter().zip(centers.iter()).enumerate() {
        let swatch = crate::ui::theme::palette(*theme);
        g.fill_circle(*c, radius + 4.0, s.palette.text.with_alpha(0.25));
        g.fill_circle(*c, radius, swatch.swatch);
        if *theme == s.theme {
            g.stroke_circle(*c, radius, ACCENT, 2.5);
        } else {
            g.stroke_circle(*c, radius, s.palette.divider.with_alpha(0.6), 1.2);
        }
        if s.picker_hover == Some(i) && s.picker >= 1.0 {
            let anchor = RectF::new(c.x - radius, c.y - radius, radius * 2.0, radius * 2.0);
            draw_tooltip(g, crate::ui::theme::label(*theme), anchor, (s.layout.width, s.layout.height), 6.0);
        }
    }
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
}
