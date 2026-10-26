//! 단일 인스턴스: 명명된 뮤텍스로 판정하고, 이미 실행 중이면 기존 인스턴스에 "보이기"를 보낸다.

use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HANDLE, LPARAM, WPARAM};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{ASFW_ANY, AllowSetForegroundWindow, FindWindowW, PostMessageW, WM_APP};
use windows::core::HSTRING;

pub const MUTEX_NAME: &str = "YTLauncherMutex";
/// 트레이·메시지를 받는 숨은 창의 클래스 이름
pub const MESSAGE_CLASS: &str = "YTLauncherMsg";
/// 두 번째 실행이 기존 인스턴스에 보내는 "런처 보이기" 요청
pub const WM_APP_SHOW: u32 = WM_APP + 1;

/// 뮤텍스를 만든다. (핸들, 다른 프로세스가 이미 가지고 있었는지)
/// 핸들은 프로세스가 끝날 때까지 일부러 닫지 않는다 (설치 프로그램이 실행 중 감지에 쓴다).
pub fn acquire_mutex(name: &str) -> (Option<HANDLE>, bool) {
    // SAFETY: 이름만 넘기는 단순 호출
    match unsafe { CreateMutexW(None, false, &HSTRING::from(name)) } {
        Ok(handle) => {
            let existed = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
            (Some(handle), existed)
        }
        Err(_) => (None, false),
    }
}

/// 기존 인스턴스의 숨은 창에 "보이기"를 보낸다. 창을 찾았으면 true.
pub fn notify_existing() -> bool {
    // 사용자가 방금 실행한 이쪽 프로세스가 기존 인스턴스에 전면 표시 권한을 넘겨준다
    let _ = unsafe { AllowSetForegroundWindow(ASFW_ANY) };
    // SAFETY: 클래스 이름으로 찾고 메시지만 보낸다
    match unsafe { FindWindowW(&HSTRING::from(MESSAGE_CLASS), None) } {
        Ok(hwnd) if !hwnd.0.is_null() => unsafe { PostMessageW(Some(hwnd), WM_APP_SHOW, WPARAM(0), LPARAM(0)) }.is_ok(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_acquire_reports_existing() {
        let name = format!("YTLauncherTest-{}", crate::items::new_id());
        let (first, existed1) = acquire_mutex(&name);
        assert!(first.is_some());
        assert!(!existed1);
        let (second, existed2) = acquire_mutex(&name);
        assert!(second.is_some());
        assert!(existed2);
    }
}
