//! 셸 항목: 시작 메뉴 "모든 앱" 목록(shell:AppsFolder), 앱·파일 아이콘 추출, 창의 앱 ID(AUMID).
//!
//! 모두 COM을 쓰므로 호출하는 스레드에서 COM이 초기화돼 있어야 한다.

use std::path::Path;
use windows::Win32::Foundation::GENERIC_WRITE;
use windows::Win32::Foundation::{HWND, SIZE};
use windows::Win32::Graphics::Gdi::{DeleteObject, HBITMAP, HGDIOBJ, HPALETTE};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_ContainerFormatPng, IWICImagingFactory, WICBitmapEncoderNoCache, WICBitmapUseAlpha,
};
use windows::Win32::Storage::EnhancedStorage::{PKEY_AppUserModel_ID, PKEY_Link_TargetParsingPath};
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree};
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow};
use windows::Win32::UI::Shell::{
    BHID_EnumItems, IEnumShellItems, IShellItem, IShellItem2, IShellItemImageFactory, SHCreateItemFromParsingName,
    SIGDN_NORMALDISPLAY, SIGDN_PARENTRELATIVEPARSING, SIIGBF_ICONONLY,
};
use windows::core::{GUID, HSTRING, Interface, PWSTR};

pub const APPS_FOLDER: &str = "shell:AppsFolder";

/// 시작 메뉴 "모든 앱"의 한 줄
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppEntry {
    pub name: String,
    /// shell:AppsFolder 안의 파싱 이름 (Store 앱은 AUMID)
    pub target: String,
    /// 실행 파일 경로 (데스크톱 앱이고 알아낼 수 있을 때만)
    pub exe: Option<String>,
}

fn take_pwstr(p: PWSTR) -> String {
    // SAFETY: 셸이 CoTaskMemAlloc으로 준 문자열을 읽고 해제한다
    unsafe {
        let s = p.to_string().unwrap_or_default();
        CoTaskMemFree(Some(p.0 as *const _));
        s
    }
}

/// `{알려진 폴더 GUID}\나머지` 형식이면 실제 경로로 바꾼다 (예: {Program Files}\Google\Chrome\…\chrome.exe)
pub fn expand_known_folder(target: &str) -> Option<String> {
    let rest = target.strip_prefix('{')?;
    let (guid, tail) = rest.split_once("}\\")?;
    let guid = GUID::try_from(guid).ok()?;
    // SAFETY: 알려진 폴더 경로를 받아 해제한다
    let base = unsafe {
        windows::Win32::UI::Shell::SHGetKnownFolderPath(&guid, windows::Win32::UI::Shell::KF_FLAG_DEFAULT, None)
            .ok()
            .map(take_pwstr)?
    };
    Some(Path::new(&base).join(tail).to_string_lossy().into_owned())
}

fn is_exe(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".exe")
}

fn item_exe(item: &IShellItem, target: &str) -> Option<String> {
    if let Some(path) = expand_known_folder(target).filter(|p| is_exe(p)) {
        return Some(path);
    }
    if is_exe(target) && Path::new(target).is_absolute() {
        return Some(target.to_string());
    }
    let item2: IShellItem2 = item.cast().ok()?;
    // SAFETY: 바로가기 대상 경로 속성을 문자열로 읽는다
    let link = unsafe { item2.GetString(&PKEY_Link_TargetParsingPath) }.ok().map(take_pwstr)?;
    is_exe(&link).then_some(link)
}

/// 시작 메뉴 "모든 앱"을 이름순으로 돌려준다.
pub fn list_apps() -> windows::core::Result<Vec<AppEntry>> {
    // SAFETY: 셸 폴더를 열어 항목을 하나씩 읽는다
    unsafe {
        let folder: IShellItem = SHCreateItemFromParsingName(&HSTRING::from(APPS_FOLDER), None)?;
        let items: IEnumShellItems = folder.BindToHandler(None, &BHID_EnumItems)?;
        let mut apps = Vec::new();
        loop {
            let mut fetched = [None];
            if items.Next(&mut fetched, None).is_err() {
                break;
            }
            let Some(item) = fetched[0].take() else { break };
            let name = item.GetDisplayName(SIGDN_NORMALDISPLAY).map(take_pwstr).unwrap_or_default();
            let target = item.GetDisplayName(SIGDN_PARENTRELATIVEPARSING).map(take_pwstr).unwrap_or_default();
            if name.is_empty() || target.is_empty() {
                continue;
            }
            let exe = item_exe(&item, &target);
            apps.push(AppEntry { name, target, exe });
        }
        apps.sort_by_key(|a| a.name.to_lowercase());
        apps.dedup_by(|a, b| a.target == b.target);
        Ok(apps)
    }
}

/// 직접 고른 exe/lnk 파일을 앱 항목으로: 이름은 파일 이름, 바로가기면 대상 exe를 찾아 둔다.
pub fn file_app(path: &str) -> AppEntry {
    let name = Path::new(path).file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.into());
    // SAFETY: 파일 경로로 셸 항목을 만든다
    let item: Option<IShellItem> = unsafe { SHCreateItemFromParsingName(&HSTRING::from(path), None) }.ok();
    let exe = item.and_then(|item| item_exe(&item, path));
    AppEntry { name, target: path.to_string(), exe }
}

/// 셸 항목(앱 목록 항목 또는 파일 경로)의 아이콘을 `size`×`size`px 비트맵으로 얻는다. 호출자가 DeleteObject 해야 한다.
pub fn item_image(parsing_name: &str, size: i32) -> Option<HBITMAP> {
    // SAFETY: 셸 항목 이미지 팩토리로 비트맵을 얻는다
    unsafe {
        let item: IShellItem = SHCreateItemFromParsingName(&HSTRING::from(parsing_name), None).ok()?;
        let factory: IShellItemImageFactory = item.cast().ok()?;
        factory.GetImage(SIZE { cx: size, cy: size }, SIIGBF_ICONONLY).ok()
    }
}

/// 셸 경로(파일 경로 또는 shell:AppsFolder\…)가 가리키는 항목이 있는가
pub fn item_exists(parsing_name: &str) -> bool {
    // SAFETY: 셸 항목을 만들어 보기만 한다
    unsafe { SHCreateItemFromParsingName::<_, _, IShellItem>(&HSTRING::from(parsing_name), None) }.is_ok()
}

/// 앱 목록 항목의 셸 경로
pub fn apps_folder_path(target: &str) -> String {
    format!("{APPS_FOLDER}\\{target}")
}

/// 비트맵을 PNG로 저장하고 비트맵은 해제한다.
pub fn save_bitmap_png(bitmap: HBITMAP, path: &Path) -> windows::core::Result<()> {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // SAFETY: WIC로 비트맵을 감싸 PNG로 쓴다
    let result = unsafe {
        (|| {
            let wic: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
            let source = wic.CreateBitmapFromHBITMAP(bitmap, HPALETTE::default(), WICBitmapUseAlpha)?;
            let stream = wic.CreateStream()?;
            stream.InitializeFromFilename(&HSTRING::from(path.as_os_str()), GENERIC_WRITE.0)?;
            let encoder = wic.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())?;
            encoder.Initialize(&stream, WICBitmapEncoderNoCache)?;
            let mut frame = None;
            encoder.CreateNewFrame(&mut frame, std::ptr::null_mut())?;
            let frame = frame.ok_or_else(windows::core::Error::empty)?;
            frame.Initialize(None)?;
            frame.WriteSource(&source, std::ptr::null())?;
            frame.Commit()?;
            encoder.Commit()
        })()
    };
    let _ = unsafe { DeleteObject(HGDIOBJ(bitmap.0)) };
    result
}

/// 셸 항목 아이콘을 PNG 파일로 저장한다.
pub fn save_item_icon(parsing_name: &str, path: &Path, size: i32) -> bool {
    match item_image(parsing_name, size) {
        Some(bitmap) => save_bitmap_png(bitmap, path).is_ok(),
        None => false,
    }
}

/// 창에 붙은 앱 ID (Store 앱 창, 작업표시줄 그룹용 ID를 단 데스크톱 앱 창)
pub fn window_app_id(hwnd: HWND) -> Option<String> {
    // SAFETY: 창 속성 저장소에서 문자열 하나를 읽는다
    unsafe {
        let store: IPropertyStore = SHGetPropertyStoreForWindow(hwnd).ok()?;
        let value = store.GetValue(&PKEY_AppUserModel_ID).ok()?;
        let text = PropVariantToStringAlloc(&value).ok().map(take_pwstr)?;
        (!text.is_empty()).then_some(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn com() {
        let _ = unsafe {
            windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)
        };
    }

    #[test]
    fn lists_start_menu_apps_sorted() {
        com();
        let apps = list_apps().unwrap();
        assert!(!apps.is_empty(), "모든 앱 목록이 비어 있다");
        let names: Vec<String> = apps.iter().map(|a| a.name.to_lowercase()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
        assert!(apps.iter().all(|a| !a.target.is_empty()));
    }

    #[test]
    fn expands_known_folder_paths() {
        com();
        // {Windows} 폴더
        let p = expand_known_folder(r"{F38BF404-1D43-42F2-9305-67DE0B28FC23}\System32\notepad.exe").unwrap();
        assert!(p.to_ascii_lowercase().ends_with(r"\windows\system32\notepad.exe"), "{p}");
        assert_eq!(expand_known_folder("Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"), None);
    }

    #[test]
    fn file_app_uses_file_name_and_exe_path() {
        com();
        let notepad = std::env::var("WINDIR").unwrap() + r"\System32\notepad.exe";
        let app = file_app(&notepad);
        assert_eq!(app.name, "notepad");
        assert_eq!(app.target, notepad);
        assert_eq!(app.exe.as_deref(), Some(notepad.as_str()));
    }

    #[test]
    fn checks_item_existence() {
        com();
        let notepad = std::env::var("WINDIR").unwrap() + r"\System32\notepad.exe";
        assert!(item_exists(&notepad));
        assert!(!item_exists(r"C:\없는\폴더\앱.exe"));
        assert!(!item_exists(&apps_folder_path("Nobody.NoSuchApp_0000000000000!App")));
    }

    #[test]
    fn saves_an_app_icon_as_png() {
        com();
        let apps = list_apps().unwrap();
        let path = std::env::temp_dir().join(format!("ytl-icon-{}.png", crate::items::new_id()));
        let ok = apps.iter().take(10).any(|a| save_item_icon(&apps_folder_path(&a.target), &path, 64));
        assert!(ok);
        assert!(std::fs::metadata(&path).unwrap().len() > 100);
    }
}
