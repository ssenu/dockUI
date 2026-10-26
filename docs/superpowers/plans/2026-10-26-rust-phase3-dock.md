# Rust 독 런처 — 3단계(독 모드) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 맥 독 같은 독 모드를 더한다. 화면 가장자리(아래·위·왼쪽·오른쪽) 가운데에 판이 놓이고, 마우스를 올리면 아이콘이 물결처럼 커지며 이웃을 밀어낸다. 메뉴에서 독/패널 모드와 독 위치를 바꾼다. 흐림 배경이 실제로 보이도록 고친다.

**Architecture:** 독 배치·확대는 순수 모듈 `ui::dock_layout`(판 중심 기준 along 좌표)으로 계산·테스트한다. `ui::scene::draw_dock`이 독 한 프레임을 그린다. `ui::panel`이 모드를 갖고, 독 모드에서는 유리 창을 확대된 판 크기로 매 프레임 옮기고, 더 큰 콘텐츠 창은 `SetWindowRgn`으로 지금 그려지는 부분만 창 영역으로 둔다(나머지 클릭은 뒤 창으로).

**Tech Stack:** Rust 1.99, `windows` 0.62 (Direct2D, DirectComposition, DWM, `SetWindowCompositionAttribute`).

**Spec:** `docs/superpowers/specs/2026-10-26-dock-launcher-rust-design.md` (4.2 독 모드, 4.5 테마, 4.6 메뉴, 10장의 3단계)

**검증 메모 (프로토타입에서 확인):**
- `cargo test` 70개 통과, clippy 통과. 독(아래·왼쪽) PNG 스냅샷과 실제 화면에서 아래·왼쪽 독, 패널을 확인.
- **흐림 배경 버그 발견·수정:** 2단계의 `DWMWA_SYSTEMBACKDROP_TYPE`는 테두리 없는 팝업 창에 그려지지 않았고, 유리 창을 칠하지 않아 흐림이 비칠 곳도 없었다. 6가지 조합을 실험해, 셸 창이 쓰는 `SetWindowCompositionAttribute`(ACCENT_ENABLE_ACRYLICBLURBEHIND) + 유리 창 검은 칠이 실제로 흐림을 보여 주는 것을 확인했다(Task 3).
- 독 모드 실행 중 작업 집합 약 39MB.

## Global Constraints

- 작업 브랜치 `rust-rewrite`. 파이썬 코드는 건드리지 않는다.
- 모든 명령은 저장소 루트에서 PowerShell. 각 태스크 끝에 `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test` 통과 후 커밋. 복사로 바꾼 파일은 수정 시각을 갱신한다.
- 독: 아이콘 48, 간격 10, 판 안쪽 여백 10, 두께 68, 화면 가장자리에서 8 (DIP). 확대 `1 + 0.6·cos(π/2·d/R)`, R = 2.5칸(=145), 매 프레임 목표로 25% 접근. 판은 가운데 기준으로 늘어난다.
- 끝부분 순서: 항목들 → 구분선 → ＋ → 팔레트 → ⋯. 독에는 헤더가 없다(제목은 메뉴 첫 줄).
- 독은 주 모니터 작업 영역 기준. 해상도·작업표시줄 변경(`WM_DISPLAYCHANGE`, `WM_SETTINGCHANGE`) 시 다시 배치.
- 콘텐츠 창 여유: 화면 안쪽 110, 양 끝 90 (DIP). 선택기는 판에서 화면 안쪽으로 펼친다.
- 새 설치 기본 모드 독, 0.1.x에서 넘어온 설정은 패널.
- UI 문구·주석 한국어.

## File Structure

```
src/ui/dock_layout.rs (신규) 독 배치·확대·클릭 판정·판/콘텐츠 사각형·선택기 각도
src/ui/layout.rs      (수정) fan_points (패널·독 공용 부채꼴)
src/ui/scene.rs       (수정) draw_swatches 분리, DockScene·draw_dock·dock_swatch_centers, 독 스냅샷 테스트
src/win/dwm.rs        (수정) SetWindowCompositionAttribute 아크릴 흐림 우선
src/ui/panel.rs       (수정) 유리 창 검은 칠 → 모드(독/패널), OpenSpec, SetMode, 창 영역, 화면 변경 대응
src/app.rs            (수정) OpenSpec으로 열기, 독/패널 모드·독 위치 메뉴, set_mode
```

---
### Task 1: 독 배치와 확대 물결 (dock_layout.rs)

**Files:**
- Create: `src/ui/dock_layout.rs`
- Modify: `src/ui/layout.rs`, `src/ui/mod.rs`

**Interfaces:**
- Consumes: `config::DockEdge`, `layout::{Hit, RectF}`
- Produces: `dock_layout::{ICON, GAP, PAD, THICK, MARGIN, MAGNIFY, RANGE, SEPARATOR, HEADROOM, SIDE_ROOM, Slot{Tile,Add,Palette,Menu}, Magnified{centers,scales,length,separator}, DockLayout::compute(edge,tiles)+slots/magnified(pointer,amount)/max_length/hit(m,along,across)/drop_index(along)/bar_rect(m,center)/slot_rect(m,center,i)/picker_angles, is_vertical, falloff, bar_rect(work,edge,len,thick,margin), content_rect(bar,edge,headroom,side)}`, `layout::fan_points(anchor, (start,end), count, radius)` (PanelLayout::swatch_centers가 이것을 쓴다).

- [ ] **Step 1: src/ui/dock_layout.rs 작성**

```rust
//! 독 모드 배치 (순수 함수): 아이콘 칸 위치, 맥 독식 확대 물결, 클릭 판정, 화면 가장자리 배치.
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
/// 마우스 바로 아래 아이콘이 커지는 정도 (1.6배)
pub const MAGNIFY: f32 = 0.6;
/// 확대가 퍼지는 거리 = 아이콘 칸 2.5개
pub const RANGE: f32 = 2.5 * (ICON + GAP);
/// 구분선 칸 너비 (가운데에 1px 선)
pub const SEPARATOR: f32 = GAP;
/// 콘텐츠 창이 판 바깥(화면 안쪽)으로 더 차지하는 여유: 확대 + 말풍선 + 테마 선택기
pub const HEADROOM: f32 = 110.0;
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

    /// 테마 선택기 부채꼴 각도 범위 (도). 판에서 화면 안쪽으로 펼친다.
    pub fn picker_angles(&self) -> (f32, f32) {
        match self.edge {
            DockEdge::Bottom => (-30.0, -150.0),
            DockEdge::Top => (30.0, 150.0),
            DockEdge::Left => (-60.0, 60.0),
            DockEdge::Right => (-120.0, -240.0),
        }
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
    fn picker_fans_away_from_edge() {
        let up = DockLayout::compute(DockEdge::Bottom, 0).picker_angles();
        assert!(up.0.to_radians().sin() < 0.0 && up.1.to_radians().sin() < 0.0);
        let down = DockLayout::compute(DockEdge::Top, 0).picker_angles();
        assert!(down.0.to_radians().sin() > 0.0);
        let right = DockLayout::compute(DockEdge::Left, 0).picker_angles();
        assert!(right.0.to_radians().cos() > 0.0 && right.1.to_radians().cos() > 0.0);
        let left = DockLayout::compute(DockEdge::Right, 0).picker_angles();
        assert!(left.0.to_radians().cos() < 0.0 && left.1.to_radians().cos() < 0.0);
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
```

- [ ] **Step 2: src/ui/layout.rs 작성**

```rust
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
        fan_points(self.palette_button.center(), (-90.0, -180.0), count, PICKER_RADIUS * progress)
    }
}

/// `anchor`를 중심으로 반지름 `radius`인 원호 위에 `count`개 점을 각도 범위(도)에 고르게 놓는다.
/// Qt·Win32 좌표처럼 y가 아래로 커지므로 -90°가 위쪽이다.
pub fn fan_points(anchor: Pt, angles: (f32, f32), count: usize, radius: f32) -> Vec<Pt> {
    let (start, end) = angles;
    let step = if count > 1 { (end - start) / (count - 1) as f32 } else { 0.0 };
    (0..count)
        .map(|i| {
            let a = (start + step * i as f32).to_radians();
            Pt { x: anchor.x + radius * a.cos(), y: anchor.y + radius * a.sin() }
        })
        .collect()
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
```

- [ ] **Step 3: src/ui/mod.rs 작성**

```rust
//! 화면: 테마, 배치, 애니메이션, 그리기 장치, 장면, 패널 창, 입력 창.

pub mod anim;
pub mod dialogs;
#[allow(dead_code)] // 독 모드 화면(Task 4)에서 연결하면 지운다
pub mod dock_layout;
pub mod gfx;
pub mod layout;
pub mod panel;
pub mod scene;
pub mod theme;
```

- [ ] **Step 4: 테스트**

Run: `cargo test dock_layout`
Expected: 10 passed

- [ ] **Step 5: 전체 검사**

Run: `cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test`
Expected: clippy 오류 없음, `test result: ok`

- [ ] **Step 6: Commit**

```powershell
git add src/ui
git commit -m "feat: 독 배치·맥 독식 확대 물결·클릭 판정"
```

---

### Task 2: 독 그리기 (scene.rs)

**Files:**
- Modify: `src/ui/scene.rs`, `src/ui/mod.rs` (scene에 임시 allow)

**Interfaces:**
- Consumes: `dock_layout`, `layout::{fan_points, PICKER_RADIUS}`
- Produces: `DockScene{layout,mag,center,size,palette,acrylic,tiles,pressed,tooltip,menu_hover,palette_hover,drag,picker,picker_hover,theme}`, `draw_dock(&Gfx,&DockScene)`, `dock_swatch_centers(layout,mag,center,progress)->Vec<Pt>`. 패널·독 공용 `draw_swatches`.

- [ ] **Step 1: src/ui/scene.rs 작성**

```rust
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
```

- [ ] **Step 2: src/ui/mod.rs 작성**

```rust
//! 화면: 테마, 배치, 애니메이션, 그리기 장치, 장면, 패널 창, 입력 창.

pub mod anim;
pub mod dialogs;
#[allow(dead_code)] // 독 모드 화면(Task 4)에서 연결하면 지운다
pub mod dock_layout;
pub mod gfx;
pub mod layout;
pub mod panel;
#[allow(dead_code)] // 독 모드 화면(Task 4)에서 연결하면 지운다
pub mod scene;
pub mod theme;
```

- [ ] **Step 3: 테스트와 눈으로 확인**

Run: `cargo test scene`
Expected: 4 passed. `target\snapshots\dock-bottom.png`(두 번째 아이콘 확대·이웃 밀림·말풍선 위쪽), `dock-left.png`(세로 독, 말풍선 오른쪽, 선택기가 오른쪽으로 펼쳐짐)를 연다.

- [ ] **Step 4: 전체 검사**

Run: `cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test`
Expected: clippy 오류 없음, `test result: ok`

- [ ] **Step 5: Commit**

```powershell
git add src/ui
git commit -m "feat: 독 장면 그리기와 스냅샷 테스트"
```

---

### Task 3: 흐림 배경이 실제로 보이게 (dwm.rs, 유리 창 검은 칠)

**Files:**
- Modify: `src/win/dwm.rs`, `src/ui/panel.rs` (WM_PAINT만)

**Interfaces:**
- Produces: `dwm::enable_acrylic`가 `SetWindowCompositionAttribute` 아크릴 흐림을 먼저 쓰고, 없을 때만 시스템 배경을 시도. 유리 창 WM_PAINT가 검게 칠한다.

- [ ] **Step 1: src/win/dwm.rs 작성**

```rust
//! DWM(데스크톱 창 관리자) 효과: 흐림(아크릴) 배경, 어두운 톤, 둥근 모서리.
//!
//! 테두리 없는 팝업 창에는 Windows 11의 시스템 배경(`DWMWA_SYSTEMBACKDROP_TYPE`)이 그려지지 않는다.
//! 그래서 작업표시줄·시작 메뉴 같은 셸 창이 쓰는 `SetWindowCompositionAttribute`의 아크릴 흐림을 먼저 쓰고,
//! 그것이 없을 때만 시스템 배경을 시도한다.

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DWM_SYSTEMBACKDROP_TYPE, DWM_WINDOW_CORNER_PREFERENCE, DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE,
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmExtendFrameIntoClientArea,
    DwmSetWindowAttribute,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::UI::Controls::MARGINS;
use windows::core::{BOOL, s, w};

fn set_attr<T>(hwnd: HWND, attr: windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE, value: &T) -> bool {
    // SAFETY: value의 크기를 함께 넘긴다
    unsafe { DwmSetWindowAttribute(hwnd, attr, (value as *const T).cast(), std::mem::size_of::<T>() as u32) }.is_ok()
}

/// 블러 배경의 밝기 톤 (어두운 테마면 true)
pub fn set_dark(hwnd: HWND, dark: bool) -> bool {
    let value: i32 = dark.into();
    set_attr(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &value)
}

#[repr(C)]
struct AccentPolicy {
    state: u32,
    flags: u32,
    /// ABGR. 알파가 0이면 흐림만, 덧칠은 우리가 그린다.
    gradient_color: u32,
    animation_id: u32,
}

#[repr(C)]
struct CompositionAttribData {
    attribute: u32,
    data: *mut core::ffi::c_void,
    size: usize,
}

const ACCENT_ENABLE_ACRYLICBLURBEHIND: u32 = 4;
const WCA_ACCENT_POLICY: u32 = 19;
type SetWindowCompositionAttribute = unsafe extern "system" fn(HWND, *mut CompositionAttribData) -> BOOL;

/// 셸 창과 같은 아크릴 흐림을 켠다. 함수를 찾지 못하면 false.
fn enable_accent_blur(hwnd: HWND) -> bool {
    // SAFETY: user32의 내보낸 함수를 이름으로 찾아 정해진 구조체로 호출한다
    unsafe {
        let Ok(user32) = GetModuleHandleW(w!("user32.dll")) else { return false };
        let Some(proc) = GetProcAddress(user32, s!("SetWindowCompositionAttribute")) else { return false };
        let set: SetWindowCompositionAttribute = std::mem::transmute(proc);
        let mut policy = AccentPolicy {
            state: ACCENT_ENABLE_ACRYLICBLURBEHIND,
            flags: 0,
            gradient_color: 0x0100_0000,
            animation_id: 0,
        };
        let mut data = CompositionAttribData {
            attribute: WCA_ACCENT_POLICY,
            data: (&mut policy as *mut AccentPolicy).cast(),
            size: std::mem::size_of::<AccentPolicy>(),
        };
        set(hwnd, &mut data).as_bool()
    }
}

/// 흐림 배경을 켠다. 아무 방법도 통하지 않으면(Windows 10 일부 등) false → 덧칠을 거의 불투명하게 그린다.
pub fn enable_acrylic(hwnd: HWND, dark: bool) -> bool {
    set_dark(hwnd, dark);
    let corner: DWM_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND;
    set_attr(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &corner);
    if enable_accent_blur(hwnd) {
        return true;
    }
    let margins = MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 };
    // SAFETY: 창 전체를 프레임 영역으로 넓혀 시스템 배경이 보이게 한다
    let _ = unsafe { DwmExtendFrameIntoClientArea(hwnd, &margins) };
    let backdrop: DWM_SYSTEMBACKDROP_TYPE = DWMSBT_TRANSIENTWINDOW;
    set_attr(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, &backdrop)
}
```

- [ ] **Step 2: src/ui/panel.rs 작성**

```rust
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
use windows::Win32::Graphics::Gdi::{
    BLACK_BRUSH, BeginPaint, EndPaint, FillRect, GetStockObject, HBRUSH, PAINTSTRUCT, ScreenToClient,
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
```

- [ ] **Step 3: 빌드·테스트**

Run: `cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test`
Expected: clippy 오류 없음, `test result: ok`

- [ ] **Step 4: 실제 화면 확인 (수동)**

릴리스 빌드를 임시 APPDATA로 실행해 바탕화면 위에서 판 뒤 배경이 흐리게 비치는지 본다. 화면을 볼 수 없으면 사용자 확인 항목으로 넘긴다.

- [ ] **Step 5: Commit**

```powershell
git add src/win/dwm.rs src/ui/panel.rs
git commit -m "fix: 테두리 없는 창에서 흐림 배경이 보이도록 아크릴 방식 교체"
```

---

### Task 4: 독 모드 창과 모드 전환 (panel.rs, app.rs)

**Files:**
- Modify: `src/ui/panel.rs`, `src/app.rs`, `src/ui/mod.rs` (Task 1·2의 임시 allow 제거)

**Interfaces:**
- Consumes: Task 1~3
- Produces: `panel::{OpenSpec{mode,edge,panel_rect,theme,locked,tiles}, Panel::open(OpenSpec, notify), PanelCmd::SetMode(Mode, DockEdge)}`. UiEvent는 그대로. 메뉴: 독 모드/패널 모드(체크), 독 모드일 때 독 위치 4개(체크), 패널 모드일 때 위치·크기 초기화·위치 잠금.
- Behavior: 독 모드에서 유리 창 = 확대된 판(매 프레임), 콘텐츠 창 = 가장 넓은 판 + 여유, 창 영역 = 판 + 커진 아이콘·말풍선 쪽(선택기가 열리면 콘텐츠 전체). 마우스가 판을 벗어나면 확대가 0으로 돌아간다. 독은 이동·크기 조절 없음, 위치 저장 없음.

- [ ] **Step 1: src/ui/panel.rs 작성**

```rust
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
use crate::ui::scene::{self, DockScene, DragView, SceneState, TileView, dock_swatch_centers};
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
            (dock_layout::HEADROOM * s).round() as i32,
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
        if self.picker_open {
            let (w, h) = (self.dock.content_px.2, self.dock.content_px.3);
            rects.push((0, 0, w, h));
        }
        if let Some(region) = region_from(&rects) {
            let _ = unsafe { SetWindowRgn(self.content, Some(region), true) };
        }
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
                    size: (d.content_px.2 as f32 / self.scale, d.content_px.3 as f32 / self.scale),
                    palette: self.palette(),
                    acrylic: self.acrylic,
                    tiles: &self.tiles,
                    pressed,
                    tooltip: self.dock_tooltip(),
                    menu_hover: self.hover_hit == Hit::Menu,
                    palette_hover: self.hover_hit == Hit::Palette,
                    drag: self.drag,
                    picker,
                    picker_hover: self.picker_hover,
                    theme: self.theme,
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

    fn swatch_centers(&self) -> Vec<Pt> {
        let progress = self.picker.progress();
        match self.mode {
            Mode::Panel => self.layout.swatch_centers(PICKER_ORDER.len(), progress),
            Mode::Dock => {
                let m = self.dock_mag();
                dock_swatch_centers(&self.dock.layout, &m, self.dock_center(), progress)
            }
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
                self.tiles = tiles;
                self.hover = vec![Anim::at(0.0); self.tiles.len() + 1];
                self.hover_hit = Hit::Empty;
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
```

- [ ] **Step 2: src/app.rs 작성**

```rust
//! 앱 컨트롤러: 설정·트레이·패널 화면·항목 실행·업데이트·자동 실행을 연결한다.

use crate::autostart::{AutoStart, RegistryRun};
use crate::chrome;
use crate::config::{Config, DockEdge, LoadReport, Mode, Storage, Theme};
use crate::items::Item;
use crate::logging;
use crate::tray::{Tray, WM_APP_TRAY};
use crate::ui::dialogs::{YoutubeInput, youtube_dialog};
use crate::ui::panel::{OpenSpec, Panel, PanelCmd, PanelHandle, UiEvent, initial_rect};
use crate::ui::scene::{TileKind, TileView, badge_of};
use crate::updater::{self, Release, UreqHttp};
use crate::win::instance::WM_APP_SHOW;
use crate::win::menu::{MenuEntry, show_popup};
use crate::win::msg_window::Handler;
use crate::win::shell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, IDYES, KillTimer, MB_ICONQUESTION, MB_YESNO, MessageBoxW, PostMessageW, PostQuitMessage,
    RegisterWindowMessageW, SetTimer, WM_APP, WM_CONTEXTMENU, WM_DESTROY, WM_LBUTTONUP, WM_RBUTTONUP, WM_TIMER,
};
use windows::core::{HSTRING, w};

pub const APP_NAME: &str = "YT Launcher";
pub const BYLINE: &str = "by_ssenu";
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const RELEASES_URL: &str = "https://github.com/ssenu/YT-launcher/releases/latest";

/// 백그라운드 작업 결과가 채널에 들어왔음을 알리는 메시지
pub const WM_APP_EVENT: u32 = WM_APP + 3;

const TIMER_FIRST_CHECK: usize = 1;
const TIMER_DAILY_CHECK: usize = 2;
const FIRST_CHECK_MS: u32 = 30_000;
const DAILY_CHECK_MS: u32 = 24 * 60 * 60 * 1000;
const CLICK_DEBOUNCE: Duration = Duration::from_secs(1);

const CMD_SHOW: u32 = 20;
const CMD_MODE_DOCK: u32 = 30;
const CMD_MODE_PANEL: u32 = 31;
const CMD_EDGE_BOTTOM: u32 = 32;
const CMD_EDGE_TOP: u32 = 33;
const CMD_EDGE_LEFT: u32 = 34;
const CMD_EDGE_RIGHT: u32 = 35;
const CMD_RESET_GEOMETRY: u32 = 21;
const CMD_LOCK: u32 = 22;
const CMD_AUTOSTART: u32 = 10;
const CMD_CHECK_UPDATE: u32 = 11;
const CMD_INSTALL_UPDATE: u32 = 12;
const CMD_RELEASES: u32 = 13;
const CMD_QUIT: u32 = 14;

pub enum Event {
    Checked { manual: bool, result: Result<Option<Release>, String> },
    Downloaded(Result<PathBuf, String>),
    Ui(UiEvent),
    Opened(chrome::Opened),
}

/// 업데이트 확인 결과를 보고 무엇을 할지 정한다 (순수 함수).
#[derive(Debug, PartialEq, Eq)]
pub enum UpdateDecision {
    /// 새 버전 발견. `notify`면 이번이 이 버전의 첫 알림
    Available {
        version: String,
        notify: bool,
    },
    UpToDate,
    Failed,
}

pub fn decide_update(result: &Result<Option<Release>, String>, current: &str, notified: &str) -> UpdateDecision {
    match result {
        Err(_) => UpdateDecision::Failed,
        Ok(Some(r)) if r.is_installable() && updater::is_newer(&r.version, current) => {
            UpdateDecision::Available { version: r.version.clone(), notify: r.version != notified }
        }
        Ok(_) => UpdateDecision::UpToDate,
    }
}

/// 시작할 때 띄울 알림 문구 (설정 손상 복구, 업데이트 완료). 순수 함수.
pub fn startup_notices(report: LoadReport, last_run: &str, current: &str) -> Vec<String> {
    let mut notices = Vec::new();
    if report.recovered {
        notices.push("설정 파일이 손상되어 초기화했습니다. 이전 파일은 config.json.bak에 있습니다.".to_string());
    }
    if !last_run.is_empty() && updater::is_newer(current, last_run) {
        notices.push(format!("v{current} 버전으로 업데이트되었습니다."));
    }
    notices
}

/// 설정의 항목 → 화면에 그릴 타일 (순수 함수)
pub fn tile_views(items: &[Item]) -> Vec<TileView> {
    items
        .iter()
        .map(|item| {
            let label = item.display_name();
            let (kind, badge) = match item {
                Item::Youtube { query, .. } => (TileKind::Youtube, badge_of(query)),
                Item::App { .. } => (TileKind::App, label.chars().take(1).collect()),
                Item::Web { .. } => (TileKind::Web, label.chars().take(1).collect::<String>().to_uppercase()),
            };
            TileView { id: item.id().to_string(), label, badge, kind }
        })
        .collect()
}

/// 화면에서 바뀐 순서(id 목록)대로 항목을 다시 늘어놓는다. 목록에 없는 항목은 뒤에 붙인다.
pub fn apply_order(items: &[Item], ids: &[String]) -> Vec<Item> {
    let mut rest: Vec<Item> = items.to_vec();
    let mut ordered = Vec::with_capacity(items.len());
    for id in ids {
        if let Some(pos) = rest.iter().position(|i| i.id() == id) {
            ordered.push(rest.remove(pos));
        }
    }
    ordered.extend(rest);
    ordered
}

/// 같은 항목을 짧은 시간 안에 두 번 누르면 무시한다.
#[derive(Default)]
pub struct Debounce {
    last: HashMap<String, Instant>,
}

impl Debounce {
    pub fn allow(&mut self, id: &str, now: Instant) -> bool {
        match self.last.get(id) {
            Some(t) if now.duration_since(*t) < CLICK_DEBOUNCE => false,
            _ => {
                self.last.insert(id.to_string(), now);
                true
            }
        }
    }
}

pub struct App {
    hwnd: HWND,
    storage: Storage,
    config: Config,
    tray: Option<Tray>,
    panel: Option<PanelHandle>,
    autostart: AutoStart<RegistryRun>,
    release: Option<Release>,
    downloading: bool,
    debounce: Debounce,
    events_tx: Sender<Event>,
    events_rx: Receiver<Event>,
    taskbar_created: u32,
    pending_notices: Vec<String>,
}

impl App {
    pub fn new(storage: Storage) -> Self {
        let (config, report) = storage.load();
        let (events_tx, events_rx) = channel();
        let pending_notices = startup_notices(report, &config.last_run_version, CURRENT_VERSION);
        if report.migrated {
            logging::info("0.1.x 설정을 schema 2로 변환했습니다 (원본: config.v1.json)");
        }
        App {
            hwnd: HWND::default(),
            storage,
            config,
            tray: None,
            panel: None,
            autostart: AutoStart { backend: RegistryRun::default() },
            release: None,
            downloading: false,
            debounce: Debounce::default(),
            events_tx,
            events_rx,
            taskbar_created: unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) },
            pending_notices,
        }
    }

    fn save(&self) {
        if let Err(e) = self.storage.save(&self.config) {
            logging::error(&format!("설정 저장 실패: {e}"));
        }
    }

    fn notify(&self, message: &str) {
        if let Some(tray) = &self.tray {
            tray.notify(APP_NAME, message);
        }
    }

    fn panel_send(&self, cmd: PanelCmd) {
        if let Some(panel) = &self.panel {
            panel.send(cmd);
        }
    }

    fn refresh_tiles(&self) {
        self.panel_send(PanelCmd::SetTiles(tile_views(&self.config.items)));
    }

    /// 창이 만들어진 직후 한 번 호출된다.
    fn start(&mut self, hwnd: HWND) {
        self.hwnd = hwnd;
        let tray = Tray::new(hwnd, APP_NAME);
        if !tray.add() {
            logging::warn("트레이 아이콘 추가 실패");
        }
        self.tray = Some(tray);
        // 업데이트 설치가 Run 키를 되살려도 사용자가 고른 값을 지킨다 (개발 빌드에서는 건드리지 않음)
        if !cfg!(debug_assertions)
            && let Some(enabled) = self.config.autostart
            && let Err(e) = self.autostart.set_enabled(enabled)
        {
            logging::error(&format!("자동 실행 설정 실패: {e}"));
        }
        self.open_panel();
        for notice in std::mem::take(&mut self.pending_notices) {
            self.notify(&notice);
        }
        if self.config.last_run_version != CURRENT_VERSION {
            self.config.last_run_version = CURRENT_VERSION.to_string();
            self.save();
        }
        unsafe { SetTimer(Some(hwnd), TIMER_FIRST_CHECK, FIRST_CHECK_MS, None) };
    }

    fn open_panel(&mut self) {
        let send = self.post_event();
        let notify = Box::new(move |e: UiEvent| send(Event::Ui(e)));
        let spec = OpenSpec {
            mode: self.config.mode,
            edge: self.config.dock_edge,
            panel_rect: initial_rect(self.config.panel_window),
            theme: self.config.theme,
            locked: self.config.locked,
            tiles: tile_views(&self.config.items),
        };
        match Panel::open(spec, notify) {
            Ok(handle) => self.panel = Some(handle),
            Err(e) => logging::error(&format!("패널 창을 만들지 못했습니다: {e}")),
        }
    }

    fn post_event(&self) -> impl Fn(Event) + Send + 'static {
        let tx = self.events_tx.clone();
        let hwnd = self.hwnd.0 as isize;
        move |event| {
            if tx.send(event).is_ok() {
                let _ = unsafe { PostMessageW(Some(HWND(hwnd as *mut _)), WM_APP_EVENT, WPARAM(0), LPARAM(0)) };
            }
        }
    }

    fn dialog_owner(&self) -> HWND {
        self.panel.as_ref().map(PanelHandle::owner).unwrap_or(self.hwnd)
    }

    fn on_ui(&mut self, event: UiEvent) {
        match event {
            UiEvent::TileClicked(id) => self.open_item(&id),
            UiEvent::AddRequested => {
                if let Some(input) = youtube_dialog(self.dialog_owner(), "항목 추가", &YoutubeInput::default())
                    && let Ok(item) = Item::youtube(&input.query, &input.name)
                {
                    self.config.items.push(item);
                    self.save();
                    self.refresh_tiles();
                }
            }
            UiEvent::EditRequested(id) => self.edit_item(&id),
            UiEvent::DeleteRequested(id) => self.delete_item(&id),
            UiEvent::Reordered(ids) => {
                self.config.items = apply_order(&self.config.items, &ids);
                self.save();
            }
            UiEvent::ThemePicked(theme) => self.set_theme(theme),
            UiEvent::GeometryChanged(rect) => {
                self.config.panel_window = Some(rect);
                self.save();
            }
            UiEvent::MenuRequested => self.show_menu(),
        }
    }

    fn edit_item(&mut self, id: &str) {
        let Some(index) = self.config.items.iter().position(|i| i.id() == id) else { return };
        if let Item::Youtube { query, name, .. } = &self.config.items[index] {
            let initial = YoutubeInput { query: query.clone(), name: name.clone() };
            if let Some(input) = youtube_dialog(self.dialog_owner(), "항목 수정", &initial) {
                self.config.items[index] = Item::Youtube { id: id.to_string(), query: input.query, name: input.name };
                self.save();
                self.refresh_tiles();
            }
        }
    }

    fn delete_item(&mut self, id: &str) {
        let Some(item) = self.config.items.iter().find(|i| i.id() == id) else { return };
        let text = HSTRING::from(format!("'{}' 항목을 삭제할까요?", item.display_name()));
        let answer = unsafe {
            MessageBoxW(Some(self.dialog_owner()), &text, &HSTRING::from(APP_NAME), MB_YESNO | MB_ICONQUESTION)
        };
        if answer == IDYES {
            self.config.items.retain(|i| i.id() != id);
            self.save();
            self.refresh_tiles();
        }
    }

    fn open_item(&mut self, id: &str) {
        if !self.debounce.allow(id, Instant::now()) {
            return;
        }
        let Some(item) = self.config.items.iter().find(|i| i.id() == id).cloned() else { return };
        let send = self.post_event();
        std::thread::spawn(move || {
            let result = match &item {
                Item::Youtube { query, .. } => chrome::open_url(&chrome::build_search_url(query), Some("YouTube")),
                Item::Web { url, .. } => chrome::open_url(url, None),
                Item::App { .. } => return, // 4단계에서 연결
            };
            match result {
                Ok(opened) => send(Event::Opened(opened)),
                Err(e) => logging::error(&format!("열기 실패: {e}")),
            }
        });
    }

    fn set_mode(&mut self, mode: Mode, edge: DockEdge) {
        self.config.mode = mode;
        self.config.dock_edge = edge;
        self.save();
        self.panel_send(PanelCmd::SetMode(mode, edge));
    }

    fn set_theme(&mut self, theme: Theme) {
        self.config.theme = theme;
        self.save();
        self.panel_send(PanelCmd::SetTheme(theme));
    }

    fn check_updates(&self, manual: bool) {
        let send = self.post_event();
        std::thread::spawn(move || {
            let result = updater::fetch_latest(&UreqHttp::new(), updater::REPO).map_err(|e| e.to_string());
            if let Err(e) = &result {
                logging::warn(&format!("업데이트 확인 실패: {e}"));
            }
            send(Event::Checked { manual, result });
        });
    }

    fn on_checked(&mut self, manual: bool, result: Result<Option<Release>, String>) {
        match decide_update(&result, CURRENT_VERSION, &self.config.notified_version) {
            UpdateDecision::Available { version, notify } => {
                self.release = result.ok().flatten();
                if notify {
                    self.config.notified_version = version.clone();
                    self.save();
                }
                if notify || manual {
                    self.notify(&format!("새 버전(v{version})이 나왔습니다. 트레이 메뉴에서 설치하세요."));
                }
            }
            UpdateDecision::UpToDate if manual => self.notify("최신 버전입니다."),
            UpdateDecision::Failed if manual => {
                self.notify("업데이트를 확인하지 못했습니다. 인터넷 연결을 확인하세요.")
            }
            _ => {}
        }
    }

    fn install_update(&mut self) {
        let Some(release) = self.release.clone() else { return };
        if self.downloading {
            return;
        }
        let text =
            HSTRING::from(format!("v{} 버전으로 업데이트할까요?\n런처가 잠시 꺼졌다 켜집니다.", release.version));
        let answer =
            unsafe { MessageBoxW(Some(self.hwnd), &text, &HSTRING::from(APP_NAME), MB_YESNO | MB_ICONQUESTION) };
        if answer != IDYES {
            return;
        }
        self.downloading = true;
        self.notify("업데이트를 내려받는 중입니다…");
        let send = self.post_event();
        std::thread::spawn(move || {
            let dir = std::env::temp_dir().join("YT-Launcher-update");
            let _ = std::fs::remove_dir_all(&dir);
            let result = updater::download_and_verify(&UreqHttp::new(), &release, &dir, &mut |_, _| {})
                .map_err(|e| e.to_string());
            send(Event::Downloaded(result));
        });
    }

    fn on_downloaded(&mut self, result: Result<PathBuf, String>) {
        let launched = result.and_then(|path| updater::launch_installer(&path).map_err(|e| e.to_string()));
        match launched {
            Ok(()) => self.quit(),
            Err(e) => {
                logging::error(&format!("업데이트 실패: {e}"));
                self.downloading = false;
                self.notify(&format!("업데이트에 실패했습니다: {e}"));
            }
        }
    }

    fn set_autostart(&mut self, enabled: bool) {
        self.config.autostart = Some(enabled);
        self.save();
        if let Err(e) = self.autostart.set_enabled(enabled) {
            logging::error(&format!("자동 실행 설정 실패: {e}"));
        }
    }

    fn menu_entries(&self) -> Vec<MenuEntry> {
        let dock = self.config.mode == Mode::Dock;
        let edge = self.config.dock_edge;
        let mut entries = vec![
            MenuEntry::label(format!("{APP_NAME}  {BYLINE}  v{CURRENT_VERSION}")),
            MenuEntry::Separator,
            MenuEntry::item(CMD_SHOW, "런처 보이기"),
            MenuEntry::Separator,
            MenuEntry::checked(CMD_MODE_DOCK, "독 모드", dock),
            MenuEntry::checked(CMD_MODE_PANEL, "패널 모드", !dock),
        ];
        if dock {
            entries.extend([
                MenuEntry::checked(CMD_EDGE_BOTTOM, "    독 위치: 아래", edge == DockEdge::Bottom),
                MenuEntry::checked(CMD_EDGE_TOP, "    독 위치: 위", edge == DockEdge::Top),
                MenuEntry::checked(CMD_EDGE_LEFT, "    독 위치: 왼쪽", edge == DockEdge::Left),
                MenuEntry::checked(CMD_EDGE_RIGHT, "    독 위치: 오른쪽", edge == DockEdge::Right),
            ]);
        } else {
            entries.extend([
                MenuEntry::item(CMD_RESET_GEOMETRY, "위치·크기 초기화"),
                MenuEntry::checked(CMD_LOCK, "위치 잠금", self.config.locked),
            ]);
        }
        entries.extend([
            MenuEntry::Separator,
            MenuEntry::checked(CMD_AUTOSTART, "윈도우 시작 시 실행", self.autostart.is_enabled()),
            MenuEntry::item(CMD_CHECK_UPDATE, "업데이트 확인"),
        ]);
        if let Some(r) = &self.release {
            let text = if self.downloading {
                "업데이트 내려받는 중…".to_string()
            } else {
                format!("업데이트 설치 (v{})", r.version)
            };
            entries.push(MenuEntry::Item { id: CMD_INSTALL_UPDATE, text, checked: false, enabled: !self.downloading });
        }
        entries.extend([
            MenuEntry::item(CMD_RELEASES, "릴리스 페이지 열기"),
            MenuEntry::Separator,
            MenuEntry::item(CMD_QUIT, "종료"),
        ]);
        entries
    }

    fn show_menu(&mut self) {
        match show_popup(self.hwnd, &self.menu_entries()) {
            CMD_SHOW => self.panel_send(PanelCmd::Show),
            CMD_MODE_DOCK => self.set_mode(Mode::Dock, self.config.dock_edge),
            CMD_MODE_PANEL => self.set_mode(Mode::Panel, self.config.dock_edge),
            CMD_EDGE_BOTTOM => self.set_mode(Mode::Dock, DockEdge::Bottom),
            CMD_EDGE_TOP => self.set_mode(Mode::Dock, DockEdge::Top),
            CMD_EDGE_LEFT => self.set_mode(Mode::Dock, DockEdge::Left),
            CMD_EDGE_RIGHT => self.set_mode(Mode::Dock, DockEdge::Right),
            CMD_RESET_GEOMETRY => self.panel_send(PanelCmd::ResetGeometry),
            CMD_LOCK => {
                self.config.locked = !self.config.locked;
                self.save();
                self.panel_send(PanelCmd::SetLocked(self.config.locked));
            }
            CMD_AUTOSTART => self.set_autostart(!self.autostart.is_enabled()),
            CMD_CHECK_UPDATE => self.check_updates(true),
            CMD_INSTALL_UPDATE => self.install_update(),
            CMD_RELEASES => {
                let url = self.release.as_ref().map(|r| r.html_url.as_str()).filter(|u| !u.is_empty());
                if let Err(e) = shell::open(url.unwrap_or(RELEASES_URL)) {
                    logging::warn(&format!("릴리스 페이지 열기 실패: {e}"));
                }
            }
            CMD_QUIT => self.quit(),
            _ => {}
        }
    }

    fn quit(&mut self) {
        if let Some(tray) = self.tray.take() {
            tray.remove();
        }
        self.panel_send(PanelCmd::Close);
        let _ = unsafe { DestroyWindow(self.hwnd) };
        // 핸들러 안에서 자기 창을 없애면 WM_DESTROY가 핸들러에 오지 않으므로 여기서 루프를 끝낸다
        unsafe { PostQuitMessage(0) };
    }
}

impl Handler for App {
    fn handle(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        if self.hwnd.0.is_null() {
            self.start(hwnd);
        }
        match msg {
            WM_APP_TRAY => {
                let event = (lparam.0 & 0xFFFF) as u32;
                if matches!(event, WM_LBUTTONUP | WM_RBUTTONUP | WM_CONTEXTMENU) {
                    self.show_menu();
                }
            }
            WM_APP_EVENT => {
                while let Ok(event) = self.events_rx.try_recv() {
                    match event {
                        Event::Checked { manual, result } => self.on_checked(manual, result),
                        Event::Downloaded(result) => self.on_downloaded(result),
                        Event::Ui(ui) => self.on_ui(ui),
                        Event::Opened(chrome::Opened::DefaultBrowser) if !self.config.chrome_notice_shown => {
                            self.config.chrome_notice_shown = true;
                            self.save();
                            self.notify("크롬을 찾지 못해 기본 브라우저로 열었습니다.");
                        }
                        Event::Opened(_) => {}
                    }
                }
            }
            WM_APP_SHOW => self.panel_send(PanelCmd::Show),
            WM_TIMER => match wparam.0 {
                TIMER_FIRST_CHECK => {
                    let _ = unsafe { KillTimer(Some(hwnd), TIMER_FIRST_CHECK) };
                    unsafe { SetTimer(Some(hwnd), TIMER_DAILY_CHECK, DAILY_CHECK_MS, None) };
                    self.check_updates(false);
                }
                TIMER_DAILY_CHECK => self.check_updates(false),
                _ => return None,
            },
            WM_DESTROY => {
                if let Some(tray) = self.tray.take() {
                    tray.remove();
                }
                unsafe { PostQuitMessage(0) };
            }
            m if m == self.taskbar_created && m != 0 => {
                if let Some(tray) = &self.tray {
                    tray.add();
                }
            }
            _ => return None,
        }
        Some(LRESULT(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updater::Asset;

    fn release(version: &str, installable: bool) -> Release {
        let asset = Asset { name: "a".into(), url: "u".into(), size: 1 };
        Release {
            tag: format!("v{version}"),
            version: version.into(),
            html_url: String::new(),
            installer: installable.then(|| asset.clone()),
            sums: Some(asset),
        }
    }

    #[test]
    fn newer_installable_release_is_available_and_notified_once() {
        let r = Ok(Some(release("0.3.0", true)));
        assert_eq!(decide_update(&r, "0.2.0", ""), UpdateDecision::Available { version: "0.3.0".into(), notify: true });
        assert_eq!(
            decide_update(&r, "0.2.0", "0.3.0"),
            UpdateDecision::Available { version: "0.3.0".into(), notify: false }
        );
    }

    #[test]
    fn same_older_missing_assets_or_no_release_is_up_to_date() {
        assert_eq!(decide_update(&Ok(Some(release("0.2.0", true))), "0.2.0", ""), UpdateDecision::UpToDate);
        assert_eq!(decide_update(&Ok(Some(release("0.1.4", true))), "0.2.0", ""), UpdateDecision::UpToDate);
        assert_eq!(decide_update(&Ok(Some(release("0.3.0", false))), "0.2.0", ""), UpdateDecision::UpToDate);
        assert_eq!(decide_update(&Ok(None), "0.2.0", ""), UpdateDecision::UpToDate);
        assert_eq!(decide_update(&Err("x".into()), "0.2.0", ""), UpdateDecision::Failed);
    }

    #[test]
    fn startup_notices_cover_recovery_and_upgrade() {
        let recovered = LoadReport { recovered: true, migrated: false };
        assert_eq!(startup_notices(LoadReport::default(), "", "0.2.0"), Vec::<String>::new());
        assert_eq!(startup_notices(LoadReport::default(), "0.2.0", "0.2.0"), Vec::<String>::new());
        assert_eq!(
            startup_notices(LoadReport::default(), "0.1.4", "0.2.0"),
            vec!["v0.2.0 버전으로 업데이트되었습니다.".to_string()]
        );
        assert_eq!(startup_notices(recovered, "", "0.2.0").len(), 1);
    }

    #[test]
    fn tile_views_map_kinds_labels_and_badges() {
        let items = vec![
            Item::Youtube { id: "y".into(), query: "로파이 음악".into(), name: "".into() },
            Item::Web { id: "w".into(), url: "https://github.com".into(), name: "".into() },
            Item::App { id: "a".into(), name: "계산기".into(), target: "Calc!App".into(), exe: None },
        ];
        let views = tile_views(&items);
        assert_eq!(
            views[0],
            TileView {
                id: "y".into(), label: "로파이 음악".into(), badge: "로파".into(), kind: TileKind::Youtube
            }
        );
        assert_eq!(
            (views[1].label.as_str(), views[1].badge.as_str(), views[1].kind),
            ("github.com", "G", TileKind::Web)
        );
        assert_eq!((views[2].badge.as_str(), views[2].kind), ("계", TileKind::App));
    }

    #[test]
    fn apply_order_follows_ids_and_keeps_unknown() {
        let a = Item::youtube("a", "").unwrap();
        let b = Item::youtube("b", "").unwrap();
        let c = Item::youtube("c", "").unwrap();
        let items = vec![a.clone(), b.clone(), c.clone()];
        let ids = vec![c.id().to_string(), a.id().to_string(), "gone".to_string()];
        assert_eq!(apply_order(&items, &ids), vec![c, a, b]);
    }

    #[test]
    fn debounce_blocks_repeat_within_one_second() {
        let mut d = Debounce::default();
        let t0 = Instant::now();
        assert!(d.allow("x", t0));
        assert!(!d.allow("x", t0 + Duration::from_millis(500)));
        assert!(d.allow("y", t0 + Duration::from_millis(500)));
        assert!(d.allow("x", t0 + Duration::from_millis(1200)));
    }
}
```

- [ ] **Step 3: src/ui/mod.rs 작성**

```rust
//! 화면: 테마, 배치, 애니메이션, 그리기 장치, 장면, 패널 창, 입력 창.

pub mod anim;
pub mod dialogs;
pub mod dock_layout;
pub mod gfx;
pub mod layout;
pub mod panel;
pub mod scene;
pub mod theme;
```

- [ ] **Step 4: 테스트**

Run: `cargo test`
Expected: `test result: ok. 70 passed`

- [ ] **Step 5: 릴리스 실행·메모리·대기 CPU (임시 APPDATA, 독 모드)**

Run:
```powershell
cargo build --release
$tmpApp = Join-Path $env:TEMP "ytl-phase3"; Remove-Item -Recurse -Force $tmpApp -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force "$tmpApp\YT-Launcher" | Out-Null
'{"schema":2,"items":[{"kind":"youtube","id":"a1","query":"로파이 음악","name":""}],"mode":"dock","dock_edge":"bottom","theme":"glass"}' | Set-Content "$tmpApp\YT-Launcher\config.json" -Encoding utf8NoBOM
$psi = New-Object System.Diagnostics.ProcessStartInfo (Resolve-Path target\release\YTLauncher.exe)
$psi.UseShellExecute = $false; $psi.EnvironmentVariables["APPDATA"] = $tmpApp
$p = [System.Diagnostics.Process]::Start($psi); Start-Sleep 5; $p.Refresh(); $c0 = $p.TotalProcessorTime.TotalMilliseconds
Start-Sleep 10; $p.Refresh()
"WS={0:N1}MB Private={1:N1}MB idleCPU10s={2:N0}ms" -f ($p.WorkingSet64/1MB), ($p.PrivateMemorySize64/1MB), ($p.TotalProcessorTime.TotalMilliseconds - $c0)
Stop-Process -Id $p.Id
```
Expected: 작업 집합 약 35~45MB, 쉬는 동안 CPU 0ms. (설치된 0.1.x 런처가 실행 중이면 바로 종료된다 — 사용자에게 알리고 직접 종료하지 않는다.)

- [ ] **Step 6: 사용자 수동 점검 항목 (보고에 포함)**

독이 화면 아래 가운데(작업표시줄 위 8px)에 뜨는지, 마우스를 올리면 물결처럼 커지고 이웃이 밀리는지, 말풍선이 위에 뜨는지, 판 위 빈 곳·독 옆 바탕화면 클릭이 막히지 않는지, ⋯ 메뉴에서 독 위치 위·왼쪽·오른쪽으로 바꾸면 그 가장자리로 옮겨 가는지(세로 독), 패널 모드로 바꾸면 우상단 패널로 돌아오는지, 팔레트 선택기가 화면 안쪽으로 펼쳐지는지, 드래그 정렬, 흐림 배경.

- [ ] **Step 7: 전체 검사**

Run: `cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test`
Expected: clippy 오류 없음, `test result: ok`

- [ ] **Step 8: Commit**

```powershell
git add src/ui src/app.rs
git commit -m "feat: 독 모드 (가장자리 4방향, 확대 물결, 모드 전환 메뉴)"
```

---

