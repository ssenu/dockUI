//! 항목 추가·수정 창의 그림 (순수 배치 + 그리기): 제목, 종류 카드, 입력칸 테두리, 앱 고르기, 버튼.
//!
//! 입력 글자는 이 그림 위에 얹은 윈도우 기본 EDIT 컨트롤이 맡는다(한글 입력·붙여넣기 그대로).
//! 좌표는 DIP. 창 크기는 `WIDTH`×`HEIGHT`로 고정이다.

use crate::ui::dialogs::Kind;
use crate::ui::gfx::{Align, Gfx, Weight};
use crate::ui::layout::{Pt, RectF};
use crate::ui::theme::{ACCENT, Color};

pub const WIDTH: f32 = 440.0;
pub const HEIGHT: f32 = 412.0;
const PAD: f32 = 24.0;
/// 이 높이 위쪽(닫기 버튼 제외)을 끌면 창이 움직인다
pub const HEADER_H: f32 = 72.0;
const FOOTER_Y: f32 = 356.0;
const FIELD_RADIUS: f32 = 10.0;
/// EDIT 컨트롤 글자 크기 (DIP)
pub const EDIT_FONT: f32 = 14.0;
const EDIT_H: f32 = 20.0;

pub(crate) const BG: Color = Color::hex(0xFFFFFF, 1.0);
pub(crate) const TEXT: Color = Color::hex(0x1F232B, 1.0);
pub(crate) const SUBTEXT: Color = Color::hex(0x6B7280, 1.0);
pub(crate) const LABEL: Color = Color::hex(0x374151, 1.0);
pub(crate) const BORDER: Color = Color::hex(0xDADDE3, 1.0);
pub(crate) const HAIRLINE: Color = Color::hex(0xECEEF1, 1.0);
pub(crate) const SOFT: Color = Color::hex(0xF3F4F6, 1.0);
pub(crate) const FOOTER: Color = Color::hex(0xF8F9FB, 1.0);
const SELECTED_BG: Color = Color::hex(0xFFF5F4, 1.0);

pub const KINDS: [Kind; 4] = [Kind::Youtube, Kind::App, Kind::Web, Kind::Path];

/// 누를 수 있는 곳
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Close,
    Card(Kind),
    Pick,
    /// 파일·폴더 종류의 "파일" / "폴더" 고르기 단추
    PickFile,
    PickFolder,
    Ok,
    Cancel,
}

/// 입력칸 (포커스·오류 표시용)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Main,
    Name,
}

pub fn close_rect() -> RectF {
    RectF::new(WIDTH - 18.0 - 30.0, 18.0, 30.0, 30.0)
}

pub fn card_rect(i: usize) -> RectF {
    let n = KINDS.len() as f32;
    let w = (WIDTH - 2.0 * PAD - (n - 1.0) * 8.0) / n;
    RectF::new(PAD + i as f32 * (w + 8.0), 84.0, w, 84.0)
}

pub fn main_field() -> RectF {
    RectF::new(PAD, 208.0, WIDTH - 2.0 * PAD, 42.0)
}

pub fn name_field() -> RectF {
    RectF::new(PAD, 296.0, WIDTH - 2.0 * PAD, 42.0)
}

pub fn ok_rect() -> RectF {
    RectF::new(WIDTH - PAD - 92.0, 366.0, 92.0, 36.0)
}

pub fn cancel_rect() -> RectF {
    let ok = ok_rect();
    RectF::new(ok.x - 8.0 - 80.0, ok.y, 80.0, ok.h)
}

/// 앱 입력칸 오른쪽의 "앱 고르기" 단추
pub fn pick_button() -> RectF {
    let f = main_field();
    RectF::new(f.right() - 7.0 - 88.0, f.y + 7.0, 88.0, f.h - 14.0)
}

/// 파일·폴더 입력칸 오른쪽의 "파일" / "폴더" 단추
pub fn pick_file_button() -> RectF {
    let folder = pick_folder_button();
    RectF::new(folder.x - 6.0 - 58.0, folder.y, 58.0, folder.h)
}

pub fn pick_folder_button() -> RectF {
    let f = main_field();
    RectF::new(f.right() - 7.0 - 58.0, f.y + 7.0, 58.0, f.h - 14.0)
}

/// 입력칸 안에 얹는 EDIT 컨트롤 자리
pub fn edit_rect(field: RectF) -> RectF {
    RectF::new(field.x + 14.0, field.y + (field.h - EDIT_H) / 2.0, field.w - 28.0, EDIT_H)
}

fn card_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Youtube => "유튜브",
        Kind::App => "앱",
        Kind::Web => "웹 주소",
        Kind::Path => "파일·폴더",
    }
}

fn card_caption(kind: Kind) -> &'static str {
    match kind {
        Kind::Youtube => "검색 결과",
        Kind::App => "설치된 앱",
        Kind::Web => "사이트 열기",
        Kind::Path => "문서·폴더 열기",
    }
}

fn main_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Youtube => "유튜브 검색어",
        Kind::App => "실행할 앱",
        Kind::Web => "웹 주소",
        Kind::Path => "열 파일이나 폴더",
    }
}

/// 마우스 아래 누를 수 있는 곳. 수정 창(`fixed`)에서는 종류 카드를 누를 수 없다.
pub fn hit(p: Pt, kind: Kind, fixed: bool) -> Option<Target> {
    if close_rect().contains(p) {
        return Some(Target::Close);
    }
    if ok_rect().contains(p) {
        return Some(Target::Ok);
    }
    if cancel_rect().contains(p) {
        return Some(Target::Cancel);
    }
    if kind == Kind::App && main_field().contains(p) {
        return Some(Target::Pick);
    }
    if kind == Kind::Path {
        if pick_file_button().contains(p) {
            return Some(Target::PickFile);
        }
        if pick_folder_button().contains(p) {
            return Some(Target::PickFolder);
        }
    }
    if !fixed {
        for (i, k) in KINDS.iter().enumerate() {
            if card_rect(i).contains(p) && *k != kind {
                return Some(Target::Card(*k));
            }
        }
    }
    None
}

/// 창 끌기 자리인지 (헤더 빈 곳)
pub fn is_drag_area(p: Pt) -> bool {
    p.y < HEADER_H && !close_rect().contains(p)
}

pub struct FormView<'a> {
    pub kind: Kind,
    /// 수정 창: 종류를 바꿀 수 없다
    pub fixed: bool,
    pub title: &'a str,
    pub hover: Option<Target>,
    pub pressed: Option<Target>,
    pub focus: Option<Field>,
    /// 고른 앱 이름 (앱 종류)
    pub app_name: Option<&'a str>,
    /// 고른 파일·폴더 경로 (파일·폴더 종류)
    pub path: Option<&'a str>,
    /// 입력칸 아래 빨간 안내
    pub error: Option<(&'a str, Field)>,
}

/// 너비 안에 들어가게 앞쪽을 … 으로 줄인다 (경로는 끝의 파일 이름이 중요하다)
fn fit_tail(g: &Gfx, text: &str, size: f32, max_w: f32) -> String {
    if g.measure(text, size, Weight::Regular) <= max_w {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    (1..chars.len())
        .map(|start| format!("…{}", chars[start..].iter().collect::<String>()))
        .find(|t| g.measure(t, size, Weight::Regular) <= max_w)
        .unwrap_or_else(|| "…".to_string())
}

/// 종류를 나타내는 작은 둥근 타일 아이콘
fn draw_kind_icon(g: &Gfx, kind: Kind, r: RectF) {
    let radius = r.w * 0.28;
    let c = r.center();
    let s = r.w / 34.0;
    let white = Color::hex(0xFFFFFF, 1.0);
    match kind {
        Kind::Youtube => {
            g.fill_round_rect_gradient(r, radius, Color::hex(0xFF4436, 1.0), Color::hex(0xC4161C, 1.0));
            let tri = [
                Pt { x: c.x - 5.0 * s, y: c.y - 7.0 * s },
                Pt { x: c.x - 5.0 * s, y: c.y + 7.0 * s },
                Pt { x: c.x + 8.0 * s, y: c.y },
            ];
            g.fill_polygon(&tri, white);
        }
        Kind::App => {
            g.fill_round_rect_gradient(r, radius, Color::hex(0x6A8BE0, 1.0), Color::hex(0x3D5DB5, 1.0));
            let (cell, gap) = (6.5 * s, 3.0 * s);
            let start = Pt { x: c.x - cell - gap / 2.0, y: c.y - cell - gap / 2.0 };
            for (dx, dy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
                let cell_rect = RectF::new(start.x + dx * (cell + gap), start.y + dy * (cell + gap), cell, cell);
                g.fill_round_rect(cell_rect, 1.8 * s, white);
            }
        }
        Kind::Path => {
            g.fill_round_rect_gradient(r, radius, Color::hex(0xF7B940, 1.0), Color::hex(0xE08A0B, 1.0));
            // 폴더: 탭 + 몸통
            let (w, h) = (18.0 * s, 13.0 * s);
            let body = RectF::new(c.x - w / 2.0, c.y - h / 2.0 + 1.5 * s, w, h);
            let tab = RectF::new(body.x, body.y - 3.0 * s, w * 0.45, 5.0 * s);
            g.fill_round_rect(tab, 1.5 * s, white);
            g.fill_round_rect(body, 2.0 * s, white);
        }
        Kind::Web => {
            g.fill_round_rect_gradient(r, radius, Color::hex(0x36B3A8, 1.0), Color::hex(0x1E7F87, 1.0));
            let rr = 9.0 * s;
            let w = 1.5 * s;
            g.stroke_circle(c, rr, white, w);
            g.stroke_ellipse(c, rr * 0.45, rr, white, w);
            g.line(Pt { x: c.x - rr, y: c.y }, Pt { x: c.x + rr, y: c.y }, white, w);
            let half = rr * 0.866;
            for dy in [-rr * 0.5, rr * 0.5] {
                g.line(Pt { x: c.x - half, y: c.y + dy }, Pt { x: c.x + half, y: c.y + dy }, white, w * 0.8);
            }
        }
    }
}

fn draw_card(g: &Gfx, kind: Kind, r: RectF, selected: bool, hover: bool, disabled: bool) {
    let (bg, border, width) = if selected {
        (SELECTED_BG, ACCENT, 1.6)
    } else if hover {
        (Color::hex(0xFAFAFB, 1.0), Color::hex(0xC9CED6, 1.0), 1.0)
    } else {
        (BG, BORDER, 1.0)
    };
    if selected {
        g.soft_shadow(r, 12.0, 0.6);
    }
    g.fill_round_rect(r, 12.0, bg);
    let inset = width / 2.0;
    g.stroke_round_rect(RectF::new(r.x + inset, r.y + inset, r.w - width, r.h - width), 12.0 - inset, border, width);
    let icon = RectF::new(r.center().x - 15.0, r.y + 12.0, 30.0, 30.0);
    draw_kind_icon(g, kind, icon);
    g.text(card_label(kind), RectF::new(r.x, r.y + 46.0, r.w, 18.0), 13.0, Weight::SemiBold, Align::Center, TEXT);
    g.text(card_caption(kind), RectF::new(r.x, r.y + 63.0, r.w, 14.0), 10.5, Weight::Regular, Align::Center, SUBTEXT);
    if selected {
        // 오른쪽 위 체크 표시
        let c = Pt { x: r.right() - 13.0, y: r.y + 13.0 };
        g.fill_circle(c, 7.0, ACCENT);
        let white = Color::hex(0xFFFFFF, 1.0);
        g.line(Pt { x: c.x - 3.2, y: c.y + 0.2 }, Pt { x: c.x - 0.8, y: c.y + 2.6 }, white, 1.6);
        g.line(Pt { x: c.x - 0.8, y: c.y + 2.6 }, Pt { x: c.x + 3.4, y: c.y - 2.4 }, white, 1.6);
    }
    if disabled {
        g.fill_round_rect(r, 12.0, Color::hex(0xFFFFFF, 0.6));
    }
}

pub(crate) fn draw_field(g: &Gfx, r: RectF, focused: bool, error: bool, hover: bool) {
    let (border, width) = if error {
        (ACCENT, 1.5)
    } else if focused {
        (TEXT, 1.5)
    } else if hover {
        (Color::hex(0xB8BEC8, 1.0), 1.0)
    } else {
        (BORDER, 1.0)
    };
    if focused && !error {
        // 바깥으로 옅은 포커스 링
        let ring = RectF::new(r.x - 3.0, r.y - 3.0, r.w + 6.0, r.h + 6.0);
        g.fill_round_rect(ring, FIELD_RADIUS + 3.0, Color::hex(0x1F232B, 0.07));
    }
    g.fill_round_rect(r, FIELD_RADIUS, BG);
    let inset = width / 2.0;
    g.stroke_round_rect(
        RectF::new(r.x + inset, r.y + inset, r.w - width, r.h - width),
        FIELD_RADIUS - inset,
        border,
        width,
    );
}

pub(crate) fn draw_button(g: &Gfx, r: RectF, text: &str, primary: bool, hover: bool, pressed: bool) {
    if primary {
        let base = if pressed {
            ACCENT.shade(-0.18)
        } else if hover {
            ACCENT.shade(-0.08)
        } else {
            ACCENT
        };
        g.soft_shadow(r, 10.0, 0.7);
        g.fill_round_rect(r, 10.0, base);
        g.text(text, r, 14.0, Weight::SemiBold, Align::Center, Color::hex(0xFFFFFF, 1.0));
    } else {
        let bg = if pressed {
            Color::hex(0xE9EBEF, 1.0)
        } else if hover {
            SOFT
        } else {
            BG
        };
        g.fill_round_rect(r, 10.0, bg);
        g.stroke_round_rect(RectF::new(r.x + 0.5, r.y + 0.5, r.w - 1.0, r.h - 1.0), 9.5, BORDER, 1.0);
        g.text(text, r, 14.0, Weight::SemiBold, Align::Center, LABEL);
    }
}

pub fn draw(g: &Gfx, v: &FormView) {
    g.fill_round_rect(RectF::new(0.0, 0.0, WIDTH, HEIGHT), 0.0, BG);

    // 제목 + 닫기
    g.text(v.title, RectF::new(PAD, 20.0, 300.0, 26.0), 18.0, Weight::Bold, Align::Leading, TEXT);
    let subtitle = if v.fixed {
        "항목 내용을 고칩니다."
    } else {
        "독과 패널에 올릴 항목의 종류를 고르세요."
    };
    g.text(subtitle, RectF::new(PAD, 47.0, 360.0, 18.0), 12.5, Weight::Regular, Align::Leading, SUBTEXT);
    let close = close_rect();
    if v.hover == Some(Target::Close) {
        g.fill_round_rect(close, 8.0, SOFT);
    }
    let (c, arm) = (close.center(), 5.0);
    g.line(Pt { x: c.x - arm, y: c.y - arm }, Pt { x: c.x + arm, y: c.y + arm }, SUBTEXT, 1.6);
    g.line(Pt { x: c.x - arm, y: c.y + arm }, Pt { x: c.x + arm, y: c.y - arm }, SUBTEXT, 1.6);

    // 종류 카드
    for (i, k) in KINDS.iter().enumerate() {
        let selected = *k == v.kind;
        let hover = v.hover == Some(Target::Card(*k));
        draw_card(g, *k, card_rect(i), selected, hover, v.fixed && !selected);
    }

    // 첫 입력칸
    let main = main_field();
    g.text(main_label(v.kind), RectF::new(PAD, 186.0, 300.0, 18.0), 12.5, Weight::SemiBold, Align::Leading, LABEL);
    let main_error = matches!(v.error, Some((_, Field::Main)));
    if v.kind == Kind::App {
        let hover = matches!(v.hover, Some(Target::Pick));
        draw_field(g, main, false, main_error, hover);
        let text_r = RectF::new(main.x + 14.0, main.y, pick_button().x - main.x - 22.0, main.h);
        match v.app_name {
            Some(name) => g.text(name, text_r, EDIT_FONT, Weight::Regular, Align::Leading, TEXT),
            None => g.text("아직 고른 앱이 없습니다", text_r, EDIT_FONT, Weight::Regular, Align::Leading, SUBTEXT),
        }
        let pick = pick_button();
        let pressed = v.pressed == Some(Target::Pick);
        g.fill_round_rect(
            pick,
            7.0,
            if pressed {
                Color::hex(0xE2E5EA, 1.0)
            } else if hover {
                Color::hex(0xE9EBEF, 1.0)
            } else {
                SOFT
            },
        );
        let label = if v.app_name.is_some() { "다시 고르기" } else { "앱 고르기" };
        g.text(label, pick, 12.5, Weight::SemiBold, Align::Center, LABEL);
    } else if v.kind == Kind::Path {
        draw_field(g, main, false, main_error, false);
        let text_r = RectF::new(main.x + 14.0, main.y, pick_file_button().x - main.x - 22.0, main.h);
        match v.path {
            Some(path) => {
                let shown = fit_tail(g, path, 13.0, text_r.w);
                g.text(&shown, text_r, 13.0, Weight::Regular, Align::Leading, TEXT)
            }
            None => g.text("아직 고르지 않았습니다", text_r, EDIT_FONT, Weight::Regular, Align::Leading, SUBTEXT),
        }
        for (target, r, label) in
            [(Target::PickFile, pick_file_button(), "파일"), (Target::PickFolder, pick_folder_button(), "폴더")]
        {
            let bg = if v.pressed == Some(target) {
                Color::hex(0xE2E5EA, 1.0)
            } else if v.hover == Some(target) {
                Color::hex(0xE9EBEF, 1.0)
            } else {
                SOFT
            };
            g.fill_round_rect(r, 7.0, bg);
            g.text(label, r, 12.5, Weight::SemiBold, Align::Center, LABEL);
        }
    } else {
        draw_field(g, main, v.focus == Some(Field::Main), main_error, false);
    }
    if let Some((text, field)) = v.error {
        let y = match field {
            Field::Main => main.bottom() + 4.0,
            Field::Name => name_field().bottom() + 4.0,
        };
        g.text(text, RectF::new(PAD + 2.0, y, WIDTH - 2.0 * PAD, 16.0), 12.0, Weight::Regular, Align::Leading, ACCENT);
    }

    // 표시 이름
    let name_label = "표시 이름";
    let lw = g.measure(name_label, 12.5, Weight::SemiBold);
    g.text(name_label, RectF::new(PAD, 274.0, lw + 2.0, 18.0), 12.5, Weight::SemiBold, Align::Leading, LABEL);
    g.text("선택", RectF::new(PAD + lw + 6.0, 274.0, 60.0, 18.0), 11.5, Weight::Regular, Align::Leading, SUBTEXT);
    let name_error = matches!(v.error, Some((_, Field::Name)));
    draw_field(g, name_field(), v.focus == Some(Field::Name), name_error, false);

    // 아래 단추 줄
    g.fill_round_rect(RectF::new(0.0, FOOTER_Y, WIDTH, HEIGHT - FOOTER_Y), 0.0, FOOTER);
    g.line(Pt { x: 0.0, y: FOOTER_Y + 0.5 }, Pt { x: WIDTH, y: FOOTER_Y + 0.5 }, HAIRLINE, 1.0);
    let hint = RectF::new(PAD, ok_rect().y, 180.0, ok_rect().h);
    g.text("Enter 저장 · Esc 닫기", hint, 11.5, Weight::Regular, Align::Leading, SUBTEXT);
    let ok_label = if v.fixed { "저장" } else { "추가" };
    draw_button(g, cancel_rect(), "취소", false, v.hover == Some(Target::Cancel), v.pressed == Some(Target::Cancel));
    draw_button(g, ok_rect(), ok_label, true, v.hover == Some(Target::Ok), v.pressed == Some(Target::Ok));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_finds_buttons_cards_and_pick() {
        let center = |r: RectF| r.center();
        assert_eq!(hit(center(close_rect()), Kind::Youtube, false), Some(Target::Close));
        assert_eq!(hit(center(ok_rect()), Kind::Youtube, false), Some(Target::Ok));
        assert_eq!(hit(center(cancel_rect()), Kind::Youtube, false), Some(Target::Cancel));
        assert_eq!(hit(center(card_rect(2)), Kind::Youtube, false), Some(Target::Card(Kind::Web)));
        // 지금 종류 카드·수정 창의 카드는 누를 곳이 아니다
        assert_eq!(hit(center(card_rect(0)), Kind::Youtube, false), None);
        assert_eq!(hit(center(card_rect(2)), Kind::Youtube, true), None);
        assert_eq!(hit(center(main_field()), Kind::App, false), Some(Target::Pick));
        assert_eq!(hit(center(main_field()), Kind::Web, false), None);
        assert!(is_drag_area(Pt { x: 100.0, y: 30.0 }));
        assert!(!is_drag_area(center(close_rect())));
    }

    #[test]
    fn layout_fits_inside_window() {
        let all = [close_rect(), card_rect(0), card_rect(3), main_field(), name_field(), ok_rect(), cancel_rect()];
        for r in all {
            assert!(r.x >= 0.0 && r.right() <= WIDTH && r.y >= 0.0 && r.bottom() <= HEIGHT, "{r:?}");
        }
        assert!(cancel_rect().right() < ok_rect().x);
        assert!(card_rect(3).right() <= WIDTH - PAD + 0.01);
        assert!(pick_file_button().right() < pick_folder_button().x);
        assert_eq!(hit(pick_folder_button().center(), Kind::Path, false), Some(Target::PickFolder));
        assert_eq!(hit(pick_file_button().center(), Kind::Path, false), Some(Target::PickFile));
        let e = edit_rect(name_field());
        assert!(e.y > name_field().y && e.bottom() < name_field().bottom());
    }

    #[test]
    fn renders_dialog_snapshots() {
        let _ = unsafe {
            windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)
        };
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("snapshots");
        std::fs::create_dir_all(&dir).unwrap();
        let views = [
            (
                "dialog-new",
                FormView {
                    kind: Kind::Youtube,
                    fixed: false,
                    title: "항목 추가",
                    hover: Some(Target::Card(Kind::Web)),
                    pressed: None,
                    focus: Some(Field::Main),
                    app_name: None,
                    path: None,
                    error: None,
                },
            ),
            (
                "dialog-app",
                FormView {
                    kind: Kind::App,
                    fixed: false,
                    title: "항목 추가",
                    hover: Some(Target::Ok),
                    pressed: None,
                    focus: None,
                    app_name: None,
                    path: None,
                    error: Some(("앱을 고르세요.", Field::Main)),
                },
            ),
            (
                "dialog-edit",
                FormView {
                    kind: Kind::Web,
                    fixed: true,
                    title: "항목 수정",
                    hover: None,
                    pressed: None,
                    focus: Some(Field::Name),
                    app_name: None,
                    path: None,
                    error: None,
                },
            ),
        ];
        let views = views.into_iter().chain([(
            "dialog-path",
            FormView {
                kind: Kind::Path,
                fixed: false,
                title: "항목 추가",
                hover: Some(Target::PickFolder),
                pressed: None,
                focus: None,
                app_name: None,
                path: Some(r"C:\Users\ssenu\Documents\회의록\2026-10 주간 보고.hwpx"),
                error: None,
            },
        )]);
        for (name, view) in views {
            let gfx = Gfx::offscreen((WIDTH * 2.0) as u32, (HEIGHT * 2.0) as u32, 2.0).unwrap();
            gfx.begin();
            draw(&gfx, &view);
            gfx.end().unwrap();
            let path = dir.join(format!("{name}.png"));
            gfx.save_png(&path).unwrap();
            assert!(std::fs::metadata(&path).unwrap().len() > 1000);
        }
    }
}
