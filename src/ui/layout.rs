//! 패널 모드 배치와 클릭 판정 (순수 함수). 모든 값은 DIP(96dpi 기준 논리 픽셀).

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Pt {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RectF {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl RectF {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> RectF {
        RectF { x, y, w, h }
    }
    pub fn contains(&self, p: Pt) -> bool {
        p.x >= self.x && p.x < self.x + self.w && p.y >= self.y && p.y < self.y + self.h
    }
    pub fn center(&self) -> Pt {
        Pt { x: self.x + self.w / 2.0, y: self.y + self.h / 2.0 }
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    /// 가운데를 기준으로 배율만큼 키운 사각형
    pub fn scaled(&self, s: f32) -> RectF {
        let c = self.center();
        RectF::new(c.x - self.w * s / 2.0, c.y - self.h * s / 2.0, self.w * s, self.h * s)
    }
}

pub const PADDING: f32 = 14.0;
pub const HEADER_H: f32 = 40.0;
pub const ICON: f32 = 56.0;
pub const GAP: f32 = 16.0;
pub const EDGE: f32 = 6.0;
pub const MENU_BUTTON: f32 = 28.0;
pub const PALETTE_BUTTON: f32 = 40.0;
pub const PANEL_HOVER_SCALE: f32 = 1.15;
pub const PICKER_RADIUS: f32 = 70.0;
pub const SWATCH: f32 = 34.0;
pub const MIN_W: f32 = 240.0;
pub const MIN_H: f32 = 200.0;
pub const DEFAULT_W: f32 = 360.0;
pub const DEFAULT_H: f32 = 480.0;
pub const MARGIN: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    /// 항목 칸 (0부터, ＋ 칸 제외)
    Tile(usize),
    Add,
    Palette,
    Menu,
    /// 헤더 빈 곳 (드래그하면 창 이동)
    Header,
    /// 창 가장자리 (크기 조절). 비트: 1=왼쪽 2=오른쪽 4=위 8=아래
    Edge(u8),
    Empty,
}

pub const EDGE_LEFT: u8 = 1;
pub const EDGE_RIGHT: u8 = 2;
pub const EDGE_TOP: u8 = 4;
pub const EDGE_BOTTOM: u8 = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct PanelLayout {
    pub width: f32,
    pub height: f32,
    pub header: RectF,
    pub menu_button: RectF,
    pub palette_button: RectF,
    /// 항목 칸들, 마지막은 ＋ 칸
    pub cells: Vec<RectF>,
}

impl PanelLayout {
    pub fn compute(width: f32, height: f32, items: usize) -> PanelLayout {
        let header = RectF::new(PADDING, PADDING - 4.0, width - 2.0 * PADDING, HEADER_H);
        let menu_button = RectF::new(
            header.right() - MENU_BUTTON,
            header.y + (HEADER_H - MENU_BUTTON) / 2.0,
            MENU_BUTTON,
            MENU_BUTTON,
        );
        let palette_button = RectF::new(
            width - PADDING - PALETTE_BUTTON,
            height - PADDING - PALETTE_BUTTON,
            PALETTE_BUTTON,
            PALETTE_BUTTON,
        );
        let top = header.bottom() + 12.0;
        let usable = (width - 2.0 * PADDING + GAP).max(ICON + GAP);
        let columns = ((usable / (ICON + GAP)).floor() as usize).max(1);
        let cells = (0..=items)
            .map(|i| {
                let (row, col) = (i / columns, i % columns);
                RectF::new(PADDING + col as f32 * (ICON + GAP), top + row as f32 * (ICON + GAP), ICON, ICON)
            })
            .collect();
        PanelLayout { width, height, header, menu_button, palette_button, cells }
    }

    pub fn tile_count(&self) -> usize {
        self.cells.len() - 1
    }

    pub fn add_cell(&self) -> RectF {
        *self.cells.last().expect("＋ 칸은 항상 있다")
    }

    fn edge_at(&self, p: Pt) -> u8 {
        let mut edge = 0;
        if p.x < EDGE {
            edge |= EDGE_LEFT;
        } else if p.x >= self.width - EDGE {
            edge |= EDGE_RIGHT;
        }
        if p.y < EDGE {
            edge |= EDGE_TOP;
        } else if p.y >= self.height - EDGE {
            edge |= EDGE_BOTTOM;
        }
        edge
    }

    /// `locked`면 가장자리·헤더 판정을 하지 않는다 (이동·크기 조절 금지).
    pub fn hit_test(&self, p: Pt, locked: bool) -> Hit {
        if !locked {
            let edge = self.edge_at(p);
            if edge != 0 {
                return Hit::Edge(edge);
            }
        }
        if self.menu_button.contains(p) {
            return Hit::Menu;
        }
        if self.palette_button.contains(p) {
            return Hit::Palette;
        }
        if let Some(i) = self.cells.iter().position(|c| c.contains(p)) {
            return if i == self.tile_count() { Hit::Add } else { Hit::Tile(i) };
        }
        if self.header.contains(p) && !locked {
            return Hit::Header;
        }
        Hit::Empty
    }

    /// 드래그 중인 항목을 놓을 자리: 마우스와 가장 가까운 항목 칸의 번호.
    pub fn drop_index(&self, p: Pt) -> usize {
        let n = self.tile_count();
        if n == 0 {
            return 0;
        }
        (0..n)
            .min_by(|&a, &b| {
                let da = dist2(self.cells[a].center(), p);
                let db = dist2(self.cells[b].center(), p);
                da.total_cmp(&db)
            })
            .unwrap_or(0)
    }

    /// 테마 선택 원의 중심: 팔레트 버튼을 중심으로 위쪽(-90°)부터 왼쪽(-180°)까지 부채꼴.
    pub fn swatch_centers(&self, count: usize, progress: f32) -> Vec<Pt> {
        let anchor = self.palette_button.center();
        let (start, end) = (-90.0f32, -180.0f32);
        let step = if count > 1 { (end - start) / (count - 1) as f32 } else { 0.0 };
        (0..count)
            .map(|i| {
                let a = (start + step * i as f32).to_radians();
                let r = PICKER_RADIUS * progress;
                Pt { x: anchor.x + r * a.cos(), y: anchor.y + r * a.sin() }
            })
            .collect()
    }
}

fn dist2(a: Pt, b: Pt) -> f32 {
    (a.x - b.x).powi(2) + (a.y - b.y).powi(2)
}

/// 기존 순서에서 `from`을 빼서 `to` 자리에 넣은 결과 (드래그 정렬)
pub fn reorder<T: Clone>(items: &[T], from: usize, to: usize) -> Vec<T> {
    let mut v = items.to_vec();
    if from < v.len() {
        let item = v.remove(from);
        v.insert(to.min(v.len()), item);
    }
    v
}

/// 작업 영역 우상단에 여백을 두고 기본 크기로 놓는다. (x, y, w, h)
pub fn default_panel_rect(work: (i32, i32, i32, i32), scale: f32) -> (i32, i32, i32, i32) {
    let (wx, wy, ww, wh) = work;
    let m = (MARGIN * scale) as i32;
    let w = ((DEFAULT_W * scale) as i32).min(ww - 2 * m);
    let h = ((DEFAULT_H * scale) as i32).min(wh - 2 * m);
    (wx + ww - w - m, wy + m, w, h)
}

/// 창 면적의 30% 이상이 어느 한 작업 영역과 겹치면 true (모니터를 뺀 뒤 화면 밖에 남는 것 방지)
pub fn is_visible_on_any(rect: (i32, i32, i32, i32), works: &[(i32, i32, i32, i32)]) -> bool {
    let (x, y, w, h) = rect;
    let area = w as i64 * h as i64;
    if area <= 0 {
        return false;
    }
    works.iter().any(|&(wx, wy, ww, wh)| {
        let iw = (x + w).min(wx + ww) - x.max(wx);
        let ih = (y + h).min(wy + wh) - y.max(wy);
        let overlap = iw.max(0) as i64 * ih.max(0) as i64;
        overlap * 10 >= area * 3
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_wraps_by_width_and_add_is_last() {
        let l = PanelLayout::compute(360.0, 480.0, 5);
        // (360 - 28 + 16) / 72 = 4.83 → 4열
        assert_eq!(l.cells.len(), 6);
        assert_eq!(l.cells[0].x, PADDING);
        assert_eq!(l.cells[3].x, PADDING + 3.0 * 72.0);
        assert_eq!(l.cells[4].y, l.cells[0].y + 72.0);
        assert_eq!(l.add_cell(), l.cells[5]);
        let narrow = PanelLayout::compute(100.0, 300.0, 2);
        assert_eq!(narrow.cells[1].x, PADDING);
    }

    #[test]
    fn hit_test_finds_buttons_tiles_and_edges() {
        let l = PanelLayout::compute(360.0, 480.0, 2);
        assert_eq!(l.hit_test(l.cells[1].center(), false), Hit::Tile(1));
        assert_eq!(l.hit_test(l.cells[2].center(), false), Hit::Add);
        assert_eq!(l.hit_test(l.menu_button.center(), false), Hit::Menu);
        assert_eq!(l.hit_test(l.palette_button.center(), false), Hit::Palette);
        assert_eq!(l.hit_test(Pt { x: 60.0, y: 25.0 }, false), Hit::Header);
        assert_eq!(l.hit_test(Pt { x: 1.0, y: 1.0 }, false), Hit::Edge(EDGE_LEFT | EDGE_TOP));
        assert_eq!(l.hit_test(Pt { x: 359.0, y: 240.0 }, false), Hit::Edge(EDGE_RIGHT));
        assert_eq!(l.hit_test(Pt { x: 200.0, y: 300.0 }, false), Hit::Empty);
    }

    #[test]
    fn locked_disables_move_and_resize() {
        let l = PanelLayout::compute(360.0, 480.0, 0);
        assert_eq!(l.hit_test(Pt { x: 1.0, y: 1.0 }, true), Hit::Empty);
        assert_eq!(l.hit_test(Pt { x: 60.0, y: 25.0 }, true), Hit::Empty);
        assert_eq!(l.hit_test(l.add_cell().center(), true), Hit::Add);
    }

    #[test]
    fn drop_index_picks_nearest_tile_cell() {
        let l = PanelLayout::compute(360.0, 480.0, 3);
        assert_eq!(l.drop_index(l.cells[2].center()), 2);
        assert_eq!(l.drop_index(Pt { x: 0.0, y: 0.0 }), 0);
        // ＋ 칸 위로 끌어도 마지막 항목 자리
        assert_eq!(l.drop_index(l.add_cell().center()), 2);
        assert_eq!(PanelLayout::compute(360.0, 480.0, 0).drop_index(Pt::default()), 0);
    }

    #[test]
    fn reorder_moves_one_item() {
        assert_eq!(reorder(&['a', 'b', 'c', 'd'], 0, 2), vec!['b', 'c', 'a', 'd']);
        assert_eq!(reorder(&['a', 'b', 'c'], 2, 0), vec!['c', 'a', 'b']);
        assert_eq!(reorder(&['a', 'b'], 1, 9), vec!['a', 'b']);
        assert_eq!(reorder(&['a'], 5, 0), vec!['a']);
    }

    #[test]
    fn swatches_fan_up_and_left_of_palette_button() {
        let l = PanelLayout::compute(360.0, 480.0, 0);
        let a = l.palette_button.center();
        let s = l.swatch_centers(4, 1.0);
        assert!((s[0].x - a.x).abs() < 0.01 && (s[0].y - (a.y - PICKER_RADIUS)).abs() < 0.01);
        assert!((s[3].y - a.y).abs() < 0.01 && (s[3].x - (a.x - PICKER_RADIUS)).abs() < 0.01);
        assert!(s.iter().all(|p| p.x <= a.x + 0.01 && p.y <= a.y + 0.01));
        assert!(l.swatch_centers(4, 0.0).iter().all(|p| *p == a));
    }

    #[test]
    fn default_rect_is_top_right_and_scaled() {
        assert_eq!(default_panel_rect((0, 0, 1920, 1040), 1.0), (1544, 16, 360, 480));
        assert_eq!(default_panel_rect((0, 0, 3840, 2080), 2.0), (3840 - 720 - 32, 32, 720, 960));
        assert_eq!(default_panel_rect((0, 0, 300, 400), 1.0), (16, 16, 268, 368));
    }

    #[test]
    fn visibility_requires_30_percent_overlap() {
        let works = [(0, 0, 1920, 1080), (1920, 0, 1280, 1024)];
        assert!(is_visible_on_any((100, 100, 360, 480), &works));
        assert!(is_visible_on_any((1800, 0, 200, 100), &works[..1])); // 60%
        assert!(!is_visible_on_any((1880, 0, 200, 100), &works[..1])); // 20%
        assert!(is_visible_on_any((2500, 100, 360, 480), &works));
        assert!(!is_visible_on_any((5000, 0, 360, 480), &works));
        assert!(!is_visible_on_any((0, 0, 0, 0), &works));
    }
}
