# 한 줄 말풍선·패널 정리·부드러움 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 독·패널 이름 말풍선을 한 줄로(창 밖까지), 독 by ssenu·패널 팔레트 제거, 패널 최소 높이, 독 확대·패널 크기 조절의 끊김 줄이기.

**Architecture:** 말풍선 크기는 `scene::tooltip_size`(한 줄)로 통일하고, 화면 쪽(`Panel`)이 가장 긴 말풍선 폭을 재서 독 여유 공간과 패널 콘텐츠 창 좌우 여유를 정한다. 패널은 콘텐츠 창 안에서 `Gfx::set_offset`으로 옮겨 그린다. 독 유리 창은 최대 길이로 고정하고 흐림 모양만 `SetWindowRgn`으로 바꾼다.

**Tech Stack:** Rust 2024, `windows` 0.62 (Direct2D·DirectWrite·DirectComposition·Win32).

## Global Constraints

- 말풍선: 한 줄, 높이 26 DIP, 좌우 여백 10 DIP, 넘치면 `…`, 페이드 약 120ms(`Anim`), 픽셀 맞춤(`snap`).
- 독은 판이 늘어나는 확대 방식 유지 (`MAGNIFY = 0.3`).
- 패널 머리글 `by_ssenu`는 유지, 독 판 끝 `by ssenu`는 제거.
- 테마는 트레이·⋯ 메뉴 "테마 ▸"로만 바꾼다.
- 버퍼 올림 단위 256 px.
- 검사: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` 통과.

---

### Task 1: 독 by ssenu 제거

**Files:** Modify `src/ui/dock_layout.rs`, `src/ui/scene.rs`

**Interfaces:** Produces `place(widths, split) -> (Vec<f32>, f32, Option<f32>)` (글자 칸 없음).

- [ ] **Step 1: 테스트 바꾸기** — `credit_sits_inside_the_bar_after_the_last_slot` 삭제, `rest_layout_is_centered_and_holds_only_tiles`의 기대값을 글자 칸 없이:

```rust
        let expected = PAD * 2.0 + 2.0 * ICON + GAP;
        assert!((l.length - expected).abs() < 0.01);
        assert!((l.rest[0] + l.length / 2.0 - (PAD + ICON / 2.0)).abs() < 0.01);
        let sum: f32 = l.rest.first().unwrap() + l.rest.last().unwrap();
        assert!(sum.abs() < 0.01, "가운데 정렬");
```

- [ ] **Step 2: 실행** — `cargo test rest_layout` → FAIL (길이에 글자 칸이 더해져 있음).
- [ ] **Step 3: 구현** — `CREDIT_W`, `CREDIT_H`, `credit_len`, `DockLayout.credit`, `Magnified.credit`를 지우고 `place`를

```rust
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
```

로, 호출부(`compute`, `magnified`)를 세 값으로. `scene.rs`의 `credit_rect`, `draw_credit`, `draw_dock`의 `draw_credit(...)` 호출을 지운다.
- [ ] **Step 4: 확인** — `cargo test` PASS, `cargo clippy --all-targets -- -D warnings` 통과.
- [ ] **Step 5: 커밋** — `git commit -am "style: 독에서 by ssenu 제거"`

### Task 2: 한 줄 말풍선 + 가장 긴 이름에 맞춘 독 여유 공간

**Files:** Modify `src/ui/scene.rs`, `src/ui/gfx.rs`, `src/ui/dock_layout.rs`, `src/ui/panel.rs`

**Interfaces:**
- Produces `scene::tooltip_size(g, text, room) -> (f32, f32)` (한 줄), `scene::longest_tip(g, labels: &[&str], cap: f32) -> f32`, `DockLayout::headroom(&self, longest_tip: f32) -> f32`, `DockLayout::side_room(&self, longest_tip: f32) -> f32`.

- [ ] **Step 1: 실패하는 테스트** — `dock_layout.rs`

```rust
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
```

`scene.rs`

```rust
    #[test]
    fn tooltip_is_one_line_as_wide_as_the_name() {
        let _ = unsafe {
            windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)
        };
        let g = Gfx::offscreen(10, 10, 1.0).unwrap();
        let short = tooltip_size(&g, "뉴스", 2000.0);
        let long = tooltip_size(&g, "아침 뉴스 라이브 - 오늘의 주요 소식 모아 보기 (전체 재생목록) 경제·사회·국제", 2000.0);
        assert_eq!(short.1, 26.0);
        assert_eq!(long.1, 26.0, "한 줄");
        assert!(long.0 > 400.0, "{long:?}");
        assert_eq!(tooltip_size(&g, "아주 아주 긴 이름 아주 아주 긴 이름", 120.0).0, 120.0, "상한은 room");
        assert!(longest_tip(&g, &["뉴스", "아주 긴 이름 아주 긴 이름"], 5000.0) > short.0);
        assert_eq!(longest_tip(&g, &["아주 긴 이름 아주 긴 이름"], 50.0), 50.0);
    }
```

- [ ] **Step 2: 실행** — `cargo test room_follows tooltip_is_one_line` → 컴파일 오류.
- [ ] **Step 3: 구현** — `scene.rs`

```rust
/// 말풍선 크기 (너비, 높이): 한 줄. 이름 전체 폭에 맞추고, `room`보다 길면 `room`에서 … 으로
pub fn tooltip_size(g: &Gfx, text: &str, room: f32) -> (f32, f32) {
    ((g.measure(text, 12.0, Weight::SemiBold) + 20.0).ceil().min(room.max(40.0)).max(40.0), 26.0)
}

/// 이 이름들 중 가장 긴 말풍선 폭 (`cap`을 넘지 않게)
pub fn longest_tip(g: &Gfx, labels: &[&str], cap: f32) -> f32 {
    labels.iter().map(|t| tooltip_size(g, t, cap).0).fold(0.0, f32::max)
}
```

`draw_tooltip_box`의 글자는 `g.text(text, inner, 12.0, Weight::SemiBold, Align::Center, TOOLTIP_TEXT.with_alpha(alpha))`. `dock_tooltip_rect`의 `room`은 세로 독이면 `icon.x - 12.0`(오른쪽 독)/`bounds.0 - icon.right() - 12.0`(왼쪽 독), 가로 독이면 `bounds.0 - 8.0`. `gfx.rs`에서 `text_block`, `measure_block`, `block_format`, `DWRITE_WORD_WRAPPING_WRAP`·`DWRITE_TRIMMING_GRANULARITY_NONE`·`D2D1_DRAW_TEXT_OPTIONS_NONE` 가져오기와 형식 캐시의 `block` 갈래를 지운다.

`dock_layout.rs`: `TOOLTIP_MAX_W`, `SIDE_HEADROOM` 삭제,

```rust
    /// 콘텐츠 창이 판 양 끝으로 더 차지하는 여유: 판이 짧아도 가장 긴 말풍선이 들어가게
    pub fn side_room(&self, longest_tip: f32) -> f32 {
        if is_vertical(self.edge) { SIDE_ROOM } else { SIDE_ROOM.max((longest_tip + 16.0 - self.max_length()) / 2.0) }
    }

    /// 콘텐츠 창이 화면 안쪽으로 차지하는 여유 (세로 독은 가장 커진 아이콘 + 가장 긴 말풍선)
    pub fn headroom(&self, longest_tip: f32) -> f32 {
        if is_vertical(self.edge) { HEADROOM.max(ICON * MAGNIFY + 8.0 + longest_tip + 8.0) } else { HEADROOM }
    }
```

기존 `vertical_dock_has_room_for_wide_tooltips` 테스트는 `room_follows_the_longest_tooltip`로 대체. `panel.rs`에 `longest_tip()`:

```rust
    /// 지금 항목들 중 가장 긴 말풍선 폭 (DIP, 작업 영역 폭을 넘지 않게)
    fn longest_tip(&self) -> f32 {
        let cap = desktop::primary_work_area().2 as f32 / self.scale;
        let Some(gfx) = &self.gfx else { return 200.0 };
        let mut labels: Vec<&str> = self.tiles.iter().map(|t| t.label.as_str()).collect();
        labels.push("항목 추가");
        scene::longest_tip(gfx, &labels, cap)
    }
```

`place_dock`에서 `let tip = self.longest_tip();`를 먼저 구하고 `headroom(tip)`, `side_room(tip)`. 스냅샷 테스트도 `headroom(400.0)`, `side_room(400.0)`.
- [ ] **Step 4: 확인** — `cargo test` PASS, `dock-left.png`(세로)·`dock-bottom.png`에서 긴 이름이 한 줄.
- [ ] **Step 5: 커밋** — `git commit -am "feat: 독 말풍선을 한 줄로 (가장 긴 이름에 맞춰 공간 확보)"`

### Task 3: 패널 팔레트 제거

**Files:** Modify `src/ui/layout.rs`, `src/ui/scene.rs`, `src/ui/panel.rs`, `src/app.rs`

- [ ] **Step 1: 테스트 바꾸기** — `layout.rs`의 `swatches_fan_up_and_left_of_palette_button` 삭제, `hit_test_finds_buttons_tiles_and_edges`의 `palette_button` 단언 삭제.
- [ ] **Step 2: 구현** — 지우기: `layout.rs`의 `PALETTE_BUTTON`, `PICKER_RADIUS`, `SWATCH`, `PanelLayout.palette_button`, `swatch_centers`, `fan_points`, `Hit::Palette`(와 `hit_test`의 판정); `scene.rs`의 `draw_round_button`, `draw_palette_button`, `draw_tooltip_toward`, `draw_swatches`, `draw_picker`, `SceneState.palette_hover/picker/picker_hover`, `draw`의 팔레트·선택기 그리기; `panel.rs`의 `picker_open`, `picker`, `picker_hover`, `PICKER_MS`, `swatch_centers`, `swatch_at`, `close_picker`, `on_mouse_move`의 견본 판정, `on_button_up`의 선택기 처리와 `Hit::Palette` 갈래, `tick`의 `picker.advance`, `hit_test_nc`의 `!self.picker_open` 조건; `UiEvent::ThemePicked`와 `app.rs`의 처리(트레이 메뉴 테마는 `CMD_THEME`가 처리).
- [ ] **Step 3: 확인** — `cargo test`, clippy 통과, `panel-*.png`에 팔레트 없음.
- [ ] **Step 4: 커밋** — `git commit -am "feat: 패널 테마 팔레트 제거 (메뉴에서 테마 선택)"`

### Task 4: 패널 최소 높이와 아래 여백

**Files:** Modify `src/ui/layout.rs`, `src/ui/panel.rs`

**Interfaces:** Produces `PanelLayout::min_height(width: f32, items: usize) -> f32`.

- [ ] **Step 1: 실패하는 테스트** — `layout.rs`

```rust
    #[test]
    fn min_height_fits_every_icon_with_side_padding_below() {
        // 360 폭 = 4열. 항목 5 + ＋ = 6칸 → 2줄
        let l = PanelLayout::compute(360.0, 480.0, 5);
        let h = PanelLayout::min_height(360.0, 5);
        let last = l.cells.last().unwrap();
        let grow = ICON * (PANEL_HOVER_SCALE - 1.0) / 2.0;
        assert!((h - (last.bottom() + grow + PADDING)).abs() < 0.01, "{h}");
        // 폭을 줄여 줄이 늘면 최소 높이도 커진다
        assert!(PanelLayout::min_height(200.0, 5) > h);
        assert!(PanelLayout::min_height(360.0, 0) >= MIN_H);
    }
```

- [ ] **Step 2: 실행** — FAIL (함수 없음).
- [ ] **Step 3: 구현** — `layout.rs`

```rust
    /// 이 폭에서 모든 아이콘(마우스 오버로 커진 몫 포함)이 보이고 아래 여백이 양옆과 같은 최소 높이
    pub fn min_height(width: f32, items: usize) -> f32 {
        let l = PanelLayout::compute(width, MIN_H, items);
        let last = l.cells.last().map_or(l.header.bottom(), |c| c.bottom());
        (last + ICON * (PANEL_HOVER_SCALE - 1.0) / 2.0 + PADDING).max(MIN_H)
    }
```

`panel.rs`: `WM_GETMINMAXINFO`의 최소 높이를 `PanelLayout::min_height(지금 폭 DIP, 항목 수) * scale`로. `WM_SIZING` 처리 추가:

```rust
            WM_SIZING if self.mode == Mode::Panel => {
                // SAFETY: lparam은 시스템이 넘긴 끌기 중 사각형
                let r = unsafe { &mut *(lparam.0 as *mut RECT) };
                let w = (r.right - r.left) as f32 / self.scale;
                let min = (PanelLayout::min_height(w, self.tiles.len()) * self.scale).ceil() as i32;
                if r.bottom - r.top < min {
                    let edge = wparam.0 as u32;
                    // 위쪽 가장자리(WMSZ_TOP·TOPLEFT·TOPRIGHT)를 끄는 중이면 위를, 아니면 아래를 늘린다
                    if matches!(edge, WMSZ_TOP | WMSZ_TOPLEFT | WMSZ_TOPRIGHT) {
                        r.top = r.bottom - min;
                    } else {
                        r.bottom = r.top + min;
                    }
                }
                return Some(LRESULT(1));
            }
```

- [ ] **Step 4: 확인** — `cargo test`, clippy.
- [ ] **Step 5: 커밋** — `git commit -am "feat: 패널을 줄여도 아이콘이 가려지지 않게 최소 높이"`

### Task 5: 패널 말풍선을 패널 밖까지 + 페이드

**Files:** Modify `src/ui/gfx.rs`, `src/ui/scene.rs`, `src/ui/panel.rs`

**Interfaces:** Produces `Gfx::set_offset(&self, dx: f32, dy: f32)`, `scene::panel_tooltip_rect(g, text, anchor, panel: (f32, f32), bounds_x: (f32, f32), top_limit) -> RectF`, `SceneState.tooltip_alpha: f32`, `panel_side_px(longest_tip, panel_w, scale) -> i32`.

- [ ] **Step 1: 실패하는 테스트** — `panel.rs` 테스트 모듈(없으면 추가)

```rust
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
```

- [ ] **Step 2: 실행** — FAIL.
- [ ] **Step 3: 구현** — `gfx.rs`

```rust
    /// 이후 그리기를 (dx, dy) DIP만큼 옮긴다 (패널을 넓은 콘텐츠 창 안쪽에 그릴 때)
    pub fn set_offset(&self, dx: f32, dy: f32) {
        unsafe { self.ctx.SetTransform(&Matrix3x2::translation(dx, dy)) };
    }
```

`panel.rs`

```rust
/// 패널 콘텐츠 창이 패널 좌우로 더 차지하는 폭 (px): 가장 긴 말풍선이 패널보다 넓을 때만
fn panel_side_px(longest_tip: f32, panel_w: f32, scale: f32) -> i32 {
    (((longest_tip + 16.0 - panel_w) / 2.0).max(0.0) * scale).ceil() as i32
}
```

`Panel`에 `panel_side: i32`(px). `relayout_panel`: `self.panel_side = panel_side_px(self.longest_tip(), w / scale, scale)`, 콘텐츠 창 `(x - side, y, w + 2 * side, h)`, 그리기 버퍼도 그 크기. 그리기 때 `gfx.set_offset(side / scale, 0.0)` 후 `scene::draw`. 패널 말풍선은 `scene::draw`가 그리지 않고 `draw_frame`이 오프셋 0으로 되돌린 뒤 따로 그린다:

```rust
pub fn panel_tooltip_rect(g: &Gfx, text: &str, anchor: RectF, left: f32, right: f32, top_limit: f32) -> RectF {
    let (w, h) = tooltip_size(g, text, right - left - 8.0);
    let x = (anchor.center().x - w / 2.0).clamp(left + 4.0, (right - w - 4.0).max(left + 4.0));
    let mut y = anchor.y - h - 8.0;
    if y < top_limit {
        y = anchor.bottom() + 8.0;
    }
    RectF::new(x, y, w, h)
}
```

(`anchor`·`top_limit`은 콘텐츠 창 좌표로 옮긴 값, `left..right` = 콘텐츠 창 폭.) 말풍선 페이드는 독과 같은 `tip`/`tip_slot`을 패널 모드에서도 쓴다(`tick`에서 모드와 무관하게 갱신, 대상은 패널이면 지금의 `tooltip` 계산). 영역: 말풍선 불투명도가 0이면 패널 사각형만, 아니면 패널 ∪ 말풍선 사각형 — 상태가 바뀔 때만 `SetWindowRgn`.
- [ ] **Step 4: 확인** — `cargo test`, clippy, 스냅샷 `panel-*.png`에서 긴 이름 말풍선이 한 줄(테스트 장면의 패널 콘텐츠 폭을 넓혀 그림).
- [ ] **Step 5: 커밋** — `git commit -am "feat: 패널 말풍선 한 줄, 패널 밖까지, 페이드"`

### Task 6: 부드럽게 — 독 흐림 창 고정, 패널 창 동기 이동, 버퍼 올림

**Files:** Modify `src/ui/panel.rs`, `src/ui/gfx.rs`, `src/ui/dock_layout.rs`

**Interfaces:** Produces `gfx::buffer_size(px: u32) -> u32`, `DockLayout::glass_rect` (최대 길이 판, 콘텐츠 창 기준).

- [ ] **Step 1: 실패하는 테스트** — `gfx.rs`

```rust
#[cfg(test)]
mod buffer_tests {
    use super::buffer_size;

    #[test]
    fn buffer_grows_in_256px_steps() {
        assert_eq!(buffer_size(1), 256);
        assert_eq!(buffer_size(256), 256);
        assert_eq!(buffer_size(257), 512);
        assert_eq!(buffer_size(1000), 1024);
    }
}
```

- [ ] **Step 2: 실행** — FAIL.
- [ ] **Step 3: 구현** — `gfx.rs`

```rust
/// 그리기 버퍼 크기: 256px 단위로 올려 잡아 창 크기가 조금씩 바뀔 때마다 버퍼를 다시 만들지 않는다
pub fn buffer_size(px: u32) -> u32 {
    px.max(1).div_ceil(256) * 256
}
```

`Gfx::resize`는 `(buffer_size(w), buffer_size(h))`가 지금 버퍼와 같고 배율도 같으면 아무것도 하지 않는다(`Gfx::new`도 버퍼를 `buffer_size`로). `begin`의 `Clear`는 그대로(창 밖은 잘린다).

`panel.rs`:
- 독 유리 창: `place_dock`에서 유리 창을 최대 길이 판(`rect_around(center_px, edge, widest, thick)`)으로 한 번 옮긴다. `update_dock_glass`는 이동 대신 유리 창 영역을 지금 판 모양으로:

```rust
        // 흐림 창은 가장 커졌을 때 크기로 고정해 두고, 흐림이 보이는 모양만 지금 판으로 바꾼다
        let (gx, gy, _, _) = self.glass_px;
        let (cx, cy, _, _) = self.dock.content_px;
        let local = px(bar);
        let shape = (cx + local.0 - gx, cy + local.1 - gy, local.2, local.3);
        if self.last_glass != Some(shape) {
            self.last_glass = Some(shape);
            let radius = (WINDOW_RADIUS * 2.0 * s).round() as i32;
            let rgn = unsafe { CreateRoundRectRgn(shape.0, shape.1, shape.0 + shape.2 + 1, shape.1 + shape.3 + 1, radius, radius) };
            let _ = unsafe { SetWindowRgn(self.glass, Some(rgn), false) };
        }
```

  (`glass_px` = 놓을 때 기억한 유리 창 화면 사각형.) `local_point`·`dock_axes`는 그대로(콘텐츠 창 기준으로 바꿔 계산). 패널 모드로 갈 때 `SetWindowRgn(self.glass, None, true)`.
- 패널 동기 이동: 유리 창 `WM_WINDOWPOSCHANGING`에서 패널 모드이고 우리가 옮기는 중이 아니면, `WINDOWPOS`의 새 사각형(크기 바뀜 플래그를 보고)으로 `panel_rect`·레이아웃을 계산하고 콘텐츠 창을 먼저 옮긴 뒤 그린다. `WM_WINDOWPOSCHANGED`는 위치 저장 타이머만.
- [ ] **Step 4: 확인** — `cargo test`, clippy, `cargo build --release`.
- [ ] **Step 5: 커밋** — `git commit -am "perf: 독 흐림 창 고정·패널 창 동기 이동·그리기 버퍼 올림"`

### Task 7: 문서

**Files:** Modify `README.md`, `docs/manual-test-checklist.md`

- [ ] **Step 1** — README: 독 줄에서 `판 끝에 연한 \`by ssenu\`.` 삭제, 테마 줄을 "트레이·⋯ 메뉴 → 테마"로. 점검표: by ssenu 줄 삭제, 팔레트 항목을 "패널에 팔레트 버튼이 없고 테마는 메뉴에서", 추가:

```markdown
- [ ] 오른쪽 독에서 긴 이름에 마우스를 올리면 왼쪽으로 한 줄 말풍선, 패널에서는 패널 밖까지 한 줄
- [ ] 패널 테두리를 끌어 줄여도 아래 줄 아이콘이 가려지지 않고 아래 여백이 양옆과 같다
- [ ] 독 확대 중 판 끝 흐림이 판과 어긋나지 않고, 패널을 끌어 옮길 때 내용이 테두리를 따라 늦지 않는다
```

- [ ] **Step 2: 커밋** — `git commit -am "docs: 한 줄 말풍선·패널 정리 점검 항목"`
