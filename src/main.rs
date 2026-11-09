#![cfg_attr(not(test), windows_subsystem = "windows")]
//! Dock UI 진입점.

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

    // 단일 실행 확인이 먼저다: 이름 바꾸기가 실패해 옛 폴더로 도는 중에 두 번째 프로세스가 폴더를 옮기면 안 된다
    // 핸들은 프로세스가 끝날 때까지 쥐고 있는다 (설치 프로그램이 실행 중 감지에 씀)
    let (_mutex, existed) = acquire_mutex(MUTEX_NAME);
    if existed {
        logging::init(&config::current_data_dir(&config::appdata_dir()).join("logs"));
        logging::info("이미 실행 중: 기존 창에 보이기 요청");
        notify_existing();
        return;
    }

    let (data_dir, moved) = config::prepare_data_dir(&config::appdata_dir());
    logging::init(&data_dir.join("logs"));
    if let Some(note) = moved {
        logging::info(&note);
    }
    // 0.2.x는 업데이트 설치 파일을 이 폴더에 받았다. 새 이름에서는 쓰지 않으니 남은 것을 지운다
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join("YT-Launcher-update"));
    logging::info(&format!("{} {} 시작", app::APP_NAME, app::CURRENT_VERSION));

    // 알림 머리글에 설치 바로가기 이름(Dock UI)·아이콘이 쓰이게 앱 ID를 고정한다
    let _ = unsafe {
        windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(&windows::core::HSTRING::from(app::APP_ID))
    };

    // 셸·COM 기능(ShellExecute, 앱 목록, WIC)과 끌어 놓기(OLE)를 위해 UI 스레드를 OLE(STA)로 초기화한다
    if let Err(e) = unsafe { windows::Win32::System::Ole::OleInitialize(None) } {
        logging::warn(&format!("OLE 초기화 실패 (끌어 놓기 꺼짐): {e}"));
    }

    let app = app::App::new(config::Storage::new(&data_dir));
    if let Err(e) = win::msg_window::create(MESSAGE_CLASS, Box::new(app)) {
        logging::error(&format!("메시지 창을 만들지 못했습니다: {e}"));
        return;
    }
    std::process::exit(win::msg_window::run_loop());
}
