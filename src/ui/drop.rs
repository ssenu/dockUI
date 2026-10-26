//! 독·패널에 끌어다 놓아 항목 추가하기 (OLE 끌어 놓기 대상).
//!
//! 탐색기의 파일(exe·바로가기·.url)과 브라우저의 주소·링크를 받는다. 받은 것은 그대로 화면 쪽 채널로 넘기고,
//! 무엇을 항목으로 만들지는 앱(`App`)이 정한다. 끌어 놓기 콜백은 UI 스레드에서 불린다.

use crate::ui::panel::{PanelCmd, WM_APP_PANEL};
use std::cell::Cell;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use windows::Win32::Foundation::{HGLOBAL, HWND, LPARAM, POINTL, WPARAM};
use windows::Win32::System::Com::{DVASPECT_CONTENT, FORMATETC, IDataObject, STGMEDIUM, TYMED_HGLOBAL};
use windows::Win32::System::DataExchange::RegisterClipboardFormatW;
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::{
    CF_HDROP, CF_UNICODETEXT, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE, IDropTarget, IDropTarget_Impl,
    RegisterDragDrop, ReleaseStgMedium, RevokeDragDrop,
};
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
use windows::core::{Ref, implement, w};

/// 끌어다 놓은 것 하나
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dropped {
    File(PathBuf),
    Url(String),
}

fn format(cf: u16) -> FORMATETC {
    FORMATETC {
        cfFormat: cf,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    }
}

/// 브라우저가 주소·링크를 끌 때 함께 싣는 형식
fn url_format() -> u16 {
    unsafe { RegisterClipboardFormatW(w!("UniformResourceLocatorW")) as u16 }
}

/// HGLOBAL 안의 UTF-16 글자 (끝의 0 앞까지)
fn global_text(h: HGLOBAL) -> Option<String> {
    // SAFETY: 잠근 메모리 크기 안에서만 읽고 다시 푼다
    unsafe {
        let ptr = GlobalLock(h) as *const u16;
        if ptr.is_null() {
            return None;
        }
        let len = GlobalSize(h) / 2;
        let units = std::slice::from_raw_parts(ptr, len);
        let end = units.iter().position(|&u| u == 0).unwrap_or(len);
        let text = String::from_utf16_lossy(&units[..end]);
        let _ = GlobalUnlock(h);
        Some(text)
    }
}

fn get(data: &IDataObject, cf: u16) -> Option<STGMEDIUM> {
    unsafe { data.GetData(&format(cf)) }.ok()
}

fn release(mut medium: STGMEDIUM) {
    unsafe { ReleaseStgMedium(&mut medium) };
}

/// 받을 수 있는 형식이 실려 있는가
fn acceptable(data: &IDataObject) -> bool {
    [CF_HDROP.0, url_format(), CF_UNICODETEXT.0].iter().any(|cf| unsafe { data.QueryGetData(&format(*cf)) }.is_ok())
}

/// 글자가 웹 주소처럼 보이면 (`http(s)://`로 시작하고 빈칸이 없으면) 그 주소
pub fn url_from_text(text: &str) -> Option<String> {
    let t = text.trim();
    let lower = t.to_ascii_lowercase();
    ((lower.starts_with("http://") || lower.starts_with("https://")) && !t.contains(char::is_whitespace))
        .then(|| t.to_string())
}

/// 끌어 온 데이터에서 파일·주소를 꺼낸다. 파일이 있으면 파일만, 없으면 주소 하나.
fn read(data: &IDataObject) -> Vec<Dropped> {
    if let Some(medium) = get(data, CF_HDROP.0) {
        let mut files = Vec::new();
        // SAFETY: CF_HDROP 매체의 HGLOBAL은 HDROP이다
        unsafe {
            let hdrop = HDROP(medium.u.hGlobal.0);
            let count = DragQueryFileW(hdrop, u32::MAX, None);
            for i in 0..count {
                let len = DragQueryFileW(hdrop, i, None) as usize;
                let mut buf = vec![0u16; len + 1];
                let n = DragQueryFileW(hdrop, i, Some(&mut buf)) as usize;
                files.push(Dropped::File(PathBuf::from(String::from_utf16_lossy(&buf[..n]))));
            }
        }
        release(medium);
        if !files.is_empty() {
            return files;
        }
    }
    for cf in [url_format(), CF_UNICODETEXT.0] {
        if let Some(medium) = get(data, cf) {
            let text = unsafe { global_text(medium.u.hGlobal) };
            release(medium);
            if let Some(url) = text.as_deref().and_then(url_from_text) {
                return vec![Dropped::Url(url)];
            }
        }
    }
    Vec::new()
}

#[implement(IDropTarget)]
struct DropTarget {
    /// 화면 쪽 명령 채널 (놓은 것·끌기 상태를 보낸다)
    tx: Sender<PanelCmd>,
    glass: isize,
    accept: Cell<bool>,
}

impl DropTarget {
    fn send(&self, cmd: PanelCmd) {
        if self.tx.send(cmd).is_ok() {
            let _ = unsafe { PostMessageW(Some(HWND(self.glass as *mut _)), WM_APP_PANEL, WPARAM(0), LPARAM(0)) };
        }
    }

    fn effect(&self, out: *mut DROPEFFECT) {
        if !out.is_null() {
            // SAFETY: OLE가 넘긴 출력 포인터
            unsafe { *out = if self.accept.get() { DROPEFFECT_COPY } else { DROPEFFECT_NONE } };
        }
    }
}

impl IDropTarget_Impl for DropTarget_Impl {
    fn DragEnter(
        &self,
        data: Ref<IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        let ok = data.as_ref().is_some_and(acceptable);
        self.accept.set(ok);
        self.effect(effect);
        if ok {
            self.send(PanelCmd::DropHover(true));
        }
        Ok(())
    }

    fn DragOver(&self, _keys: MODIFIERKEYS_FLAGS, _pt: &POINTL, effect: *mut DROPEFFECT) -> windows::core::Result<()> {
        self.effect(effect);
        Ok(())
    }

    fn DragLeave(&self) -> windows::core::Result<()> {
        if self.accept.replace(false) {
            self.send(PanelCmd::DropHover(false));
        }
        Ok(())
    }

    fn Drop(
        &self,
        data: Ref<IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        self.effect(effect);
        self.accept.set(false);
        self.send(PanelCmd::DropHover(false));
        let dropped = data.as_ref().map(read).unwrap_or_default();
        if !dropped.is_empty() {
            self.send(PanelCmd::Dropped(dropped));
        }
        Ok(())
    }
}

/// 창들을 끌어 놓기 대상으로 등록한다 (UI 스레드가 OleInitialize 돼 있어야 한다).
pub fn register(windows: &[HWND], glass: HWND, tx: Sender<PanelCmd>) {
    let target: IDropTarget = DropTarget { tx, glass: glass.0 as isize, accept: Cell::new(false) }.into();
    for hwnd in windows {
        if let Err(e) = unsafe { RegisterDragDrop(*hwnd, &target) } {
            crate::logging::warn(&format!("끌어 놓기 등록 실패: {e}"));
        }
    }
}

pub fn revoke(windows: &[HWND]) {
    for hwnd in windows {
        let _ = unsafe { RevokeDragDrop(*hwnd) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 탐색기가 넘기는 것과 같은 셸 데이터 개체로 놓기 판정·놓기를 거쳐 화면 채널까지 확인한다
    #[test]
    fn dropping_a_real_file_reaches_the_panel_channel() {
        use windows::Win32::UI::Shell::{
            BHID_DataObject, IShellItem, IShellItemArray, SHCreateItemFromParsingName,
            SHCreateShellItemArrayFromShellItem,
        };
        let _ = unsafe { windows::Win32::System::Ole::OleInitialize(None) };
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
        let path = std::path::Path::new(&windir).join("notepad.exe").to_string_lossy().into_owned();
        let item: IShellItem =
            unsafe { SHCreateItemFromParsingName(&windows::core::HSTRING::from(path.as_str()), None) }.unwrap();
        // 탐색기가 선택한 파일을 끌 때처럼 항목 묶음에서 데이터 개체를 얻는다
        let items: IShellItemArray = unsafe { SHCreateShellItemArrayFromShellItem(&item) }.unwrap();
        let data: IDataObject = unsafe { items.BindToHandler(None, &BHID_DataObject) }.unwrap();

        let (tx, rx) = std::sync::mpsc::channel();
        let target: IDropTarget = DropTarget { tx, glass: 0, accept: Cell::new(false) }.into();
        let mut effect = DROPEFFECT_NONE;
        unsafe { target.DragEnter(&data, MODIFIERKEYS_FLAGS(0), POINTL::default(), &mut effect) }.unwrap();
        assert_eq!(effect, DROPEFFECT_COPY);
        assert_eq!(rx.try_recv().unwrap(), PanelCmd::DropHover(true));
        unsafe { target.Drop(&data, MODIFIERKEYS_FLAGS(0), POINTL::default(), &mut effect) }.unwrap();
        assert_eq!(rx.try_recv().unwrap(), PanelCmd::DropHover(false));
        match rx.try_recv().unwrap() {
            PanelCmd::Dropped(items) => {
                assert_eq!(items.len(), 1);
                let Dropped::File(p) = &items[0] else { panic!("파일이어야 함: {items:?}") };
                assert!(p.to_string_lossy().to_ascii_lowercase().ends_with("notepad.exe"), "{p:?}");
            }
            other => panic!("Dropped가 와야 함: {other:?}"),
        }
    }

    #[test]
    fn only_http_text_counts_as_url() {
        assert_eq!(url_from_text(" https://naver.com/a?b=1 "), Some("https://naver.com/a?b=1".into()));
        assert_eq!(url_from_text("HTTP://x.kr"), Some("HTTP://x.kr".into()));
        assert_eq!(url_from_text("naver.com"), None);
        assert_eq!(url_from_text("https://a.com 그리고 글"), None);
        assert_eq!(url_from_text(""), None);
    }
}
