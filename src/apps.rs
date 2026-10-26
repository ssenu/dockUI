//! 앱 항목: 실행 중인지 판단(순수 함수), 실행 중이면 그 창으로 전환, 아니면 실행.

use crate::win::shell;
use crate::win::shell_items::{apps_folder_path, item_exists, window_app_id};
use crate::win::windows::{TopWindow, list_top_windows, restore_and_focus};
use std::path::Path;

/// 실행 중 판단에 쓰는 창 정보
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInfo {
    pub hwnd: isize,
    pub exe: String,
    pub app_id: Option<String>,
}

/// 앱 항목 하나의 식별 정보
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppKey {
    /// shell:AppsFolder 파싱 이름(AUMID 등) 또는 직접 고른 exe/lnk 경로
    pub target: String,
    pub exe: Option<String>,
}

fn file_name(path: &str) -> String {
    Path::new(path).file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// 크롬 웹앱의 실행 파일은 실제 창을 띄우지 않는 중계 프로그램이라 경로로는 판단하지 않는다.
const LAUNCHER_ONLY_EXES: [&str; 1] = ["chrome_proxy.exe"];

/// 이 창이 이 앱의 창인가: 창의 앱 ID가 대상과 같거나, 실행 파일 경로가 같으면.
pub fn window_matches(app: &AppKey, w: &WindowInfo) -> bool {
    if w.app_id.as_deref().is_some_and(|id| id.eq_ignore_ascii_case(&app.target)) {
        return true;
    }
    match &app.exe {
        Some(exe) if !LAUNCHER_ONLY_EXES.contains(&file_name(exe).as_str()) => {
            !w.exe.is_empty() && w.exe.eq_ignore_ascii_case(exe)
        }
        _ => false,
    }
}

/// 앱마다 실행 중인 창이 있는지
pub fn running_flags(apps: &[Option<AppKey>], windows: &[WindowInfo]) -> Vec<bool> {
    apps.iter().map(|app| app.as_ref().is_some_and(|a| windows.iter().any(|w| window_matches(a, w)))).collect()
}

/// 이 앱의 창 중 맨 위(Z순서 첫 번째) 창
pub fn find_window(app: &AppKey, windows: &[WindowInfo]) -> Option<isize> {
    windows.iter().find(|w| window_matches(app, w)).map(|w| w.hwnd)
}

/// 지금 보이는 최상위 창들 (앱 ID 포함). 우리 프로세스 창은 뺀다.
pub fn current_windows() -> Vec<WindowInfo> {
    let own = std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    list_top_windows()
        .into_iter()
        .filter(|w: &TopWindow| !w.exe.eq_ignore_ascii_case(&own))
        .map(|w| WindowInfo { hwnd: w.hwnd, app_id: window_app_id(w.handle()), exe: w.exe })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Launched {
    /// 이미 실행 중이라 그 창을 앞으로 가져왔다
    Focused,
    Started,
}

/// 실행 중이면 창을 복원해 앞으로, 아니면 실행한다. 실행할 대상을 못 찾으면 Err.
pub fn launch(app: &AppKey) -> std::io::Result<Launched> {
    if let Some(hwnd) = find_window(app, &current_windows()) {
        restore_and_focus(windows::Win32::Foundation::HWND(hwnd as *mut _));
        return Ok(Launched::Focused);
    }
    let target = if Path::new(&app.target).is_absolute() { app.target.clone() } else { apps_folder_path(&app.target) };
    // 지워진 앱은 셸 오류 창을 띄우지 않고 바로 실패로 돌려준다
    if !item_exists(&target) {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, target));
    }
    crate::win::windows::allow_any_foreground();
    shell::open(&target).map(|_| Launched::Started)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(hwnd: isize, exe: &str, id: Option<&str>) -> WindowInfo {
        WindowInfo { hwnd, exe: exe.into(), app_id: id.map(str::to_string) }
    }

    #[test]
    fn matches_store_app_by_app_id() {
        let calc = AppKey { target: "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App".into(), exe: None };
        let w = win(
            1,
            r"C:\Windows\System32\ApplicationFrameHost.exe",
            Some("microsoft.windowscalculator_8wekyb3d8bbwe!app"),
        );
        assert!(window_matches(&calc, &w));
        assert!(!window_matches(&calc, &win(2, r"C:\x.exe", None)));
    }

    #[test]
    fn matches_desktop_app_by_exe_case_insensitively() {
        let kakao = AppKey { target: "Kakao".into(), exe: Some(r"C:\Program Files\Kakao\KakaoTalk.exe".into()) };
        assert!(window_matches(&kakao, &win(1, r"c:\program files\kakao\kakaotalk.exe", None)));
        assert!(!window_matches(&kakao, &win(1, "", None)));
    }

    #[test]
    fn chrome_web_apps_need_app_id_not_proxy_exe() {
        let pwa = AppKey {
            target: "Chrome._crx_abc".into(),
            exe: Some(r"C:\Program Files\Google\Chrome\Application\chrome_proxy.exe".into()),
        };
        let chrome = r"C:\Program Files\Google\Chrome\Application\chrome.exe";
        assert!(!window_matches(&pwa, &win(1, r"C:\Program Files\Google\Chrome\Application\chrome_proxy.exe", None)));
        assert!(!window_matches(&pwa, &win(1, chrome, Some("Chrome"))));
        assert!(window_matches(&pwa, &win(1, chrome, Some("Chrome._crx_abc"))));
    }

    #[test]
    fn running_flags_and_topmost_window() {
        let a = AppKey { target: "A!App".into(), exe: None };
        let b = AppKey { target: "B".into(), exe: Some(r"C:\b.exe".into()) };
        let windows =
            vec![win(10, r"C:\b.exe", None), win(20, r"C:\other.exe", Some("A!App")), win(30, r"C:\b.exe", None)];
        assert_eq!(running_flags(&[Some(a.clone()), Some(b.clone()), None], &windows), vec![true, true, false]);
        assert_eq!(find_window(&b, &windows), Some(10));
        assert_eq!(find_window(&AppKey { target: "none".into(), exe: None }, &windows), None);
    }
}
