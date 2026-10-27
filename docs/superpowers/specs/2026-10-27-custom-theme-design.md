# 말풍선 전체 길이, 독 최대 크기 테두리 제거, 사용자 지정 테마

작성: 2026-10-27 · 대상 버전: 0.2.7 (글꼴 메모리 누수 수정 `f4a3cb2`와 함께 릴리스)

## 1. 말풍선: 생략 없이 전체 길이

원인 두 가지.
- 말풍선 안쪽 여백을 픽셀에 맞추며 반올림하면(`round(10 * scale) / scale`) 175% 등에서 10 DIP보다 커져, 글자 칸이 글자보다 1px 이하로 좁아지고 DirectWrite가 끝을 `…`로 바꾼다.
- 독 여유 공간 계산에서 이름 폭을 아이콘 크기 배율로 나누고(`longest_tip() / zoom`), 폭 상한(작업 영역)을 둔다.

바꾼다.
- `tooltip_size(g, text) -> (f32, f32)`: `room` 인자 제거, 너비 = `ceil(measure + 24)`(여백 10×2 + 반올림 여유 4), 높이 26.
- 말풍선 글자는 말줄임 없는 형식(`Gfx::text_untrimmed`, 줄바꿈 없음·자르기 없음)으로 그린다.
- `longest_tip(g, labels) -> f32`: 상한 없음. 독은 배율로 나누지 않는다(말풍선 글자도 독 배율로 그려지므로 독 DIP 그대로).
- 말풍선 위치는 그릴 수 있는 범위 안에서 고르되, 그래도 넘치면 잘린 채(창 경계) 그린다 — 이름은 줄이지 않는다.

## 2. 독 최대 크기 테두리 제거

흐림 창을 가장 커졌을 때 크기로 고정하면서, Windows 11이 그 창 외곽에 1px 테두리와 둥근 모서리를 그린다.
- `dwm::set_frame(hwnd, panel: bool)`: 독이면 `DWMWA_WINDOW_CORNER_PREFERENCE = DWMWCP_DONOTROUND`, `DWMWA_BORDER_COLOR = DWMWA_COLOR_NONE(0xFFFFFFFE)`. 패널이면 `DWMWCP_ROUND`, `DWMWA_COLOR_DEFAULT(0xFFFFFFFF)`.
- `place_windows`가 모드에 맞게 부른다.

## 3. 사용자 지정 테마

### 데이터
- `config::Theme`에 `Custom` 추가 (직렬화 `"custom"`, 예전 버전은 `lenient`로 기본 테마).
- `config::CustomTheme { color: u32 (0xRRGGBB), opacity: u8 (0..=100), blur: bool }`, 기본 `0x1E1E1E`, 60, true. `Config.custom_theme`.

### 색표 (A: 글자색 자동)
`theme::custom_palette(&CustomTheme) -> Palette`:
- 밝기 `L = (0.2126 r + 0.7152 g + 0.0722 b)` (0..1), `dark = L < 0.55`.
- `tint = color @ opacity/100`, `text = dark ? #FFFFFF : #1F232B`, `subtext = text @ 0.68`, `divider = text @ (dark ? 0.18 : 0.12)`, `add_tile = text @ (dark ? 0.10 : 0.07)`, `swatch = color @ 1`, `dark_backdrop = dark`.
- 화면은 `Panel::theme_palette(theme)`로 내장 테마와 사용자 지정을 고른다.

### 흐림 끄기
- 흐림을 끄면 유리 창의 아크릴을 끄고(`dwm::disable_blur`: 강조 정책 `ACCENT_DISABLED`), 유리 창을 레이어드 창(불투명도 1/255)으로 바꿔 검은 바탕이 보이지 않게 하되 클릭은 그대로 받는다. 켜면 레이어드를 풀고 아크릴을 다시 켠다.
- 흐림이 꺼진 사용자 지정 테마는 바탕 불투명도를 그대로 쓴다(아크릴 실패 시 쓰는 0.92로 올리지 않음).

### 메뉴
트레이·⋯ 메뉴 "테마 ▸": 라이트·다크·세피아·투명, 구분선, `사용자 지정`(지금이면 체크), `사용자 지정 설정…`.

### 설정 창 (`ui/theme_form.rs` 그림 + `ui/theme_dialog.rs` 창)
항목 추가 창과 같은 방식(직접 그린 바탕 + HEX 칸만 EDIT). 380 × 356 DIP.
- 제목 "사용자 지정 테마", 설명 "독과 패널 바탕을 원하는 색과 투명도로".
- 배경색: 추천 색 12개(2줄 × 6, 지름 28), 오른쪽에 HEX 칸(`#1E1E1E`)과 "더 많은 색…"(윈도우 색 선택 창 `ChooseColorW`).
- 투명도: 슬라이더(0~100%, 누르고 끌기), 오른쪽에 값.
- 흐림: 스위치(켜기/끄기)와 설명.
- 아래: 취소 / 저장. Enter 저장, Esc 취소.
- 바뀔 때마다 콜백으로 독·패널에 바로 적용(미리보기). 취소하면 열기 전 테마·사용자 지정 값으로 되돌린다. 저장하면 테마를 `Custom`으로 하고 설정 파일에 남긴다.
- 추천 색: `1E1E1E 2B2D42 1F3A5F 0F4C5C 2D6A4F 5C3D2E 6B2737 4A3B6B F5F5F5 F4ECD8 DDE7F0 FFE4E1`.

### 순수 함수
`theme::parse_hex(&str) -> Option<u32>` (`#RGB`, `#RRGGBB`, `#` 없이도), `theme::to_hex(u32) -> String`(`#1E1E1E`), `theme_form::opacity_at(x) -> u8`(슬라이더), `theme_form::hit(p) -> Option<Target>`.

## 테스트
- 단위: 말풍선 폭(여유 포함, 상한 없음), `custom_palette` 밝기별 글자색, `parse_hex`/`to_hex`, 설정 직렬화(사용자 지정 왕복, 모르는 테마 → 기본), 슬라이더 값, 설정 창 판정.
- 스냅샷: 설정 창, 사용자 지정 테마(밝은 색·어두운 색) 패널.
- 수동: 흐림 끄기 모습, 미리보기·취소 복원, 독 테두리 없음, 175%에서 말풍선에 `…` 없음.
