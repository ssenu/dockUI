# YT Launcher

바탕화면에 깔리는 Windows 런처. 유튜브 검색어, PC에 설치된 앱, 웹 주소를 아이콘으로 올려 두고 한 번에 연다.

- **독 모드:** 화면 가장자리(아래·위·왼쪽·오른쪽) 가운데에 놓이고, 마우스를 올리면 아이콘이 물결처럼 커진다.
- **패널 모드:** 0.1.x처럼 우상단 창. 이동·크기 조절·위치 잠금.
- **항목:** 유튜브(크롬으로 검색 결과), 앱(실행 중이면 그 창으로 전환, 실행 중이면 아래 점), 웹 주소(크롬 새 탭, 사이트 아이콘).
- **테마:** 라이트 / 다크 / 세피아 / 투명. 판 뒤 배경은 흐리게 비친다.
- 다른 창 뒤에 깔리고, 트레이 메뉴와 ⋯ 메뉴에서 모드·위치·자동 실행·업데이트를 다룬다.

## 설치

[Releases](https://github.com/ssenu/dockUI/releases/latest)에서 `YT-Launcher-Setup-<버전>.exe`를 받아 실행한다.
코드 서명이 없어 처음에 "Windows의 PC 보호" 창이 뜨면 **추가 정보 → 실행**을 누른다.
0.1.x를 쓰고 있었다면 트레이의 업데이트 설치로 바로 넘어오고, 설정은 그대로 옮겨진다(원본은 `%APPDATA%\YT-Launcher\config.v1.json`).

## 개발

Rust(stable)와 Visual Studio Build Tools(C++)가 필요하다.

```powershell
cargo test
cargo clippy --all-targets -- -D warnings
cargo run --release
```

화면 그리기 테스트는 `target\snapshots\*.png`에 그림을 남긴다.

로컬 설치 프로그램 빌드 (Inno Setup 6 필요): `./scripts/build.ps1` → `dist\installer\`

## 릴리스

```powershell
git tag v0.2.0
git push origin v0.2.0
```

GitHub Actions가 테스트 → 태그 버전을 `Cargo.toml`에 반영 → 빌드 → 자산 이름 검사 → 릴리스 생성까지 한다.
설치된 앱은 시작 30초 뒤와 하루 한 번 새 릴리스를 확인해 트레이로 알린다.
릴리스 전에 `docs/manual-test-checklist.md`를 확인한다.
