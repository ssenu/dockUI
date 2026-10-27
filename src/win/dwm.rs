//! DWM(데스크톱 창 관리자) 효과: 흐림(아크릴) 배경, 어두운 톤, 둥근 모서리.
//!
//! 테두리 없는 팝업 창에는 Windows 11의 시스템 배경(`DWMWA_SYSTEMBACKDROP_TYPE`)이 그려지지 않는다.
//! 그래서 작업표시줄·시작 메뉴 같은 셸 창이 쓰는 `SetWindowCompositionAttribute`의 아크릴 흐림을 먼저 쓰고,
//! 그것이 없을 때만 시스템 배경을 시도한다.

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DWM_SYSTEMBACKDROP_TYPE, DWM_WINDOW_CORNER_PREFERENCE, DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE,
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DWMWINDOWATTRIBUTE,
    DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
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

const DWMWA_BORDER_COLOR: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(34);
/// 테두리 그리지 않기 / 시스템 기본 테두리
const COLOR_NONE: u32 = 0xFFFF_FFFE;
const COLOR_DEFAULT: u32 = 0xFFFF_FFFF;

/// 창 외곽: 둥근 모서리는 늘 켜고(판 모서리와 맞춤), 테두리선은 `border`일 때만 (독은 끈다)
pub fn set_frame(hwnd: HWND, border: bool) {
    let corner: DWM_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND;
    set_attr(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &corner);
    set_attr(hwnd, DWMWA_BORDER_COLOR, &(if border { COLOR_DEFAULT } else { COLOR_NONE }));
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

/// 아크릴 흐림을 끈다 (사용자 지정 테마에서 흐림 끄기)
pub fn disable_blur(hwnd: HWND) {
    set_accent(hwnd, ACCENT_DISABLED);
}

const ACCENT_DISABLED: u32 = 0;

/// 셸 창과 같은 아크릴 흐림을 켠다. 함수를 찾지 못하면 false.
fn enable_accent_blur(hwnd: HWND) -> bool {
    set_accent(hwnd, ACCENT_ENABLE_ACRYLICBLURBEHIND)
}

/// 창의 강조(흐림) 정책을 정한다. 함수를 찾지 못하면 false.
fn set_accent(hwnd: HWND, state: u32) -> bool {
    // SAFETY: user32의 내보낸 함수를 이름으로 찾아 정해진 구조체로 호출한다
    unsafe {
        let Ok(user32) = GetModuleHandleW(w!("user32.dll")) else { return false };
        let Some(proc) = GetProcAddress(user32, s!("SetWindowCompositionAttribute")) else { return false };
        let set: SetWindowCompositionAttribute = std::mem::transmute(proc);
        let mut policy = AccentPolicy { state, flags: 0, gradient_color: 0x0100_0000, animation_id: 0 };
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::win::msg_window::{Handler, WindowSpec, create_window};
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        BITMAPINFO, BITMAPINFOHEADER, BLACK_BRUSH, BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC,
        CreateRoundRectRgn, DIB_RGB_COLORS, DeleteDC, DeleteObject, EndPaint, FillRect, GetDC, GetDIBits,
        GetStockObject, HBRUSH, PAINTSTRUCT, ReleaseDC, SRCCOPY, SelectObject, SetWindowRgn,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        DestroyWindow, DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, SW_SHOWNOACTIVATE, SWP_NOACTIVATE,
        SWP_NOCOPYBITS, SWP_NOOWNERZORDER, SWP_NOSENDCHANGING, SWP_NOZORDER, SetWindowPos, ShowWindow,
        TranslateMessage, WM_PAINT, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    };

    struct Black;
    impl Handler for Black {
        fn handle(&mut self, hwnd: HWND, msg: u32, _: WPARAM, _: LPARAM) -> Option<LRESULT> {
            if msg == WM_PAINT {
                let mut ps = PAINTSTRUCT::default();
                unsafe {
                    let dc = BeginPaint(hwnd, &mut ps);
                    FillRect(dc, &ps.rcPaint, HBRUSH(GetStockObject(BLACK_BRUSH).0));
                    let _ = EndPaint(hwnd, &ps);
                }
                return Some(LRESULT(0));
            }
            None
        }
    }

    fn pump(ms: u64) {
        let end = std::time::Instant::now() + std::time::Duration::from_millis(ms);
        let mut msg = MSG::default();
        while std::time::Instant::now() < end {
            unsafe {
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    fn capture_bmp(x: i32, y: i32, w: i32, h: i32, path: &std::path::Path) {
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: h,
                biPlanes: 1,
                biBitCount: 32,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        unsafe {
            let screen = GetDC(None);
            let mem = CreateCompatibleDC(Some(screen));
            let bmp = CreateCompatibleBitmap(screen, w, h);
            let old = SelectObject(mem, bmp.into());
            let _ = BitBlt(mem, 0, 0, w, h, Some(screen), x, y, SRCCOPY);
            SelectObject(mem, old);
            GetDIBits(mem, bmp, 0, h as u32, Some(pixels.as_mut_ptr().cast()), &mut info, DIB_RGB_COLORS);
            let _ = DeleteObject(bmp.into());
            let _ = DeleteDC(mem);
            ReleaseDC(None, screen);
        }
        let mut file = Vec::new();
        file.extend_from_slice(b"BM");
        file.extend_from_slice(&(54 + pixels.len() as u32).to_le_bytes());
        file.extend_from_slice(&0u32.to_le_bytes());
        file.extend_from_slice(&54u32.to_le_bytes());
        file.extend_from_slice(&40u32.to_le_bytes());
        file.extend_from_slice(&w.to_le_bytes());
        file.extend_from_slice(&h.to_le_bytes());
        file.extend_from_slice(&1u16.to_le_bytes());
        file.extend_from_slice(&32u16.to_le_bytes());
        file.extend_from_slice(&[0u8; 24]);
        file.extend_from_slice(&pixels);
        std::fs::write(path, file).unwrap();
    }

    /// 실험: 흐림 창에 판 모양 영역을 걸 때 다시 그리기(bRedraw) 여부에 따라 흐림이 모양을 따르는지 화면으로 본다.
    /// 사용자 화면에 0.5초쯤 시험 창 두 개가 뜬다: `cargo test blur_region_probe -- --ignored`
    #[test]
    #[ignore]
    fn blur_region_probe() {
        let (x0, y0, w, h) = (300, 200, 100, 400);
        let mut wins = Vec::new();
        for i in 0..3 {
            let spec = WindowSpec {
                class: "DockUIBlurProbe",
                title: "probe",
                ex_style: WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                style: WS_POPUP,
                rect: (x0 + i * 160, y0, w, h),
                owner: None,
            };
            let hwnd = create_window(&spec, Box::new(Black)).unwrap();
            enable_acrylic(hwnd, true);
            set_frame(hwnd, false);
            let _ = unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
            wins.push(hwnd);
        }
        pump(300);
        // 아래 60px을 잘라낸 둥근 판 모양: 왼쪽은 다시 그리기 없이, 오른쪽은 다시 그리기와 함께
        for (i, hwnd) in wins.iter().take(2).enumerate() {
            unsafe {
                let rgn = CreateRoundRectRgn(0, 20, w + 1, h - 60 + 1, 16, 16);
                let _ = SetWindowRgn(*hwnd, Some(rgn), i == 1);
            }
        }
        // 세 번째: 영역 대신 창 크기 자체를 같은 판 크기로 줄인다 (독이 쓰는 방식)
        let _ = unsafe {
            SetWindowPos(
                wins[2],
                None,
                x0 + 320,
                y0 + 20,
                w,
                h - 80,
                SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOSENDCHANGING | SWP_NOCOPYBITS,
            )
        };
        pump(500);
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("snapshots");
        std::fs::create_dir_all(&dir).unwrap();
        capture_bmp(x0 - 20, y0 - 20, 480, h + 40, &dir.join("blur-probe.bmp"));
        for hwnd in wins {
            let _ = unsafe { DestroyWindow(hwnd) };
        }
    }
}
