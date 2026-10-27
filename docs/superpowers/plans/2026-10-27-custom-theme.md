# 말풍선 전체 길이·독 테두리·사용자 지정 테마 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 말풍선을 말줄임 없이 전체 길이로, 독 흐림 창의 최대 크기 테두리 제거, 사용자 지정 테마(배경색·투명도·흐림) 추가.

**Architecture:** 말풍선 크기는 `tooltip_size`(여유 폭 포함)와 말줄임 없는 글자 그리기로. 독/패널 창 외곽 처리는 `dwm::set_frame`. 사용자 지정 테마는 설정(`CustomTheme`) → 순수 색표 함수(`custom_palette`) → 화면(`Panel::theme_palette`, 흐림 켜기/끄기) → 전용 설정 창(`theme_form` 그림 + `theme_dialog` 창) → 앱 메뉴·미리보기 흐름.

**Tech Stack:** Rust 2024, `windows` 0.62 (Direct2D·DirectWrite·DWM·Win32 공용 대화상자 `ChooseColorW`), serde.

## Global Constraints

- 사용자 지정 기본값: `#1E1E1E`, 불투명도 60%, 흐림 켜기. 추천 색 `1E1E1E 2B2D42 1F3A5F 0F4C5C 2D6A4F 5C3D2E 6B2737 4A3B6B F5F5F5 F4ECD8 DDE7F0 FFE4E1`.
- 글자색 자동: 밝기 `L = 0.2126 r + 0.7152 g + 0.0722 b`, `L < 0.55`면 흰 글자(`#FFFFFF`), 아니면 `#1F232B`.
- 설정 창 380 × 356 DIP, 항목 추가 창과 같은 그림 방식.
- 말풍선 폭 = `ceil(measure + 24)`, 높이 26, 말줄임 없음.
- 독 창: `DWMWCP_DONOTROUND` + `DWMWA_BORDER_COLOR = 0xFFFFFFFE`, 패널 창: `DWMWCP_ROUND` + `0xFFFFFFFF`.
- 검사: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.

---

### Task 1: 말풍선 전체 길이

**Files:** `src/ui/scene.rs`, `src/ui/gfx.rs`, `src/ui/panel.rs`

**Interfaces:** Produces `scene::tooltip_size(g, text) -> (f32, f32)`, `scene::longest_tip(g, labels) -> f32`, `Gfx::text_untrimmed(text, r, size, weight, align, c)`.

- [ ] **Step 1: 테스트** — `tooltip_is_one_line_as_wide_as_the_name`를

```rust
        let short = tooltip_size(&g, "뉴스");
        let long = tooltip_size(&g, "아침 뉴스 라이브 - 오늘의 주요 소식 모아 보기 (전체 재생목록) 경제·사회·국제");
        assert_eq!((short.1, long.1), (26.0, 26.0));
        assert!(long.0 > 360.0, "{long:?}");
        // 여백 10×2에 반올림 여유가 있어야 175% 등에서 글자 칸이 글자보다 좁아지지 않는다
        let text_w = g.measure("뉴스", 12.0, Weight::SemiBold);
        assert!(short.0 >= text_w + 2.0 * (10.0 * 1.75f32).round() / 1.75);
        assert!(longest_tip(&g, &["뉴스", "아주 긴 이름 아주 긴 이름"]) > short.0);
```

- [ ] **Step 2: 실행** — 컴파일 오류(인자 수).
- [ ] **Step 3: 구현**

```rust
/// 말풍선 크기 (너비, 높이): 한 줄, 이름 전체. 여백 10×2 + 픽셀 맞춤 반올림 여유 4
pub fn tooltip_size(g: &Gfx, text: &str) -> (f32, f32) {
    ((g.measure(text, 12.0, Weight::SemiBold) + 24.0).ceil(), 26.0)
}

/// 이 이름들 중 가장 긴 말풍선 폭
pub fn longest_tip(g: &Gfx, labels: &[&str]) -> f32 {
    labels.iter().map(|t| tooltip_size(g, t).0).fold(0.0, f32::max)
}
```

`draw_tooltip_box`는 `g.text_untrimmed(...)`. `gfx.rs`: 형식 캐시 키에 `trim: bool`을 더해 `false`면 `DWRITE_TRIMMING_GRANULARITY_NONE`(줄바꿈 없음)으로 만들고, `text_untrimmed`는 그 형식으로 그린다(쓰지 않게 된 `block` 갈래는 지운다). `dock_tooltip_rect`·`panel_tooltip_rect`는 `tooltip_size(g, text)`를 쓰고 범위 안으로만 옮긴다. `panel.rs`의 `longest_tip()`은 상한 없이 `scene::longest_tip(gfx, &labels)`, `place_dock`의 `/ zoom_for(...)` 제거.
- [ ] **Step 4: 확인** — `cargo test`, clippy.
- [ ] **Step 5: 커밋** — `git commit -am "fix: 말풍선 이름이 … 으로 잘리던 문제 (여유 폭, 말줄임 없음, 독 배율 계산)"`

### Task 2: 독 최대 크기 테두리 제거

**Files:** `src/win/dwm.rs`, `src/ui/panel.rs`

- [ ] **Step 1: 구현** — `dwm.rs`

```rust
const DWMWA_BORDER_COLOR: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(34);
const COLOR_NONE: u32 = 0xFFFF_FFFE;
const COLOR_DEFAULT: u32 = 0xFFFF_FFFF;

/// 창 외곽: 패널은 둥근 모서리 + 기본 테두리, 독은 모서리·테두리 없음
/// (독 흐림 창은 가장 커졌을 때 크기로 고정해 두므로 외곽선이 보이면 안 된다)
pub fn set_frame(hwnd: HWND, panel: bool) {
    let corner = if panel { DWMWCP_ROUND } else { DWMWCP_DONOTROUND };
    set_attr(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &corner);
    set_attr(hwnd, DWMWA_BORDER_COLOR, &(if panel { COLOR_DEFAULT } else { COLOR_NONE }));
}
```

`panel.rs` `place_windows`: 패널 갈래 처음에 `dwm::set_frame(self.glass, true)`, 독 갈래 처음에 `dwm::set_frame(self.glass, false)`.
- [ ] **Step 2: 확인** — `cargo build`, clippy.
- [ ] **Step 3: 커밋** — `git commit -am "fix: 독 흐림 창의 최대 크기 테두리 제거"`

### Task 3: 사용자 지정 테마 데이터·색표

**Files:** `src/config.rs`, `src/ui/theme.rs`

**Interfaces:** Produces `config::Theme::Custom`, `config::CustomTheme { color: u32, opacity: u8, blur: bool }` (+ `Default`), `Config.custom_theme`, `theme::custom_palette(&CustomTheme) -> Palette`, `theme::parse_hex(&str) -> Option<u32>`, `theme::to_hex(u32) -> String`, `theme::PRESETS: [u32; 12]`.

- [ ] **Step 1: 테스트** — `theme.rs`

```rust
    #[test]
    fn custom_palette_picks_readable_text() {
        let dark = custom_palette(&CustomTheme { color: 0x1E1E1E, opacity: 60, blur: true });
        assert_eq!(dark.text, Color::hex(0xFFFFFF, 1.0));
        assert!(dark.dark_backdrop);
        assert!((dark.tint.a - 0.6).abs() < 1e-6);
        let light = custom_palette(&CustomTheme { color: 0xF4ECD8, opacity: 90, blur: false });
        assert_eq!(light.text, Color::hex(0x1F232B, 1.0));
        assert!(!light.dark_backdrop);
    }

    #[test]
    fn hex_parses_short_long_and_bare() {
        assert_eq!(parse_hex("#1E1E1E"), Some(0x1E1E1E));
        assert_eq!(parse_hex("1e1e1e"), Some(0x1E1E1E));
        assert_eq!(parse_hex(" #abc "), Some(0xAABBCC));
        assert_eq!(parse_hex("#12345"), None);
        assert_eq!(parse_hex("zzzzzz"), None);
        assert_eq!(to_hex(0x0F4C5C), "#0F4C5C");
    }
```

`config.rs`

```rust
    #[test]
    fn custom_theme_round_trips_and_unknown_theme_falls_back() {
        let mut c = Config::default();
        c.theme = Theme::Custom;
        c.custom_theme = CustomTheme { color: 0x2D6A4F, opacity: 35, blur: false };
        let back: Config = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!((back.theme, back.custom_theme), (Theme::Custom, c.custom_theme));
        let old: Config = serde_json::from_str(r#"{"theme":"rainbow"}"#).unwrap();
        assert_eq!(old.theme, Theme::default());
        assert_eq!(old.custom_theme, CustomTheme::default());
    }
```

- [ ] **Step 2: 실행** — 컴파일 오류.
- [ ] **Step 3: 구현** — `config.rs`: `Theme`에 `Custom`(serde 이름 `custom`), 

```rust
/// 사용자 지정 테마: 바탕색(0xRRGGBB), 불투명도(0~100), 흐림
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomTheme {
    pub color: u32,
    pub opacity: u8,
    pub blur: bool,
}

impl Default for CustomTheme {
    fn default() -> Self {
        CustomTheme { color: 0x1E1E1E, opacity: 60, blur: true }
    }
}
```

`Config.custom_theme: CustomTheme`(기본값). `theme.rs`:

```rust
pub const PRESETS: [u32; 12] = [
    0x1E1E1E, 0x2B2D42, 0x1F3A5F, 0x0F4C5C, 0x2D6A4F, 0x5C3D2E, 0x6B2737, 0x4A3B6B, 0xF5F5F5, 0xF4ECD8, 0xDDE7F0, 0xFFE4E1,
];

/// 사용자 지정 색표: 글자·구분선은 바탕 밝기에 맞춰 자동
pub fn custom_palette(c: &CustomTheme) -> Palette {
    let bg = Color::hex(c.color, 1.0);
    let lum = 0.2126 * bg.r + 0.7152 * bg.g + 0.0722 * bg.b;
    let dark = lum < 0.55;
    let text = if dark { Color::hex(0xFFFFFF, 1.0) } else { Color::hex(0x1F232B, 1.0) };
    Palette {
        tint: bg.with_alpha(c.opacity.min(100) as f32 / 100.0),
        text,
        subtext: text.with_alpha(0.68),
        divider: text.with_alpha(if dark { 0.18 } else { 0.12 }),
        add_tile: text.with_alpha(if dark { 0.10 } else { 0.07 }),
        swatch: bg,
        dark_backdrop: dark,
    }
}

/// `#RGB`, `#RRGGBB` (`#` 없어도)
pub fn parse_hex(s: &str) -> Option<u32> {
    let h = s.trim().trim_start_matches('#');
    let full: String = match h.len() {
        3 => h.chars().flat_map(|c| [c, c]).collect(),
        6 => h.to_string(),
        _ => return None,
    };
    u32::from_str_radix(&full, 16).ok()
}

pub fn to_hex(rgb: u32) -> String {
    format!("#{:06X}", rgb & 0xFFFFFF)
}
```

`palette(Theme::Custom)`은 `custom_palette(&CustomTheme::default())`, `label(Theme::Custom)`은 `"사용자 지정"`. `PICKER_ORDER`(메뉴 내장 4개)는 그대로.
- [ ] **Step 4: 확인** — `cargo test`.
- [ ] **Step 5: 커밋** — `git commit -am "feat: 사용자 지정 테마 설정·색표"`

### Task 4: 화면에 사용자 지정 테마·흐림 끄기

**Files:** `src/ui/panel.rs`, `src/win/dwm.rs`

**Interfaces:** Consumes Task 3. Produces `PanelCmd::SetCustomTheme(CustomTheme)`, `OpenSpec.custom_theme`, `dwm::disable_blur(hwnd)`.

- [ ] **Step 1: 구현** — `dwm.rs`: `fn set_accent(hwnd, state: u32) -> bool`로 묶고 `pub fn disable_blur(hwnd: HWND) { set_accent(hwnd, 0); }`(0 = `ACCENT_DISABLED`). `panel.rs`:
  - 필드 `custom: CustomTheme`, `blur_on: bool`.
  - `fn theme_palette(&self, t: Theme) -> Palette { if t == Theme::Custom { theme::custom_palette(&self.custom) } else { theme::palette(t) } }` — `palette()`, `fade_from` 초기값, `set_dark` 호출이 이것을 쓴다.
  - `fn apply_backdrop(&mut self)`: `let want = self.theme != Theme::Custom || self.custom.blur;` 바뀌었으면, 켤 때 유리 창 확장 스타일에서 `WS_EX_LAYERED`를 빼고 `dwm::enable_acrylic`, 끌 때 `dwm::disable_blur` 후 `WS_EX_LAYERED`를 더하고 `SetLayeredWindowAttributes(glass, 0, 1, LWA_ALPHA)`. `blur_on = want`.
  - 장면에 넘기는 `acrylic`은 `self.acrylic || !self.blur_on` (흐림을 끈 사용자 지정은 고른 불투명도 그대로).
  - `PanelCmd::SetTheme`·`SetCustomTheme` 처리 끝에 `apply_backdrop()`. `SetCustomTheme(c)`: `fade_from = palette(); custom = c; fade` 시작.
- [ ] **Step 2: 확인** — `cargo build`, clippy, `cargo test`.
- [ ] **Step 3: 커밋** — `git commit -am "feat: 화면에 사용자 지정 테마와 흐림 끄기 적용"`

### Task 5: 설정 창

**Files:** Create `src/ui/theme_form.rs`, `src/ui/theme_dialog.rs`; Modify `src/ui/mod.rs`, `src/ui/dialogs.rs`(공용 도우미 `pub(crate)`)

**Interfaces:** Produces `theme_dialog::custom_theme_dialog(owner: HWND, initial: CustomTheme, on_change: &dyn Fn(CustomTheme)) -> Option<CustomTheme>`; `theme_form::{WIDTH, HEIGHT, Target, hit, opacity_at, draw, FormView}`.

- [ ] **Step 1: 테스트** — `theme_form.rs`

```rust
    #[test]
    fn hit_and_slider() {
        assert_eq!(hit(swatch_center(0)), Some(Target::Swatch(0)));
        assert_eq!(hit(swatch_center(11)), Some(Target::Swatch(11)));
        assert_eq!(hit(more_rect().center()), Some(Target::More));
        assert_eq!(hit(slider_rect().center()), Some(Target::Slider));
        assert_eq!(hit(switch_rect().center()), Some(Target::Blur));
        assert_eq!(hit(ok_rect().center()), Some(Target::Ok));
        let s = slider_rect();
        assert_eq!(opacity_at(s.x - 50.0), 0);
        assert_eq!(opacity_at(s.right() + 50.0), 100);
        assert_eq!(opacity_at(s.x + s.w / 2.0), 50);
    }
```

- [ ] **Step 2: 실행** — FAIL.
- [ ] **Step 3: 구현** — `theme_form.rs`: 배치 함수(`swatch_center(i)`, `hex_field()`, `more_rect()`, `slider_rect()`, `switch_rect()`, `ok_rect()`, `cancel_rect()`, `close_rect()`), `Target { Close, Swatch(usize), More, Slider, Blur, Ok, Cancel }`, `hit`, `opacity_at`, `draw(g, &FormView { theme, hover, focus_hex, dragging })` — 제목·설명, 추천 색 원(선택 색은 강조 링), HEX 칸 테두리, "더 많은 색…" 단추, 투명도 트랙·채움·손잡이·값, 흐림 스위치, 바닥 줄 취소/저장. `theme_dialog.rs`: `dialogs.rs`의 모달 루프·창 틀·EDIT 방식을 그대로 써서(`run_modal`, `create_window`, `child`, `gdi_font`, `Gfx::offscreen` + `copy_to_dc`) 마우스로 견본·슬라이더(누르고 끌기, `SetCapture`)·스위치·단추 처리, HEX `EN_CHANGE` → `parse_hex` 성공 시 적용, "더 많은 색…" → `ChooseColorW`(`CC_FULLOPEN | CC_RGBINIT`). 값이 바뀔 때마다 `on_change`. 저장 = `Some`, 취소·닫기 = `None`. 스냅샷 테스트 `theme-dialog.png`.
- [ ] **Step 4: 확인** — `cargo test`, clippy, 스냅샷.
- [ ] **Step 5: 커밋** — `git commit -am "feat: 사용자 지정 테마 설정 창"`

### Task 6: 메뉴·미리보기 흐름

**Files:** `src/app.rs`

- [ ] **Step 1: 테스트** — 메뉴 순수 함수 `theme_entries(current: Theme) -> Vec<MenuEntry>`

```rust
    #[test]
    fn theme_menu_has_custom_entries() {
        let texts: Vec<String> = theme_entries(Theme::Custom)
            .into_iter()
            .filter_map(|e| match e { MenuEntry::Item { text, checked, .. } => Some(format!("{text}{}", if checked { "✓" } else { "" })), _ => None })
            .collect();
        assert!(texts.contains(&"사용자 지정✓".to_string()), "{texts:?}");
        assert!(texts.iter().any(|t| t.starts_with("사용자 지정 설정")));
        assert!(texts.contains(&"다크".to_string()));
    }
```

- [ ] **Step 2: 구현** — `CMD_THEME_CUSTOM = 45`, `CMD_THEME_EDIT = 46`; `theme_entries`를 테마 하위 메뉴에 사용. `CMD_THEME_CUSTOM` → `set_theme(Theme::Custom)`. `CMD_THEME_EDIT` → `edit_custom_theme()`:

```rust
    fn edit_custom_theme(&mut self) {
        let (before_theme, before) = (self.config.theme, self.config.custom_theme);
        let panel = self.panel.clone();
        let preview = move |c: CustomTheme| {
            if let Some(p) = &panel {
                p.send(PanelCmd::SetCustomTheme(c));
                p.send(PanelCmd::SetTheme(Theme::Custom));
            }
        };
        match custom_theme_dialog(self.dialog_owner(), before, &preview) {
            Some(c) => {
                self.config.custom_theme = c;
                self.set_theme(Theme::Custom);
            }
            None => {
                self.panel_send(PanelCmd::SetCustomTheme(before));
                self.panel_send(PanelCmd::SetTheme(before_theme));
            }
        }
    }
```

  `set_theme`은 사용자 지정일 때 `SetCustomTheme(config.custom_theme)`도 보낸다. `OpenSpec.custom_theme = config.custom_theme`.
- [ ] **Step 3: 확인** — `cargo test`, clippy, `cargo build --release`.
- [ ] **Step 4: 커밋** — `git commit -am "feat: 테마 메뉴에 사용자 지정과 설정 창, 실시간 미리보기"`

### Task 7: 문서·릴리스

- [ ] README 테마 줄에 "사용자 지정(색·투명도·흐림)", 점검표에 사용자 지정·테두리·말풍선 항목.
- [ ] 커밋, 푸시, `v0.2.7` 태그, 워크플로 확인, 바뀐 점 정리.
