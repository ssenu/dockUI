//! 앱 선택 창: 시작 메뉴 "모든 앱" 목록(검색, 아이콘) + exe/lnk 직접 찾기.
//!
//! 목록은 백그라운드 스레드가 읽고, 아이콘은 화면에 보이는 줄이 요청할 때만 같은 스레드가 하나씩 만든다.

use crate::ui::dialogs::{Ctl, centered_on, child, message_font, paint_background, run_modal, warn, window_text};
use crate::win::msg_window::{Handler, WindowSpec, create_window};
use crate::win::shell_items::{AppEntry, apps_folder_path, file_app, item_image, list_apps};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{DeleteObject, HBITMAP, HGDIOBJ, InvalidateRect};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_NODEREFERENCELINKS, OPENFILENAMEW,
};
use windows::Win32::UI::Controls::{
    HIMAGELIST, I_IMAGENONE, ICC_LISTVIEW_CLASSES, ILC_COLOR32, INITCOMMONCONTROLSEX, ImageList_Add, ImageList_Create,
    ImageList_Destroy, InitCommonControlsEx, LIST_VIEW_ITEM_STATE_FLAGS, LVCF_WIDTH, LVCOLUMNW, LVIF_IMAGE, LVIF_TEXT,
    LVIS_FOCUSED, LVIS_SELECTED, LVITEMW, LVM_ENSUREVISIBLE, LVM_GETNEXTITEM, LVM_INSERTCOLUMNW,
    LVM_SETEXTENDEDLISTVIEWSTYLE, LVM_SETIMAGELIST, LVM_SETITEMCOUNT, LVM_SETITEMSTATE, LVN_GETDISPINFOW,
    LVNI_SELECTED, LVS_EX_DOUBLEBUFFER, LVS_EX_FULLROWSELECT, LVS_NOCOLUMNHEADER, LVS_OWNERDATA, LVS_REPORT,
    LVS_SHOWSELALWAYS, LVS_SINGLESEL, LVSIL_SMALL, NM_DBLCLK, NMHDR, NMLVDISPINFOW,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, EN_CHANGE, GetDlgItem, IDCANCEL, IDOK, PostMessageW, SW_HIDE, SW_SHOW, SendMessageW, SetWindowTextW,
    ShowWindow, WINDOW_STYLE, WM_APP, WM_CLOSE, WM_COMMAND, WM_CTLCOLORSTATIC, WM_DESTROY, WM_ERASEBKGND, WM_NOTIFY,
    WS_BORDER, WS_CAPTION, WS_EX_CLIENTEDGE, WS_EX_DLGMODALFRAME, WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_VSCROLL,
};
use windows::core::{HSTRING, PWSTR};

pub use crate::win::shell_items::AppEntry as AppChoice;

const PICKER_CLASS: &str = "YTLauncherAppPicker";
const ID_SEARCH: i32 = 201;
const ID_LIST: i32 = 202;
const ID_STATUS: i32 = 203;
const ID_BROWSE: i32 = 204;
/// 백그라운드 스레드 결과가 채널에 들어왔음
const WM_APP_LOADED: u32 = WM_APP + 21;

/// 검색어에 맞는 앱의 인덱스 (이름 부분 일치, 대소문자 무시). 순수 함수.
pub fn filter_apps(apps: &[AppEntry], query: &str) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    apps.iter()
        .enumerate()
        .filter(|(_, a)| q.is_empty() || a.name.to_lowercase().contains(&q))
        .map(|(i, _)| i)
        .collect()
}

enum Loaded {
    Apps(Result<Vec<AppEntry>, String>),
    /// (앱 인덱스, 아이콘 비트맵 핸들)
    Icon(usize, isize),
}

struct PickerState {
    hwnd: HWND,
    list: HWND,
    apps: Vec<AppEntry>,
    visible: Vec<usize>,
    /// 앱마다 이미지 목록 안 번호 (None = 아직 없음)
    icons: Vec<Option<i32>>,
    requested: Vec<bool>,
    images: HIMAGELIST,
    requests: Option<Sender<(usize, String)>>,
    loaded: Receiver<Loaded>,
    text: Vec<u16>,
    result: Option<AppChoice>,
    done: bool,
}

impl PickerState {
    fn status(&self, text: Option<&str>) {
        let status = unsafe { GetDlgItem(Some(self.hwnd), ID_STATUS) }.unwrap_or_default();
        if let Some(text) = text {
            let _ = unsafe { SetWindowTextW(status, &HSTRING::from(text)) };
        }
        let _ = unsafe { ShowWindow(status, if text.is_some() { SW_SHOW } else { SW_HIDE }) };
    }

    fn refilter(&mut self) {
        let search = unsafe { GetDlgItem(Some(self.hwnd), ID_SEARCH) }.unwrap_or_default();
        self.visible = filter_apps(&self.apps, &window_text(search));
        unsafe {
            SendMessageW(self.list, LVM_SETITEMCOUNT, Some(WPARAM(self.visible.len())), Some(LPARAM(0)));
            let _ = InvalidateRect(Some(self.list), None, true);
        }
        if !self.visible.is_empty() {
            self.select_row(0);
        }
        self.status(self.visible.is_empty().then_some("찾는 앱이 없습니다."));
    }

    fn select_row(&self, row: usize) {
        let state = LIST_VIEW_ITEM_STATE_FLAGS(LVIS_SELECTED.0 | LVIS_FOCUSED.0);
        let mut item = LVITEMW { stateMask: state, state, ..Default::default() };
        unsafe {
            SendMessageW(self.list, LVM_SETITEMSTATE, Some(WPARAM(row)), Some(LPARAM(&mut item as *mut _ as isize)));
            SendMessageW(self.list, LVM_ENSUREVISIBLE, Some(WPARAM(row)), Some(LPARAM(0)));
        }
    }

    fn selected(&self) -> Option<AppEntry> {
        let row = unsafe {
            SendMessageW(self.list, LVM_GETNEXTITEM, Some(WPARAM(usize::MAX)), Some(LPARAM(LVNI_SELECTED as isize)))
        };
        let row = usize::try_from(row.0).ok()?;
        self.visible.get(row).map(|&i| self.apps[i].clone())
    }

    fn on_loaded(&mut self) {
        while let Ok(msg) = self.loaded.try_recv() {
            match msg {
                Loaded::Apps(Ok(apps)) => {
                    self.icons = vec![None; apps.len()];
                    self.requested = vec![false; apps.len()];
                    self.apps = apps;
                    self.refilter();
                }
                Loaded::Apps(Err(e)) => {
                    self.status(Some(&format!("앱 목록을 불러오지 못했습니다. [직접 찾기]를 쓰세요.\n({e})")));
                }
                Loaded::Icon(index, bitmap) => {
                    let bitmap = HBITMAP(bitmap as *mut _);
                    // SAFETY: 받은 비트맵을 이미지 목록에 복사하고 원본은 지운다
                    let slot = unsafe {
                        let slot = ImageList_Add(self.images, bitmap, None);
                        let _ = DeleteObject(HGDIOBJ(bitmap.0));
                        slot
                    };
                    if slot >= 0 && index < self.icons.len() {
                        self.icons[index] = Some(slot);
                        let _ = unsafe { InvalidateRect(Some(self.list), None, false) };
                    }
                }
            }
        }
    }

    /// 목록이 한 줄을 그리려 할 때: 이름을 주고, 아이콘이 없으면 만들어 달라고 요청한다.
    fn display_info(&mut self, info: &mut NMLVDISPINFOW) {
        let row = info.item.iItem as usize;
        let Some(&index) = self.visible.get(row) else { return };
        if info.item.mask.contains(LVIF_TEXT) && !info.item.pszText.is_null() && info.item.cchTextMax > 0 {
            self.text = self.apps[index].name.encode_utf16().take(info.item.cchTextMax as usize - 1).collect();
            self.text.push(0);
            // SAFETY: 목록이 준 버퍼 크기 안에서 복사한다
            unsafe { std::ptr::copy_nonoverlapping(self.text.as_ptr(), info.item.pszText.0, self.text.len()) };
        }
        if info.item.mask.contains(LVIF_IMAGE) {
            info.item.iImage = self.icons[index].unwrap_or(I_IMAGENONE);
            if self.icons[index].is_none() && !self.requested[index] {
                self.requested[index] = true;
                if let Some(tx) = &self.requests {
                    let _ = tx.send((index, self.apps[index].target.clone()));
                }
            }
        }
    }
}

struct PickerWindow {
    state: Rc<RefCell<PickerState>>,
}

impl PickerWindow {
    fn finish(&self, hwnd: HWND, result: Option<AppChoice>) {
        {
            let mut st = self.state.borrow_mut();
            st.result = result;
            st.done = true;
            st.requests = None;
        }
        let _ = unsafe { DestroyWindow(hwnd) };
    }
}

fn browse(owner: HWND) -> Option<AppChoice> {
    let filter: Vec<u16> = "프로그램 (*.exe, *.lnk)\0*.exe;*.lnk\0모든 파일\0*.*\0\0".encode_utf16().collect();
    let mut file = vec![0u16; 1024];
    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner,
        lpstrFilter: windows::core::PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(file.as_mut_ptr()),
        nMaxFile: file.len() as u32,
        // 바로가기(.lnk)를 고르면 대상이 아니라 바로가기 자체를 받는다
        Flags: OFN_FILEMUSTEXIST | OFN_NODEREFERENCELINKS,
        ..Default::default()
    };
    // SAFETY: 버퍼와 필터는 호출이 끝날 때까지 살아 있다
    if !unsafe { GetOpenFileNameW(&mut ofn) }.as_bool() {
        return None;
    }
    let len = file.iter().position(|&c| c == 0).unwrap_or(file.len());
    Some(file_app(&String::from_utf16_lossy(&file[..len])))
}

impl Handler for PickerWindow {
    fn handle(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        match msg {
            WM_COMMAND => {
                let id = (wparam.0 & 0xFFFF) as i32;
                let code = (wparam.0 >> 16) as u32;
                if id == IDOK.0 {
                    let (selected, only) = {
                        let st = self.state.borrow();
                        let only = (st.visible.len() == 1).then(|| st.apps[st.visible[0]].clone());
                        (st.selected(), only)
                    };
                    match selected.or(only) {
                        Some(app) => self.finish(hwnd, Some(app)),
                        None => warn(hwnd, "목록에서 앱을 고르세요."),
                    }
                } else if id == IDCANCEL.0 {
                    self.finish(hwnd, None);
                } else if id == ID_BROWSE {
                    if let Some(app) = browse(hwnd) {
                        self.finish(hwnd, Some(app));
                    }
                } else if id == ID_SEARCH && code == EN_CHANGE {
                    self.state.borrow_mut().refilter();
                }
                Some(LRESULT(0))
            }
            WM_NOTIFY => {
                // SAFETY: WM_NOTIFY의 lparam은 NMHDR로 시작하는 알림 구조체
                let header = unsafe { &*(lparam.0 as *const NMHDR) };
                if header.idFrom != ID_LIST as usize {
                    return None;
                }
                if header.code == LVN_GETDISPINFOW {
                    let info = unsafe { &mut *(lparam.0 as *mut NMLVDISPINFOW) };
                    self.state.borrow_mut().display_info(info);
                } else if header.code == NM_DBLCLK {
                    let selected = self.state.borrow().selected();
                    if selected.is_some() {
                        self.finish(hwnd, selected);
                    }
                }
                Some(LRESULT(0))
            }
            WM_APP_LOADED => {
                self.state.borrow_mut().on_loaded();
                Some(LRESULT(0))
            }
            WM_ERASEBKGND | WM_CTLCOLORSTATIC => paint_background(hwnd, msg, wparam),
            WM_CLOSE => {
                self.finish(hwnd, None);
                Some(LRESULT(0))
            }
            WM_DESTROY => None,
            _ => None,
        }
    }
}

/// 백그라운드 스레드: 앱 목록을 읽어 보내고, 요청받은 아이콘을 하나씩 만든다. 요청 채널이 닫히면 끝난다.
fn spawn_loader(hwnd: HWND, px: i32) -> (Sender<(usize, String)>, Receiver<Loaded>) {
    let (req_tx, req_rx) = channel::<(usize, String)>();
    let (out_tx, out_rx) = channel();
    let hwnd = hwnd.0 as isize;
    std::thread::spawn(move || {
        let post = |m: Loaded| {
            if out_tx.send(m).is_ok() {
                let _ = unsafe { PostMessageW(Some(HWND(hwnd as *mut _)), WM_APP_LOADED, WPARAM(0), LPARAM(0)) };
            }
        };
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        post(Loaded::Apps(list_apps().map_err(|e| e.message())));
        while let Ok((index, target)) = req_rx.recv() {
            if let Some(bitmap) = item_image(&apps_folder_path(&target), px) {
                post(Loaded::Icon(index, bitmap.0 as isize));
            }
        }
        unsafe { CoUninitialize() };
    });
    (req_tx, out_rx)
}

/// 앱 선택 창을 띄우고 닫힐 때까지 기다린다. 고르면 `Some`.
pub fn pick_app(owner: HWND) -> Option<AppChoice> {
    let dpi = unsafe { GetDpiForWindow(owner) }.max(96);
    let s = |v: i32| v * dpi as i32 / 96;
    let (w, h) = (s(436), s(540));
    let (x, y) = centered_on(owner, w, h);
    let icc = INITCOMMONCONTROLSEX {
        dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
        dwICC: ICC_LISTVIEW_CLASSES,
    };
    let _ = unsafe { InitCommonControlsEx(&icc) };
    let icon_px = s(32);
    let images = unsafe { ImageList_Create(icon_px, icon_px, ILC_COLOR32, 64, 64) };
    let (_, placeholder) = channel();
    let state = Rc::new(RefCell::new(PickerState {
        hwnd: HWND::default(),
        list: HWND::default(),
        apps: Vec::new(),
        visible: Vec::new(),
        icons: Vec::new(),
        requested: Vec::new(),
        images,
        requests: None,
        loaded: placeholder,
        text: Vec::new(),
        result: None,
        done: false,
    }));
    let spec = WindowSpec {
        class: PICKER_CLASS,
        title: "앱 선택",
        ex_style: WS_EX_DLGMODALFRAME,
        style: WS_POPUP | WS_CAPTION | WS_SYSMENU,
        rect: (x, y, w, h),
        owner: Some(owner),
    };
    let picker = create_window(&spec, Box::new(PickerWindow { state: state.clone() })).ok()?;
    let font = message_font(dpi);
    let search = child(picker, font, Ctl::edit("", (s(16), s(14), s(388), s(26)), ID_SEARCH));
    let list_style = WINDOW_STYLE(LVS_REPORT | LVS_OWNERDATA | LVS_SINGLESEL | LVS_NOCOLUMNHEADER | LVS_SHOWSELALWAYS)
        | WS_TABSTOP
        | WS_VSCROLL
        | WS_BORDER;
    let list = child(
        picker,
        font,
        Ctl {
            class: "SysListView32",
            text: "",
            style: list_style,
            ex: WS_EX_CLIENTEDGE,
            rect: (s(16), s(50), s(388), s(388)),
            id: ID_LIST,
        },
    );
    child(picker, font, Ctl::label("불러오는 중…", (s(28), s(62), s(364), s(40)), ID_STATUS));
    child(picker, font, Ctl::button("직접 찾기…", (s(16), s(452), s(110), s(30)), ID_BROWSE, false));
    child(picker, font, Ctl::button("선택", (s(224), s(452), s(86), s(30)), IDOK.0, true));
    child(picker, font, Ctl::button("취소", (s(318), s(452), s(86), s(30)), IDCANCEL.0, false));
    unsafe {
        let ex = (LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER) as isize;
        SendMessageW(list, LVM_SETEXTENDEDLISTVIEWSTYLE, Some(WPARAM(ex as usize)), Some(LPARAM(ex)));
        SendMessageW(list, LVM_SETIMAGELIST, Some(WPARAM(LVSIL_SMALL as usize)), Some(LPARAM(images.0)));
        let column = LVCOLUMNW { mask: LVCF_WIDTH, cx: s(360), ..Default::default() };
        SendMessageW(list, LVM_INSERTCOLUMNW, Some(WPARAM(0)), Some(LPARAM(&column as *const _ as isize)));
    }
    let (requests, loaded) = spawn_loader(picker, icon_px);
    {
        let mut st = state.borrow_mut();
        st.hwnd = picker;
        st.list = list;
        st.requests = Some(requests);
        st.loaded = loaded;
    }
    run_modal(owner, picker, search, &|| state.borrow().done);
    let mut st = state.borrow_mut();
    st.requests = None;
    // 아직 오지 않은 아이콘 비트맵은 정리한다
    while let Ok(msg) = st.loaded.try_recv() {
        if let Loaded::Icon(_, bitmap) = msg {
            let _ = unsafe { DeleteObject(HGDIOBJ(bitmap as *mut _)) };
        }
    }
    let _ = unsafe { ImageList_Destroy(Some(st.images)) };
    let _ = unsafe { DeleteObject(HGDIOBJ(font.0)) };
    st.result.take()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::win::msg_window::create;
    use windows::Win32::UI::Controls::LVM_GETITEMCOUNT;
    use windows::Win32::UI::WindowsAndMessaging::FindWindowW;

    fn app(name: &str) -> AppEntry {
        AppEntry { name: name.into(), target: name.into(), exe: None }
    }

    #[test]
    fn filters_by_name_case_insensitively() {
        let apps = vec![app("Google Chrome"), app("계산기"), app("KakaoTalk"), app("Chrome Remote Desktop")];
        assert_eq!(filter_apps(&apps, ""), vec![0, 1, 2, 3]);
        assert_eq!(filter_apps(&apps, "  chrome "), vec![0, 3]);
        assert_eq!(filter_apps(&apps, "계산"), vec![1]);
        assert!(filter_apps(&apps, "없는앱").is_empty());
    }

    struct Owner;
    impl Handler for Owner {
        fn handle(&mut self, _: HWND, _: u32, _: WPARAM, _: LPARAM) -> Option<LRESULT> {
            None
        }
    }

    #[test]
    fn loads_list_and_picks_selected_app() {
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let owner = create("YTLauncherTestPickerOwner", Box::new(Owner)).unwrap();
        // 목록이 채워지면 [선택]을 누른다: 첫 줄이 자동으로 선택돼 있다
        std::thread::spawn(|| {
            for _ in 0..400 {
                std::thread::sleep(std::time::Duration::from_millis(50));
                let Ok(h) = (unsafe { FindWindowW(&HSTRING::from(PICKER_CLASS), None) }) else { continue };
                let list = unsafe { GetDlgItem(Some(h), ID_LIST) }.unwrap_or_default();
                let count = unsafe { SendMessageW(list, LVM_GETITEMCOUNT, None, None) }.0;
                if !h.0.is_null() && count > 0 {
                    let _ = unsafe { PostMessageW(Some(h), WM_COMMAND, WPARAM(IDOK.0 as usize), LPARAM(0)) };
                    return;
                }
            }
        });
        let picked = pick_app(owner).expect("첫 앱이 선택돼야 한다");
        let first = list_apps().unwrap().into_iter().next().unwrap();
        assert_eq!(picked, first);
    }
}
