//! 최상위 창 목록, 창 최대화·복원·전면화.

use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, TRUE};
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentThreadId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    ASFW_ANY, AllowSetForegroundWindow, BringWindowToTop, EnumWindows, GetClassNameW, GetForegroundWindow,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible, SW_MAXIMIZE, SW_RESTORE,
    SetForegroundWindow, ShowWindow,
};
use windows::core::{BOOL, PWSTR};

#[derive(Debug, Clone)]
pub struct TopWindow {
    pub hwnd: isize,
    pub title: String,
    pub class: String,
    /// 창을 만든 프로세스의 실행 파일 전체 경로 (알 수 없으면 빈 문자열)
    pub exe: String,
}

impl TopWindow {
    pub fn handle(&self) -> HWND {
        HWND(self.hwnd as *mut _)
    }
}

fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    // SAFETY: 버퍼 크기 전달
    let n = unsafe { GetClassNameW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

fn window_text(hwnd: HWND) -> String {
    // SAFETY: 길이를 묻고 그만큼 버퍼를 준비
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; len as usize + 1];
    let n = unsafe { GetWindowTextW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

pub fn process_image(pid: u32) -> String {
    // SAFETY: 핸들은 아래에서 닫는다
    let Ok(handle) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }) else {
        return String::new();
    };
    let mut buf = [0u16; 1024];
    let mut size = buf.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut size) };
    let _ = unsafe { CloseHandle(handle) };
    if ok.is_ok() { String::from_utf16_lossy(&buf[..size as usize]) } else { String::new() }
}

/// 보이는 최상위 창을 Z순서(위가 먼저)로 돌려준다. 제목 없는 창은 뺀다.
pub fn list_top_windows() -> Vec<TopWindow> {
    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: lparam은 아래에서 넘긴 Vec 포인터
        let out = unsafe { &mut *(lparam.0 as *mut Vec<TopWindow>) };
        if unsafe { IsWindowVisible(hwnd) }.as_bool() {
            let title = window_text(hwnd);
            if !title.is_empty() {
                let mut pid = 0u32;
                unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
                out.push(TopWindow { hwnd: hwnd.0 as isize, title, class: class_name(hwnd), exe: process_image(pid) });
            }
        }
        TRUE
    }
    let mut out: Vec<TopWindow> = Vec::new();
    // SAFETY: 콜백은 EnumWindows가 끝나기 전에만 out을 쓴다
    let _ = unsafe { EnumWindows(Some(collect), LPARAM(&mut out as *mut _ as isize)) };
    out
}

/// 다른 프로세스가 전면으로 나오는 것을 허용 (크롬 실행 직전에 호출)
pub fn allow_any_foreground() {
    let _ = unsafe { AllowSetForegroundWindow(ASFW_ANY) };
}

fn force_foreground(hwnd: HWND) -> bool {
    // SAFETY: 모든 호출은 핸들만 사용
    unsafe {
        if SetForegroundWindow(hwnd).as_bool() {
            return true;
        }
        let fg = GetForegroundWindow();
        let fg_thread = if fg.0.is_null() { 0 } else { GetWindowThreadProcessId(fg, None) };
        let current = GetCurrentThreadId();
        let attached = fg_thread != 0 && fg_thread != current && AttachThreadInput(current, fg_thread, true).as_bool();
        let _ = BringWindowToTop(hwnd);
        let ok = SetForegroundWindow(hwnd).as_bool();
        if attached {
            let _ = AttachThreadInput(current, fg_thread, false);
        }
        ok
    }
}

/// 최대화(F11 전체화면 아님) 후 전면으로.
pub fn maximize_and_focus(hwnd: HWND) -> bool {
    let _ = unsafe { ShowWindow(hwnd, SW_MAXIMIZE) };
    force_foreground(hwnd)
}

/// 최소화돼 있으면 복원하고 전면으로 (실행 중인 앱으로 전환할 때).
pub fn restore_and_focus(hwnd: HWND) -> bool {
    if unsafe { IsIconic(hwnd) }.as_bool() {
        let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
    }
    force_foreground(hwnd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_visible_windows_with_titles() {
        for w in list_top_windows() {
            assert!(!w.title.is_empty());
            assert_ne!(w.hwnd, 0);
        }
    }

    #[test]
    fn own_process_image_is_known() {
        let exe = process_image(std::process::id());
        assert!(exe.to_ascii_lowercase().ends_with(".exe"), "{exe}");
    }
}
