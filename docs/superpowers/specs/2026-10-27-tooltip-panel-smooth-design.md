# 한 줄 말풍선, 독 by ssenu 제거, 패널 팔레트·크기 정리, 부드러움

작성: 2026-10-27 · 대상 버전: 0.2.6

## 목표

1. 독 아이콘 이름 말풍선을 줄바꿈 없이 **한 줄**로. 오른쪽 독이면 왼쪽으로, 아래 독이면 위로 길게.
2. 독 판 끝 `by ssenu` 글자 칸 제거 (패널 머리글 `by_ssenu`는 유지).
3. 패널 이름 말풍선도 한 줄. 패널 폭보다 길면 **패널 밖으로** 나간다.
4. 패널 오른쪽 아래 팔레트(테마) 버튼과 색 원 선택기 제거. 테마는 트레이·⋯ 메뉴 "테마 ▸".
5. 패널 아래 여백을 양옆과 같게, 테두리를 끌어 줄일 때 모든 아이콘이 보이는 높이 아래로는 줄지 않게.
6. 독 확대·패널 마우스 오버·패널 크기 조절·이동을 부드럽게. 독은 판이 늘어나는 지금 방식 유지.

## 1. 한 줄 말풍선 (독·패널 공용)

- `tooltip_size`: 글자 너비 + 좌우 여백 10씩, 높이 26 고정. 넓이 상한은 `room`(그릴 수 있는 폭)뿐이고, 넘치면 한 줄 그대로 끝을 `…`(기존 `text` 말줄임).
- 말풍선 상자는 `text`(한 줄)로 그린다. 여러 줄 코드(`text_block`, `measure_block`, `block_format`, `TOOLTIP_MAX_W`)는 지운다.
- 가장 긴 말풍선 폭 `longest_tip` = 항목 이름(과 "항목 추가") 중 가장 긴 `measure + 20`, 작업 영역 폭으로 상한. 화면 쪽이 계산한다(그리기 장치가 있을 때 `measure`, 없으면 200).
- 독 여유 공간: 세로 독 `headroom = ICON*MAGNIFY + 8 + longest_tip + 8` (기존 `HEADROOM`보다 작으면 `HEADROOM`), 가로 독 `side_room = max(SIDE_ROOM, (longest_tip + 16 - max_length) / 2)`. `DockLayout::headroom`·`side_room`이 `longest_tip`을 인자로 받는다.

## 2. 독 by ssenu 제거

`CREDIT_W`, `CREDIT_H`, `credit_len`, `DockLayout.credit`, `Magnified.credit`, `place`의 `tail`, `credit_rect`, `draw_credit`를 지운다. 판 길이 기대값 테스트는 글자 칸 없이 되돌린다.

## 3. 패널 말풍선을 패널 밖까지

- 패널 콘텐츠 창 = 패널 사각형을 좌우로 `side_px`씩 넓힌 것. `side_px = ceil(max(0, longest_tip + 16 - 패널 폭) / 2 * 배율)` (패널 폭이 충분하면 0).
- 패널 장면은 콘텐츠 창 안 `(side, 0)`에서 시작하게 그린다: `Gfx::set_offset(dx, dy)`(Direct2D 변환)로 옮긴다.
- 표시 영역: 쉼 = 패널 사각형, 말풍선이 보이는 동안 = 패널 + 말풍선 사각형. 말풍선이 나타나고 사라질 때만 바뀐다.
- 패널 말풍선도 `Anim` 페이드(약 120ms)와 픽셀 맞춤. 패널 마우스 판정·드래그 좌표는 지금처럼 유리 창 기준이므로 바뀌지 않는다.

## 4. 패널 팔레트 제거

`PanelLayout.palette_button`, `swatch_centers`, `fan_points`, `PALETTE_BUTTON`, `PICKER_RADIUS`, `SWATCH`, `Hit::Palette`, `draw_palette_button`, `draw_round_button`, `draw_swatches`, `draw_picker`, `draw_tooltip_toward`, `Panel`의 `picker_open`·`picker`·`picker_hover`·`swatch_at`·`close_picker`, `SceneState`의 `palette_hover`·`picker`·`picker_hover`, `UiEvent::ThemePicked`(다른 곳에서 쓰지 않으면)를 지운다. 테마 메뉴는 그대로.

## 5. 패널 크기

- `PanelLayout::min_height(width, items) -> f32`: 그리드 위 + 줄 수 × (아이콘 + 간격) − 간격 + 마우스 오버로 커지는 몫 + 아래 여백(`PADDING`, 양옆과 같음). 줄 수는 지금 너비의 열 수로 계산.
- `WM_SIZING`에서 제안된 사각형의 높이가 `max(MIN_H, min_height(폭))`보다 작으면 끄는 쪽 가장자리(위/아래)를 늘려 맞춘다. `WM_GETMINMAXINFO`의 최소 높이도 같은 값(지금 폭 기준).

## 6. 부드럽게

- **독 흐림 창**: 유리 창을 가장 커졌을 때 길이(`max_length`)로 놓을 때 한 번만 옮기고, 매 프레임은 `SetWindowRgn`으로 흐림 모양만 지금 판(둥근 사각형, 반지름 `WINDOW_RADIUS`)으로 바꾼다. 모양이 전과 같으면 건너뛴다. 클릭 판정도 같은 모양으로 잘린다(판 밖은 아래 창으로).
- **패널 크기·이동**: 유리 창의 `WM_WINDOWPOSCHANGING`에서 새 위치·크기로 콘텐츠 창을 함께 옮기고 그린다(지금은 옮겨진 뒤 `WM_WINDOWPOSCHANGED`에서 따라감).
- **그리기 버퍼**: `Gfx::resize`는 버퍼를 256px 단위로 올림해 잡고, 필요한 크기가 그 안이면 버퍼를 다시 만들지 않는다(콘텐츠 창 밖은 창 경계에서 잘린다).

## 테스트

- 단위: 한 줄 말풍선 폭(이름 길이에 비례, 상한 `room`), `longest_tip`에 따른 독 `headroom`·`side_room`과 패널 `side_px`, `PanelLayout::min_height`(열 수·줄 수별), 독 흐림 모양 사각형 = 판 사각형, 버퍼 올림 규칙(`buffer_size`).
- 스냅샷: 오른쪽 독(왼쪽으로 긴 한 줄 말풍선), 패널 밖으로 나간 말풍선, 팔레트 없는 패널.
- 수동(점검표): 독 확대·패널 마우스 오버·크기 조절·이동이 끊기지 않음, 판 끝 흐림이 판과 어긋나지 않음, 패널을 줄여도 아이콘이 가려지지 않음.
