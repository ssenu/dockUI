//! DWM(데스크톱 창 관리자) 효과: 아크릴 배경, 어두운 톤, 둥근 모서리.

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DWM_SYSTEMBACKDROP_TYPE, DWM_WINDOW_CORNER_PREFERENCE, DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE,
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmExtendFrameIntoClientArea,
    DwmSetWindowAttribute,
};
use windows::Win32::UI::Controls::MARGINS;

fn set_attr<T>(hwnd: HWND, attr: windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE, value: &T) -> bool {
    // SAFETY: value의 크기를 함께 넘긴다
    unsafe { DwmSetWindowAttribute(hwnd, attr, (value as *const T).cast(), std::mem::size_of::<T>() as u32) }.is_ok()
}

/// 블러 배경의 밝기 톤 (어두운 테마면 true)
pub fn set_dark(hwnd: HWND, dark: bool) -> bool {
    let value: i32 = dark.into();
    set_attr(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &value)
}

/// Windows 11 아크릴 배경을 켠다. 지원하지 않으면(Windows 10 등) false.
pub fn enable_acrylic(hwnd: HWND, dark: bool) -> bool {
    set_dark(hwnd, dark);
    let corner: DWM_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND;
    set_attr(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &corner);
    let margins = MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 };
    // SAFETY: 창 전체를 프레임 영역으로 넓혀 배경 효과가 보이게 한다
    let _ = unsafe { DwmExtendFrameIntoClientArea(hwnd, &margins) };
    let backdrop: DWM_SYSTEMBACKDROP_TYPE = DWMSBT_TRANSIENTWINDOW;
    set_attr(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, &backdrop)
}
