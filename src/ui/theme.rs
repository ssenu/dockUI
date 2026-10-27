//! 테마 4가지(라이트·다크·세피아·투명)의 색표. 유리(아크릴) 위에 덧칠하는 색과 글자색.

use crate::config::{CustomTheme, Theme};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color {
        Color { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a }
    }

    pub const fn hex(rgb: u32, a: f32) -> Color {
        Color::rgba((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, a)
    }

    pub fn with_alpha(self, a: f32) -> Color {
        Color { a, ..self }
    }

    pub fn lerp(self, other: Color, t: f32) -> Color {
        if t <= 0.0 {
            return self;
        }
        if t >= 1.0 {
            return other;
        }
        Color {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }

    /// 밝게(+) / 어둡게(-) 섞기. amount는 -1..1
    pub fn shade(self, amount: f32) -> Color {
        let target = if amount >= 0.0 { Color::hex(0xFFFFFF, self.a) } else { Color::hex(0x000000, self.a) };
        self.lerp(target, amount.abs())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// 유리 위에 덧칠하는 색 (아크릴을 쓸 때)
    pub tint: Color,
    pub text: Color,
    pub subtext: Color,
    pub divider: Color,
    /// ＋ 칸 바탕 (글자색을 옅게)
    pub add_tile: Color,
    /// 테마 선택 원에 보이는 색
    pub swatch: Color,
    /// 블러 배경을 어두운 톤으로 할지
    pub dark_backdrop: bool,
}

/// 아크릴을 못 쓰는 환경(Windows 10 등)에서 덧칠 불투명도
pub const FALLBACK_ALPHA: f32 = 0.92;
pub const TOOLTIP_BG: Color = Color::hex(0x14161A, 0.92);
pub const TOOLTIP_TEXT: Color = Color::hex(0xFFFFFF, 1.0);
pub const ACCENT: Color = Color::hex(0xE53935, 1.0);

pub fn palette(theme: Theme) -> Palette {
    match theme {
        Theme::Light => Palette {
            tint: Color::hex(0xFFFFFF, 0.55),
            text: Color::hex(0x1F232B, 1.0),
            subtext: Color::hex(0x7A818C, 1.0),
            divider: Color::hex(0x1F232B, 0.12),
            add_tile: Color::hex(0x1F232B, 0.07),
            swatch: Color::hex(0xF7F8FA, 1.0),
            dark_backdrop: false,
        },
        Theme::Dark => Palette {
            tint: Color::hex(0x181A1F, 0.60),
            text: Color::hex(0xF1F3F5, 1.0),
            subtext: Color::hex(0x8A919C, 1.0),
            divider: Color::hex(0xFFFFFF, 0.10),
            add_tile: Color::hex(0xFFFFFF, 0.08),
            swatch: Color::hex(0x181A1F, 1.0),
            dark_backdrop: true,
        },
        Theme::Sepia => Palette {
            tint: Color::hex(0xF4ECD8, 0.60),
            text: Color::hex(0x4B3A26, 1.0),
            subtext: Color::hex(0x8C7656, 1.0),
            divider: Color::hex(0x4B3A26, 0.15),
            add_tile: Color::hex(0x4B3A26, 0.08),
            swatch: Color::hex(0xF4ECD8, 1.0),
            dark_backdrop: false,
        },
        Theme::Glass => Palette {
            tint: Color::hex(0x000000, 0.30),
            text: Color::hex(0xFFFFFF, 1.0),
            subtext: Color::hex(0xFFFFFF, 0.70),
            divider: Color::hex(0xFFFFFF, 0.18),
            add_tile: Color::hex(0xFFFFFF, 0.14),
            swatch: Color::hex(0x3A3D44, 1.0),
            dark_backdrop: true,
        },
        Theme::Custom => custom_palette(&CustomTheme::default()),
    }
}

/// 사용자 지정 설정 창의 추천 색
pub const PRESETS: [u32; 12] = [
    0x1E1E1E, 0x2B2D42, 0x1F3A5F, 0x0F4C5C, 0x2D6A4F, 0x5C3D2E, 0x6B2737, 0x4A3B6B, 0xF5F5F5, 0xF4ECD8, 0xDDE7F0,
    0xFFE4E1,
];

/// 사용자 지정 색표: 바탕은 고른 색·불투명도, 글자·구분선은 바탕 밝기에 맞춰 자동 (어두우면 흰 글자)
pub fn custom_palette(c: &CustomTheme) -> Palette {
    let bg = Color::hex(c.color, 1.0);
    let lum = 0.2126 * bg.r + 0.7152 * bg.g + 0.0722 * bg.b;
    let dark = lum < 0.55;
    let text = if dark { Color::hex(0xFFFFFF, 1.0) } else { Color::hex(0x1F232B, 1.0) };
    Palette {
        tint: bg.with_alpha(c.opacity.min(100) as f32 / 100.0),
        text,
        subtext: text.with_alpha(0.68),
        divider: text.with_alpha(if dark { 0.18 } else { 0.12 }),
        add_tile: text.with_alpha(if dark { 0.10 } else { 0.07 }),
        swatch: bg,
        dark_backdrop: dark,
    }
}

/// `#RGB`, `#RRGGBB` (`#` 없어도) → 0xRRGGBB
pub fn parse_hex(s: &str) -> Option<u32> {
    let h = s.trim().trim_start_matches('#');
    if !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let full: String = match h.len() {
        3 => h.chars().flat_map(|c| [c, c]).collect(),
        6 => h.to_string(),
        _ => return None,
    };
    u32::from_str_radix(&full, 16).ok()
}

/// 0xRRGGBB → `#RRGGBB`
pub fn to_hex(rgb: u32) -> String {
    format!("#{:06X}", rgb & 0xFF_FFFF)
}

/// 테마 선택기에 나오는 순서
pub const PICKER_ORDER: [Theme; 4] = [Theme::Light, Theme::Dark, Theme::Sepia, Theme::Glass];

pub fn label(theme: Theme) -> &'static str {
    match theme {
        Theme::Light => "라이트",
        Theme::Dark => "다크",
        Theme::Sepia => "세피아",
        Theme::Glass => "투명",
        Theme::Custom => "사용자 지정",
    }
}

/// 테마 전환 페이드: from → to 사이를 t(0..1)로 섞은 색표
pub fn blend(from: &Palette, to: &Palette, t: f32) -> Palette {
    Palette {
        tint: from.tint.lerp(to.tint, t),
        text: from.text.lerp(to.text, t),
        subtext: from.subtext.lerp(to.subtext, t),
        divider: from.divider.lerp(to.divider, t),
        add_tile: from.add_tile.lerp(to.add_tile, t),
        swatch: to.swatch,
        dark_backdrop: to.dark_backdrop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_palette_picks_readable_text() {
        let dark = custom_palette(&CustomTheme { color: 0x1E1E1E, opacity: 60, blur: true });
        assert_eq!(dark.text, Color::hex(0xFFFFFF, 1.0));
        assert!(dark.dark_backdrop);
        assert!((dark.tint.a - 0.6).abs() < 1e-6);
        let light = custom_palette(&CustomTheme { color: 0xF4ECD8, opacity: 90, blur: false });
        assert_eq!(light.text, Color::hex(0x1F232B, 1.0));
        assert!(!light.dark_backdrop);
    }

    #[test]
    fn hex_parses_short_long_and_bare() {
        assert_eq!(parse_hex("#1E1E1E"), Some(0x1E1E1E));
        assert_eq!(parse_hex("1e1e1e"), Some(0x1E1E1E));
        assert_eq!(parse_hex(" #abc "), Some(0xAABBCC));
        assert_eq!(parse_hex("#12345"), None);
        assert_eq!(parse_hex("zzzzzz"), None);
        assert_eq!(parse_hex("+12345"), None);
        assert_eq!(to_hex(0x0F4C5C), "#0F4C5C");
    }

    #[test]
    fn hex_parses_channels() {
        let c = Color::hex(0xFF8000, 0.5);
        assert_eq!((c.r, c.g, c.a), (1.0, 128.0 / 255.0, 0.5));
        assert_eq!(c.b, 0.0);
    }

    #[test]
    fn lerp_clamps_and_interpolates() {
        let a = Color::hex(0x000000, 0.0);
        let b = Color::hex(0xFFFFFF, 1.0);
        assert_eq!(a.lerp(b, 0.5).r, 0.5);
        assert_eq!(a.lerp(b, 2.0), b);
        assert_eq!(a.lerp(b, -1.0), a);
    }

    #[test]
    fn glass_is_smoky_black_30_percent() {
        let p = palette(Theme::Glass);
        assert_eq!(p.tint, Color::hex(0x000000, 0.30));
        assert!(p.dark_backdrop);
        assert!(!palette(Theme::Light).dark_backdrop);
    }

    #[test]
    fn blend_ends_at_target() {
        let from = palette(Theme::Light);
        let to = palette(Theme::Dark);
        assert_eq!(blend(&from, &to, 1.0).tint, to.tint);
        assert_eq!(blend(&from, &to, 0.0).text, from.text);
    }

    #[test]
    fn picker_lists_four_themes_with_korean_labels() {
        let labels: Vec<_> = PICKER_ORDER.iter().map(|t| label(*t)).collect();
        assert_eq!(labels, ["라이트", "다크", "세피아", "투명"]);
    }
}
