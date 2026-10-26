//! 바탕화면 위젯 동작: 바탕화면에 붙이기, 맨 아래로 보내기, 모니터 작업 영역, DPI.

use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint,
};
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GWLP_HWNDPARENT, HWND_BOTTOM, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowLongPtrW, SetWindowPos,
};
use windows::core::{BOOL, w};

/// (x, y, w, h) 실제 픽셀
pub type PxRect = (i32, i32, i32, i32);

/// 창의 소유자를 바탕화면(Progman)으로 지정해 Win+D에도 남게 한다.
pub fn pin_to_desktop(hwnd: HWND) -> bool {
    // SAFETY: 소유자 핸들만 바꾼다
    unsafe {
        match FindWindowW(w!("Progman"), None) {
            Ok(progman) if !progman.0.is_null() => {
                SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, progman.0 as isize);
                true
            }
            _ => false,
        }
    }
}

/// 다른 창들 뒤로 보낸다 (활성화하지 않음).
pub fn send_to_bottom(hwnd: HWND) {
    let _ = unsafe { SetWindowPos(hwnd, Some(HWND_BOTTOM), 0, 0, 0, 0, SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE) };
}

fn rect_tuple(r: RECT) -> PxRect {
    (r.left, r.top, r.right - r.left, r.bottom - r.top)
}

/// 주 모니터의 작업 영역 (작업표시줄 제외)
pub fn primary_work_area() -> PxRect {
    // SAFETY: (0,0)이 있는 모니터 = 주 모니터
    unsafe {
        let monitor = MonitorFromPoint(Default::default(), MONITOR_DEFAULTTOPRIMARY);
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if GetMonitorInfoW(monitor, &mut info).as_bool() { rect_tuple(info.rcWork) } else { (0, 0, 1280, 720) }
    }
}

/// 모든 모니터의 작업 영역
pub fn work_areas() -> Vec<PxRect> {
    unsafe extern "system" fn collect(monitor: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        // SAFETY: data는 아래에서 넘긴 Vec 포인터
        let out = unsafe { &mut *(data.0 as *mut Vec<PxRect>) };
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
            out.push(rect_tuple(info.rcWork));
        }
        true.into()
    }
    let mut out: Vec<PxRect> = Vec::new();
    let _ = unsafe { EnumDisplayMonitors(None, None, Some(collect), LPARAM(&mut out as *mut _ as isize)) };
    out
}

/// 시스템(주 모니터) 배율. 창을 만들기 전 기본 크기 계산에 쓴다.
pub fn system_scale() -> f32 {
    let dpi = unsafe { GetDpiForSystem() };
    if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 }
}

/// 창이 있는 모니터의 배율 (100% = 1.0)
pub fn scale_for(hwnd: HWND) -> f32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_areas_include_primary() {
        let primary = primary_work_area();
        assert!(primary.2 > 0 && primary.3 > 0);
        assert!(work_areas().contains(&primary));
    }
}
