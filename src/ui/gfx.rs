//! Direct2D + DirectComposition 그리기 장치. 창 하나(콘텐츠 창)에 픽셀 단위 투명도로 GPU 렌더링한다.
//!
//! 좌표는 DIP다. `set_scale`로 DPI 배율을 주면 Direct2D가 실제 픽셀로 바꿔 그린다.

use crate::ui::layout::{Pt, RectF};
use crate::ui::theme::Color;
use windows::Win32::Foundation::{HMODULE, HWND};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED,
    D2D1_GRADIENT_STOP, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_TARGET,
    D2D1_BITMAP_PROPERTIES1, D2D1_BRUSH_PROPERTIES, D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_DRAW_TEXT_OPTIONS_CLIP,
    D2D1_ELLIPSE, D2D1_EXTEND_MODE_CLAMP, D2D1_FACTORY_OPTIONS, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_GAMMA_2_2,
    D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_ROUNDED_RECT, D2D1CreateFactory, ID2D1Bitmap1, ID2D1DeviceContext,
    ID2D1Factory1, ID2D1RenderTarget, ID2D1SolidColorBrush,
};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT, DWRITE_TEXT_ALIGNMENT, DWRITE_TEXT_METRICS,
    DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP, DWriteCreateFactory,
    IDWriteFactory5, IDWriteFontCollection1, IDWriteTextFormat,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory2, DXGI_CREATE_FACTORY_FLAGS, DXGI_PRESENT, DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1,
    DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIDevice,
    IDXGIFactory2, IDXGISurface, IDXGISwapChain1,
};
use windows::Win32::Graphics::Imaging::{IWICBitmap, IWICImagingFactory};
use windows::core::{HSTRING, Interface, Result, w};
use windows_numerics::{Matrix3x2, Vector2};

static FONT_REGULAR: &[u8] = include_bytes!("../../assets/Pretendard-Regular.otf");
static FONT_SEMIBOLD: &[u8] = include_bytes!("../../assets/Pretendard-SemiBold.otf");
static FONT_BOLD: &[u8] = include_bytes!("../../assets/Pretendard-Bold.otf");
pub const FONT_FAMILY: &str = "Pretendard";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Regular,
    SemiBold,
    Bold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Leading,
    Center,
}

fn color(c: Color) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r: c.r, g: c.g, b: c.b, a: c.a }
}

fn rect(r: RectF) -> D2D_RECT_F {
    D2D_RECT_F { left: r.x, top: r.y, right: r.x + r.w, bottom: r.y + r.h }
}

fn point(p: Pt) -> Vector2 {
    Vector2 { X: p.x, Y: p.y }
}

fn create_d3d_device() -> Result<ID3D11Device> {
    let mut last = Err(windows::core::Error::empty());
    for driver in [D3D_DRIVER_TYPE_WARP, D3D_DRIVER_TYPE_HARDWARE] {
        last = try_device(driver);
        if last.is_ok() {
            break;
        }
    }
    last
}

fn try_device(driver: D3D_DRIVER_TYPE) -> Result<ID3D11Device> {
    let mut device = None;
    // SAFETY: 출력 포인터 하나만 받는 표준 호출
    unsafe {
        D3D11CreateDevice(
            None,
            driver,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )?;
    }
    device.ok_or_else(windows::core::Error::empty)
}

/// exe에 들어 있는 Pretendard를 DirectWrite 메모리 글꼴 모음으로 등록한다.
fn load_fonts(factory: &IDWriteFactory5) -> Result<IDWriteFontCollection1> {
    // SAFETY: 글꼴 데이터는 'static이라 로더가 계속 참조해도 안전하다
    unsafe {
        let loader = factory.CreateInMemoryFontFileLoader()?;
        factory.RegisterFontFileLoader(&loader)?;
        let builder = factory.CreateFontSetBuilder()?;
        for data in [FONT_REGULAR, FONT_SEMIBOLD, FONT_BOLD] {
            let file =
                loader.CreateInMemoryFontFileReference(factory, data.as_ptr().cast(), data.len() as u32, None)?;
            builder.AddFontFile(&file)?;
        }
        let set = builder.CreateFontSet()?;
        factory.CreateFontCollectionFromFontSet(&set)
    }
}

/// 그림이 나가는 곳: 창(DirectComposition 스왑체인) 또는 이미지(PNG 확인용)
enum Surface {
    Window {
        swap: IDXGISwapChain1,
        target_bitmap: Option<ID2D1Bitmap1>,
        // 아래 셋은 화면 합성 연결을 유지하려고 들고 있는다
        _dcomp: IDCompositionDevice,
        _target: IDCompositionTarget,
        _visual: IDCompositionVisual,
    },
    #[cfg_attr(not(test), allow(dead_code))]
    Image { wic: IWICImagingFactory, bitmap: IWICBitmap },
}

pub struct Gfx {
    factory: ID2D1Factory1,
    dwrite: IDWriteFactory5,
    fonts: Option<IDWriteFontCollection1>,
    ctx: ID2D1DeviceContext,
    surface: Surface,
    width: u32,
    height: u32,
    scale: f32,
}

fn text_resources() -> Result<(IDWriteFactory5, Option<IDWriteFontCollection1>)> {
    let dwrite: IDWriteFactory5 = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
    let fonts = load_fonts(&dwrite).ok();
    Ok((dwrite, fonts))
}

impl Gfx {
    /// `hwnd`는 `WS_EX_NOREDIRECTIONBITMAP`으로 만든 창이어야 한다.
    pub fn new(hwnd: HWND, width: u32, height: u32, scale: f32) -> Result<Gfx> {
        let (width, height) = (width.max(1), height.max(1));
        // SAFETY: 아래는 모두 COM 객체 생성 순서대로의 표준 호출
        unsafe {
            let d3d = create_d3d_device()?;
            let dxgi_device: IDXGIDevice = d3d.cast()?;
            let dxgi_factory: IDXGIFactory2 = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))?;
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: width,
                Height: height,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
                ..Default::default()
            };
            let swap = dxgi_factory.CreateSwapChainForComposition(&dxgi_device, &desc, None)?;
            let options = D2D1_FACTORY_OPTIONS::default();
            let factory: ID2D1Factory1 = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, Some(&options))?;
            let device = factory.CreateDevice(&dxgi_device)?;
            let ctx = device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
            let dcomp: IDCompositionDevice = DCompositionCreateDevice(&dxgi_device)?;
            let target = dcomp.CreateTargetForHwnd(hwnd, true)?;
            let visual = dcomp.CreateVisual()?;
            visual.SetContent(&swap)?;
            target.SetRoot(&visual)?;
            dcomp.Commit()?;
            let (dwrite, fonts) = text_resources()?;
            let surface =
                Surface::Window { swap, target_bitmap: None, _dcomp: dcomp, _target: target, _visual: visual };
            let mut gfx = Gfx { factory, dwrite, fonts, ctx, surface, width, height, scale };
            gfx.bind_target()?;
            Ok(gfx)
        }
    }

    fn bind_target(&mut self) -> Result<()> {
        let dpi = 96.0 * self.scale;
        let Surface::Window { swap, target_bitmap, .. } = &mut self.surface else { return Ok(()) };
        // SAFETY: 스왑체인 0번 버퍼를 Direct2D 대상 비트맵으로 감싼다
        unsafe {
            let surface: IDXGISurface = swap.GetBuffer(0)?;
            let props = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: dpi,
                dpiY: dpi,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                ..Default::default()
            };
            let bitmap = self.ctx.CreateBitmapFromDxgiSurface(&surface, Some(&props))?;
            self.ctx.SetTarget(&bitmap);
            self.ctx.SetDpi(dpi, dpi);
            *target_bitmap = Some(bitmap);
        }
        Ok(())
    }

    /// 창 크기(실제 픽셀)나 DPI가 바뀌면 호출한다.
    pub fn resize(&mut self, width: u32, height: u32, scale: f32) -> Result<()> {
        let (width, height) = (width.max(1), height.max(1));
        if (width, height, scale) == (self.width, self.height, self.scale) {
            return Ok(());
        }
        let Surface::Window { swap, target_bitmap, .. } = &mut self.surface else { return Ok(()) };
        // SAFETY: 버퍼를 다시 만들기 전에 대상 비트맵을 놓는다
        unsafe {
            self.ctx.SetTarget(None);
            *target_bitmap = None;
            swap.ResizeBuffers(2, width, height, DXGI_FORMAT_UNKNOWN, DXGI_SWAP_CHAIN_FLAG(0))?;
        }
        self.width = width;
        self.height = height;
        self.scale = scale;
        self.bind_target()
    }

    pub fn begin(&self) {
        // SAFETY: 그리기 시작 + 완전 투명으로 지움
        unsafe {
            self.ctx.BeginDraw();
            self.ctx.SetTransform(&Matrix3x2::identity());
            self.ctx.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
            self.ctx.Clear(Some(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }));
        }
    }

    pub fn end(&self) -> Result<()> {
        // SAFETY: 그리기 끝내고 화면에 보냄
        unsafe {
            self.ctx.EndDraw(None, None)?;
            match &self.surface {
                Surface::Window { swap, .. } => swap.Present(1, DXGI_PRESENT(0)).ok(),
                Surface::Image { .. } => Ok(()),
            }
        }
    }

    fn brush(&self, c: Color) -> Option<ID2D1SolidColorBrush> {
        unsafe { self.ctx.CreateSolidColorBrush(&color(c), None) }.ok()
    }

    pub fn fill_round_rect(&self, r: RectF, radius: f32, c: Color) {
        if let Some(b) = self.brush(c) {
            let rr = D2D1_ROUNDED_RECT { rect: rect(r), radiusX: radius, radiusY: radius };
            unsafe { self.ctx.FillRoundedRectangle(&rr, &b) };
        }
    }

    pub fn stroke_round_rect(&self, r: RectF, radius: f32, c: Color, width: f32) {
        if let Some(b) = self.brush(c) {
            let rr = D2D1_ROUNDED_RECT { rect: rect(r), radiusX: radius, radiusY: radius };
            unsafe { self.ctx.DrawRoundedRectangle(&rr, &b, width, None) };
        }
    }

    /// 위→아래 그라데이션으로 채운 둥근 사각형
    pub fn fill_round_rect_gradient(&self, r: RectF, radius: f32, top: Color, bottom: Color) {
        let stops = [
            D2D1_GRADIENT_STOP { position: 0.0, color: color(top) },
            D2D1_GRADIENT_STOP { position: 1.0, color: color(bottom) },
        ];
        // SAFETY: 지역 배열로 그라데이션 정지점을 만든다
        unsafe {
            let Ok(rt) = self.ctx.cast::<ID2D1RenderTarget>() else { return };
            let Ok(collection) = rt.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP) else {
                return;
            };
            let props = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                startPoint: Vector2 { X: r.x, Y: r.y },
                endPoint: Vector2 { X: r.x, Y: r.y + r.h },
            };
            let brush_props = D2D1_BRUSH_PROPERTIES { opacity: 1.0, transform: Matrix3x2::identity() };
            if let Ok(brush) = self.ctx.CreateLinearGradientBrush(&props, Some(&brush_props), &collection) {
                let rr = D2D1_ROUNDED_RECT { rect: rect(r), radiusX: radius, radiusY: radius };
                self.ctx.FillRoundedRectangle(&rr, &brush);
            }
        }
    }

    /// 둥근 사각형 아래로 옅게 번지는 그림자 (겹친 사각형으로 근사)
    pub fn soft_shadow(&self, r: RectF, radius: f32, strength: f32) {
        const STEPS: i32 = 6;
        for i in (1..=STEPS).rev() {
            let spread = i as f32;
            let alpha = strength * (STEPS - i + 1) as f32 / STEPS as f32 * 0.05;
            let shadow = RectF::new(r.x - spread, r.y - spread + 2.0, r.w + 2.0 * spread, r.h + 2.0 * spread);
            self.fill_round_rect(shadow, radius + spread, Color::hex(0x000000, alpha));
        }
    }

    pub fn fill_circle(&self, center: Pt, radius: f32, c: Color) {
        if let Some(b) = self.brush(c) {
            let e = D2D1_ELLIPSE { point: point(center), radiusX: radius, radiusY: radius };
            unsafe { self.ctx.FillEllipse(&e, &b) };
        }
    }

    pub fn stroke_circle(&self, center: Pt, radius: f32, c: Color, width: f32) {
        if let Some(b) = self.brush(c) {
            let e = D2D1_ELLIPSE { point: point(center), radiusX: radius, radiusY: radius };
            unsafe { self.ctx.DrawEllipse(&e, &b, width, None) };
        }
    }

    pub fn line(&self, a: Pt, b: Pt, c: Color, width: f32) {
        if let Some(brush) = self.brush(c) {
            unsafe { self.ctx.DrawLine(point(a), point(b), &brush, width, None) };
        }
    }

    /// 채운 다각형 (재생 삼각형 등)
    pub fn fill_polygon(&self, points: &[Pt], c: Color) {
        let (Some(first), Some(brush)) = (points.first(), self.brush(c)) else { return };
        // SAFETY: 경로 도형을 만들어 바로 채운다
        unsafe {
            let Ok(geometry) = self.factory.CreatePathGeometry() else { return };
            let Ok(sink) = geometry.Open() else { return };
            sink.BeginFigure(point(*first), D2D1_FIGURE_BEGIN_FILLED);
            for p in &points[1..] {
                sink.AddLine(point(*p));
            }
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            if sink.Close().is_ok() {
                self.ctx.FillGeometry(&geometry, &brush, None);
            }
        }
    }

    fn text_format(&self, size: f32, weight: Weight, align: Align) -> Option<IDWriteTextFormat> {
        let weight = DWRITE_FONT_WEIGHT(match weight {
            Weight::Regular => 400,
            Weight::SemiBold => 600,
            Weight::Bold => 700,
        });
        // SAFETY: 글꼴 모음이 없으면 시스템 글꼴로 대체된다
        unsafe {
            let format = self
                .dwrite
                .CreateTextFormat(
                    &HSTRING::from(FONT_FAMILY),
                    self.fonts.as_ref().and_then(|f| f.cast().ok()).as_ref(),
                    weight,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    size,
                    w!("ko-kr"),
                )
                .ok()?;
            let _ = format.SetTextAlignment(match align {
                Align::Leading => DWRITE_TEXT_ALIGNMENT(0),
                Align::Center => DWRITE_TEXT_ALIGNMENT(2),
            });
            let _ = format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT(2)); // 세로 가운데
            let _ = format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP);
            let trimming =
                DWRITE_TRIMMING { granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER, delimiter: 0, delimiterCount: 0 };
            let _ = format.SetTrimming(&trimming, None);
            Some(format)
        }
    }

    /// 사각형 안에 한 줄 텍스트 (세로 가운데, 넘치면 잘림)
    pub fn text(&self, text: &str, r: RectF, size: f32, weight: Weight, align: Align, c: Color) {
        let (Some(format), Some(brush)) = (self.text_format(size, weight, align), self.brush(c)) else { return };
        let wide: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            self.ctx.DrawText(
                &wide,
                &format,
                &rect(r),
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            )
        };
    }

    /// 한 줄 텍스트의 너비(DIP)
    pub fn measure(&self, text: &str, size: f32, weight: Weight) -> f32 {
        let Some(format) = self.text_format(size, weight, Align::Leading) else { return 0.0 };
        let wide: Vec<u16> = text.encode_utf16().collect();
        // SAFETY: 측정용 레이아웃을 만들어 크기만 읽는다
        unsafe {
            let Ok(layout) = self.dwrite.CreateTextLayout(&wide, &format, 10_000.0, 1_000.0) else { return 0.0 };
            let mut metrics = DWRITE_TEXT_METRICS::default();
            match layout.GetMetrics(&mut metrics) {
                Ok(()) => metrics.widthIncludingTrailingWhitespace,
                Err(_) => 0.0,
            }
        }
    }
}

/// 테스트에서 창 없이 장면을 PNG로 그려 보는 기능
#[cfg(test)]
mod offscreen {
    use super::*;
    use windows::Win32::Foundation::GENERIC_WRITE;
    use windows::Win32::Graphics::Direct2D::D2D1_RENDER_TARGET_PROPERTIES;
    use windows::Win32::Graphics::Imaging::{
        CLSID_WICImagingFactory, GUID_ContainerFormatPng, GUID_WICPixelFormat32bppPBGRA, WICBitmapCacheOnLoad,
        WICBitmapEncoderNoCache,
    };
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};

    impl Gfx {
        /// 창 없이 이미지에 그린다 (화면 확인용 PNG 저장). COM이 초기화돼 있어야 한다.
        pub fn offscreen(width: u32, height: u32, scale: f32) -> Result<Gfx> {
            // SAFETY: WIC 비트맵을 만들고 그 위에 Direct2D 대상을 만든다
            unsafe {
                let wic: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
                let bitmap = wic.CreateBitmap(width, height, &GUID_WICPixelFormat32bppPBGRA, WICBitmapCacheOnLoad)?;
                let options = D2D1_FACTORY_OPTIONS::default();
                let factory: ID2D1Factory1 = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, Some(&options))?;
                let props = D2D1_RENDER_TARGET_PROPERTIES {
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                    },
                    dpiX: 96.0 * scale,
                    dpiY: 96.0 * scale,
                    ..Default::default()
                };
                let rt = factory.CreateWicBitmapRenderTarget(&bitmap, &props)?;
                let ctx: ID2D1DeviceContext = rt.cast()?;
                let (dwrite, fonts) = text_resources()?;
                Ok(Gfx { factory, dwrite, fonts, ctx, surface: Surface::Image { wic, bitmap }, width, height, scale })
            }
        }

        /// 이미지 대상이면 PNG로 저장한다.
        pub fn save_png(&self, path: &std::path::Path) -> Result<()> {
            let Surface::Image { wic, bitmap } = &self.surface else { return Ok(()) };
            // SAFETY: WIC 인코더로 파일에 쓴다
            unsafe {
                let stream = wic.CreateStream()?;
                stream.InitializeFromFilename(&HSTRING::from(path.as_os_str()), GENERIC_WRITE.0)?;
                let encoder = wic.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())?;
                encoder.Initialize(&stream, WICBitmapEncoderNoCache)?;
                let mut frame = None;
                encoder.CreateNewFrame(&mut frame, std::ptr::null_mut())?;
                let frame = frame.ok_or_else(windows::core::Error::empty)?;
                frame.Initialize(None)?;
                frame.WriteSource(bitmap, std::ptr::null())?;
                frame.Commit()?;
                encoder.Commit()
            }
        }
    }
}
