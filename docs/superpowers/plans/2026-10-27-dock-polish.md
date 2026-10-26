# Dock UI 다듬기 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `by ssenu`를 독 판 안 끝에 연하게, 독 말풍선 깨짐(깜빡임·잘림·거친 모서리) 제거, 트레이 메뉴의 독 위치 4줄 제거, 알림 머리글 이름을 `Dock UI`로.

**Architecture:** 독 배치(`dock_layout.rs`, 순수 함수)에 끝 글자 칸을 더하고, 독 콘텐츠 창 표시 영역을 '쉼/활성' 두 상태로만 바꾸며(`panel.rs`), 말풍선은 불투명도 애니메이션과 픽셀 맞춤으로 그린다(`scene.rs`). 실행 파일 버전 정보는 `build.rs`가 만든 리소스 스크립트로, 알림 이름은 고정 앱 ID + 같은 ID의 설치 바로가기로 맞춘다.

**Tech Stack:** Rust 2024, `windows` 0.62 (Direct2D·DirectWrite·Win32), `embed-resource` 3, Inno Setup 6.

## Global Constraints

- 보이는 이름은 `Dock UI`. exe 이름(`YTLauncher.exe`), 설치 파일 이름(`YT-Launcher-Setup-<버전>.exe`), 설정 폴더(`%APPDATA%\YT-Launcher`), 뮤텍스(`YTLauncherMutex`), Run 값(`YTLauncher`)은 바꾸지 않는다 (업데이트 호환).
- 앱 ID: `ssenu.DockUI`.
- 글자 칸: 가로 독 폭 `CREDIT_W = 36` DIP, 세로 독 높이 `CREDIT_H = 30` DIP, 글자 `by` / `ssenu` 두 줄, 9 DIP SemiBold, 테마 `subtext` 알파 0.55.
- 말풍선 페이드는 기존 `Anim`(60Hz 기준 프레임마다 남은 거리의 25%)으로 약 120ms.
- 검사: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` 모두 통과.

---

### Task 1: 판 끝 글자 칸 (by ssenu)

**Files:**
- Modify: `src/ui/dock_layout.rs` (상수, `DockLayout`/`Magnified`의 `credit`, `place`, `edge_margin`, `content_rect`, 테스트)
- Modify: `src/ui/scene.rs` (`credit_rect`·`draw_credit` 교체, `edit_hint_rect`의 Top 위치, 스냅샷 크기)
- Modify: `src/ui/panel.rs` (`place_dock`의 `content_rect` 인자, `update_dock_glass`의 글자 사각형 제거)

**Interfaces:**
- Produces: `dock_layout::CREDIT_W`, `dock_layout::CREDIT_H`, `dock_layout::credit_len(edge) -> f32`, `DockLayout.credit: f32`, `Magnified.credit: f32` (판 중심 기준 글자 칸 가운데 along), `content_rect(bar, edge, headroom, side) -> PxRect` (footer 인자 제거), `scene::credit_rect(layout, mag, center) -> RectF`.

- [ ] **Step 1: 실패하는 테스트** — `dock_layout.rs` 테스트 모듈에 추가

```rust
    #[test]
    fn credit_sits_inside_the_bar_after_the_last_slot() {
        let l = DockLayout::compute(DockEdge::Bottom, 2, false);
        let expected = PAD * 2.0 + 2.0 * ICON + GAP + GAP + CREDIT_W;
        assert!((l.length - expected).abs() < 0.01, "{} vs {expected}", l.length);
        assert!(l.credit > l.rest[1] + ICON / 2.0);
        assert!(l.credit + CREDIT_W / 2.0 <= l.length / 2.0 - PAD + 0.01);
        let m = l.magnified(None, 0.0);
        assert_eq!(l.hit(&m, l.credit, THICK / 2.0), Hit::Empty);
        // 수정 모드에서도 맨 끝 (완료 단추 뒤)
        let e = DockLayout::compute(DockEdge::Left, 2, true);
        assert!(e.credit > *e.rest.last().unwrap());
        assert_eq!(credit_len(DockEdge::Left), CREDIT_H);
        assert_eq!(credit_len(DockEdge::Bottom), CREDIT_W);
    }
```

- [ ] **Step 2: 실행해 실패 확인** — `cargo test credit_sits_inside` → 컴파일 오류(`CREDIT_W`, `credit` 없음).

- [ ] **Step 3: 구현** — `dock_layout.rs`

```rust
/// 판 끝 "by ssenu" 글자 칸: 가로 독은 폭, 세로 독은 높이 (DIP)
pub const CREDIT_W: f32 = 36.0;
pub const CREDIT_H: f32 = 30.0;

/// 이 가장자리 독에서 글자 칸이 판 길이 방향으로 차지하는 길이
pub fn credit_len(edge: DockEdge) -> f32 {
    if is_vertical(edge) { CREDIT_H } else { CREDIT_W }
}
```

`DockLayout`·`Magnified`에 `pub credit: f32` (판 중심 기준 글자 칸 가운데). `place`에 `tail: f32`를 더해 마지막 칸 뒤 `GAP` 다음에 글자 칸을 두고 길이에 포함:

```rust
fn place(widths: &[f32], split: Option<usize>, tail: f32) -> (Vec<f32>, f32, Option<f32>, f32) {
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
    let credit = x + tail / 2.0;
    let length = x + tail + PAD;
    let half = length / 2.0;
    (centers.iter().map(|c| c - half).collect(), length, separator.map(|s| s - half), credit - half)
}
```

`compute`/`magnified`는 `place(&widths, self.split(), credit_len(edge))`의 네 번째 값을 `credit`에 넣는다. `edge_margin`은 `MARGIN`만 돌려주게(아래 독 추가 여백 제거), `content_rect`는 `footer` 인자를 지우고 Bottom을 `(x - side, y - headroom, w + 2 * side, h + headroom)`로. 기존 테스트의 `content_rect(..., 14)` 인자를 지우고 Bottom 기대값을 `(670, 854, 580, 178)`로, `edge_margin` 비교 단언을 `assert_eq!(edge_margin(DockEdge::Bottom), MARGIN);`로, 길이 기대값이 있는 테스트(`rest_layout_is_centered_and_holds_only_tiles`)는 `+ GAP + CREDIT_W`를 더한다.

`scene.rs`: 판 아래 알약을 지우고 판 안 글자 칸으로

```rust
/// 판 끝 "by ssenu" 글자 칸 (콘텐츠 창 기준 DIP)
pub fn credit_rect(layout: &DockLayout, mag: &Magnified, center: (f32, f32)) -> RectF {
    let bar = layout.bar_rect(mag, center);
    if dock_layout::is_vertical(layout.edge) {
        RectF::new(bar.x, center.1 + mag.credit - dock_layout::CREDIT_H / 2.0, bar.w, dock_layout::CREDIT_H)
    } else {
        RectF::new(center.0 + mag.credit - dock_layout::CREDIT_W / 2.0, bar.y, dock_layout::CREDIT_W, bar.h)
    }
}

fn draw_credit(g: &Gfx, r: RectF, p: &Palette) {
    let c = p.subtext.with_alpha(0.55);
    let line = 11.0;
    let top = r.center().y - line;
    g.text("by", RectF::new(r.x, top, r.w, line), 9.0, Weight::SemiBold, Align::Center, c);
    g.text("ssenu", RectF::new(r.x, top + line, r.w, line), 9.0, Weight::SemiBold, Align::Center, c);
}
```

`draw_dock`에서 `draw_credit(g, credit_rect(s.layout, s.mag, s.center), p);`로 바꾸고, `edit_hint_rect`의 Top 경우 `bar.bottom() + 10.0`, `CREDIT` 상수는 지운다. 스냅샷 테스트의 `+ dock_layout::CREDIT_H`(창 높이·중심) 두 곳을 지운다.

`panel.rs`: `place_dock`의 `content_rect` 마지막 인자 `px(dock_layout::CREDIT_H)` 삭제, `update_dock_glass`의 `rects.push(px(scene::credit_rect(gfx, bar)));` 삭제.

- [ ] **Step 4: 통과 확인** — `cargo test` 전체 PASS, `target/snapshots/dock-bottom.png`·`dock-left.png`에서 판 끝 연한 두 줄 확인.

- [ ] **Step 5: 커밋**

```bash
git add src/ui/dock_layout.rs src/ui/scene.rs src/ui/panel.rs
git commit -m "style: by ssenu를 독 판 안 끝에 연한 글씨로"
```

### Task 2: 말풍선 깨짐 — 영역 2단계 + 페이드 + 픽셀 맞춤

**Files:**
- Modify: `src/ui/dock_layout.rs` (`region_active`)
- Modify: `src/ui/scene.rs` (`snap`, `draw_tooltip_box`에 불투명도·배율, `DockScene`/`SceneState`에 `px_scale`, `DockScene`에 `tooltip_alpha`)
- Modify: `src/ui/panel.rs` (`Dock.inside`, `tip: Anim`, `tip_slot`, `update_dock_glass` 영역, `tick`, 마우스 처리, 렌더 인자)

**Interfaces:**
- Consumes: Task 1의 `content_rect`, `credit_rect`.
- Produces: `dock_layout::region_active(inside: bool, busy: bool, amount: f32, tooltip: f32) -> bool`, `scene::snap(r: RectF, scale: f32) -> RectF`.

- [ ] **Step 1: 실패하는 테스트**

`dock_layout.rs`:

```rust
    #[test]
    fn region_grows_on_hover_and_shrinks_only_when_settled() {
        assert!(region_active(true, false, 0.0, 0.0), "마우스가 위에 있으면 활성");
        assert!(region_active(false, false, 0.3, 0.0), "확대가 남아 있으면 활성");
        assert!(region_active(false, false, 0.0, 0.2), "말풍선이 사라지는 중이면 활성");
        assert!(region_active(false, true, 0.0, 0.0), "끌기·옮기기 중이면 활성");
        assert!(!region_active(false, false, 0.0, 0.0), "모두 끝나면 쉼");
    }
```

`scene.rs`:

```rust
    #[test]
    fn snap_aligns_rect_to_device_pixels() {
        let r = snap(RectF::new(10.3, 20.6, 50.2, 26.1), 1.5);
        for v in [r.x, r.y, r.right(), r.bottom()] {
            assert!(((v * 1.5) - (v * 1.5).round()).abs() < 1e-4, "{v}");
        }
        assert_eq!(snap(RectF::new(1.0, 2.0, 3.0, 4.0), 1.0), RectF::new(1.0, 2.0, 3.0, 4.0));
    }
```

- [ ] **Step 2: 실패 확인** — `cargo test region_grows snap_aligns` → 컴파일 오류.

- [ ] **Step 3: 구현**

`dock_layout.rs`:

```rust
/// 독 콘텐츠 창 표시 영역을 넓혀 둘지: 마우스가 위에 있거나, 끌기 중이거나, 확대·말풍선이 남아 있으면.
/// 매 프레임 영역을 바꾸면 새 그림과 한 프레임씩 어긋나 말풍선이 잘리므로 두 상태로만 바꾼다.
pub fn region_active(inside: bool, busy: bool, amount: f32, tooltip: f32) -> bool {
    inside || busy || amount > 0.0 || tooltip > 0.0
}
```

`scene.rs`:

```rust
/// 사각형을 실제 화면 픽셀 경계에 맞춘다 (`scale` = DIP → px 배율)
pub fn snap(r: RectF, scale: f32) -> RectF {
    let f = |v: f32| (v * scale).round() / scale;
    let (x, y) = (f(r.x), f(r.y));
    RectF::new(x, y, f(r.right()) - x, f(r.bottom()) - y)
}

/// 말풍선 상자. `alpha`(0..1)는 페이드, `scale`은 픽셀 맞춤용 배율.
fn draw_tooltip_box(g: &Gfx, text: &str, r: RectF, alpha: f32, scale: f32) {
    let r = snap(r, scale);
    let half = 0.5 / scale;
    g.fill_round_rect(r, 8.0, TOOLTIP_BG.with_alpha(TOOLTIP_BG.a * alpha));
    let edge = RectF::new(r.x + half, r.y + half, r.w - 2.0 * half, r.h - 2.0 * half);
    g.stroke_round_rect(edge, 8.0 - half, Color::hex(0xFFFFFF, 0.14 * alpha), 1.0 / scale);
    let inner = RectF::new(r.x + 10.0, r.y, r.w - 20.0, r.h);
    g.text_block(text, inner, 12.0, Weight::SemiBold, Align::Center, TOOLTIP_TEXT.with_alpha(alpha));
}
```

`SceneState`·`DockScene`에 `pub px_scale: f32`, `DockScene`에 `pub tooltip_alpha: f32`. `draw_tooltip`·`draw_tooltip_toward`에 `scale` 인자를 더해 `draw_tooltip_box(..., 1.0, scale)`, 독 말풍선은 `draw_tooltip_box(g, text, rect, s.tooltip_alpha, s.px_scale)`, 수정 안내는 `(..., 1.0, s.px_scale)`. 테스트 장면들은 `px_scale: 2.0`, `tooltip_alpha: 1.0`.

`panel.rs`:
- `Dock`에 `inside: bool`(마우스가 판 위). `Panel`에 `tip: Anim`, `tip_slot: Option<usize>`.
- `on_mouse_move`(독 모드): `self.dock.inside = true;`. `on_mouse_leave`: `self.dock.inside = false; self.start_animation();`.
- `tick`: 독 모드에서

```rust
            let target = self.dock_tooltip();
            if let Some(i) = target {
                self.tip_slot = Some(i);
            }
            self.tip.set(if target.is_some() { 1.0 } else { 0.0 });
            moving |= self.tip.step(dt);
            if self.tip.value == 0.0 && self.tip.target == 0.0 {
                self.tip_slot = None;
            }
```

- 수정 모드 단추 호버(`on_mouse_move`의 수정 모드 분기)는 즉시 그리기 대신 `self.set_hover(hit)`로 애니메이션을 타게 한다.
- `update_dock_glass` 영역:

```rust
        let busy = self.drag.is_some() || self.dock.moving.is_some();
        let active = dock_layout::region_active(self.dock.inside, busy, self.dock.amount.value, self.tip.value);
        let rects = if active {
            let (_, _, w, h) = self.dock.content_px;
            vec![(0, 0, w, h)]
        } else {
            let mut v = vec![px(bar)];
            if self.dock.editing && let Some(gfx) = &self.gfx {
                let r = scene::edit_hint_rect(gfx, bar, self.dock.edge, self.dock_size());
                v.push(px(RectF::new(r.x - 2.0, r.y - 2.0, r.w + 4.0, r.h + 4.0)));
            }
            v
        };
```

  (`outward`·`grow`·`extra`·`dock_tooltip_rect` 영역 코드와 `TOOLTIP_ROOM` 상수, `fn dock_tooltip_rect` 삭제.)
- 렌더: 독 장면 `tooltip: self.tip_slot.filter(|&i| i < self.dock.layout.slots().len() && self.tip.value > 0.0)`, `tooltip_alpha: self.tip.value`, `px_scale: self.dock_scale()`; 패널 장면 `px_scale: self.scale`.

- [ ] **Step 4: 통과 확인** — `cargo test` 전체 PASS, `cargo clippy --all-targets -- -D warnings` 통과, 스냅샷에서 말풍선 모서리·테두리 확인.

- [ ] **Step 5: 커밋**

```bash
git add src/ui/dock_layout.rs src/ui/scene.rs src/ui/panel.rs
git commit -m "fix: 독 말풍선 깜빡임·잘림·거친 모서리 (영역 2단계, 페이드, 픽셀 맞춤)"
```

### Task 3: 트레이 메뉴에서 독 위치 4줄 제거

**Files:**
- Modify: `src/app.rs` (`CMD_EDGE_*`, `set_edge`, `menu_entries`, `show_menu`, 테스트)
- Modify: `src/ui/panel.rs` (`PanelCmd::SetDockPlacement`와 처리 삭제)

**Interfaces:**
- Produces: 없음 (삭제).

- [ ] **Step 1: 실패하는 테스트** — `app.rs` 테스트에 추가 (메뉴 글자만 보는 순수 도우미가 필요하므로 `menu_labels`를 테스트에서 `App` 없이 확인할 수 있게, 독 모드 항목 목록을 만드는 순수 함수 `mode_entries(dock: bool, editing: bool, locked: bool) -> Vec<MenuEntry>`로 분리)

```rust
    #[test]
    fn dock_menu_has_edit_mode_but_no_edge_items() {
        let texts: Vec<String> = mode_entries(true, false, false)
            .into_iter()
            .filter_map(|e| match e {
                MenuEntry::Item { text, .. } => Some(text),
                _ => None,
            })
            .collect();
        assert!(texts.iter().any(|t| t.contains("독 수정 모드")));
        assert!(!texts.iter().any(|t| t.contains("독 위치")), "{texts:?}");
        let panel: Vec<_> = mode_entries(false, false, true);
        assert!(panel.iter().any(|e| matches!(e, MenuEntry::Item { text, .. } if text.contains("위치 잠금"))));
    }
```

- [ ] **Step 2: 실패 확인** — `cargo test dock_menu_has_edit_mode` → 컴파일 오류(`mode_entries` 없음).

- [ ] **Step 3: 구현** — `app.rs`

```rust
/// 모드 부분 메뉴 (순수 함수): 독 모드 아래에는 독 수정 모드만, 패널 모드면 위치 초기화·잠금
fn mode_entries(dock: bool, editing: bool, locked: bool) -> Vec<MenuEntry> {
    let mut entries = vec![
        MenuEntry::checked(CMD_MODE_DOCK, "독 모드", dock),
        MenuEntry::checked(CMD_DOCK_EDIT, "    독 수정 모드 (옮기기·크기·순서)", dock && editing),
        MenuEntry::checked(CMD_MODE_PANEL, "패널 모드", !dock),
    ];
    if !dock {
        entries.extend([
            MenuEntry::item(CMD_RESET_GEOMETRY, "위치·크기 초기화"),
            MenuEntry::checked(CMD_LOCK, "위치 잠금", locked),
        ]);
    }
    entries
}
```

`menu_entries`에서 모드 세 줄과 `if dock { ...독 위치... } else { ... }` 블록을 `entries.extend(mode_entries(dock, self.dock_editing, self.config.locked));`로 바꾸고, 쓰이지 않게 된 `edge` 변수, `CMD_EDGE_*` 상수 넷, `show_menu`의 `CMD_EDGE_*` 처리, `fn set_edge`를 지운다. `panel.rs`의 `PanelCmd::SetDockPlacement` 변형과 `apply`의 처리 분기를 지운다.

- [ ] **Step 4: 통과 확인** — `cargo test`, `cargo clippy --all-targets -- -D warnings`.

- [ ] **Step 5: 커밋**

```bash
git add src/app.rs src/ui/panel.rs
git commit -m "feat: 트레이 메뉴에서 독 위치 4줄 제거 (독 수정 모드에서 끌어 옮기기)"
```

### Task 4: 알림 이름 — 버전 정보 + 앱 ID

**Files:**
- Modify: `build.rs` (버전 정보가 든 리소스 스크립트를 `OUT_DIR`에 만들어 컴파일)
- Delete: `app.rc` (내용은 `build.rs`가 만든다)
- Modify: `src/app.rs` (`APP_ID`), `src/main.rs` (앱 ID 설정), `installer/setup.iss` (`AppUserModelID`)

**Interfaces:**
- Produces: `app::APP_ID: &str = "ssenu.DockUI"`.

- [ ] **Step 1: 구현** — `build.rs`

```rust
//! exe 리소스: 아이콘(1번), 매니페스트(DPI 인식·공용 컨트롤 v6), 버전 정보(알림·속성에 보이는 이름 "Dock UI").

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=assets/app.manifest");
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap().replace('\\', "/");
    let num = |k: &str| std::env::var(k).unwrap().parse::<u32>().unwrap_or(0);
    let (major, minor, patch) =
        (num("CARGO_PKG_VERSION_MAJOR"), num("CARGO_PKG_VERSION_MINOR"), num("CARGO_PKG_VERSION_PATCH"));
    let version = format!("{major}.{minor}.{patch}");
    let rc = format!(
        r#"#pragma code_page(65001)
1 ICON "{dir}/assets/icon.ico"
1 24 "{dir}/assets/app.manifest"
1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "041204b0"
    BEGIN
      VALUE "CompanyName", "ssenu"
      VALUE "FileDescription", "Dock UI"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "YTLauncher"
      VALUE "OriginalFilename", "YTLauncher.exe"
      VALUE "ProductName", "Dock UI"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x0412, 1200
  END
END
"#
    );
    let out = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("app.rc");
    std::fs::write(&out, rc).unwrap();
    embed_resource::compile(&out, embed_resource::NONE).manifest_required().unwrap();
}
```

`app.rc` 삭제(`git rm app.rc`).

`src/app.rs`:

```rust
/// 알림·작업 표시줄이 이 앱을 알아보는 ID (설치 바로가기에도 같은 값)
pub const APP_ID: &str = "ssenu.DockUI";
```

`src/main.rs` — 단일 인스턴스 확인 뒤, OLE 초기화 앞에:

```rust
    // 알림 머리글에 설치 바로가기 이름(Dock UI)·아이콘이 쓰이게 앱 ID를 고정한다
    let _ = unsafe {
        windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(&windows::core::HSTRING::from(app::APP_ID))
    };
```

`installer/setup.iss` `[Icons]` 두 줄 끝에 `; AppUserModelID: "ssenu.DockUI"`.

- [ ] **Step 2: 확인** — `cargo build --release` 후

Run: `powershell -NoProfile -Command "(Get-Item target/release/YTLauncher.exe).VersionInfo | Format-List ProductName,FileDescription,ProductVersion"`
Expected: `ProductName : Dock UI`, `FileDescription : Dock UI`, `ProductVersion : <Cargo.toml 버전>`

그리고 `cargo test` 전체 PASS (아이콘·매니페스트가 그대로 들어가 창 테스트가 돈다).

- [ ] **Step 3: 커밋**

```bash
git add build.rs src/app.rs src/main.rs installer/setup.iss
git rm app.rc
git commit -m "feat: 알림·속성 이름 Dock UI (exe 버전 정보, 앱 ID ssenu.DockUI)"
```

### Task 5: 문서

**Files:**
- Modify: `README.md`, `docs/manual-test-checklist.md`

- [ ] **Step 1: 수정** — README 독 모드 줄의 `판 아래에 \`by ssenu\`.`를 `판 끝에 연한 \`by ssenu\`.`로. 점검표 "표시" 절의 by ssenu 줄을 `독 판 안 끝(마지막 아이콘 뒤)에 연한 "by ssenu" 두 줄`로, "독 수정 모드" 절의 `메뉴의 독 위치(아래·위·왼쪽·오른쪽)를 고르면 그 가장자리 가운데로 간다` 줄을 `트레이 메뉴에 독 위치(아래·위·왼쪽·오른쪽) 줄이 없다`로 바꾸고, 다음 두 줄을 "표시" 절에 추가:

```markdown
- [ ] 독 위에서 마우스를 좌우로 빠르게 움직여도 이름 말풍선이 잘리거나 잔상·깜빡임 없이 부드럽게 나타나고 사라진다 (125%·150% 배율에서도 모서리·테두리 또렷)
- [ ] 업데이트·끌어 놓기 알림 머리글이 "Dock UI" (설치한 뒤 첫 알림부터)
```

- [ ] **Step 2: 커밋**

```bash
git add README.md docs/manual-test-checklist.md
git commit -m "docs: 독 다듬기 점검 항목"
```
