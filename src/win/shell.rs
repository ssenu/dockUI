//! ShellExecute로 URL·파일 열기 (기본 브라우저 폴백, 릴리스 페이지 열기).

use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, w};

pub fn open(target: &str) -> std::io::Result<()> {
    // SAFETY: 문자열 인자만 넘긴다. 반환값이 32 이하면 실패.
    let result = unsafe { ShellExecuteW(None, w!("open"), &HSTRING::from(target), None, None, SW_SHOWNORMAL) };
    if result.0 as isize > 32 {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("ShellExecute 실패 ({})", result.0 as isize)))
    }
}
