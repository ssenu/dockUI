//! 아이콘 캐시(`%APPDATA%\YT-Launcher\icons\<항목id>.png`): 앱 아이콘 추출, 웹 파비콘 받기.

use crate::items::{Item, host_of};
use crate::updater::{Http, UreqHttp};
use crate::win::shell_items::{apps_folder_path, save_item_icon};
use std::io::Read;
use std::path::{Path, PathBuf};
use windows::Win32::Foundation::{E_INVALIDARG, GENERIC_WRITE};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_ContainerFormatBmp, GUID_ContainerFormatGif, GUID_ContainerFormatIco,
    GUID_ContainerFormatJpeg, GUID_ContainerFormatPng, GUID_WICPixelFormat32bppPBGRA, IWICBitmapSource,
    IWICImagingFactory, WICBitmapDitherTypeNone, WICBitmapEncoderNoCache, WICBitmapInterpolationModeFant,
    WICBitmapPaletteTypeMedianCut, WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx};
use windows::core::{GUID, HSTRING, Interface};

/// 아이콘 만들기 하위 프로세스를 부르는 인자: `YTLauncher.exe --fetch-icons <데이터 폴더> <항목 JSON>`
pub const FETCH_ARG: &str = "--fetch-icons";

/// 저장하는 아이콘 크기 (px)
pub const ICON_PX: u32 = 256;
const MAX_DOWNLOAD: u64 = 1_000_000;
const MAX_HTML: u64 = 512 * 1024;

pub fn icons_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("icons")
}

pub fn icon_path(data_dir: &Path, id: &str) -> PathBuf {
    icons_dir(data_dir).join(format!("{id}.png"))
}

/// 앱 아이콘을 캐시에 저장한다. `target`이 절대 경로면 그 파일, 아니면 시작 메뉴 앱 목록 항목.
pub fn save_app_icon(target: &str, dest: &Path) -> bool {
    let parsing = if Path::new(target).is_absolute() { target.to_string() } else { apps_folder_path(target) };
    save_item_icon(&parsing, dest, ICON_PX as i32)
}

fn attr_value<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(pos) = lower[from..].find(name) {
        let start = from + pos;
        let before_ok = start == 0 || lower.as_bytes()[start - 1].is_ascii_whitespace();
        let rest = lower[start + name.len()..].trim_start();
        if before_ok && rest.starts_with('=') {
            let offset = tag.len() - rest.len() + 1;
            let value = tag[offset..].trim_start();
            let (quote, body) = match value.chars().next() {
                Some(q @ ('"' | '\'')) => (Some(q), &value[1..]),
                _ => (None, value),
            };
            let end = match quote {
                Some(q) => body.find(q).unwrap_or(body.len()),
                None => body.find(|c: char| c.is_whitespace() || c == '>').unwrap_or(body.len()),
            };
            return Some(&body[..end]);
        }
        from = start + name.len();
    }
    None
}

/// `href`를 `page`(페이지 URL) 기준 절대 URL로
pub fn resolve_url(page: &str, href: &str) -> Option<String> {
    let href = href.trim();
    if href.starts_with("http://") || href.starts_with("https://") {
        return Some(href.to_string());
    }
    if href.starts_with("data:") || href.is_empty() {
        return None;
    }
    let scheme = page.split_once("://")?.0;
    if let Some(rest) = href.strip_prefix("//") {
        return Some(format!("{scheme}://{rest}"));
    }
    let host = host_of(page)?;
    let origin = format!("{scheme}://{host}");
    if href.starts_with('/') {
        return Some(format!("{origin}{href}"));
    }
    let path = page.split_once("://")?.1;
    let dir = match path.find('/') {
        Some(i) => {
            let p = &path[i..];
            let p = p.split(['?', '#']).next().unwrap_or("/");
            &p[..p.rfind('/').unwrap_or(0) + 1]
        }
        None => "/",
    };
    Some(format!("{origin}{dir}{href}"))
}

/// HTML에서 아이콘 링크를 찾아 절대 URL로 돌려준다. 큰 아이콘(apple-touch-icon)을 먼저.
pub fn icon_links(html: &str, page: &str) -> Vec<String> {
    let mut touch = Vec::new();
    let mut normal = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut from = 0;
    while let Some(pos) = lower[from..].find("<link") {
        let start = from + pos;
        let end = lower[start..].find('>').map_or(lower.len(), |e| start + e + 1);
        let tag = &html[start..end];
        if let (Some(rel), Some(href)) = (attr_value(tag, "rel"), attr_value(tag, "href")) {
            let rel = rel.to_ascii_lowercase();
            if let Some(url) = resolve_url(page, href) {
                if rel.contains("apple-touch-icon") {
                    touch.push(url);
                } else if rel.split_whitespace().any(|r| r == "icon") {
                    normal.push(url);
                }
            }
        }
        from = end;
    }
    touch.extend(normal);
    touch
}

fn fetch(http: &dyn Http, url: &str, limit: u64) -> Option<Vec<u8>> {
    let reader = http.get(url, "*/*").ok()?;
    let mut bytes = Vec::new();
    reader.take(limit).read_to_end(&mut bytes).ok()?;
    (!bytes.is_empty()).then_some(bytes)
}

/// 파일 앞부분으로 이미지 형식을 알아낸다 (PNG, ICO, JPEG, GIF, BMP). 순수 함수.
pub fn image_format(bytes: &[u8]) -> Option<GUID> {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => Some(GUID_ContainerFormatPng),
        [0, 0, 1, 0, ..] => Some(GUID_ContainerFormatIco),
        [0xFF, 0xD8, ..] => Some(GUID_ContainerFormatJpeg),
        [b'G', b'I', b'F', ..] => Some(GUID_ContainerFormatGif),
        [b'B', b'M', ..] => Some(GUID_ContainerFormatBmp),
        _ => None,
    }
}

/// 이미지 바이트(PNG·ICO·JPEG·GIF·BMP)를 가장 큰 프레임으로 골라 최대 ICON_PX 크기 PNG로 저장한다.
pub fn save_image_png(bytes: &[u8], dest: &Path) -> windows::core::Result<()> {
    if let Some(dir) = dest.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // SAFETY: 메모리 스트림을 WIC로 디코드해 PNG로 다시 쓴다
    unsafe {
        let wic: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let input = wic.CreateStream()?;
        input.InitializeFromMemory(bytes)?;
        // 형식을 직접 골라 디코더를 만든다: 자동 판별은 설치된 모든 코덱(HEIF·RAW 등)을 불러와 메모리를 크게 쓴다
        let format = image_format(bytes).ok_or_else(|| windows::core::Error::from(E_INVALIDARG))?;
        let decoder = wic.CreateDecoder(&format, std::ptr::null())?;
        decoder.Initialize(&input, WICDecodeMetadataCacheOnDemand)?;
        let mut best = (0u32, 0u32);
        for i in 0..decoder.GetFrameCount()? {
            let frame = decoder.GetFrame(i)?;
            let (mut w, mut h) = (0, 0);
            frame.GetSize(&mut w, &mut h)?;
            if w * h > best.1 {
                best = (i, w * h);
            }
        }
        let frame = decoder.GetFrame(best.0)?;
        let (mut w, mut h) = (0, 0);
        frame.GetSize(&mut w, &mut h)?;
        let converter = wic.CreateFormatConverter()?;
        converter.Initialize(
            &frame,
            &GUID_WICPixelFormat32bppPBGRA,
            WICBitmapDitherTypeNone,
            None,
            0.0,
            WICBitmapPaletteTypeMedianCut,
        )?;
        let mut source: IWICBitmapSource = converter.cast()?;
        let side = w.max(h);
        if side > ICON_PX {
            let scaler = wic.CreateBitmapScaler()?;
            let (nw, nh) = ((w * ICON_PX / side).max(1), (h * ICON_PX / side).max(1));
            scaler.Initialize(&source, nw, nh, WICBitmapInterpolationModeFant)?;
            source = scaler.cast()?;
        }
        let stream = wic.CreateStream()?;
        stream.InitializeFromFilename(&HSTRING::from(dest.as_os_str()), GENERIC_WRITE.0)?;
        let encoder = wic.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())?;
        encoder.Initialize(&stream, WICBitmapEncoderNoCache)?;
        let mut out = None;
        encoder.CreateNewFrame(&mut out, std::ptr::null_mut())?;
        let out = out.ok_or_else(windows::core::Error::empty)?;
        out.Initialize(None)?;
        out.WriteSource(&source, std::ptr::null())?;
        out.Commit()?;
        encoder.Commit()
    }
}

/// 사이트 아이콘을 받아 저장한다: 페이지의 `<link rel=icon>`들 → `/favicon.ico` 순으로 시도.
pub fn save_favicon(http: &dyn Http, page_url: &str, dest: &Path) -> bool {
    let mut candidates = Vec::new();
    if let Some(html) = fetch(http, page_url, MAX_HTML) {
        candidates.extend(icon_links(&String::from_utf8_lossy(&html), page_url));
    }
    if let Some(root) = resolve_url(page_url, "/favicon.ico") {
        candidates.push(root);
    }
    candidates.dedup();
    candidates
        .iter()
        .filter(|url| !url.to_ascii_lowercase().ends_with(".svg"))
        .any(|url| fetch(http, url, MAX_DOWNLOAD).is_some_and(|bytes| save_image_png(&bytes, dest).is_ok()))
}

/// 캐시에 아이콘이 없는 앱·웹 항목 (유튜브는 직접 그린다)
pub fn missing_icons(data_dir: &Path, items: &[Item]) -> Vec<Item> {
    items
        .iter()
        .filter(|i| !matches!(i, Item::Youtube { .. }) && !icon_path(data_dir, i.id()).exists())
        .cloned()
        .collect()
}

/// 하위 프로세스 본체. 하나라도 저장하면 0, 아니면 1.
pub fn run_fetch_process(args: &[String]) -> i32 {
    let (Some(dir), Some(json)) = (args.first(), args.get(1)) else { return 2 };
    let Ok(items) = serde_json::from_str::<Vec<Item>>(json) else { return 2 };
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let dir = Path::new(dir);
    let http = UreqHttp::new();
    let mut saved = false;
    for item in missing_icons(dir, &items) {
        let dest = icon_path(dir, item.id());
        saved |= match &item {
            Item::App { target, .. } => save_app_icon(target, &dest),
            Item::Path { path, .. } => save_item_icon(path, &dest, ICON_PX as i32),
            Item::Web { url, .. } => save_favicon(&http, url, &dest),
            Item::Youtube { .. } => false,
        };
    }
    if saved { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updater::HttpFailure;
    use std::collections::HashMap;

    #[test]
    fn resolves_relative_icon_urls() {
        let page = "https://example.com/blog/post.html?x=1";
        assert_eq!(resolve_url(page, "/favicon.ico").unwrap(), "https://example.com/favicon.ico");
        assert_eq!(resolve_url(page, "icon.png").unwrap(), "https://example.com/blog/icon.png");
        assert_eq!(resolve_url(page, "//cdn.example.com/i.png").unwrap(), "https://cdn.example.com/i.png");
        assert_eq!(resolve_url(page, "https://a.b/c.ico").unwrap(), "https://a.b/c.ico");
        assert_eq!(resolve_url("https://naver.com", "favicon.ico").unwrap(), "https://naver.com/favicon.ico");
        assert_eq!(resolve_url(page, "data:image/png;base64,xx"), None);
    }

    #[test]
    fn only_app_and_web_items_without_cached_icon_are_missing() {
        let dir = std::env::temp_dir().join(format!("ytl-icons-{}", crate::items::new_id()));
        std::fs::create_dir_all(icons_dir(&dir)).unwrap();
        std::fs::write(icon_path(&dir, "cached"), b"png").unwrap();
        let items = vec![
            Item::Youtube { id: "y".into(), query: "q".into(), name: "".into() },
            Item::App { id: "cached".into(), name: "a".into(), target: "A!App".into(), exe: None },
            Item::App { id: "new".into(), name: "b".into(), target: "B!App".into(), exe: None },
            Item::Web { id: "w".into(), url: "https://a.com".into(), name: "".into() },
        ];
        let ids: Vec<String> = missing_icons(&dir, &items).iter().map(|i| i.id().to_string()).collect();
        assert_eq!(ids, vec!["new", "w"]);
    }

    #[test]
    fn detects_image_formats_by_signature() {
        assert_eq!(image_format(b"\x89PNG\r\n"), Some(GUID_ContainerFormatPng));
        assert_eq!(image_format(&[0, 0, 1, 0, 2]), Some(GUID_ContainerFormatIco));
        assert_eq!(image_format(&[0xFF, 0xD8, 0xFF]), Some(GUID_ContainerFormatJpeg));
        assert_eq!(image_format(b"GIF89a"), Some(GUID_ContainerFormatGif));
        assert_eq!(image_format(b"<!doctype html>"), None);
        assert_eq!(image_format(b"<svg"), None);
    }

    #[test]
    fn finds_icon_links_touch_icon_first() {
        let html = r#"<head><link rel="stylesheet" href="/a.css">
            <LINK REL='icon' type="image/png" href="/fav-32.png">
            <link href="/apple.png" rel="apple-touch-icon" sizes="180x180">
            <link rel="shortcut icon" href=/old.ico></head>"#;
        assert_eq!(
            icon_links(html, "https://site.kr/"),
            vec!["https://site.kr/apple.png", "https://site.kr/fav-32.png", "https://site.kr/old.ico"]
        );
        assert!(icon_links("<html></html>", "https://x.y").is_empty());
    }

    struct FakeHttp(HashMap<String, Vec<u8>>);
    impl Http for FakeHttp {
        fn get(&self, url: &str, _: &str) -> Result<Box<dyn Read + Send>, HttpFailure> {
            match self.0.get(url) {
                Some(b) => Ok(Box::new(std::io::Cursor::new(b.clone()))),
                None => Err(HttpFailure::Status(404)),
            }
        }
    }

    #[test]
    fn saves_favicon_from_page_link_and_scales_down() {
        let _ = unsafe {
            windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)
        };
        // 앱 아이콘(256px PNG)을 사이트 아이콘처럼 내준다
        let png = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icon.png")).unwrap();
        let http = FakeHttp(HashMap::from([
            ("https://site.kr".to_string(), br#"<link rel="icon" href="/i.png">"#.to_vec()),
            ("https://site.kr/i.png".to_string(), png),
        ]));
        let dest = std::env::temp_dir().join(format!("ytl-fav-{}.png", crate::items::new_id()));
        assert!(save_favicon(&http, "https://site.kr", &dest));
        assert!(std::fs::metadata(&dest).unwrap().len() > 100);
        let missing = std::env::temp_dir().join(format!("ytl-fav-{}.png", crate::items::new_id()));
        assert!(!save_favicon(&FakeHttp(HashMap::new()), "https://none.kr", &missing));
    }
}
