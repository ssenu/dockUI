# 독 아래 흐린 사각형 제거, 패널 아이콘 크기

작성: 2026-10-27 · 대상 버전: 0.2.8

## 1. 독 아래쪽 흐린 사각형

현상(실제 화면 캡처로 확인): 오른쪽 세로 독의 판(둥근 모서리) 아래 끝 밑으로, 판이 가장 커졌을 때 길이까지 모서리가 각진 흐린 띠(약 22px)가 남는다. 창 영역(`GetWindowRgnBox`)은 판 모양으로 맞게 잘려 있다.

실험(`dwm::tests::blur_region_probe`, 사용자 화면에서 캡처): 아크릴 흐림 창에 판 모양 영역(위 20px·아래 60px 잘라냄)을 걸면 `bRedraw`가 false든 true든 흐림은 영역을 무시하고 창 전체(400px)에 보인다. **아크릴 흐림은 창 영역(`SetWindowRgn`)을 따르지 않는다.** 0.2.6의 "흐림 창을 최대 크기로 고정하고 모양만 바꾸기"는 쓸 수 없다.

수정:
- 흐림(유리) 창을 다시 지금 판 크기에 맞춰 옮기고 크기를 바꾼다(예전 방식). 영역은 쓰지 않는다.
- 끊김을 줄이려고 화면 px로 바뀌었을 때만, `SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOSENDCHANGING | SWP_NOCOPYBITS`로 옮긴다.
- 흐림 창이 다시 판 크기이므로 둥근 모서리(`DWMWCP_ROUND`)를 켜서 판과 맞추고, 독에서는 테두리선만 끈다(`dwm::set_frame(hwnd, border)`).

## 2. 패널 아이콘 크기

- 설정 `Config.panel_icon_size: u32`, 기본 56(지금 크기). 독 크기(`dock_size`)와 따로.
- 메뉴(패널 모드일 때, 트레이·⋯ 공용): `아이콘 크기 ▸ 작게 (40) / 보통 (56) / 크게 (72) / 더 크게 (88)`, 지금 크기에 체크.
- `PanelLayout::compute(width, height, items, icon)`: 칸 크기 = `icon`. 열 수·간격(최소 16, 상한 28)·줄 수·`min_height(width, items, icon)`도 `icon` 기준. 머리글·글자 크기는 그대로. 타일 그림은 이미 칸 크기에 비례한다.
- 바꾸면 `PanelCmd::SetPanelIcon(size)` → 레이아웃 다시, 아래 줄이 가려지면 패널을 늘린다(기존 `SetTiles` 처리와 같은 규칙).

## 테스트
- 단위: 크기별 배치(칸 크기, 열 수, `min_height`가 클수록 큼), 메뉴 항목·체크, 설정 직렬화(기본 56, 왕복).
- 스냅샷: 아이콘 72 패널.
- 실제 화면: 1번 시험 창 캡처 비교.
