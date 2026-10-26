//! 알림 영역(트레이) 아이콘과 풍선 알림.

use windows::Win32::Foundation::HWND;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, HICON, IMAGE_ICON, LR_DEFAULTCOLOR, LoadImageW, SM_CXSMICON, SM_CYSMICON, WM_APP,
};
use windows::core::PCWSTR;

/// 트레이 아이콘에서 오는 마우스 메시지
pub const WM_APP_TRAY: u32 = WM_APP + 2;
const TRAY_ID: u32 = 1;
/// build.rs가 exe에 넣는 아이콘 리소스 번호
pub const ICON_RESOURCE_ID: u16 = 1;

fn copy_wide(dst: &mut [u16], text: &str) {
    let mut i = 0;
    for unit in text.encode_utf16().take(dst.len() - 1) {
        dst[i] = unit;
        i += 1;
    }
    dst[i] = 0;
}

pub fn app_icon() -> HICON {
    // SAFETY: exe 자신의 아이콘 리소스를 작은 아이콘 크기로 읽는다
    unsafe {
        let module = GetModuleHandleW(None).unwrap_or_default();
        LoadImageW(
            Some(module.into()),
            PCWSTR(ICON_RESOURCE_ID as usize as *const u16),
            IMAGE_ICON,
            GetSystemMetrics(SM_CXSMICON),
            GetSystemMetrics(SM_CYSMICON),
            LR_DEFAULTCOLOR,
        )
        .map(|h| HICON(h.0))
        .unwrap_or_default()
    }
}

pub struct Tray {
    hwnd: HWND,
    icon: HICON,
    tooltip: String,
}

impl Tray {
    pub fn new(hwnd: HWND, tooltip: &str) -> Self {
        Tray { hwnd, icon: app_icon(), tooltip: tooltip.to_string() }
    }

    fn data(&self) -> NOTIFYICONDATAW {
        let mut data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: TRAY_ID,
            ..Default::default()
        };
        copy_wide(&mut data.szTip, &self.tooltip);
        data
    }

    /// 탐색기가 다시 시작된 뒤(TaskbarCreated)에도 다시 호출한다.
    pub fn add(&self) -> bool {
        let mut data = self.data();
        data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        data.uCallbackMessage = WM_APP_TRAY;
        data.hIcon = self.icon;
        unsafe { Shell_NotifyIconW(NIM_ADD, &data) }.as_bool()
    }

    pub fn remove(&self) {
        let data = self.data();
        let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &data) };
    }

    /// 윈도우 알림(풍선)을 띄운다.
    pub fn notify(&self, title: &str, message: &str) {
        let mut data = self.data();
        data.uFlags = NIF_INFO;
        data.dwInfoFlags = NIIF_INFO;
        copy_wide(&mut data.szInfoTitle, title);
        copy_wide(&mut data.szInfo, message);
        let _ = unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) };
    }
}
