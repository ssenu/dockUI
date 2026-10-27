//! 독 모드 배치 (순수 함수): 항목 아이콘 칸 위치, 맥 독식 확대 물결, 클릭 판정, 화면 가장자리 배치.
//!
//! 좌표는 DIP. `along`은 독이 늘어선 방향(가로 독은 x, 세로 독은 y)으로 판 중심이 0이고,
//! `across`는 그에 수직인 방향으로 판 안에서의 위치(0..두께)다.

use crate::config::DockEdge;
use crate::ui::layout::{Hit, RectF};

pub const ICON: f32 = 48.0;
pub const GAP: f32 = 10.0;
pub const PAD: f32 = 10.0;
/// 판 두께 (아이콘 + 위아래 여백)
pub const THICK: f32 = ICON + 2.0 * PAD;
/// 화면 가장자리에서 띄우는 간격
pub const MARGIN: f32 = 8.0;
/// 마우스 바로 아래 아이콘이 커지는 정도 (1.3배, 은은하게)
pub const MAGNIFY: f32 = 0.3;
/// 확대가 퍼지는 거리 = 아이콘 칸 1.5개 (바로 옆 아이콘만 조금)
pub const RANGE: f32 = 1.5 * (ICON + GAP);
/// 콘텐츠 창이 판 바깥(화면 안쪽)으로 더 차지하는 여유: 확대 + 여러 줄 말풍선
pub const HEADROOM: f32 = 140.0;
/// 콘텐츠 창이 판 양 끝으로 더 차지하는 여유 (확대로 늘어나는 길이 + 선택기)
pub const SIDE_ROOM: f32 = 90.0;
/// 독 아이콘 크기(설정값, DIP): 기본·최소·최대·한 단계
pub const DEFAULT_SIZE: u32 = 48;
pub const MIN_SIZE: u32 = 32;
pub const MAX_SIZE: u32 = 80;
pub const SIZE_STEP: u32 = 8;

/// 아이콘 크기 설정 → 독 전체 배율 (배치는 48px 기준으로 하고 그리기 배율로 키운다)
pub fn zoom_for(size: u32) -> f32 {
    size.clamp(MIN_SIZE, MAX_SIZE) as f32 / ICON
}

/// 한 단계 작게(-1) / 크게(+1)
pub fn step_size(size: u32, dir: i32) -> u32 {
    let next = size as i32 + dir * SIZE_STEP as i32;
    (next.max(0) as u32).clamp(MIN_SIZE, MAX_SIZE)
}

/// 독 칸: 항목 아이콘만 둔다. 항목이 하나도 없을 때만 ＋ 칸 하나 (나머지 기능은 트레이 메뉴).
/// 독 수정 모드에서는 끝에 작게 / 크게 / 완료 단추가 붙는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Tile(usize),
    Add,
    Shrink,
    Grow,
    Done,
}

/// 확대가 적용된 한 프레임의 배치. 위치는 판 중심 기준(along).
#[derive(Debug, Clone, PartialEq)]
pub struct Magnified {
    pub centers: Vec<f32>,
    pub scales: Vec<f32>,
    pub length: f32,
    /// 항목과 수정 단추 사이 구분선 (판 중심 기준, 수정 모드에서만)
    pub separator: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DockLayout {
    pub edge: DockEdge,
    pub tiles: usize,
    /// 독 수정 모드 (끝에 수정 단추)
    pub editing: bool,
    /// 확대 전 칸 중심 (판 중심 기준)
    pub rest: Vec<f32>,
    pub length: f32,
    pub separator: Option<f32>,
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
    pub fn compute(edge: DockEdge, tiles: usize, editing: bool) -> DockLayout {
        let mut layout = DockLayout { edge, tiles, editing, rest: Vec::new(), length: 0.0, separator: None };
        let widths = vec![ICON; layout.slots().len()];
        (layout.rest, layout.length, layout.separator) = place(&widths, layout.split());
        layout
    }

    pub fn slots(&self) -> Vec<Slot> {
        let mut v = if self.tiles == 0 { vec![Slot::Add] } else { (0..self.tiles).map(Slot::Tile).collect() };
        if self.editing {
            v.extend([Slot::Shrink, Slot::Grow, Slot::Done]);
        }
        v
    }

    /// 구분선이 들어갈 칸 번호 (수정 단추 앞)
    fn split(&self) -> Option<usize> {
        self.editing.then_some(self.tiles.max(1))
    }

    /// 콘텐츠 창이 판 양 끝으로 더 차지하는 여유: 판이 짧아도 가장 긴 말풍선(`longest_tip`, DIP)이 들어가게
    pub fn side_room(&self, longest_tip: f32) -> f32 {
        if is_vertical(self.edge) { SIDE_ROOM } else { SIDE_ROOM.max((longest_tip + 16.0 - self.max_length()) / 2.0) }
    }

    /// 콘텐츠 창이 화면 안쪽으로 차지하는 여유 (세로 독은 가장 커진 아이콘 + 가장 긴 말풍선)
    pub fn headroom(&self, longest_tip: f32) -> f32 {
        if is_vertical(self.edge) { HEADROOM.max(ICON * MAGNIFY + 8.0 + longest_tip + 8.0) } else { HEADROOM }
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
        let (centers, length, separator) = place(&widths, self.split());
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
                    Slot::Shrink => Hit::Shrink,
                    Slot::Grow => Hit::Grow,
                    Slot::Done => Hit::Done,
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
}

/// 칸 너비들을 간격·여백과 함께 늘어놓고, 판 중심이 0이 되도록 옮긴다.
/// `split` 칸 앞에는 구분선 자리를 둔다. 반환: (칸 중심들, 전체 길이, 구분선 위치)
fn place(widths: &[f32], split: Option<usize>) -> (Vec<f32>, f32, Option<f32>) {
    let mut x = PAD;
    let mut centers = Vec::with_capacity(widths.len());
    let mut separator = None;
    for (i, w) in widths.iter().enumerate() {
        if split == Some(i) {
            separator = Some(x + GAP / 2.0);
            x += 2.0 * GAP;
        }
        centers.push(x + w / 2.0);
        x += w + GAP;
    }
    let length = x - GAP + PAD;
    let half = length / 2.0;
    (centers.iter().map(|c| c - half).collect(), length, separator.map(|s| s - half))
}

/// 독 콘텐츠 창 표시 영역을 넓혀 둘지: 마우스가 위에 있거나, 끌기 중이거나, 확대·말풍선이 남아 있으면.
/// 매 프레임 영역을 바꾸면 새 그림과 한 프레임씩 어긋나 말풍선이 잘리므로 두 상태로만 바꾼다.
pub fn region_active(inside: bool, busy: bool, amount: f32, tooltip: f32) -> bool {
    inside || busy || amount > 0.0 || tooltip > 0.0
}

/// 화면 가장자리에서 판까지 띄우는 간격 (DIP)
pub fn edge_margin(_edge: DockEdge) -> f32 {
    MARGIN
}

/// 판 중심 (px). `widest`(가장 커졌을 때 길이)가 작업 영역 안에 들어가는 범위에서 `offset`(0..1)만큼 옮긴다.
/// 0.5면 가운데.
pub fn bar_center(work: PxRect, edge: DockEdge, widest: i32, thick: i32, margin: i32, offset: f32) -> (i32, i32) {
    let (wx, wy, ww, wh) = work;
    let along = |start: i32, len: i32| {
        start + widest / 2 + ((len - widest).max(0) as f32 * offset.clamp(0.0, 1.0)).round() as i32
    };
    match edge {
        DockEdge::Bottom => (along(wx, ww), wy + wh - margin - thick / 2),
        DockEdge::Top => (along(wx, ww), wy + margin + thick / 2),
        DockEdge::Left => (wx + margin + thick / 2, along(wy, wh)),
        DockEdge::Right => (wx + ww - margin - thick / 2, along(wy, wh)),
    }
}

/// 판 중심 `center`(px)에 놓인 길이 `length`의 판 사각형 (px)
pub fn rect_around(center: (i32, i32), edge: DockEdge, length: i32, thick: i32) -> PxRect {
    if is_vertical(edge) {
        (center.0 - thick / 2, center.1 - length / 2, thick, length)
    } else {
        (center.0 - length / 2, center.1 - thick / 2, length, thick)
    }
}

/// 화면 점에서 가장 가까운 작업 영역 가장자리
pub fn nearest_edge(work: PxRect, p: (i32, i32)) -> DockEdge {
    let (wx, wy, ww, wh) = work;
    let d = [
        (p.1 - wy, DockEdge::Top),
        (wy + wh - p.1, DockEdge::Bottom),
        (p.0 - wx, DockEdge::Left),
        (wx + ww - p.0, DockEdge::Right),
    ];
    d.iter().min_by_key(|(dist, _)| *dist).map(|(_, e)| *e).unwrap_or(DockEdge::Bottom)
}

/// 판 중심의 along 좌표(px) → `bar_center`의 offset(0..1)
pub fn offset_for(work: PxRect, edge: DockEdge, widest: i32, center_along: i32) -> f32 {
    let (wx, wy, ww, wh) = work;
    let (start, len) = if is_vertical(edge) { (wy, wh) } else { (wx, ww) };
    let free = (len - widest).max(1) as f32;
    ((center_along - start - widest / 2) as f32 / free).clamp(0.0, 1.0)
}

type PxRect = (i32, i32, i32, i32);

/// 콘텐츠 창의 화면 사각형 (px): 판을 화면 안쪽으로 `headroom`, 양 끝으로 `side`만큼 넓힌다.
pub fn content_rect(bar: PxRect, edge: DockEdge, headroom: i32, side: i32) -> PxRect {
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
    fn rest_layout_is_centered_and_holds_only_tiles() {
        let l = DockLayout::compute(DockEdge::Bottom, 2, false);
        assert_eq!(l.slots(), vec![Slot::Tile(0), Slot::Tile(1)]);
        let expected = PAD * 2.0 + 2.0 * ICON + GAP;
        assert!((l.length - expected).abs() < 0.01);
        assert!((l.rest[0] + l.length / 2.0 - (PAD + ICON / 2.0)).abs() < 0.01);
        let sum: f32 = l.rest.first().unwrap() + l.rest.last().unwrap();
        assert!(sum.abs() < 0.01, "가운데 정렬");
    }

    #[test]
    fn empty_dock_shows_only_add() {
        let l = DockLayout::compute(DockEdge::Bottom, 0, false);
        assert_eq!(l.slots(), vec![Slot::Add]);
        assert_eq!(l.rest.len(), 1);
        let m = l.magnified(None, 0.0);
        assert_eq!(l.hit(&m, l.rest[0], THICK / 2.0), Hit::Add);
    }

    #[test]
    fn room_follows_the_longest_tooltip() {
        let side = DockLayout::compute(DockEdge::Right, 3, false);
        assert_eq!(side.headroom(100.0), HEADROOM);
        let long = side.headroom(600.0);
        assert!(THICK + long >= PAD + ICON * (1.0 + MAGNIFY) + 8.0 + 600.0, "{long}");
        let bottom = DockLayout::compute(DockEdge::Bottom, 1, false);
        assert!(bottom.max_length() + 2.0 * bottom.side_room(700.0) >= 700.0 + 16.0);
        assert_eq!(bottom.side_room(10.0), SIDE_ROOM);
    }

    #[test]
    fn region_grows_on_hover_and_shrinks_only_when_settled() {
        assert!(region_active(true, false, 0.0, 0.0), "마우스가 위에 있으면 활성");
        assert!(region_active(false, false, 0.3, 0.0), "확대가 남아 있으면 활성");
        assert!(region_active(false, false, 0.0, 0.2), "말풍선이 사라지는 중이면 활성");
        assert!(region_active(false, true, 0.0, 0.0), "끌기·옮기기 중이면 활성");
        assert!(!region_active(false, false, 0.0, 0.0), "모두 끝나면 쉼");
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
        let l = DockLayout::compute(DockEdge::Bottom, 5, false);
        let m = l.magnified(Some(l.rest[2]), 1.0);
        assert!((m.scales[2] - (1.0 + MAGNIFY)).abs() < 1e-4);
        assert!(m.scales[1] < m.scales[2] && m.scales[1] > 1.0);
        assert_eq!(m.scales[1], m.scales[3]);
        assert!(m.length > l.length);
        // 커진 만큼 왼쪽 아이콘은 더 왼쪽으로 밀린다
        assert!(m.centers[0] < l.rest[0]);
        assert_eq!(l.magnified(None, 1.0).scales, vec![1.0; 5]);
        assert_eq!(l.magnified(Some(l.rest[2]), 0.0).length, l.length);
        assert!(l.max_length() >= m.length);
    }

    #[test]
    fn hit_uses_magnified_positions() {
        let l = DockLayout::compute(DockEdge::Bottom, 2, false);
        let m = l.magnified(None, 0.0);
        assert_eq!(l.hit(&m, l.rest[0], THICK / 2.0), Hit::Tile(0));
        assert_eq!(l.hit(&m, l.rest[1], THICK / 2.0), Hit::Tile(1));
        assert_eq!(l.hit(&m, l.rest[0], THICK + 5.0), Hit::Empty);
        assert_eq!(l.hit(&m, l.length, THICK / 2.0), Hit::Empty);
    }

    #[test]
    fn drop_index_picks_nearest_tile() {
        let l = DockLayout::compute(DockEdge::Bottom, 3, false);
        assert_eq!(l.drop_index(l.rest[2]), 2);
        assert_eq!(l.drop_index(-1000.0), 0);
        assert_eq!(l.drop_index(1000.0), 2);
        assert_eq!(DockLayout::compute(DockEdge::Bottom, 0, false).drop_index(0.0), 0);
    }

    #[test]
    fn bar_sits_on_each_edge_and_slides_with_offset() {
        let work = (0, 0, 1920, 1040);
        assert_eq!(bar_center(work, DockEdge::Bottom, 400, 68, 8, 0.5), (960, 998));
        assert_eq!(bar_center(work, DockEdge::Top, 400, 68, 8, 0.5), (960, 42));
        assert_eq!(bar_center(work, DockEdge::Left, 400, 68, 8, 0.5), (42, 520));
        assert_eq!(bar_center(work, DockEdge::Right, 400, 68, 8, 0.5), (1878, 520));
        // 끝까지 밀어도 가장 커진 판이 작업 영역 안에 남는다
        assert_eq!(bar_center(work, DockEdge::Bottom, 400, 68, 8, 0.0).0, 200);
        assert_eq!(bar_center(work, DockEdge::Bottom, 400, 68, 8, 1.0).0, 1720);
        assert_eq!(rect_around((960, 998), DockEdge::Bottom, 400, 68), (760, 964, 400, 68));
        assert_eq!(rect_around((42, 520), DockEdge::Left, 400, 68), (8, 320, 68, 400));
        assert_eq!(edge_margin(DockEdge::Bottom), MARGIN);
    }

    #[test]
    fn dragging_picks_nearest_edge_and_offset() {
        let work = (0, 0, 1920, 1040);
        assert_eq!(nearest_edge(work, (960, 1000)), DockEdge::Bottom);
        assert_eq!(nearest_edge(work, (960, 30)), DockEdge::Top);
        assert_eq!(nearest_edge(work, (20, 500)), DockEdge::Left);
        assert_eq!(nearest_edge(work, (1900, 500)), DockEdge::Right);
        let o = offset_for(work, DockEdge::Bottom, 400, 960);
        assert!((o - 0.5).abs() < 0.01);
        assert_eq!(offset_for(work, DockEdge::Bottom, 400, -500), 0.0);
        assert_eq!(offset_for(work, DockEdge::Bottom, 400, 5000), 1.0);
        // offset → 중심 → offset 왕복
        let c = bar_center(work, DockEdge::Left, 300, 68, 8, 0.25);
        assert!((offset_for(work, DockEdge::Left, 300, c.1) - 0.25).abs() < 0.01);
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
    fn editing_adds_size_and_done_buttons_after_a_separator() {
        let l = DockLayout::compute(DockEdge::Bottom, 2, true);
        assert_eq!(l.slots(), vec![Slot::Tile(0), Slot::Tile(1), Slot::Shrink, Slot::Grow, Slot::Done]);
        let sep = l.separator.unwrap();
        assert!(sep > l.rest[1] && sep < l.rest[2]);
        let m = l.magnified(None, 0.0);
        assert_eq!(l.hit(&m, l.rest[2], THICK / 2.0), Hit::Shrink);
        assert_eq!(l.hit(&m, l.rest[3], THICK / 2.0), Hit::Grow);
        assert_eq!(l.hit(&m, l.rest[4], THICK / 2.0), Hit::Done);
        assert!(DockLayout::compute(DockEdge::Bottom, 2, false).separator.is_none());
        assert_eq!(step_size(48, 1), 56);
        assert_eq!(step_size(MAX_SIZE, 1), MAX_SIZE);
        assert_eq!(step_size(MIN_SIZE, -1), MIN_SIZE);
        assert!((zoom_for(DEFAULT_SIZE) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn icons_grow_away_from_the_screen_edge() {
        let center = (300.0, 200.0);
        let bottom = DockLayout::compute(DockEdge::Bottom, 1, false);
        let m = bottom.magnified(Some(bottom.rest[0]), 1.0);
        let bar = bottom.bar_rect(&m, center);
        let r = bottom.slot_rect(&m, center, 0);
        assert!((r.bottom() - (bar.bottom() - PAD)).abs() < 0.01, "아래 면 고정");
        assert!(r.y < bar.y + PAD, "위로 커짐");
        let left = DockLayout::compute(DockEdge::Left, 1, false);
        let ml = left.magnified(Some(left.rest[0]), 1.0);
        let lr = left.slot_rect(&ml, center, 0);
        assert!((lr.x - (left.bar_rect(&ml, center).x + PAD)).abs() < 0.01, "왼쪽 면 고정");
        assert!(left.bar_rect(&ml, center).h > left.bar_rect(&ml, center).w, "세로 판");
    }
}
