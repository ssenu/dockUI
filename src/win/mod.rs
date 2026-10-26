//! Win32 API를 감싼 얇은 래퍼들.

pub mod desktop;
pub mod dwm;
pub mod instance;
pub mod menu;
pub mod msg_window;
pub mod reg;
pub mod shell;
#[allow(dead_code)] // 앱·웹 항목 연결(Task 4)에서 지운다
pub mod shell_items;
pub mod windows;
