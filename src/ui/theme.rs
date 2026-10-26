//! 테마 4가지(라이트·다크·세피아·투명)의 색표. 유리(아크릴) 위에 덧칠하는 색과 글자색.

use crate::config::Theme;

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
            add_tile: Color::hex(0xA9AFB8, 0.9),
            swatch: Color::hex(0xF7F8FA, 1.0),
            dark_backdrop: false,
        },
        Theme::Dark => Palette {
            tint: Color::hex(0x181A1F, 0.60),
            text: Color::hex(0xF1F3F5, 1.0),
            subtext: Color::hex(0x8A919C, 1.0),
            divider: Color::hex(0xFFFFFF, 0.10),
            add_tile: Color::hex(0x2B2F3A, 0.95),
            swatch: Color::hex(0x181A1F, 1.0),
            dark_backdrop: true,
        },
        Theme::Sepia => Palette {
            tint: Color::hex(0xF4ECD8, 0.60),
            text: Color::hex(0x4B3A26, 1.0),
            subtext: Color::hex(0x8C7656, 1.0),
            divider: Color::hex(0x4B3A26, 0.15),
            add_tile: Color::hex(0xA8916B, 0.9),
            swatch: Color::hex(0xF4ECD8, 1.0),
            dark_backdrop: false,
        },
        Theme::Glass => Palette {
            tint: Color::hex(0x000000, 0.30),
            text: Color::hex(0xFFFFFF, 1.0),
            subtext: Color::hex(0xFFFFFF, 0.70),
            divider: Color::hex(0xFFFFFF, 0.18),
            add_tile: Color::hex(0xFFFFFF, 0.22),
            swatch: Color::hex(0x3A3D44, 1.0),
            dark_backdrop: true,
        },
    }
}

/// 테마 선택기에 나오는 순서
pub const PICKER_ORDER: [Theme; 4] = [Theme::Light, Theme::Dark, Theme::Sepia, Theme::Glass];

pub fn label(theme: Theme) -> &'static str {
    match theme {
        Theme::Light => "라이트",
        Theme::Dark => "다크",
        Theme::Sepia => "세피아",
        Theme::Glass => "투명",
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
