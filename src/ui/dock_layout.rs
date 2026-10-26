//! 독 모드 배치 (순수 함수): 아이콘 칸 위치, 맥 독식 확대 물결, 클릭 판정, 화면 가장자리 배치.
//!
//! 좌표는 DIP. `along`은 독이 늘어선 방향(가로 독은 x, 세로 독은 y)으로 판 중심이 0이고,
//! `across`는 그에 수직인 방향으로 판 안에서의 위치(0..두께)다.

use crate::config::DockEdge;
use crate::ui::layout::{Hit, Pt, RectF, SWATCH};

pub const ICON: f32 = 48.0;
pub const GAP: f32 = 10.0;
pub const PAD: f32 = 10.0;
/// 판 두께 (아이콘 + 위아래 여백)
pub const THICK: f32 = ICON + 2.0 * PAD;
/// 화면 가장자리에서 띄우는 간격
pub const MARGIN: f32 = 8.0;
/// 마우스 바로 아래 아이콘이 커지는 정도 (1.6배)
pub const MAGNIFY: f32 = 0.6;
/// 확대가 퍼지는 거리 = 아이콘 칸 2.5개
pub const RANGE: f32 = 2.5 * (ICON + GAP);
/// 구분선 칸 너비 (가운데에 1px 선)
pub const SEPARATOR: f32 = GAP;
/// 콘텐츠 창이 판 바깥(화면 안쪽)으로 더 차지하는 여유: 확대 + 말풍선 + 테마 선택기
pub const HEADROOM: f32 = 130.0;
/// 테마 선택 원 사이 간격
pub const SWATCH_GAP: f32 = 10.0;
/// 판 바깥 면에서 테마 선택 원 줄까지 띄우는 거리
pub const SWATCH_LIFT: f32 = 16.0;
/// 콘텐츠 창이 판 양 끝으로 더 차지하는 여유 (확대로 늘어나는 길이 + 선택기)
pub const SIDE_ROOM: f32 = 90.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Tile(usize),
    Add,
    Palette,
    Menu,
}

/// 확대가 적용된 한 프레임의 배치. 위치는 판 중심 기준(along).
#[derive(Debug, Clone, PartialEq)]
pub struct Magnified {
    pub centers: Vec<f32>,
    pub scales: Vec<f32>,
    pub length: f32,
    /// 구분선 위치 (판 중심 기준)
    pub separator: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DockLayout {
    pub edge: DockEdge,
    pub tiles: usize,
    /// 확대 전 칸 중심 (판 중심 기준)
    pub rest: Vec<f32>,
    pub length: f32,
    pub separator: f32,
}

pub fn is_vertical(edge: DockEdge) -> bool {
    matches!(edge, DockEdge::Left | DockEdge::Right)
}

/// 확대 곡선: 거리 0에서 1, RANGE 밖에서 0
pub fn falloff(distance: f32) -> f32 {
    let d = distance.abs();
    if d >= RANGE { 0.0 } else { (std::f32::consts::FRAC_PI_2 * d / RANGE).cos() }
}

impl DockLayout {
    pub fn compute(edge: DockEdge, tiles: usize) -> DockLayout {
        let widths = vec![ICON; tiles + 3];
        let (rest, separator, length) = place(&widths, tiles);
        DockLayout { edge, tiles, rest, length, separator }
    }

    pub fn slots(&self) -> Vec<Slot> {
        let mut v: Vec<Slot> = (0..self.tiles).map(Slot::Tile).collect();
        v.extend([Slot::Add, Slot::Palette, Slot::Menu]);
        v
    }

    /// `pointer`: 마우스 위치(판 중심 기준 along, 확대 전 좌표). `amount`: 확대 정도 0..1 (애니메이션).
    pub fn magnified(&self, pointer: Option<f32>, amount: f32) -> Magnified {
        let scales: Vec<f32> = self
            .rest
            .iter()
            .map(|&c| match pointer {
                Some(p) => 1.0 + MAGNIFY * amount * falloff(c - p),
                None => 1.0,
            })
            .collect();
        let widths: Vec<f32> = scales.iter().map(|s| ICON * s).collect();
        let (centers, separator, length) = place(&widths, self.tiles);
        Magnified { centers, scales, length, separator }
    }

    /// 가장 많이 커졌을 때의 길이 (콘텐츠 창 크기 계산용)
    pub fn max_length(&self) -> f32 {
        self.rest.iter().map(|&c| self.magnified(Some(c), 1.0).length).fold(self.length, f32::max)
    }

    /// along/across(판 안 좌표, along은 판 중심 기준)로 칸을 찾는다.
    pub fn hit(&self, m: &Magnified, along: f32, across: f32) -> Hit {
        if !(0.0..THICK).contains(&across) || along.abs() > m.length / 2.0 {
            return Hit::Empty;
        }
        for (i, (&c, &s)) in m.centers.iter().zip(&m.scales).enumerate() {
            if (along - c).abs() <= ICON * s / 2.0 + GAP / 2.0 {
                return match self.slots()[i] {
                    Slot::Tile(t) => Hit::Tile(t),
                    Slot::Add => Hit::Add,
                    Slot::Palette => Hit::Palette,
                    Slot::Menu => Hit::Menu,
                };
            }
        }
        Hit::Empty
    }

    /// 드래그한 항목을 놓을 자리: 확대 전 칸 중 가장 가까운 항목 칸
    pub fn drop_index(&self, along: f32) -> usize {
        (0..self.tiles)
            .min_by(|&a, &b| (self.rest[a] - along).abs().total_cmp(&(self.rest[b] - along).abs()))
            .unwrap_or(0)
    }

    /// 판 사각형 (콘텐츠 창 기준). `center`는 판 중심.
    pub fn bar_rect(&self, m: &Magnified, center: (f32, f32)) -> RectF {
        let (cx, cy) = center;
        if is_vertical(self.edge) {
            RectF::new(cx - THICK / 2.0, cy - m.length / 2.0, THICK, m.length)
        } else {
            RectF::new(cx - m.length / 2.0, cy - THICK / 2.0, m.length, THICK)
        }
    }

    /// i번째 칸의 아이콘 사각형. 커질 때 화면 가장자리 쪽 면은 고정하고 화면 안쪽으로 자란다.
    pub fn slot_rect(&self, m: &Magnified, center: (f32, f32), i: usize) -> RectF {
        let bar = self.bar_rect(m, center);
        let size = ICON * m.scales[i];
        let along = m.centers[i];
        match self.edge {
            DockEdge::Bottom => RectF::new(center.0 + along - size / 2.0, bar.bottom() - PAD - size, size, size),
            DockEdge::Top => RectF::new(center.0 + along - size / 2.0, bar.y + PAD, size, size),
            DockEdge::Left => RectF::new(bar.x + PAD, center.1 + along - size / 2.0, size, size),
            DockEdge::Right => RectF::new(bar.right() - PAD - size, center.1 + along - size / 2.0, size, size),
        }
    }

    /// 화면 안쪽(판에서 멀어지는 쪽) 단위 벡터
    pub fn outward(&self) -> (f32, f32) {
        match self.edge {
            DockEdge::Bottom => (0.0, -1.0),
            DockEdge::Top => (0.0, 1.0),
            DockEdge::Left => (1.0, 0.0),
            DockEdge::Right => (-1.0, 0.0),
        }
    }

    /// 테마 선택 원 중심: `slot` 칸 바깥쪽에 판과 나란히 한 줄. `progress`(0..1)만큼 칸 중심에서 펼쳐진다.
    pub fn swatch_row(&self, m: &Magnified, center: (f32, f32), slot: usize, count: usize, progress: f32) -> Vec<Pt> {
        let (ox, oy) = self.outward();
        let (ax, ay) = (oy.abs(), ox.abs());
        let along = m.centers[slot];
        let anchor = Pt { x: center.0 + ax * along, y: center.1 + ay * along };
        let lift = THICK / 2.0 + SWATCH_LIFT + SWATCH / 2.0;
        let step = SWATCH + SWATCH_GAP;
        let first = -(count.saturating_sub(1) as f32) * step / 2.0;
        (0..count)
            .map(|i| {
                let a = first + i as f32 * step;
                Pt { x: anchor.x + progress * (ox * lift + ax * a), y: anchor.y + progress * (oy * lift + ay * a) }
            })
            .collect()
    }
}

/// 칸 너비들을 간격·여백·구분선과 함께 늘어놓고, 판 중심이 0이 되도록 옮긴다.
/// 반환: (칸 중심들, 구분선 위치, 전체 길이)
fn place(widths: &[f32], tiles: usize) -> (Vec<f32>, f32, f32) {
    let mut x = PAD;
    let mut centers = Vec::with_capacity(widths.len());
    let mut separator = 0.0;
    for (i, w) in widths.iter().enumerate() {
        if i == tiles {
            // 항목들과 ＋·팔레트·메뉴 사이 구분선
            if tiles > 0 {
                separator = x + SEPARATOR / 2.0;
                x += SEPARATOR + GAP;
            } else {
                separator = f32::NAN;
            }
        }
        centers.push(x + w / 2.0);
        x += w + GAP;
    }
    let length = x - GAP + PAD;
    let half = length / 2.0;
    (centers.iter().map(|c| c - half).collect(), separator - half, length)
}

/// 판의 화면 사각형 (px). `work`는 모니터 작업 영역, 길이·두께는 px.
pub fn bar_rect(
    work: (i32, i32, i32, i32),
    edge: DockEdge,
    length: i32,
    thick: i32,
    margin: i32,
) -> (i32, i32, i32, i32) {
    let (wx, wy, ww, wh) = work;
    match edge {
        DockEdge::Bottom => (wx + (ww - length) / 2, wy + wh - margin - thick, length, thick),
        DockEdge::Top => (wx + (ww - length) / 2, wy + margin, length, thick),
        DockEdge::Left => (wx + margin, wy + (wh - length) / 2, thick, length),
        DockEdge::Right => (wx + ww - margin - thick, wy + (wh - length) / 2, thick, length),
    }
}

/// 콘텐츠 창의 화면 사각형 (px): 판을 화면 안쪽으로 `headroom`, 양 끝으로 `side`만큼 넓힌다.
pub fn content_rect(bar: (i32, i32, i32, i32), edge: DockEdge, headroom: i32, side: i32) -> (i32, i32, i32, i32) {
    let (x, y, w, h) = bar;
    match edge {
        DockEdge::Bottom => (x - side, y - headroom, w + 2 * side, h + headroom),
        DockEdge::Top => (x - side, y, w + 2 * side, h + headroom),
        DockEdge::Left => (x, y - side, w + headroom, h + 2 * side),
        DockEdge::Right => (x - headroom, y - side, w + headroom, h + 2 * side),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rest_layout_is_centered_with_separator() {
        let l = DockLayout::compute(DockEdge::Bottom, 2);
        // 칸 5개(항목 2 + ＋·팔레트·메뉴) + 구분선 칸
        let expected = PAD * 2.0 + 5.0 * ICON + 4.0 * GAP + SEPARATOR + GAP;
        assert!((l.length - expected).abs() < 0.01);
        assert!((l.rest[0] + l.length / 2.0 - (PAD + ICON / 2.0)).abs() < 0.01);
        assert!(l.separator > l.rest[1] && l.separator < l.rest[2]);
        let sum: f32 = l.rest.first().unwrap() + l.rest.last().unwrap();
        assert!(sum.abs() < 0.01, "가운데 정렬");
    }

    #[test]
    fn no_tiles_means_no_separator() {
        let l = DockLayout::compute(DockEdge::Bottom, 0);
        assert_eq!(l.slots(), vec![Slot::Add, Slot::Palette, Slot::Menu]);
        assert!(l.separator.is_nan());
    }

    #[test]
    fn falloff_peaks_under_pointer_and_fades() {
        assert_eq!(falloff(0.0), 1.0);
        assert!(falloff(ICON) < 1.0 && falloff(ICON) > 0.5);
        assert_eq!(falloff(RANGE), 0.0);
        assert_eq!(falloff(-RANGE * 2.0), 0.0);
    }

    #[test]
    fn magnify_grows_pointed_icon_most_and_pushes_neighbors() {
        let l = DockLayout::compute(DockEdge::Bottom, 5);
        let m = l.magnified(Some(l.rest[2]), 1.0);
        assert!((m.scales[2] - 1.6).abs() < 1e-4);
        assert!(m.scales[1] < m.scales[2] && m.scales[1] > 1.0);
        assert_eq!(m.scales[1], m.scales[3]);
        assert!(m.length > l.length);
        // 커진 만큼 왼쪽 아이콘은 더 왼쪽으로 밀린다
        assert!(m.centers[0] < l.rest[0]);
        assert_eq!(l.magnified(None, 1.0).scales, vec![1.0; 8]);
        assert_eq!(l.magnified(Some(l.rest[2]), 0.0).length, l.length);
        assert!(l.max_length() >= m.length);
    }

    #[test]
    fn hit_uses_magnified_positions() {
        let l = DockLayout::compute(DockEdge::Bottom, 2);
        let m = l.magnified(None, 0.0);
        assert_eq!(l.hit(&m, l.rest[0], THICK / 2.0), Hit::Tile(0));
        assert_eq!(l.hit(&m, l.rest[2], THICK / 2.0), Hit::Add);
        assert_eq!(l.hit(&m, l.rest[3], THICK / 2.0), Hit::Palette);
        assert_eq!(l.hit(&m, l.rest[4], THICK / 2.0), Hit::Menu);
        assert_eq!(l.hit(&m, l.rest[0], THICK + 5.0), Hit::Empty);
        assert_eq!(l.hit(&m, l.length, THICK / 2.0), Hit::Empty);
    }

    #[test]
    fn drop_index_picks_nearest_tile() {
        let l = DockLayout::compute(DockEdge::Bottom, 3);
        assert_eq!(l.drop_index(l.rest[2]), 2);
        assert_eq!(l.drop_index(-1000.0), 0);
        assert_eq!(l.drop_index(1000.0), 2);
        assert_eq!(DockLayout::compute(DockEdge::Bottom, 0).drop_index(0.0), 0);
    }

    #[test]
    fn bar_sits_on_each_edge_with_margin() {
        let work = (0, 0, 1920, 1040);
        assert_eq!(bar_rect(work, DockEdge::Bottom, 400, 68, 8), (760, 964, 400, 68));
        assert_eq!(bar_rect(work, DockEdge::Top, 400, 68, 8), (760, 8, 400, 68));
        assert_eq!(bar_rect(work, DockEdge::Left, 400, 68, 8), (8, 320, 68, 400));
        assert_eq!(bar_rect(work, DockEdge::Right, 400, 68, 8), (1844, 320, 68, 400));
    }

    #[test]
    fn content_extends_into_the_screen() {
        let bar = (760, 964, 400, 68);
        assert_eq!(content_rect(bar, DockEdge::Bottom, 110, 90), (670, 854, 580, 178));
        assert_eq!(content_rect((760, 8, 400, 68), DockEdge::Top, 110, 90), (670, 8, 580, 178));
        assert_eq!(content_rect((8, 320, 68, 400), DockEdge::Left, 110, 90), (8, 230, 178, 580));
        assert_eq!(content_rect((1844, 320, 68, 400), DockEdge::Right, 110, 90), (1734, 230, 178, 580));
    }

    #[test]
    fn swatches_line_up_outside_the_bar() {
        let center = (300.0, 200.0);
        for edge in [DockEdge::Bottom, DockEdge::Top, DockEdge::Left, DockEdge::Right] {
            let l = DockLayout::compute(edge, 2);
            let m = l.magnified(None, 0.0);
            let bar = l.bar_rect(&m, center);
            let palette = l.slot_rect(&m, center, 3).center();
            let row = l.swatch_row(&m, center, 3, 4, 1.0);
            let r = SWATCH / 2.0;
            for p in &row {
                let clear = match edge {
                    DockEdge::Bottom => p.y + r < bar.y,
                    DockEdge::Top => p.y - r > bar.bottom(),
                    DockEdge::Left => p.x - r > bar.right(),
                    DockEdge::Right => p.x + r < bar.x,
                };
                assert!(clear, "{edge:?}: 판과 겹치지 않음");
            }
            // 팔레트 칸을 가운데로 나란히, 서로 겹치지 않게
            let mid = Pt { x: (row[0].x + row[3].x) / 2.0, y: (row[0].y + row[3].y) / 2.0 };
            let (ox, oy) = l.outward();
            assert!(((mid.x - palette.x) * oy).abs() < 0.01 && ((mid.y - palette.y) * ox).abs() < 0.01);
            let d = ((row[1].x - row[0].x).powi(2) + (row[1].y - row[0].y).powi(2)).sqrt();
            assert!(d >= SWATCH + SWATCH_GAP - 0.01);
            assert!(l.swatch_row(&m, center, 3, 4, 0.0).iter().all(|p| *p == palette));
        }
    }

    #[test]
    fn icons_grow_away_from_the_screen_edge() {
        let center = (300.0, 200.0);
        let bottom = DockLayout::compute(DockEdge::Bottom, 1);
        let m = bottom.magnified(Some(bottom.rest[0]), 1.0);
        let bar = bottom.bar_rect(&m, center);
        let r = bottom.slot_rect(&m, center, 0);
        assert!((r.bottom() - (bar.bottom() - PAD)).abs() < 0.01, "아래 면 고정");
        assert!(r.y < bar.y + PAD, "위로 커짐");
        let left = DockLayout::compute(DockEdge::Left, 1);
        let ml = left.magnified(Some(left.rest[0]), 1.0);
        let lr = left.slot_rect(&ml, center, 0);
        assert!((lr.x - (left.bar_rect(&ml, center).x + PAD)).abs() < 0.01, "왼쪽 면 고정");
        assert!(left.bar_rect(&ml, center).h > left.bar_rect(&ml, center).w, "세로 판");
    }
}
