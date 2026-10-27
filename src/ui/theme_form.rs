//! 사용자 지정 테마 설정 창의 그림 (순수 배치 + 그리기): 추천 색, HEX 칸 테두리, 더 많은 색, 투명도 슬라이더,
//! 흐림 스위치, 취소·저장. HEX 글자 입력만 위에 얹은 기본 EDIT 컨트롤이 맡는다. 좌표는 DIP.

use crate::config::CustomTheme;
use crate::ui::form::{BG, BORDER, FOOTER, HAIRLINE, LABEL, SOFT, SUBTEXT, TEXT, draw_button, draw_field};
use crate::ui::gfx::{Align, Gfx, Weight};
use crate::ui::layout::{Pt, RectF};
use crate::ui::theme::{ACCENT, Color, PRESETS};

pub const WIDTH: f32 = 380.0;
pub const HEIGHT: f32 = 356.0;
const PAD: f32 = 24.0;
/// 이 높이 위쪽(닫기 단추 제외)을 끌면 창이 움직인다
pub const HEADER_H: f32 = 72.0;
const FOOTER_Y: f32 = 300.0;
const SWATCH_D: f32 = 28.0;
const SWATCH_GAP: f32 = 10.0;
const COLUMNS: usize = 6;
/// HEX EDIT 글자 크기 (DIP)
pub const EDIT_FONT: f32 = 14.0;

/// 누를 수 있는 곳
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Close,
    Swatch(usize),
    More,
    Slider,
    Blur,
    Ok,
    Cancel,
}

pub fn close_rect() -> RectF {
    RectF::new(WIDTH - 18.0 - 30.0, 18.0, 30.0, 30.0)
}

pub fn swatch_center(i: usize) -> Pt {
    let (row, col) = (i / COLUMNS, i % COLUMNS);
    Pt {
        x: PAD + SWATCH_D / 2.0 + col as f32 * (SWATCH_D + SWATCH_GAP),
        y: 106.0 + SWATCH_D / 2.0 + row as f32 * (SWATCH_D + SWATCH_GAP),
    }
}

pub fn hex_field() -> RectF {
    RectF::new(256.0, 106.0, WIDTH - PAD - 256.0, 32.0)
}

/// HEX 칸 안에 얹는 EDIT 자리
pub fn hex_edit_rect() -> RectF {
    let f = hex_field();
    RectF::new(f.x + 12.0, f.y + (f.h - 20.0) / 2.0, f.w - 24.0, 20.0)
}

pub fn more_rect() -> RectF {
    RectF::new(256.0, 144.0, WIDTH - PAD - 256.0, 28.0)
}

/// 투명도 슬라이더 판정 자리 (트랙은 가운데 6 DIP)
pub fn slider_rect() -> RectF {
    RectF::new(PAD, 216.0, WIDTH - 2.0 * PAD, 24.0)
}

pub fn switch_rect() -> RectF {
    RectF::new(WIDTH - PAD - 44.0, 262.0, 44.0, 24.0)
}

pub fn ok_rect() -> RectF {
    RectF::new(WIDTH - PAD - 92.0, FOOTER_Y + 10.0, 92.0, 36.0)
}

pub fn cancel_rect() -> RectF {
    let ok = ok_rect();
    RectF::new(ok.x - 8.0 - 80.0, ok.y, 80.0, ok.h)
}

/// 슬라이더 위 x(DIP) → 불투명도 0..=100
pub fn opacity_at(x: f32) -> u8 {
    let s = slider_rect();
    (((x - s.x) / s.w).clamp(0.0, 1.0) * 100.0).round() as u8
}

/// 마우스 아래 누를 수 있는 곳
pub fn hit(p: Pt) -> Option<Target> {
    let near = |c: Pt, r: f32| (c.x - p.x).powi(2) + (c.y - p.y).powi(2) <= r * r;
    if close_rect().contains(p) {
        return Some(Target::Close);
    }
    if let Some(i) = (0..PRESETS.len()).find(|&i| near(swatch_center(i), SWATCH_D / 2.0 + 2.0)) {
        return Some(Target::Swatch(i));
    }
    let checks = [
        (more_rect(), Target::More),
        (slider_rect(), Target::Slider),
        (switch_rect(), Target::Blur),
        (ok_rect(), Target::Ok),
        (cancel_rect(), Target::Cancel),
    ];
    checks.iter().find(|(r, _)| r.contains(p)).map(|(_, t)| *t)
}

/// 창 끌기 자리인지 (머리글 빈 곳)
pub fn is_drag_area(p: Pt) -> bool {
    p.y < HEADER_H && !close_rect().contains(p)
}

pub struct FormView {
    pub theme: CustomTheme,
    pub hover: Option<Target>,
    pub pressed: Option<Target>,
    /// HEX 칸에 키보드 포커스
    pub hex_focus: bool,
    /// HEX 칸 글자가 올바른 색이 아님
    pub hex_error: bool,
}

fn draw_swatch(g: &Gfx, c: Pt, rgb: u32, selected: bool, hover: bool) {
    let r = SWATCH_D / 2.0 * if hover { 1.06 } else { 1.0 };
    g.fill_circle(c, r, Color::hex(rgb, 1.0));
    g.stroke_circle(c, r - 0.5, Color::hex(0x000000, 0.14), 1.0);
    if selected {
        g.stroke_circle(c, r + 3.5, ACCENT, 2.0);
    }
}

fn draw_slider(g: &Gfx, opacity: u8, active: bool) {
    let s = slider_rect();
    let track = RectF::new(s.x, s.center().y - 3.0, s.w, 6.0);
    g.fill_round_rect(track, 3.0, SOFT);
    g.stroke_round_rect(track, 3.0, BORDER, 1.0);
    let x = s.x + s.w * opacity as f32 / 100.0;
    g.fill_round_rect(RectF::new(track.x, track.y, x - track.x, track.h), 3.0, ACCENT);
    let knob = Pt { x, y: s.center().y };
    g.soft_shadow(RectF::new(knob.x - 9.0, knob.y - 9.0, 18.0, 18.0), 9.0, 0.7);
    g.fill_circle(knob, 9.0, BG);
    g.stroke_circle(knob, 8.5, if active { ACCENT } else { BORDER }, if active { 2.0 } else { 1.0 });
}

fn draw_switch(g: &Gfx, on: bool, hover: bool) {
    let r = switch_rect();
    let bg = if on {
        ACCENT
    } else if hover {
        Color::hex(0xC9CED6, 1.0)
    } else {
        Color::hex(0xD5D9DF, 1.0)
    };
    g.fill_round_rect(r, r.h / 2.0, bg);
    let knob = Pt { x: if on { r.right() - r.h / 2.0 } else { r.x + r.h / 2.0 }, y: r.center().y };
    g.fill_circle(knob, r.h / 2.0 - 3.0, Color::hex(0xFFFFFF, 1.0));
}

pub fn draw(g: &Gfx, v: &FormView) {
    g.fill_round_rect(RectF::new(0.0, 0.0, WIDTH, HEIGHT), 0.0, BG);

    // 제목 + 닫기
    g.text("사용자 지정 테마", RectF::new(PAD, 20.0, 260.0, 26.0), 18.0, Weight::Bold, Align::Leading, TEXT);
    let subtitle = "독과 패널 바탕을 원하는 색과 투명도로";
    g.text(subtitle, RectF::new(PAD, 47.0, 320.0, 18.0), 12.5, Weight::Regular, Align::Leading, SUBTEXT);
    let close = close_rect();
    if v.hover == Some(Target::Close) {
        g.fill_round_rect(close, 8.0, SOFT);
    }
    let (c, arm) = (close.center(), 5.0);
    g.line(Pt { x: c.x - arm, y: c.y - arm }, Pt { x: c.x + arm, y: c.y + arm }, SUBTEXT, 1.6);
    g.line(Pt { x: c.x - arm, y: c.y + arm }, Pt { x: c.x + arm, y: c.y - arm }, SUBTEXT, 1.6);

    // 배경색
    g.text("배경색", RectF::new(PAD, 84.0, 200.0, 18.0), 12.5, Weight::SemiBold, Align::Leading, LABEL);
    for (i, rgb) in PRESETS.iter().enumerate() {
        let hover = v.hover == Some(Target::Swatch(i));
        draw_swatch(g, swatch_center(i), *rgb, *rgb == v.theme.color, hover);
    }
    draw_field(g, hex_field(), v.hex_focus, v.hex_error, false);
    let more = more_rect();
    let more_bg = if v.pressed == Some(Target::More) {
        Color::hex(0xE2E5EA, 1.0)
    } else if v.hover == Some(Target::More) {
        Color::hex(0xE9EBEF, 1.0)
    } else {
        SOFT
    };
    g.fill_round_rect(more, 7.0, more_bg);
    g.text("더 많은 색…", more, 12.5, Weight::SemiBold, Align::Center, LABEL);

    // 투명도
    g.text("투명도", RectF::new(PAD, 194.0, 200.0, 18.0), 12.5, Weight::SemiBold, Align::Leading, LABEL);
    let value = format!("{}%", v.theme.opacity);
    let value_r = RectF::new(WIDTH - PAD - 60.0, 194.0, 60.0, 18.0);
    g.text(
        &value,
        RectF::new(value_r.x + value_r.w - 40.0, value_r.y, 40.0, 18.0),
        12.5,
        Weight::SemiBold,
        Align::Center,
        TEXT,
    );
    draw_slider(g, v.theme.opacity, matches!(v.pressed, Some(Target::Slider)) || v.hover == Some(Target::Slider));

    // 흐림
    g.text("흐림", RectF::new(PAD, 256.0, 200.0, 20.0), 13.0, Weight::SemiBold, Align::Leading, TEXT);
    let desc = "뒤 배경을 흐리게 비춥니다";
    g.text(desc, RectF::new(PAD, 276.0, 240.0, 16.0), 11.5, Weight::Regular, Align::Leading, SUBTEXT);
    draw_switch(g, v.theme.blur, v.hover == Some(Target::Blur));

    // 아래 단추 줄
    g.fill_round_rect(RectF::new(0.0, FOOTER_Y, WIDTH, HEIGHT - FOOTER_Y), 0.0, FOOTER);
    g.line(Pt { x: 0.0, y: FOOTER_Y + 0.5 }, Pt { x: WIDTH, y: FOOTER_Y + 0.5 }, HAIRLINE, 1.0);
    let hint = RectF::new(PAD, ok_rect().y, 140.0, ok_rect().h);
    g.text("바꾸면 바로 보입니다", hint, 11.5, Weight::Regular, Align::Leading, SUBTEXT);
    draw_button(g, cancel_rect(), "취소", false, v.hover == Some(Target::Cancel), v.pressed == Some(Target::Cancel));
    draw_button(g, ok_rect(), "저장", true, v.hover == Some(Target::Ok), v.pressed == Some(Target::Ok));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_and_slider() {
        assert_eq!(hit(swatch_center(0)), Some(Target::Swatch(0)));
        assert_eq!(hit(swatch_center(11)), Some(Target::Swatch(11)));
        assert_eq!(hit(more_rect().center()), Some(Target::More));
        assert_eq!(hit(slider_rect().center()), Some(Target::Slider));
        assert_eq!(hit(switch_rect().center()), Some(Target::Blur));
        assert_eq!(hit(ok_rect().center()), Some(Target::Ok));
        assert_eq!(hit(cancel_rect().center()), Some(Target::Cancel));
        assert_eq!(hit(hex_field().center()), None, "HEX 칸은 EDIT가 받는다");
        let s = slider_rect();
        assert_eq!(opacity_at(s.x - 50.0), 0);
        assert_eq!(opacity_at(s.right() + 50.0), 100);
        assert_eq!(opacity_at(s.x + s.w / 2.0), 50);
        // 추천 색은 HEX 칸과 겹치지 않는다
        assert!(swatch_center(COLUMNS - 1).x + SWATCH_D / 2.0 < hex_field().x);
    }

    #[test]
    fn renders_theme_dialog_snapshot() {
        let _ = unsafe {
            windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)
        };
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("snapshots");
        std::fs::create_dir_all(&dir).unwrap();
        let gfx = Gfx::offscreen((WIDTH * 2.0) as u32, (HEIGHT * 2.0) as u32, 2.0).unwrap();
        gfx.begin();
        let view = FormView {
            theme: CustomTheme { color: 0x0F4C5C, opacity: 45, blur: true },
            hover: Some(Target::Swatch(4)),
            pressed: None,
            hex_focus: false,
            hex_error: false,
        };
        draw(&gfx, &view);
        gfx.end().unwrap();
        let path = dir.join("theme-dialog.png");
        gfx.save_png(&path).unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() > 1000);
    }
}
