# Rust 독 런처 — 5단계(전환·출시) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 빌드·CI를 Rust로 바꾸고, 0.1.4 → 0.2.0 업그레이드를 확인하고, 파이썬 코드를 지운 뒤 v0.2.0을 낸다.

**Spec:** `docs/superpowers/specs/2026-10-26-dock-launcher-rust-design.md` 7장(호환 계약, 빌드·CI), 10장 5단계

## Global Constraints

- 호환 계약을 지킨다: AppId `{8F3C2A71-5B4E-4C9D-9A16-3E7D2B0C5F48}`, 설치 경로 `{localappdata}\Programs\YT-Launcher`, exe `YTLauncher.exe`, 뮤텍스 `YTLauncherMutex`, Run 값 `YTLauncher`, 자산 `YT-Launcher-Setup-<X.Y.Z>.exe` + `SHA256SUMS.txt`(`<소문자 해시>  <파일명>`), 정식 릴리스, 태그 `vX.Y.Z`.
- `[InstallDelete] {app}\_internal`은 남긴다 (파이썬 잔여물 제거).
- push·태그·릴리스·`main` 병합은 사용자 승인 뒤에만.

---

### Task 1: 빌드 스크립트와 설치 스크립트

**Files:** Modify `scripts/build.ps1`, `installer/setup.iss`

- [ ] `build.ps1`: `Cargo.toml` 버전을 읽고 `-Version`과 다르면 멈춤 → `cargo build --release` → `dist\YTLauncher\YTLauncher.exe` 한 파일 → ISCC → `SHA256SUMS.txt`. (태그로 버전을 바꾸면 `Cargo.lock`의 자기 줄도 바뀌므로 `--locked`는 쓰지 않는다.)
- [ ] `setup.iss`: `SetupIconFile=..\assets\icon.ico`, `[Files]`는 `..\dist\YTLauncher\YTLauncher.exe` 하나.
- [ ] Run: `./scripts/build.ps1` → Expected: `dist\installer\YT-Launcher-Setup-0.2.0.exe`(약 4.6MB), 해시 한 줄.

### Task 2: CI

**Files:** Modify `.github/workflows/ci.yml`, `.github/workflows/release.yml`

- [ ] ci.yml: `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked` (cargo 캐시).
- [ ] release.yml: `cargo test --locked` → 태그 형식 검사 → `Cargo.toml` 첫 `version` 줄을 태그 버전으로 → `cargo metadata`로 확인 → Inno Setup 설치 → `build.ps1 -Version` → 자산 이름·해시 줄 형식 검사 → `softprops/action-gh-release@v2`(`fail_on_unmatched_files: true`).
- [ ] 로컬 모의: 임시 복사본에서 태그 `v0.2.1`로 버전 단계를 돌려 `cargo metadata`가 0.2.1을 내는지, 해시 줄 정규식이 통과하는지 확인.

### Task 3: 파이썬 코드 삭제와 문서

**Files:** Delete `src/yt_launcher/`, `tests/`, `launcher.py`, `pyproject.toml`, `tools/make_icon.py`. Modify `.gitignore`, `README.md`, `docs/manual-test-checklist.md`

- [ ] `git rm`으로 지우고, 남은 `__pycache__`·`*.egg-info`·PyInstaller `YTLauncher.spec`도 지운다.
- [ ] README: 독·패널·항목 3종·테마, Rust 개발 명령, 릴리스 절차.
- [ ] 점검표: 독·패널·항목·테마·설치·업데이트(0.1.4 → 0.2.0 포함).
- [ ] Run: `cargo fmt --check; cargo clippy --all-targets -- -D warnings; cargo test` → 통과.

### Task 4: 0.1.4 → 0.2.0 업그레이드 확인 (사용자 승인 필요)

설치된 0.1.4를 실제로 올리므로 실행 중인 런처가 꺼지고 설정이 schema 2로 바뀐다. 먼저 `%APPDATA%\YT-Launcher`를 백업한다.

- [ ] `dist\installer\YT-Launcher-Setup-0.2.0.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART` (0.1.x 업데이터가 쓰는 인자 그대로)
- [ ] 확인: 설치 폴더에 `YTLauncher.exe`만 있고 `_internal` 없음, 0.2.0이 다시 실행됨, `config.v1.json` 백업과 schema 2 설정, 타일 그대로(패널 모드), Run 값 유지, "설치된 앱"의 버전 0.2.0.

### Task 5: 출시 (사용자 승인 필요)

- [ ] `rust-rewrite` push → `main`에 병합 → push.
- [ ] `git tag v0.2.0; git push origin v0.2.0` → Actions 릴리스 확인 → 자산 2개.
- [ ] 0.1.x 사용자 화면에서 업데이트 알림이 뜨는지(시작 30초 뒤) 확인.
