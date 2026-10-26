//! 마우스 위치에 뜨는 팝업 메뉴 (트레이 메뉴, ⋯ 메뉴, 항목 우클릭 메뉴 공용).

use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, MF_CHECKED, MF_GRAYED, MF_SEPARATOR, MF_STRING,
    SetForegroundWindow, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu,
};
use windows::core::HSTRING;

pub enum MenuEntry {
    Item { id: u32, text: String, checked: bool, enabled: bool },
    Separator,
}

impl MenuEntry {
    pub fn item(id: u32, text: impl Into<String>) -> Self {
        MenuEntry::Item { id, text: text.into(), checked: false, enabled: true }
    }
    pub fn checked(id: u32, text: impl Into<String>, checked: bool) -> Self {
        MenuEntry::Item { id, text: text.into(), checked, enabled: true }
    }
    pub fn label(text: impl Into<String>) -> Self {
        MenuEntry::Item { id: 0, text: text.into(), checked: false, enabled: false }
    }
}

/// 마우스 위치에 메뉴를 띄우고 고른 항목 id를 돌려준다 (취소하면 0).
pub fn show_popup(owner: HWND, entries: &[MenuEntry]) -> u32 {
    // SAFETY: 메뉴는 이 함수 안에서 만들고 없앤다
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return 0 };
        for entry in entries {
            match entry {
                MenuEntry::Separator => {
                    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
                }
                MenuEntry::Item { id, text, checked, enabled } => {
                    let mut flags = MF_STRING;
                    if *checked {
                        flags |= MF_CHECKED;
                    }
                    if !*enabled {
                        flags |= MF_GRAYED;
                    }
                    let _ = AppendMenuW(menu, flags, *id as usize, &HSTRING::from(text.as_str()));
                }
            }
        }
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        // 메뉴 바깥을 누르면 닫히도록 먼저 전면으로
        let _ = SetForegroundWindow(owner);
        let chosen = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, pt.x, pt.y, None, owner, None);
        let _ = DestroyMenu(menu);
        chosen.0 as u32
    }
}
