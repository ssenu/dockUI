//! 화면: 테마, 배치, 애니메이션, 그리기 장치, 장면, 패널 창, 입력 창.

pub mod anim;
pub mod dialogs;
#[allow(dead_code)] // 독 모드 화면(Task 4)에서 연결하면 지운다
pub mod dock_layout;
pub mod gfx;
pub mod layout;
pub mod panel;
#[allow(dead_code)] // 독 모드 화면(Task 4)에서 연결하면 지운다
pub mod scene;
pub mod theme;
