//! DWM(데스크톱 창 관리자) 효과: 흐림(아크릴) 배경, 어두운 톤, 둥근 모서리.
//!
//! 테두리 없는 팝업 창에는 Windows 11의 시스템 배경(`DWMWA_SYSTEMBACKDROP_TYPE`)이 그려지지 않는다.
//! 그래서 작업표시줄·시작 메뉴 같은 셸 창이 쓰는 `SetWindowCompositionAttribute`의 아크릴 흐림을 먼저 쓰고,
//! 그것이 없을 때만 시스템 배경을 시도한다.

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DWM_SYSTEMBACKDROP_TYPE, DWM_WINDOW_CORNER_PREFERENCE, DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE,
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmExtendFrameIntoClientArea,
    DwmSetWindowAttribute,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::UI::Controls::MARGINS;
use windows::core::{BOOL, s, w};

fn set_attr<T>(hwnd: HWND, attr: windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE, value: &T) -> bool {
    // SAFETY: value의 크기를 함께 넘긴다
    unsafe { DwmSetWindowAttribute(hwnd, attr, (value as *const T).cast(), std::mem::size_of::<T>() as u32) }.is_ok()
}

/// Windows 11 둥근 모서리 (그 전 버전에서는 아무 일도 하지 않는다)
pub fn round_corners(hwnd: HWND) {
    let corner: DWM_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND;
    set_attr(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &corner);
}

/// 블러 배경의 밝기 톤 (어두운 테마면 true)
pub fn set_dark(hwnd: HWND, dark: bool) -> bool {
    let value: i32 = dark.into();
    set_attr(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &value)
}

#[repr(C)]
struct AccentPolicy {
    state: u32,
    flags: u32,
    /// ABGR. 알파가 0이면 흐림만, 덧칠은 우리가 그린다.
    gradient_color: u32,
    animation_id: u32,
}

#[repr(C)]
struct CompositionAttribData {
    attribute: u32,
    data: *mut core::ffi::c_void,
    size: usize,
}

const ACCENT_ENABLE_ACRYLICBLURBEHIND: u32 = 4;
const WCA_ACCENT_POLICY: u32 = 19;
type SetWindowCompositionAttribute = unsafe extern "system" fn(HWND, *mut CompositionAttribData) -> BOOL;

/// 셸 창과 같은 아크릴 흐림을 켠다. 함수를 찾지 못하면 false.
fn enable_accent_blur(hwnd: HWND) -> bool {
    // SAFETY: user32의 내보낸 함수를 이름으로 찾아 정해진 구조체로 호출한다
    unsafe {
        let Ok(user32) = GetModuleHandleW(w!("user32.dll")) else { return false };
        let Some(proc) = GetProcAddress(user32, s!("SetWindowCompositionAttribute")) else { return false };
        let set: SetWindowCompositionAttribute = std::mem::transmute(proc);
        let mut policy = AccentPolicy {
            state: ACCENT_ENABLE_ACRYLICBLURBEHIND,
            flags: 0,
            gradient_color: 0x0100_0000,
            animation_id: 0,
        };
        let mut data = CompositionAttribData {
            attribute: WCA_ACCENT_POLICY,
            data: (&mut policy as *mut AccentPolicy).cast(),
            size: std::mem::size_of::<AccentPolicy>(),
        };
        set(hwnd, &mut data).as_bool()
    }
}

/// 흐림 배경을 켠다. 아무 방법도 통하지 않으면(Windows 10 일부 등) false → 덧칠을 거의 불투명하게 그린다.
pub fn enable_acrylic(hwnd: HWND, dark: bool) -> bool {
    set_dark(hwnd, dark);
    let corner: DWM_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND;
    set_attr(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &corner);
    if enable_accent_blur(hwnd) {
        return true;
    }
    let margins = MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 };
    // SAFETY: 창 전체를 프레임 영역으로 넓혀 시스템 배경이 보이게 한다
    let _ = unsafe { DwmExtendFrameIntoClientArea(hwnd, &margins) };
    let backdrop: DWM_SYSTEMBACKDROP_TYPE = DWMSBT_TRANSIENTWINDOW;
    set_attr(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, &backdrop)
}
