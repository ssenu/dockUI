# 독 흐림 창 크기 되돌리기·패널 아이콘 크기 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 독 판 아래(위)로 남는 흐린 사각형을 없애고, 패널 모드에 메뉴로 아이콘 크기(40/56/72/88)를 고를 수 있게 한다.

**Architecture:** 아크릴 흐림이 창 영역을 따르지 않으므로(실험으로 확인), 흐림 창을 다시 판 크기에 맞춰 옮긴다(가벼운 `SetWindowPos` 플래그, 바뀔 때만). 패널 배치(`PanelLayout`)에 아이콘 크기 인자를 더하고, 설정·메뉴·화면 명령으로 잇는다.

**Tech Stack:** Rust 2024, `windows` 0.62.

## Global Constraints

- 흐림 창 이동 플래그: `SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOSENDCHANGING | SWP_NOCOPYBITS`, 화면 px로 바뀌었을 때만.
- 독 흐림 창: 둥근 모서리 켬, 테두리선 끔. 패널: 둥근 모서리 + 기본 테두리.
- 패널 아이콘 크기 선택지 `[40, 56, 72, 88]`, 기본 56, 설정 `panel_icon_size`.
- 검사: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.

---

### Task 1: 독 흐림 창을 판 크기에 맞춰 옮기기

**Files:** `src/ui/panel.rs`, `src/win/dwm.rs`

- [ ] **Step 1: 구현** — `dwm.rs`의 `set_frame`을

```rust
/// 창 외곽: 둥근 모서리는 늘 켜고(판 모서리와 맞춤), 테두리선은 `border`일 때만 (독은 끈다)
pub fn set_frame(hwnd: HWND, border: bool) {
    let corner: DWM_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND;
    set_attr(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &corner);
    set_attr(hwnd, DWMWA_BORDER_COLOR, &(if border { COLOR_DEFAULT } else { COLOR_NONE }));
}
```

  (`DWMWCP_DONOTROUND` 가져오기 삭제). `panel.rs`: `place_dock`에서 `glass_px`/최대 크기 이동 블록을 지우고, `update_dock_glass`의 영역 블록을

```rust
        // 아크릴 흐림은 창 영역을 따르지 않으므로(실험으로 확인) 흐림 창을 지금 판 크기에 맞춰 옮긴다.
        // 화면 px로 바뀌었을 때만, 가벼운 플래그로 (활성화·z순서·크기 변경 알림·비트 복사 생략)
        let local = px(bar);
        let glass = (cx + local.0, cy + local.1, local.2, local.3);
        if self.last_glass != Some(glass) {
            self.last_glass = Some(glass);
            self.positioning = true;
            // SAFETY: 우리 유리 창 위치·크기만 바꾼다
            let _ = unsafe {
                SetWindowPos(
                    self.glass,
                    None,
                    glass.0,
                    glass.1,
                    glass.2,
                    glass.3,
                    SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOSENDCHANGING | SWP_NOCOPYBITS,
                )
            };
            self.positioning = false;
        }
```

  로 바꾼다. `glass_px` 필드와 `CreateRoundRectRgn`·`WINDOW_RADIUS` 가져오기, 패널 갈래의 `SetWindowRgn(self.glass, None, ...)`(이제 걸지 않으므로)를 지운다.
- [ ] **Step 2: 확인** — `cargo build`, clippy, `cargo test`.
- [ ] **Step 3: 화면 확인** — `dwm::tests::blur_region_probe`를 "크기를 줄인 창"으로도 찍어(아래 60px을 `SetWindowPos`로 줄임) 흐림이 창 크기를 따르는지 본다.
- [ ] **Step 4: 커밋** — `git commit -am "fix: 독 판 끝 밖으로 남던 흐린 사각형 (흐림 창을 판 크기에 맞춤)"`

### Task 2: 패널 아이콘 크기

**Files:** `src/ui/layout.rs`, `src/ui/panel.rs`, `src/ui/scene.rs`(테스트), `src/config.rs`, `src/app.rs`

**Interfaces:** Produces `PanelLayout::compute(width, height, items, icon: f32)`, `PanelLayout::min_height(width, items, icon)`, `layout::PANEL_ICON_SIZES: [u32; 4]`, `Config.panel_icon_size: u32`, `PanelCmd::SetPanelIcon(u32)`, `OpenSpec.panel_icon_size`.

- [ ] **Step 1: 테스트** — `layout.rs`

```rust
    #[test]
    fn icon_size_changes_cells_columns_and_min_height() {
        let small = PanelLayout::compute(360.0, 480.0, 5, 40.0);
        let big = PanelLayout::compute(360.0, 480.0, 5, 88.0);
        assert_eq!((small.cells[0].w, big.cells[0].w), (40.0, 88.0));
        let cols = |l: &PanelLayout| l.cells.iter().filter(|c| (c.y - l.cells[0].y).abs() < 0.01).count();
        assert!(cols(&small) > cols(&big));
        assert!(PanelLayout::min_height(360.0, 5, 88.0) > PanelLayout::min_height(360.0, 5, 40.0));
        assert_eq!(PANEL_ICON_SIZES, [40, 56, 72, 88]);
    }
```

  기존 `compute`/`min_height` 호출은 아이콘 인자 `ICON`(56)을 더한다. `config.rs`:

```rust
    #[test]
    fn panel_icon_size_defaults_and_round_trips() {
        assert_eq!(Config::default().panel_icon_size, 56);
        let c = Config { panel_icon_size: 72, ..Config::default() };
        let back: Config = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back.panel_icon_size, 72);
        let old: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(old.panel_icon_size, 56);
    }
```

  `app.rs`(`mode_entries`에 아이콘 크기 인자 추가):

```rust
    #[test]
    fn panel_menu_has_icon_sizes_with_check() {
        let entries = mode_entries(false, false, false, 72);
        let sub = entries.iter().find_map(|e| match e {
            MenuEntry::Submenu { text, entries } if text == "아이콘 크기" => Some(entries),
            _ => None,
        });
        let sub = sub.expect("패널 모드에 아이콘 크기 메뉴");
        let checked: Vec<&str> = sub
            .iter()
            .filter_map(|e| match e {
                MenuEntry::Item { text, checked: true, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(checked, vec!["크게 (72)"]);
        assert!(!mode_entries(true, false, false, 72).iter().any(|e| matches!(e, MenuEntry::Submenu { .. })));
    }
```

- [ ] **Step 2: 실행** — 컴파일 오류(인자 수).
- [ ] **Step 3: 구현** — `layout.rs`: `pub const PANEL_ICON_SIZES: [u32; 4] = [40, 56, 72, 88];`, `compute`/`min_height`에 `icon: f32` 인자, 그 안의 `ICON`을 `icon`으로 (열 수, 간격, 칸, 오버 몫). `config.rs`: `pub panel_icon_size: u32` 기본 `56`. `panel.rs`: `OpenSpec.panel_icon_size`, 필드 `panel_icon: f32`, 모든 `PanelLayout::compute`/`min_height` 호출에 `self.panel_icon`, `PanelCmd::SetPanelIcon(size)` → `panel_icon = size as f32`, 패널 모드면 `SetTiles`와 같은 규칙(최소 높이보다 작으면 늘림)으로 `place_windows` 후 그리기. `app.rs`: `CMD_PANEL_ICON = 50`(+번호), `mode_entries(dock, editing, locked, icon)`가 패널 모드일 때 `MenuEntry::submenu("아이콘 크기", [작게 (40), 보통 (56), 크게 (72), 더 크게 (88)])` 추가, 고르면 `config.panel_icon_size` 저장 + `PanelCmd::SetPanelIcon`.
- [ ] **Step 4: 확인** — `cargo test`, clippy, 스냅샷 `panel-icon-72.png`(scene 테스트에 추가).
- [ ] **Step 5: 커밋** — `git commit -am "feat: 패널 모드 아이콘 크기 (메뉴에서 작게·보통·크게·더 크게)"`

### Task 3: 문서·릴리스

- [ ] README·점검표에 패널 아이콘 크기, 독 흐린 사각형 항목. 커밋, 푸시, `v0.2.8`, 워크플로 확인.
