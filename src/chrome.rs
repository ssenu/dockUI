//! 크롬으로 URL 열기 (유튜브 검색·웹 주소 공용). 크롬이 없으면 기본 브라우저.

use crate::win::reg::{self, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use crate::win::windows::{TopWindow, allow_any_foreground, list_top_windows, maximize_and_focus};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const SEARCH_URL: &str = "https://www.youtube.com/results?search_query=";
const APP_PATHS_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe";
const CHROME_SUBPATH: &str = r"Google\Chrome\Application\chrome.exe";
const ENV_ROOTS: [&str; 3] = ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"];
const CHROME_CLASS: &str = "Chrome_WidgetWin_1";

/// 파이썬 `urllib.parse.quote_plus`와 같은 인코딩 (공백 → `+`).
pub fn quote_plus(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for b in text.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn build_search_url(query: &str) -> String {
    format!("{SEARCH_URL}{}", quote_plus(query.trim()))
}

pub fn registry_chrome_path() -> Option<PathBuf> {
    [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE].into_iter().find_map(|root| {
        let value = reg::get_string(root, APP_PATHS_KEY, "")?;
        let path = PathBuf::from(value.trim().trim_matches('"'));
        path.is_file().then_some(path)
    })
}

pub fn candidate_paths(env: &dyn Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    ENV_ROOTS.iter().filter_map(|var| env(var)).map(|root| Path::new(&root).join(CHROME_SUBPATH)).collect()
}

pub fn find_chrome_with(
    registry: &dyn Fn() -> Option<PathBuf>,
    env: &dyn Fn(&str) -> Option<String>,
) -> Option<PathBuf> {
    registry().or_else(|| candidate_paths(env).into_iter().find(|p| p.is_file()))
}

pub fn find_chrome() -> Option<PathBuf> {
    find_chrome_with(&registry_chrome_path, &|var| std::env::var(var).ok())
}

fn is_chrome_window(w: &TopWindow) -> bool {
    w.class == CHROME_CLASS
        && Path::new(&w.exe).file_name().and_then(|n| n.to_str()).is_some_and(|n| n.eq_ignore_ascii_case("chrome.exe"))
}

/// 방금 연 탭의 크롬 창을 찾아 최대화·전면화. `prefer_title`이 제목에 들어간 창을 우선한다.
pub fn focus_chrome_window(prefer_title: Option<&str>, timeout: Duration) -> bool {
    std::thread::sleep(Duration::from_millis(500));
    let deadline = Instant::now() + timeout;
    loop {
        let windows: Vec<TopWindow> = list_top_windows().into_iter().filter(is_chrome_window).collect();
        let preferred = prefer_title.and_then(|t| windows.iter().find(|w| w.title.contains(t)));
        if let Some(w) = preferred {
            return maximize_and_focus(w.handle());
        }
        if Instant::now() >= deadline {
            return windows.first().is_some_and(|w| maximize_and_focus(w.handle()));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opened {
    Chrome,
    DefaultBrowser,
}

/// URL을 크롬 새 탭으로 열고 그 창을 최대화한다. 크롬이 없으면 기본 브라우저.
/// 최대 약 5.5초 기다리므로 UI 스레드가 아닌 곳에서 호출한다.
pub fn open_url(url: &str, prefer_title: Option<&str>) -> std::io::Result<Opened> {
    match find_chrome() {
        Some(chrome) => {
            allow_any_foreground();
            std::process::Command::new(chrome).arg(url).spawn()?;
            focus_chrome_window(prefer_title, Duration::from_secs(5));
            Ok(Opened::Chrome)
        }
        None => {
            crate::win::shell::open(url)?;
            Ok(Opened::DefaultBrowser)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_url_matches_python_quote_plus() {
        assert_eq!(
            build_search_url("로파이 음악"),
            "https://www.youtube.com/results?search_query=%EB%A1%9C%ED%8C%8C%EC%9D%B4+%EC%9D%8C%EC%95%85"
        );
        assert!(build_search_url("lofi & chill #1").ends_with("lofi+%26+chill+%231"));
        assert!(build_search_url("  a b  ").ends_with("=a+b"));
    }

    #[test]
    fn find_chrome_prefers_registry_then_env() {
        let dir = std::env::temp_dir().join(format!("ytl-chrome-{}", crate::items::new_id()));
        let exe = dir.join(CHROME_SUBPATH);
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"").unwrap();
        let reg_hit = PathBuf::from("C:\\from-registry\\chrome.exe");
        assert_eq!(find_chrome_with(&|| Some(reg_hit.clone()), &|_| None), Some(reg_hit.clone()));
        let root = dir.to_string_lossy().to_string();
        let env = move |var: &str| (var == "LOCALAPPDATA").then(|| root.clone());
        assert_eq!(find_chrome_with(&|| None, &env), Some(exe));
        assert_eq!(find_chrome_with(&|| None, &|_| None), None);
    }

    #[test]
    fn registry_lookup_does_not_panic() {
        if let Some(p) = registry_chrome_path() {
            assert!(p.is_file());
        }
    }
}
