//! 앱 아이콘(`assets/icon.png`, `assets/icon.ico`) 생성과 검사 (테스트 전용).
//! 다시 만들기: `cargo test generate_app_icon -- --ignored`

use crate::ui::gfx::Gfx;
use crate::ui::layout::{Pt, RectF};
use crate::ui::theme::Color;
use std::path::{Path, PathBuf};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_ContainerFormatPng, GUID_WICPixelFormat32bppBGRA, IWICImagingFactory,
    WICBitmapEncoderNoCache,
};
use windows::Win32::System::Com::StructuredStorage::CreateStreamOnHGlobal;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, STATFLAG_NONAME, STATSTG, STREAM_SEEK_SET};

/// ICO에 넣는 크기 (px)
const SIZES: [u32; 8] = [16, 20, 24, 32, 40, 48, 64, 256];
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")
}

/// PNG 머리(IHDR)의 너비·높이
fn png_size(png: &[u8]) -> (u32, u32) {
    let be = |i: usize| u32::from_be_bytes([png[i], png[i + 1], png[i + 2], png[i + 3]]);
    (be(16), be(20))
}

#[test]
fn icon_files_have_every_size_as_png_frames() {
    let png = std::fs::read(assets().join("icon.png")).unwrap();
    assert!(png.starts_with(&PNG_SIGNATURE));
    assert_eq!(png_size(&png), (256, 256));

    let ico = std::fs::read(assets().join("icon.ico")).unwrap();
    let u16_at = |i: usize| u16::from_le_bytes([ico[i], ico[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes([ico[i], ico[i + 1], ico[i + 2], ico[i + 3]]);
    assert_eq!((u16_at(0), u16_at(2), u16_at(4) as usize), (0, 1, SIZES.len()));
    for (n, size) in SIZES.iter().enumerate() {
        let e = 6 + 16 * n;
        let dim = |b: u8| if b == 0 { 256 } else { b as u32 };
        assert_eq!((dim(ico[e]), dim(ico[e + 1])), (*size, *size), "프레임 {n}");
        assert_eq!((u16_at(e + 4), u16_at(e + 6)), (1, 32), "평면·비트 수");
        let (len, at) = (u32_at(e + 8) as usize, u32_at(e + 12) as usize);
        assert!(at + len <= ico.len(), "프레임 {n}이 파일 밖을 가리킴");
        let frame = &ico[at..at + len];
        assert!(frame.starts_with(&PNG_SIGNATURE), "프레임 {n}은 PNG");
        assert_eq!(png_size(frame), (*size, *size));
    }
}

/// 256 단위 좌표로 아이콘을 그린다 (그리기 배율이 크기를 정한다)
fn draw(g: &Gfx) {
    let board = RectF::new(12.0, 12.0, 232.0, 232.0);
    g.fill_round_rect_linear(
        board,
        56.0,
        Pt { x: board.x, y: board.y },
        Pt { x: board.right(), y: board.bottom() },
        Color::hex(0x6A7CF7, 1.0),
        Color::hex(0x8E4FD8, 1.0),
    );
    for (x, y, alpha) in [(54.0, 54.0, 1.0), (136.0, 54.0, 0.55), (54.0, 136.0, 0.55), (136.0, 136.0, 0.85)] {
        g.fill_round_rect(RectF::new(x, y, 66.0, 66.0), 18.0, Color::hex(0xFFFFFF, alpha));
    }
}

/// 곱해진(premultiplied) BGRA를 곱해지지 않은 BGRA로 되돌린다
fn unpremultiply(pixels: &mut [u8]) {
    let (chunks, _) = pixels.as_chunks_mut::<4>();
    for px in chunks {
        let a = px[3] as u32;
        match (a > 0).then_some(a) {
            None => *px = [0, 0, 0, 0],
            Some(a) => {
                for c in &mut px[..3] {
                    *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
                }
            }
        }
    }
}

/// `size`×`size` PNG 바이트 (투명도가 곱해지지 않은 BGRA)
fn render_png(size: u32) -> Vec<u8> {
    let gfx = Gfx::offscreen(size, size, size as f32 / 256.0).unwrap();
    gfx.begin();
    draw(&gfx);
    gfx.end().unwrap();
    let mut pixels = gfx.pixels();
    unpremultiply(&mut pixels);
    // SAFETY: WIC로 메모리 스트림에 PNG를 쓰고 바이트를 읽어 온다
    unsafe {
        let wic: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).unwrap();
        let bitmap = wic.CreateBitmapFromMemory(size, size, &GUID_WICPixelFormat32bppBGRA, size * 4, &pixels).unwrap();
        let stream = CreateStreamOnHGlobal(Default::default(), true).unwrap();
        let encoder = wic.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null()).unwrap();
        encoder.Initialize(&stream, WICBitmapEncoderNoCache).unwrap();
        let mut frame = None;
        encoder.CreateNewFrame(&mut frame, std::ptr::null_mut()).unwrap();
        let frame = frame.unwrap();
        frame.Initialize(None).unwrap();
        frame.SetSize(size, size).unwrap();
        let mut format = GUID_WICPixelFormat32bppBGRA;
        frame.SetPixelFormat(&mut format).unwrap();
        frame.WriteSource(&bitmap, std::ptr::null()).unwrap();
        frame.Commit().unwrap();
        encoder.Commit().unwrap();

        let mut stat = STATSTG::default();
        stream.Stat(&mut stat, STATFLAG_NONAME).unwrap();
        let mut out = vec![0u8; stat.cbSize as usize];
        stream.Seek(0, STREAM_SEEK_SET, None).unwrap();
        let mut read = 0u32;
        stream.Read(out.as_mut_ptr().cast(), out.len() as u32, Some(&mut read)).ok().unwrap();
        out.truncate(read as usize);
        out
    }
}

/// PNG 프레임들로 ICO 파일 바이트를 만든다
fn build_ico(frames: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(frames.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * frames.len() as u32;
    for (size, png) in frames {
        let dim = if *size >= 256 { 0 } else { *size as u8 };
        out.extend_from_slice(&[dim, dim, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for (_, png) in frames {
        out.extend_from_slice(png);
    }
    out
}

/// 아이콘 파일을 다시 만든다: `cargo test generate_app_icon -- --ignored`
#[test]
#[ignore]
fn generate_app_icon() {
    let _ = unsafe {
        windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)
    };
    let frames: Vec<(u32, Vec<u8>)> = SIZES.iter().map(|&s| (s, render_png(s))).collect();
    std::fs::write(assets().join("icon.ico"), build_ico(&frames)).unwrap();
    std::fs::write(assets().join("icon.png"), &frames.last().unwrap().1).unwrap();
    // 눈으로 확인할 확대 그림: target/snapshots/app-icon-<크기>.png
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("snapshots");
    std::fs::create_dir_all(&dir).unwrap();
    for (size, png) in &frames {
        std::fs::write(dir.join(format!("app-icon-{size}.png")), png).unwrap();
    }
}
