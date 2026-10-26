#![cfg_attr(not(test), windows_subsystem = "windows")]
//! YT Launcher 진입점.

mod app;
mod apps;
mod autostart;
mod chrome;
mod config;
mod icons;
mod items;
mod logging;
mod tray;
mod ui;
mod updater;
mod win;

use win::instance::{MESSAGE_CLASS, MUTEX_NAME, acquire_mutex, notify_existing};

fn main() {
    // 아이콘 만들기 전용 하위 프로세스: 셸 아이콘·인증서 검사가 남기는 메모리를 런처에 남기지 않는다
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some(icons::FETCH_ARG) {
        std::process::exit(icons::run_fetch_process(&args[2..]));
    }

    let data_dir = config::default_dir();
    logging::init(&data_dir.join("logs"));
    logging::info(&format!("YT Launcher {} 시작", app::CURRENT_VERSION));

    // 핸들은 프로세스가 끝날 때까지 쥐고 있는다 (설치 프로그램이 실행 중 감지에 씀)
    let (_mutex, existed) = acquire_mutex(MUTEX_NAME);
    if existed {
        notify_existing();
        return;
    }

    // 셸·COM을 쓰는 기능(ShellExecute, 4단계의 앱 목록)을 위해 UI 스레드를 STA로 초기화한다
    let _ = unsafe {
        windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)
    };

    let app = app::App::new(config::Storage::new(&data_dir));
    if let Err(e) = win::msg_window::create(MESSAGE_CLASS, Box::new(app)) {
        logging::error(&format!("메시지 창을 만들지 못했습니다: {e}"));
        return;
    }
    std::process::exit(win::msg_window::run_loop());
}
