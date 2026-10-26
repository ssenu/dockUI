# YT Launcher 0.2 — Rust 독 런처 설계

- 작성일: 2026-10-26
- 저장소: https://github.com/ssenu/YT-launcher (작업 브랜치 `rust-rewrite`)
- 이전 설계: `2026-10-21-yt-launcher-design.md` (파이썬 0.1.x)
- 상태: 승인됨

## 1. 목표

0.1.x(파이썬 + PySide6)를 Rust로 다시 만들어 메모리를 줄이고(목표 10~20MB), 기능을 넓힌다.

- 항목 종류 3가지: **유튜브**(검색어 → 크롬 검색 결과), **앱**(PC에 설치된 앱 실행/전환), **웹 주소**(크롬 새 탭).
- 맥 독(Dock) 같은 디자인: 반투명 유리 배경 위에 아이콘을 나열하고, 마우스를 올리면 확대·강조.
- **독 모드**와 **패널 모드** 두 가지 배치를 설정에서 전환.
- 테마 4가지: 라이트, 다크, 세피아, 투명(검은빛 스모키 글래스).
- 0.1.x 사용자는 기존 원클릭 업데이트만으로 0.2.0으로 넘어오고 설정이 이어진다.

### 범위 밖 (YAGNI)

- 이미 열린 특정 크롬 탭으로 전환 (확장 프로그램 필요)
- 독 자동 숨김, 항상 위 표시 (독도 다른 창 뒤에 깔린다)
- 항목 종류 변경 (수정 창에서 종류 고정, 바꾸려면 삭제 후 추가)
- 코드 서명, 유튜브 썸네일, 그룹/폴더

## 2. 확정된 결정

| 항목 | 결정 |
|---|---|
| 언어/UI | Rust. 독·패널은 Win32 + DirectComposition + Direct2D로 직접 그림. 입력이 필요한 창(추가·수정, 앱 선택)은 Win32 기본 컨트롤 |
| 크레이트 | `windows`, `serde`, `serde_json`, `ureq`, `sha2` (+ 빌드용 `embed-resource`) |
| 모드 | 독 모드 / 패널 모드 둘 다, 설정에서 전환 |
| 겹침 | 두 모드 모두 다른 창 뒤에 깔림 (바탕화면이 보일 때 보임), 작업표시줄 아이콘 없음, 트레이만 |
| 앱 목록 | 시작 메뉴 "모든 앱"과 동일(`shell:AppsFolder`, 데스크톱 + Store 앱) + 검색 + exe/lnk 직접 찾기 |
| 실행 중 앱 클릭 | 실행 중이면 그 창을 복원·전면화, 아니면 실행. 실행 중이면 아이콘 아래 점 |
| 이름 표시 | 마우스를 올렸을 때 말풍선만. 유튜브 아이콘에는 검색어 두 글자 배지 |
| 투명 테마 | 검정 30% 덧칠 스모키 글래스 |
| 버전 | v0.2.0, 같은 저장소·AppId·exe 이름·릴리스 파일 이름 유지 |

## 3. 구조

```
Cargo.toml, build.rs           (아이콘 리소스 포함)
src/
├─ main.rs          진입점, 단일 인스턴스
├─ config.rs        설정 로드/저장, schema 1→2 변환
├─ items.rs         Item 모델, 입력 검증
├─ launch/
│  ├─ chrome.rs     크롬 탐색·실행·최대화·전면화
│  ├─ apps.rs       AppsFolder 열거, 앱 실행, 실행 중 창 탐지·전면화
│  └─ icons.rs      앱 아이콘 추출, 파비콘, 아이콘 캐시
├─ ui/
│  ├─ window.rs     유리 창 + 콘텐츠 창, DComp/D2D 렌더 루프, 바탕화면 레이어
│  ├─ layout.rs     독/패널 배치·확대 계산 (순수 함수)
│  ├─ dock.rs       아이콘·말풍선·점·배지 그리기, 입력, 드래그 정렬
│  ├─ theme.rs      테마 색표
│  ├─ picker.rs     부채꼴 테마 선택기
│  ├─ dialogs.rs    추가·수정 창
│  └─ app_list.rs   앱 선택 창
├─ tray.rs          트레이 아이콘·메뉴
├─ updater.rs       릴리스 확인·다운로드·해시 검증·조용한 설치
└─ autostart.rs     HKCU Run
```

원칙: `config`, `items`, `layout`, `updater`의 판단 로직과 `chrome`의 URL 생성은 Win32에 의존하지 않는 순수 Rust로 두고 `cargo test`로 검증한다. Win32/COM 호출은 얇은 래퍼에 모은다.

### 3.1 단일 인스턴스

- 명명된 뮤텍스 `YTLauncherMutex` (설치 프로그램의 실행 중 감지에도 사용). 이미 있으면 기존 인스턴스의 숨은 메시지 창(클래스 `YTLauncherMsg`)을 `FindWindow`로 찾아 `WM_APP_SHOW`를 보내고 종료.

## 4. 화면

### 4.1 창 구조

- **유리 창**: 판 크기와 같음. `WS_EX_TOOLWINDOW`, DWM 아크릴(`DWMWA_SYSTEMBACKDROP_TYPE=3`), 둥근 모서리(`DWMWCP_ROUND`, 약 8px, Windows가 정함), 테마 색 덧칠.
- **콘텐츠 창**: 유리 창이 소유자(owner)인 투명 창. `WS_EX_NOREDIRECTIONBITMAP` + DirectComposition + Direct2D 스왑체인(프리멀티플 알파). 아이콘 확대가 판 밖으로 나갈 수 있게 판보다 크다. 투명 영역 클릭은 뒤 창으로 통과해야 하므로 `SetWindowRgn`으로 창 영역을 "판 + 현재 확대된 아이콘·말풍선 영역"으로 제한하고, 애니메이션 프레임마다 갱신한다. (`HTTRANSPARENT`는 같은 스레드의 창에만 통과하므로 쓰지 않는다.)
- 두 창 모두 바탕화면(Progman) 소유, 비활성화 시 `HWND_BOTTOM`.
- 애니메이션이 있을 때만 16ms 타이머로 다시 그림. 정지 시 CPU 0%.
- Per-Monitor DPI Aware v2. 모든 치수는 논리 px × 배율.
- Windows 10 등 아크릴 미지원: 유리 창을 테마 색 92% 불투명 단색으로 그림.

### 4.2 독 모드

- 기본 위치: 주 모니터 작업 영역 아래 가운데, 가장자리에서 8px. 설정으로 아래/위/왼쪽/오른쪽(좌·우는 세로 배치).
- 아이콘 48px, 간격 10px, 판 안쪽 여백 10px. 판은 가운데 기준으로 늘어난다.
- 확대: 마우스와 아이콘 중심 거리 d, R = 아이콘 2.5개 거리일 때 `scale = 1 + 0.6·cos(π/2·d/R)` (d ≥ R이면 1). 커진 만큼 이웃이 밀려난다. 표시 배율은 매 프레임 목표값으로 25%씩 접근. 마우스가 벗어나면 1로 복귀.
- 끝부분: 구분선 → ＋ → 팔레트 → ⋯ 메뉴. 헤더 없음(제목·by_ssenu는 ⋯ 메뉴 첫 줄).

### 4.3 패널 모드

- 0.1.x와 같은 위젯: 기본 우상단(여백 16px, 360×480), 이동·크기 조절, 위치 잠금, 위치 저장(500ms 디바운스).
- 상단 헤더: `YT Launcher` + `by_ssenu` + ⋯. 오른쪽 아래 팔레트 버튼.
- 여러 줄 배치. 마우스를 올린 아이콘만 1.15배 + 밝게(이웃 이동 없음).

### 4.4 아이콘

| 종류 | 그림 |
|---|---|
| 유튜브 | 빨간 둥근 사각형 + 흰 재생 삼각형 + 아래 검색어 두 글자 배지 |
| 앱 | 셸 아이콘 256px를 고품질 축소 |
| 웹 | 흰 둥근 판 위 파비콘. 실패 시 도메인 첫 글자 원형 아바타 |
| ＋ | 반투명 원 + ＋ |

- 모든 아이콘에 옅은 그림자. 실행 중인 앱은 아래 4px 점. 실행 실패(앱 없음) 항목은 흐리게.
- 마우스를 올리면 판 반대쪽(독) 또는 아이콘 위(패널)에 이름 말풍선.
- 글꼴: Pretendard(Regular/SemiBold/Bold)를 exe에 포함, DirectWrite 메모리 폰트로 로드.

### 4.5 테마

| 테마 | 덧칠 | 글자·말풍선 |
|---|---|---|
| light | 흰색 55% | 진회색 `#1F232B` |
| dark | `#181A1F` 60% | `#F1F3F5` |
| sepia | `#F4ECD8` 60% | `#4B3A26` |
| glass(투명) | 검정 30% | 흰색 |

- 팔레트 버튼 → 색 원 4개가 부채꼴로 펼쳐짐(독이 아래면 위로). 선택 시 0.2초 페이드 후 저장.
- 아크릴 밝기 톤은 light/sepia는 밝게, dark/glass는 어둡게(`DWMWA_USE_IMMERSIVE_DARK_MODE`).

### 4.6 메뉴

⋯ 메뉴와 트레이 메뉴(같은 항목): 제목줄(YT Launcher by_ssenu, 비활성) / 런처 보이기 / 독 모드·패널 모드 / 독 위치(아래·위·왼쪽·오른쪽) / 위치·크기 초기화 / 위치 잠금 / 윈도우 시작 시 실행 / 업데이트 확인 / 릴리스 페이지 열기 / 종료.

## 5. 항목 추가·수정과 실행

### 5.1 추가·수정 창

상단에 종류 버튼(유튜브 / 앱 / 웹 주소), 아래 종류별 입력칸, [취소][저장].

| 종류 | 입력 | 저장 조건 |
|---|---|---|
| 유튜브 | 검색어(필수), 표시 이름(비우면 검색어) | 검색어 trim 후 비어 있지 않음 |
| 앱 | [앱 선택…] → 선택 앱 아이콘·이름, 표시 이름 | 앱 선택됨 |
| 웹 주소 | 주소(필수), 표시 이름(비우면 호스트명) | 스킴 없으면 `https://` 추가, 최종 스킴이 http/https, 호스트가 있음 |

- 우클릭 메뉴: 수정 / 삭제(확인). 수정 시 종류 고정.
- 드래그 정렬: 누른 채 6px 이상 이동하면 시작, 다른 아이콘이 자리를 비켜 줌, 놓으면 저장. ＋·팔레트·⋯는 이동 불가, 항상 끝.

### 5.2 앱 선택 창

- 검색칸 + 아이콘 목록(ListView, 32px) + [직접 찾기…](`.exe`, `.lnk`) + [취소][선택].
- `shell:AppsFolder`를 백그라운드 스레드에서 열거(`SHCreateItemFromParsingName` + `BHID_EnumItems`), 이름순. 로딩 중 "불러오는 중…". 아이콘은 보이는 줄부터 지연 로드.
- 검색: 입력 즉시 이름 부분 일치 필터(대소문자 무시).
- 저장값: `target` = AppsFolder 파싱 이름(Store 앱은 AUMID), 가능하면 `exe` = 실제 실행 파일 경로.

### 5.3 실행

| 종류 | 동작 |
|---|---|
| 유튜브 | `https://www.youtube.com/results?search_query=<인코딩>`을 크롬으로, 창 최대화·전면화 (0.1.x 로직 이식) |
| 웹 | 같은 방식으로 크롬 새 탭. 크롬 없으면 기본 브라우저(`ShellExecuteW`) |
| 앱 | 실행 중이면 가장 위 창을 `SW_RESTORE` 후 전면화(`AttachThreadInput` 폴백). 아니면 `ShellExecuteW("shell:AppsFolder\<target>")`. 직접 찾기로 고른 exe/lnk는 경로로 실행 |

- 같은 항목 1초 이내 재클릭 무시. 실행은 백그라운드 스레드.
- 실행 중 판정: `SetWinEventHook`(창 생성·소멸·표시)으로 변화가 있을 때만 300ms 디바운스 후 재계산. 데스크톱 앱은 창 프로세스 exe 경로, Store 앱은 창의 `PKEY_AppUserModel_ID`로 매칭.

### 5.4 아이콘 캐시

- `%APPDATA%\YT-Launcher\icons\<항목id>.png` (256px).
- 앱: `IShellItemImageFactory::GetImage(256)` → WIC로 PNG 저장.
- 웹: `https://<호스트>/favicon.ico` → 실패 시 HTML의 `<link rel="icon">` → WIC로 디코드·PNG 저장. 실패하면 글자 아바타, 다음 실행 때 재시도.

## 6. 설정 (schema 2)

경로 `%APPDATA%\YT-Launcher\config.json`, 임시 파일에 쓰고 교체(원자적), 손상 시 `config.json.bak`으로 옮기고 기본값.

```json
{
  "schema": 2,
  "items": [
    {"id": "…", "kind": "youtube", "query": "로파이 음악", "name": ""},
    {"id": "…", "kind": "app", "name": "카카오톡", "target": "<AppsFolder 파싱 이름>", "exe": "C:\\…\\KakaoTalk.exe"},
    {"id": "…", "kind": "web", "url": "https://github.com", "name": "github.com"}
  ],
  "mode": "dock",
  "dock_edge": "bottom",
  "panel_window": {"x": 0, "y": 0, "w": 360, "h": 480},
  "theme": "glass",
  "locked": false,
  "autostart": null,
  "last_run_version": "0.2.0",
  "notified_version": "",
  "chrome_notice_shown": false
}
```

- schema 1(0.1.x) 변환: 원본을 `config.v1.json`으로 복사 → `tiles[]`를 `kind: youtube` 항목으로(id·query·name 유지, color 버림), `window` → `panel_window`, theme·locked·autostart·버전 필드 유지, `mode`는 `panel`.
- 새 설치 기본값: `mode: dock`, `theme: glass`, 항목 없음.
- 알 수 없는 theme/mode/dock_edge 값은 기본값으로.

## 7. 업데이트·설치·릴리스

### 7.1 0.1.x → 0.2.0 호환 계약 (반드시 유지)

- 정식 릴리스(초안·프리릴리스 아님), 태그 `vX.Y.Z`.
- 자산 이름 `YT-Launcher-Setup-<X.Y.Z>.exe`, `SHA256SUMS.txt`(`<소문자 해시>  <파일명>`).
- Inno Setup AppId `{8F3C2A71-5B4E-4C9D-9A16-3E7D2B0C5F48}`, 설치 경로 `{localappdata}\Programs\YT-Launcher`, exe `YTLauncher.exe`.
- 뮤텍스 `YTLauncherMutex`, 조용한 설치 인자 `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART`, 설치 후 재실행.
- Run 값 이름 `YTLauncher`. `[InstallDelete] {app}\_internal`로 파이썬 잔여물 제거.

### 7.2 Rust 업데이터

0.1.x와 같은 동작: 시작 30초 후 + 24시간마다 + 수동 확인. GitHub API `releases/latest`, 404는 "릴리스 없음", 버전 비교(semver), 설치 파일·해시 둘 다 있을 때만 배지 "업데이트 있음 vX", 버전당 트레이 알림 1회, `.part`에 받으며 해시 계산 → 일치 시에만 최종 이름 → 조용한 설치 분리 실행 → 즉시 종료. 재실행 후 이전 버전보다 높으면 "vX 버전으로 업데이트되었습니다".

### 7.3 빌드·CI

- `Cargo.toml` release 프로필: `lto = true`, `strip = true`, `panic = "abort"`, `codegen-units = 1`, `opt-level = 3`.
- `build.rs`로 앱 아이콘 리소스 포함. 폰트는 `include_bytes!`.
- `scripts/build.ps1`: `cargo build --release` → `dist\YTLauncher\YTLauncher.exe` → ISCC → SHA256SUMS. `-Version` 인자.
- release.yml: `cargo test` → 태그 형식 검사 → `Cargo.toml` 버전 반영 후 `cargo metadata`로 확인 → 빌드 → 자산 이름 규칙 검사 → 릴리스(`fail_on_unmatched_files: true`).
- ci.yml: push/PR 시 `cargo test`, `cargo clippy -- -D warnings`.

## 8. 오류 처리

| 상황 | 동작 |
|---|---|
| 앱 실행 실패(삭제됨 등) | 트레이 알림 "앱을 찾을 수 없습니다. 수정에서 다시 선택하세요", 아이콘 흐리게 |
| 파비콘 실패 | 글자 아바타, 다음 실행 때 재시도 |
| 크롬 없음 | 기본 브라우저, 최초 1회 안내 |
| 앱 목록 열거 실패 | 목록 자리에 오류 문구, [직접 찾기]는 사용 가능 |
| 설정 손상 | `.bak`으로 옮기고 기본값, 트레이 알림 |
| 아크릴 미지원 | 단색 유리로 대체 |
| 업데이트 확인·다운로드 실패 | 0.1.x와 동일(조용히 재시도 / "업데이트 실패 · 다시 시도") |
| 패닉 | `panic = "abort"` 전에 패닉 훅으로 로그 기록 |

- 로그: `%APPDATA%\YT-Launcher\logs\launcher.log` (1MB 넘으면 `.1`로 돌림, 3개 유지).

## 9. 테스트

- 자동(`cargo test`):
  - config: 실제 0.1.4 `config.json` 샘플 변환, 손상 파일 복구, 알 수 없는 값 기본값 처리
  - items: 웹 주소 정규화·거부, 표시 이름 기본값
  - layout: 독 확대 배율, 아이콘 위치·판 크기, 패널 줄바꿈, 히트 테스트
  - updater: 버전 비교, 릴리스 JSON 해석, 해시 검증, 다운로드 중단 시 `.part` 정리
  - chrome: 검색 URL 인코딩
  - Windows 연동 스모크: AppsFolder 열거 결과 1개 이상
- 수동 점검표(`docs/manual-test-checklist.md` 갱신):
  - 독/패널 전환, 독 위치 4방향, 확대 물결, 드래그 정렬
  - 4개 테마 유리(Windows 10 대체 포함), 테마 페이드
  - 데스크톱 앱·Store 앱 실행/전면화/실행 중 점
  - 웹 파비콘, 유튜브 배지, 크롬 최대화
  - **0.1.4 설치 상태에서 원클릭 업데이트로 0.2.0 전환, 설정 유지, `_internal` 삭제**
  - 메모리 측정(목표 10~20MB)

## 10. 작업 단계

모든 작업은 `rust-rewrite` 브랜치. 파이썬 0.1.x는 5단계 완료 전까지 `main`에 유지. 릴리스는 5단계 끝에 한 번. 각 단계는 별도 구현 계획을 쓴다.

| 단계 | 내용 | 완료 기준 |
|---|---|---|
| 1. 기반 | Cargo 프로젝트, config(+변환), items, 단일 인스턴스, 트레이, autostart, updater, chrome, 로그 | 트레이만 있는 상태로 실행, 메뉴·업데이트 확인 동작, `cargo test` 통과 |
| 2. 화면 엔진 + 패널 모드 | 유리/콘텐츠 2창, DComp/D2D, 테마 4개, 부채꼴 선택기, 유튜브 아이콘, 확대, 드래그 정렬, 추가·수정 창(유튜브) | 0.1.4 기능을 Rust로 재현 |
| 3. 독 모드 | 확대 물결, 가장자리 4방향, 모드 전환 | 맥 독처럼 동작 |
| 4. 앱·웹 | 앱 선택 창, 앱 실행·전면화·실행 중 점, 웹 주소·파비콘, 종류 선택 | 새 기능 완성 |
| 5. 전환·출시 | 빌드/CI 교체, 업그레이드 테스트, 파이썬 코드 삭제, 문서, v0.2.0 릴리스 | 0.1.x 사용자가 원클릭 업데이트로 전환 |

개발 환경: `winget install Rustlang.Rustup`(stable, `x86_64-pc-windows-msvc`). MSVC 빌드 도구는 설치되어 있음(Visual Studio Build Tools 2026).
