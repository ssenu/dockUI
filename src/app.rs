//! 앱 컨트롤러: 설정·트레이·패널 화면·항목 실행·업데이트·자동 실행을 연결한다.

use crate::apps::{self, AppKey};
use crate::autostart::{AutoStart, RegistryRun};
use crate::chrome;
use crate::config::{Config, DockEdge, LoadReport, Mode, Storage, Theme};
use crate::icons::{FETCH_ARG, icon_path, missing_icons};
use crate::items::{Item, new_id, normalize_url};
use crate::logging;
use crate::tray::{Tray, WM_APP_TRAY};
use crate::ui::app_list::AppChoice;
use crate::ui::dialogs::{Initial, ItemInput, item_dialog};
use crate::ui::drop::Dropped;
use crate::ui::panel::{OpenSpec, Panel, PanelCmd, PanelHandle, UiEvent, initial_rect};
use crate::ui::scene::{TileKind, TileView, badge_of};
use crate::ui::theme::{self, PICKER_ORDER};
use crate::updater::{self, Release, UreqHttp};
use crate::win::instance::WM_APP_SHOW;
use crate::win::menu::{MenuEntry, show_popup};
use crate::win::msg_window::Handler;
use crate::win::shell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    CHILDID_SELF, DestroyWindow, EVENT_OBJECT_HIDE, EVENT_OBJECT_SHOW, GA_ROOT, GetAncestor, IDYES, KillTimer,
    MB_ICONQUESTION, MB_YESNO, MessageBoxW, OBJID_WINDOW, PostMessageW, PostQuitMessage, RegisterWindowMessageW,
    SetTimer, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_APP, WM_CONTEXTMENU, WM_DESTROY, WM_LBUTTONUP,
    WM_RBUTTONUP, WM_TIMER,
};
use windows::core::{HSTRING, w};

/// 화면에 보이는 이름 (exe·설치 파일·데이터 폴더 이름은 업데이트 호환을 위해 그대로 둔다)
pub const APP_NAME: &str = "Dock UI";
pub const BYLINE: &str = "by_ssenu";
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const RELEASES_URL: &str = "https://github.com/ssenu/dockUI/releases/latest";

/// 백그라운드 작업 결과가 채널에 들어왔음을 알리는 메시지
pub const WM_APP_EVENT: u32 = WM_APP + 3;

const TIMER_FIRST_CHECK: usize = 1;
const TIMER_DAILY_CHECK: usize = 2;
/// 창이 생기거나 없어진 뒤 실행 중 점을 다시 계산 (짧게 모아서 한 번, 제목이 늦게 붙는 창을 위해 한 번 더)
const TIMER_RUNNING: usize = 3;
const TIMER_RUNNING_LATE: usize = 4;
const RUNNING_DEBOUNCE_MS: u32 = 300;
const RUNNING_LATE_MS: u32 = 2000;
const FIRST_CHECK_MS: u32 = 30_000;
const DAILY_CHECK_MS: u32 = 24 * 60 * 60 * 1000;
const CLICK_DEBOUNCE: Duration = Duration::from_secs(1);

/// 새 버전이 있을 때 목록 끝에 붙는 업데이트 단추의 id (항목 id와 겹치지 않는다)
const UPDATE_TILE_ID: &str = "__update__";
const CMD_SHOW: u32 = 20;
const CMD_ADD: u32 = 23;
/// 테마 고르기: CMD_THEME + PICKER_ORDER 번호
const CMD_THEME: u32 = 40;
const CMD_MODE_DOCK: u32 = 30;
const CMD_MODE_PANEL: u32 = 31;
const CMD_DOCK_EDIT: u32 = 36;
const CMD_EDGE_BOTTOM: u32 = 32;
const CMD_EDGE_TOP: u32 = 33;
const CMD_EDGE_LEFT: u32 = 34;
const CMD_EDGE_RIGHT: u32 = 35;
const CMD_RESET_GEOMETRY: u32 = 21;
const CMD_LOCK: u32 = 22;
const CMD_AUTOSTART: u32 = 10;
const CMD_CHECK_UPDATE: u32 = 11;
const CMD_INSTALL_UPDATE: u32 = 12;
const CMD_RELEASES: u32 = 13;
const CMD_QUIT: u32 = 14;

pub enum Event {
    Checked {
        manual: bool,
        result: Result<Option<Release>, String>,
    },
    Downloaded(Result<PathBuf, String>),
    /// 업데이트 내려받기 진행률 (0..=100)
    Progress(u8),
    Ui(UiEvent),
    Opened(chrome::Opened),
    /// 아이콘 캐시에 새 파일이 생김
    IconsReady,
    /// 앱 실행 대상을 찾지 못함 (항목 id)
    LaunchFailed(String),
}

/// 업데이트 확인 결과를 보고 무엇을 할지 정한다 (순수 함수).
#[derive(Debug, PartialEq, Eq)]
pub enum UpdateDecision {
    /// 새 버전 발견. `notify`면 이번이 이 버전의 첫 알림
    Available {
        version: String,
        notify: bool,
    },
    UpToDate,
    Failed,
}

pub fn decide_update(result: &Result<Option<Release>, String>, current: &str, notified: &str) -> UpdateDecision {
    match result {
        Err(_) => UpdateDecision::Failed,
        Ok(Some(r)) if r.is_installable() && updater::is_newer(&r.version, current) => {
            UpdateDecision::Available { version: r.version.clone(), notify: r.version != notified }
        }
        Ok(_) => UpdateDecision::UpToDate,
    }
}

/// 시작할 때 띄울 알림 문구 (설정 손상 복구, 업데이트 완료). 순수 함수.
pub fn startup_notices(report: LoadReport, last_run: &str, current: &str) -> Vec<String> {
    let mut notices = Vec::new();
    if report.recovered {
        notices.push("설정 파일이 손상되어 초기화했습니다. 이전 파일은 config.json.bak에 있습니다.".to_string());
    }
    if !last_run.is_empty() && updater::is_newer(current, last_run) {
        notices.push(format!("v{current} 버전으로 업데이트되었습니다."));
    }
    notices
}

/// 타일에 덧붙일 상태: 캐시된 아이콘 찾기, 실행 중인 앱, 실행 대상을 찾지 못한 앱 (항목 id)
pub struct TileStatus<'a> {
    pub icon_of: &'a dyn Fn(&str) -> Option<PathBuf>,
    pub running: &'a HashSet<String>,
    pub missing: &'a HashSet<String>,
}

/// 설정의 항목 → 화면에 그릴 타일 (순수 함수)
pub fn tile_views(items: &[Item], status: &TileStatus) -> Vec<TileView> {
    items
        .iter()
        .map(|item| {
            let id = item.id();
            let label = item.display_name();
            let (kind, badge) = match item {
                Item::Youtube { query, .. } => (TileKind::Youtube, badge_of(query)),
                Item::App { .. } => (TileKind::App, label.chars().take(1).collect()),
                Item::Web { .. } => (TileKind::Web, label.chars().take(1).collect::<String>().to_uppercase()),
                Item::Path { .. } => (TileKind::Path, label.chars().take(1).collect::<String>().to_uppercase()),
            };
            TileView {
                id: id.to_string(),
                badge,
                kind,
                icon: if kind == TileKind::Youtube { None } else { (status.icon_of)(id) },
                running: status.running.contains(id),
                missing: status.missing.contains(id),
                label,
            }
        })
        .collect()
}

/// 입력 창 결과 → 항목 (순수 함수). 앱의 표시 이름을 비우면 앱 이름을 쓴다.
pub fn item_from_input(id: String, input: ItemInput) -> Item {
    match input {
        ItemInput::Youtube { query, name } => Item::Youtube { id, query, name },
        ItemInput::App { app, name } => {
            Item::App { id, name: if name.is_empty() { app.name } else { name }, target: app.target, exe: app.exe }
        }
        ItemInput::Web { url, name } => Item::Web { id, url, name },
        ItemInput::Path { path, name } => Item::Path { id, path, name },
    }
}

/// 항목 → 수정 창의 처음 값 (순수 함수)
pub fn input_of(item: &Item) -> ItemInput {
    match item.clone() {
        Item::Youtube { query, name, .. } => ItemInput::Youtube { query, name },
        Item::App { name, target, exe, .. } => {
            ItemInput::App { app: AppChoice { name: name.clone(), target, exe }, name }
        }
        Item::Web { url, name, .. } => ItemInput::Web { url, name },
        Item::Path { path, name, .. } => ItemInput::Path { path, name },
    }
}

/// 실행 대상이 바뀌어 아이콘을 다시 받아야 하는가 (순수 함수)
pub fn icon_source_changed(before: &Item, after: &Item) -> bool {
    match (before, after) {
        (Item::App { target: a, .. }, Item::App { target: b, .. }) => a != b,
        (Item::Web { url: a, .. }, Item::Web { url: b, .. }) => a != b,
        (Item::Path { path: a, .. }, Item::Path { path: b, .. }) => a != b,
        _ => false,
    }
}

/// 실행 중 판정에 쓰는 앱 식별 정보 (앱이 아닌 항목은 None)
pub fn app_keys(items: &[Item]) -> Vec<Option<AppKey>> {
    items
        .iter()
        .map(|item| match item {
            Item::App { target, exe, .. } => Some(AppKey { target: target.clone(), exe: exe.clone() }),
            _ => None,
        })
        .collect()
}

/// 창 변화 알림을 받을 앱 창 (WinEvent 콜백은 인자를 받지 못한다)
static APP_HWND: AtomicIsize = AtomicIsize::new(0);

/// 최상위 창이 보이거나 숨으면 실행 중 점 재계산 타이머를 (다시) 건다. 앱 스레드에서 불린다.
unsafe extern "system" fn on_window_event(
    _: HWINEVENTHOOK,
    _: u32,
    hwnd: HWND,
    object: i32,
    child: i32,
    _: u32,
    _: u32,
) {
    if object != OBJID_WINDOW.0 || child != CHILDID_SELF as i32 || hwnd.0.is_null() {
        return;
    }
    // SAFETY: 핸들 값만 쓰는 호출
    unsafe {
        if GetAncestor(hwnd, GA_ROOT) != hwnd {
            return;
        }
        let app = HWND(APP_HWND.load(Ordering::Relaxed) as *mut _);
        if !app.0.is_null() {
            SetTimer(Some(app), TIMER_RUNNING, RUNNING_DEBOUNCE_MS, None);
            SetTimer(Some(app), TIMER_RUNNING_LATE, RUNNING_LATE_MS, None);
        }
    }
}

/// 새 버전 업데이트 단추. 내려받는 중이면 배지·이름으로 알린다.
fn update_tile(version: &str, downloading: bool, progress: Option<u8>) -> TileView {
    let (label, badge) = match (downloading, progress) {
        (true, Some(p)) => (format!("업데이트 내려받는 중… {p}%"), format!("{p}%")),
        (true, None) => ("업데이트 내려받는 중…".to_string(), "받는 중".to_string()),
        (false, _) => (format!("새 버전 v{version} 설치"), format!("v{version}")),
    };
    TileView { id: UPDATE_TILE_ID.to_string(), label, badge, kind: TileKind::Update, ..Default::default() }
}

/// 끌어다 놓은 것 → 입력 (exe·바로가기는 앱, .url 파일과 주소는 웹, 그 밖의 파일·폴더는 파일 항목)
fn input_from_drop(d: &Dropped) -> Option<ItemInput> {
    match d {
        Dropped::Url(url) => normalize_url(url).ok().map(|url| ItemInput::Web { url, name: String::new() }),
        Dropped::File(path) => {
            let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
            match ext.as_str() {
                "exe" | "lnk" | "appref-ms" => Some(ItemInput::App {
                    app: crate::win::shell_items::file_app(&path.to_string_lossy()),
                    name: String::new(),
                }),
                "url" => {
                    let text = std::fs::read_to_string(path).ok()?;
                    let url = internet_shortcut_url(&text)?;
                    normalize_url(&url).ok().map(|url| ItemInput::Web { url, name: String::new() })
                }
                _ if path.exists() => {
                    Some(ItemInput::Path { path: path.to_string_lossy().into_owned(), name: String::new() })
                }
                _ => None,
            }
        }
    }
}

/// 인터넷 바로가기(.url) 파일의 `URL=` 줄 (순수 함수)
fn internet_shortcut_url(text: &str) -> Option<String> {
    text.lines().find_map(|l| l.trim().strip_prefix("URL=")).map(|u| u.trim().to_string()).filter(|u| !u.is_empty())
}

/// 같은 앱·같은 주소를 가리키는 항목인가 (순수 함수)
fn same_target(a: &Item, b: &Item) -> bool {
    match (a, b) {
        (Item::App { target: x, .. }, Item::App { target: y, .. }) => x.eq_ignore_ascii_case(y),
        (Item::Web { url: x, .. }, Item::Web { url: y, .. }) => x.trim_end_matches('/') == y.trim_end_matches('/'),
        (Item::Path { path: x, .. }, Item::Path { path: y, .. }) => x.eq_ignore_ascii_case(y),
        _ => false,
    }
}

/// 끌어 놓기 결과 알림 문구 (순수 함수)
fn drop_message(added: usize, duplicate: usize, unsupported: usize) -> String {
    let mut parts = Vec::new();
    if added > 0 {
        parts.push(format!("{added}개 항목을 추가했습니다."));
    }
    if duplicate > 0 {
        parts.push(format!("이미 있는 {duplicate}개는 건너뛰었습니다."));
    }
    if unsupported > 0 {
        parts.push(format!("읽을 수 없는 {unsupported}개는 건너뛰었습니다."));
    }
    parts.join(" ")
}

/// 내려받은 양 → 0..=100 (전체 크기를 모르면 None)
fn percent(done: u64, total: u64) -> Option<u8> {
    (total > 0).then(|| (done.saturating_mul(100) / total).min(100) as u8)
}

/// 업데이트 확인 질문: 버전 + 바뀐 점(릴리스 본문 요약)
fn update_question(release: &Release) -> String {
    let mut text = format!("v{} 버전으로 업데이트할까요?", release.version);
    let notes = updater::summarize_notes(&release.notes, 8);
    if !notes.is_empty() {
        text.push_str("\n\n바뀐 점");
        for line in notes {
            text.push_str(&format!("\n • {line}"));
        }
    }
    text.push_str("\n\n런처가 잠시 꺼졌다 켜집니다.");
    text
}

/// 화면에서 바뀐 순서(id 목록)대로 항목을 다시 늘어놓는다. 목록에 없는 항목은 뒤에 붙인다.
pub fn apply_order(items: &[Item], ids: &[String]) -> Vec<Item> {
    let mut rest: Vec<Item> = items.to_vec();
    let mut ordered = Vec::with_capacity(items.len());
    for id in ids {
        if let Some(pos) = rest.iter().position(|i| i.id() == id) {
            ordered.push(rest.remove(pos));
        }
    }
    ordered.extend(rest);
    ordered
}

/// 같은 항목을 짧은 시간 안에 두 번 누르면 무시한다.
#[derive(Default)]
pub struct Debounce {
    last: HashMap<String, Instant>,
}

impl Debounce {
    pub fn allow(&mut self, id: &str, now: Instant) -> bool {
        match self.last.get(id) {
            Some(t) if now.duration_since(*t) < CLICK_DEBOUNCE => false,
            _ => {
                self.last.insert(id.to_string(), now);
                true
            }
        }
    }
}

pub struct App {
    hwnd: HWND,
    storage: Storage,
    config: Config,
    tray: Option<Tray>,
    panel: Option<PanelHandle>,
    autostart: AutoStart<RegistryRun>,
    release: Option<Release>,
    downloading: bool,
    /// 내려받기 진행률 (내려받는 중일 때만)
    progress: Option<u8>,
    /// 독 수정 모드가 켜져 있는지 (트레이 메뉴 체크)
    dock_editing: bool,
    debounce: Debounce,
    events_tx: Sender<Event>,
    events_rx: Receiver<Event>,
    taskbar_created: u32,
    pending_notices: Vec<String>,
    running: HashSet<String>,
    missing: HashSet<String>,
    window_hook: Option<HWINEVENTHOOK>,
}

impl App {
    pub fn new(storage: Storage) -> Self {
        let (config, report) = storage.load();
        let (events_tx, events_rx) = channel();
        let pending_notices = startup_notices(report, &config.last_run_version, CURRENT_VERSION);
        if report.migrated {
            logging::info("0.1.x 설정을 schema 2로 변환했습니다 (원본: config.v1.json)");
        }
        App {
            hwnd: HWND::default(),
            storage,
            config,
            tray: None,
            panel: None,
            autostart: AutoStart { backend: RegistryRun::default() },
            release: None,
            downloading: false,
            progress: None,
            dock_editing: false,
            debounce: Debounce::default(),
            events_tx,
            events_rx,
            taskbar_created: unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) },
            pending_notices,
            running: HashSet::new(),
            missing: HashSet::new(),
            window_hook: None,
        }
    }

    fn save(&self) {
        if let Err(e) = self.storage.save(&self.config) {
            logging::error(&format!("설정 저장 실패: {e}"));
        }
    }

    fn notify(&self, message: &str) {
        if let Some(tray) = &self.tray {
            tray.notify(APP_NAME, message);
        }
    }

    fn panel_send(&self, cmd: PanelCmd) {
        if let Some(panel) = &self.panel {
            panel.send(cmd);
        }
    }

    fn tiles(&self) -> Vec<TileView> {
        let dir = self.storage.dir.clone();
        let icon_of = move |id: &str| Some(icon_path(&dir, id)).filter(|p| p.exists());
        let status = TileStatus { icon_of: &icon_of, running: &self.running, missing: &self.missing };
        let mut tiles = tile_views(&self.config.items, &status);
        if let Some(release) = &self.release {
            tiles.push(update_tile(&release.version, self.downloading, self.progress));
        }
        tiles
    }

    fn refresh_tiles(&self) {
        self.panel_send(PanelCmd::SetTiles(self.tiles()));
    }

    /// 캐시에 아이콘이 없는 앱·웹 항목의 아이콘을 만든다 (파비콘 실패는 다음 실행 때 다시).
    /// 셸 아이콘 추출과 HTTPS 인증서 검사는 수십 MB를 남기므로 잠깐 뜨는 하위 프로세스에 맡긴다.
    fn fetch_icons(&self, items: Vec<Item>) {
        let dir = self.storage.dir.clone();
        let todo = missing_icons(&dir, &items);
        if todo.is_empty() {
            return;
        }
        let Ok(json) = serde_json::to_string(&todo) else { return };
        let send = self.post_event();
        std::thread::spawn(move || {
            let status = std::env::current_exe()
                .and_then(|exe| std::process::Command::new(exe).arg(FETCH_ARG).arg(&dir).arg(json).status());
            match status {
                Ok(s) if s.success() => send(Event::IconsReady),
                Ok(_) => logging::info("일부 아이콘을 만들지 못했습니다 (다음 실행 때 다시 시도)"),
                Err(e) => logging::warn(&format!("아이콘 프로세스 실행 실패: {e}")),
            }
        });
    }

    fn remove_icon(&self, id: &str) {
        let _ = std::fs::remove_file(icon_path(&self.storage.dir, id));
    }

    /// 지금 열린 창으로 실행 중인 앱을 다시 계산하고, 바뀌었으면 화면에 알린다.
    fn update_running(&mut self) {
        let keys = app_keys(&self.config.items);
        let running: HashSet<String> = if keys.iter().all(Option::is_none) {
            HashSet::new()
        } else {
            let flags = apps::running_flags(&keys, &apps::current_windows());
            self.config.items.iter().zip(flags).filter(|(_, on)| *on).map(|(i, _)| i.id().to_string()).collect()
        };
        if running != self.running {
            // 실행 중인 앱은 찾지 못함 표시를 지운다
            self.missing.retain(|id| !running.contains(id));
            self.running = running;
            self.refresh_tiles();
        }
    }

    fn watch_windows(&mut self, hwnd: HWND) {
        APP_HWND.store(hwnd.0 as isize, Ordering::Relaxed);
        // SAFETY: 우리 스레드 메시지 루프로 불리는 창 표시·숨김 알림
        let hook = unsafe {
            SetWinEventHook(
                EVENT_OBJECT_SHOW,
                EVENT_OBJECT_HIDE,
                None,
                Some(on_window_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            )
        };
        if hook.0.is_null() {
            logging::warn("창 변화 알림을 등록하지 못했습니다 (실행 중 표시 없음)");
        } else {
            self.window_hook = Some(hook);
        }
    }

    fn unwatch_windows(&mut self) {
        if let Some(hook) = self.window_hook.take() {
            let _ = unsafe { UnhookWinEvent(hook) };
        }
        APP_HWND.store(0, Ordering::Relaxed);
    }

    /// 창이 만들어진 직후 한 번 호출된다.
    fn start(&mut self, hwnd: HWND) {
        self.hwnd = hwnd;
        let tray = Tray::new(hwnd, APP_NAME);
        if !tray.add() {
            logging::warn("트레이 아이콘 추가 실패");
        }
        self.tray = Some(tray);
        // 업데이트 설치가 Run 키를 되살려도 사용자가 고른 값을 지킨다 (개발 빌드에서는 건드리지 않음)
        if !cfg!(debug_assertions)
            && let Some(enabled) = self.config.autostart
            && let Err(e) = self.autostart.set_enabled(enabled)
        {
            logging::error(&format!("자동 실행 설정 실패: {e}"));
        }
        self.update_running_silently();
        self.open_panel();
        self.watch_windows(hwnd);
        self.fetch_icons(self.config.items.clone());
        for notice in std::mem::take(&mut self.pending_notices) {
            self.notify(&notice);
        }
        if self.config.last_run_version != CURRENT_VERSION {
            self.config.last_run_version = CURRENT_VERSION.to_string();
            self.save();
        }
        unsafe { SetTimer(Some(hwnd), TIMER_FIRST_CHECK, FIRST_CHECK_MS, None) };
    }

    /// 화면을 열기 전 첫 계산 (화면에 보낼 곳이 아직 없다)
    fn update_running_silently(&mut self) {
        let keys = app_keys(&self.config.items);
        if keys.iter().any(Option::is_some) {
            let flags = apps::running_flags(&keys, &apps::current_windows());
            self.running =
                self.config.items.iter().zip(flags).filter(|(_, on)| *on).map(|(i, _)| i.id().to_string()).collect();
        }
    }

    fn open_panel(&mut self) {
        let send = self.post_event();
        let notify = Box::new(move |e: UiEvent| send(Event::Ui(e)));
        let spec = OpenSpec {
            mode: self.config.mode,
            edge: self.config.dock_edge,
            panel_rect: initial_rect(self.config.panel_window),
            theme: self.config.theme,
            locked: self.config.locked,
            tiles: self.tiles(),
            dock_offset: self.config.dock_offset,
            dock_size: self.config.dock_size,
        };
        match Panel::open(spec, notify) {
            Ok(handle) => self.panel = Some(handle),
            Err(e) => logging::error(&format!("패널 창을 만들지 못했습니다: {e}")),
        }
    }

    fn post_event(&self) -> impl Fn(Event) + Send + 'static {
        let tx = self.events_tx.clone();
        let hwnd = self.hwnd.0 as isize;
        move |event| {
            if tx.send(event).is_ok() {
                let _ = unsafe { PostMessageW(Some(HWND(hwnd as *mut _)), WM_APP_EVENT, WPARAM(0), LPARAM(0)) };
            }
        }
    }

    fn dialog_owner(&self) -> HWND {
        self.panel.as_ref().map(PanelHandle::owner).unwrap_or(self.hwnd)
    }

    fn on_ui(&mut self, event: UiEvent) {
        match event {
            UiEvent::TileClicked(id) if id == UPDATE_TILE_ID => self.install_update(),
            UiEvent::TileClicked(id) => self.open_item(&id),
            UiEvent::AddRequested => self.add_item(),
            UiEvent::EditRequested(id) => self.edit_item(&id),
            UiEvent::DeleteRequested(id) => self.delete_item(&id),
            UiEvent::Reordered(ids) => {
                self.config.items = apply_order(&self.config.items, &ids);
                self.save();
            }
            UiEvent::ThemePicked(theme) => self.set_theme(theme),
            UiEvent::GeometryChanged(rect) => {
                self.config.panel_window = Some(rect);
                self.save();
            }
            UiEvent::MenuRequested => self.show_menu(),
            UiEvent::Dropped(dropped) => self.add_dropped(dropped),
            UiEvent::DockChanged { edge, offset, size } => {
                self.config.dock_edge = edge;
                self.config.dock_offset = offset;
                self.config.dock_size = size;
                self.save();
            }
            UiEvent::EditingChanged(on) => self.dock_editing = on,
        }
    }

    /// 끌어다 놓은 파일·주소를 항목으로 추가한다 (이미 있는 것과 지원하지 않는 것은 건너뛴다).
    fn add_dropped(&mut self, dropped: Vec<Dropped>) {
        let (mut added, mut duplicate, mut unsupported) = (Vec::new(), 0, 0);
        for d in &dropped {
            let Some(input) = input_from_drop(d) else {
                unsupported += 1;
                continue;
            };
            let item = item_from_input(new_id(), input);
            if self.config.items.iter().chain(&added).any(|i| same_target(i, &item)) {
                duplicate += 1;
                continue;
            }
            added.push(item);
        }
        if !added.is_empty() {
            self.config.items.extend(added.iter().cloned());
            self.save();
            self.update_running();
            self.refresh_tiles();
        }
        self.notify(&drop_message(added.len(), duplicate, unsupported));
        self.fetch_icons(added);
    }

    fn add_item(&mut self) {
        if let Some(input) = item_dialog(self.dialog_owner(), "항목 추가", &Initial::New) {
            let item = item_from_input(new_id(), input);
            self.config.items.push(item.clone());
            self.save();
            self.update_running();
            self.refresh_tiles();
            self.fetch_icons(vec![item]);
        }
    }

    fn edit_item(&mut self, id: &str) {
        let Some(index) = self.config.items.iter().position(|i| i.id() == id) else { return };
        let initial = Initial::Edit(input_of(&self.config.items[index]));
        let Some(input) = item_dialog(self.dialog_owner(), "항목 수정", &initial) else { return };
        let item = item_from_input(id.to_string(), input);
        if icon_source_changed(&self.config.items[index], &item) {
            self.remove_icon(id);
            self.missing.remove(id);
        }
        self.config.items[index] = item.clone();
        self.save();
        self.update_running();
        self.refresh_tiles();
        self.fetch_icons(vec![item]);
    }

    fn delete_item(&mut self, id: &str) {
        let Some(item) = self.config.items.iter().find(|i| i.id() == id) else { return };
        let text = HSTRING::from(format!("'{}' 항목을 삭제할까요?", item.display_name()));
        let answer = unsafe {
            MessageBoxW(Some(self.dialog_owner()), &text, &HSTRING::from(APP_NAME), MB_YESNO | MB_ICONQUESTION)
        };
        if answer == IDYES {
            self.config.items.retain(|i| i.id() != id);
            self.save();
            self.remove_icon(id);
            self.running.remove(id);
            self.missing.remove(id);
            self.refresh_tiles();
        }
    }

    fn open_item(&mut self, id: &str) {
        if !self.debounce.allow(id, Instant::now()) {
            return;
        }
        let Some(item) = self.config.items.iter().find(|i| i.id() == id).cloned() else { return };
        let send = self.post_event();
        std::thread::spawn(move || {
            let result = match &item {
                Item::Youtube { query, .. } => chrome::open_url(&chrome::build_search_url(query), Some("YouTube")),
                Item::Web { url, .. } => chrome::open_url(url, None),
                Item::Path { id, path, .. } => {
                    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
                    let opened = if std::path::Path::new(path).exists() {
                        shell::open(path)
                    } else {
                        Err(std::io::Error::from(std::io::ErrorKind::NotFound))
                    };
                    if let Err(e) = opened {
                        logging::warn(&format!("파일 열기 실패 ({path}): {e}"));
                        send(Event::LaunchFailed(id.clone()));
                    }
                    unsafe { CoUninitialize() };
                    return;
                }
                Item::App { id, target, exe, .. } => {
                    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
                    let key = AppKey { target: target.clone(), exe: exe.clone() };
                    if let Err(e) = apps::launch(&key) {
                        logging::warn(&format!("앱 실행 실패 ({target}): {e}"));
                        send(Event::LaunchFailed(id.clone()));
                    }
                    unsafe { CoUninitialize() };
                    return;
                }
            };
            match result {
                Ok(opened) => send(Event::Opened(opened)),
                Err(e) => logging::error(&format!("열기 실패: {e}")),
            }
        });
    }

    fn set_mode(&mut self, mode: Mode, edge: DockEdge) {
        self.config.mode = mode;
        self.config.dock_edge = edge;
        self.save();
        self.panel_send(PanelCmd::SetMode(mode, edge));
    }

    /// 메뉴에서 독 가장자리를 고르면 그 가장자리 가운데로
    fn set_edge(&mut self, edge: DockEdge) {
        self.config.dock_offset = 0.5;
        self.panel_send(PanelCmd::SetDockPlacement { offset: 0.5, size: self.config.dock_size });
        self.set_mode(Mode::Dock, edge);
    }

    fn set_theme(&mut self, theme: Theme) {
        self.config.theme = theme;
        self.save();
        self.panel_send(PanelCmd::SetTheme(theme));
    }

    fn check_updates(&self, manual: bool) {
        let send = self.post_event();
        std::thread::spawn(move || {
            let result = updater::fetch_latest(&UreqHttp::new(), updater::REPO).map_err(|e| e.to_string());
            if let Err(e) = &result {
                logging::warn(&format!("업데이트 확인 실패: {e}"));
            }
            send(Event::Checked { manual, result });
        });
    }

    fn on_checked(&mut self, manual: bool, result: Result<Option<Release>, String>) {
        match decide_update(&result, CURRENT_VERSION, &self.config.notified_version) {
            UpdateDecision::Available { version, notify } => {
                self.release = result.ok().flatten();
                self.refresh_tiles();
                if notify {
                    self.config.notified_version = version.clone();
                    self.save();
                }
                if notify || manual {
                    self.notify(&format!("새 버전(v{version})이 나왔습니다. 목록 끝의 업데이트 단추를 누르세요."));
                }
            }
            UpdateDecision::UpToDate if manual => self.notify("최신 버전입니다."),
            UpdateDecision::Failed if manual => {
                self.notify("업데이트를 확인하지 못했습니다. 인터넷 연결을 확인하세요.")
            }
            _ => {}
        }
    }

    fn install_update(&mut self) {
        let Some(release) = self.release.clone() else { return };
        if self.downloading {
            return;
        }
        let text = HSTRING::from(update_question(&release));
        let answer =
            unsafe { MessageBoxW(Some(self.hwnd), &text, &HSTRING::from(APP_NAME), MB_YESNO | MB_ICONQUESTION) };
        if answer != IDYES {
            return;
        }
        self.downloading = true;
        self.progress = None;
        self.refresh_tiles();
        self.notify("업데이트를 내려받는 중입니다…");
        let send = self.post_event();
        let report = self.post_event();
        std::thread::spawn(move || {
            let dir = std::env::temp_dir().join("YT-Launcher-update");
            let _ = std::fs::remove_dir_all(&dir);
            // 진행률은 1% 바뀔 때만 알린다
            let mut last = None;
            let mut on_progress = |done: u64, total: u64| {
                let Some(p) = percent(done, total) else { return };
                if last != Some(p) {
                    last = Some(p);
                    report(Event::Progress(p));
                }
            };
            let result = updater::download_and_verify(&UreqHttp::new(), &release, &dir, &mut on_progress)
                .map_err(|e| e.to_string());
            send(Event::Downloaded(result));
        });
    }

    fn on_downloaded(&mut self, result: Result<PathBuf, String>) {
        let launched = result.and_then(|path| updater::launch_installer(&path).map_err(|e| e.to_string()));
        match launched {
            Ok(()) => self.quit(),
            Err(e) => {
                logging::error(&format!("업데이트 실패: {e}"));
                self.downloading = false;
                self.progress = None;
                self.refresh_tiles();
                self.notify(&format!("업데이트에 실패했습니다: {e}"));
            }
        }
    }

    fn set_autostart(&mut self, enabled: bool) {
        self.config.autostart = Some(enabled);
        self.save();
        if let Err(e) = self.autostart.set_enabled(enabled) {
            logging::error(&format!("자동 실행 설정 실패: {e}"));
        }
    }

    fn menu_entries(&self) -> Vec<MenuEntry> {
        let dock = self.config.mode == Mode::Dock;
        let edge = self.config.dock_edge;
        let mut entries = vec![
            MenuEntry::label(format!("{APP_NAME}  {BYLINE}  v{CURRENT_VERSION}")),
            MenuEntry::Separator,
            MenuEntry::item(CMD_SHOW, "런처 보이기"),
            MenuEntry::item(CMD_ADD, "항목 추가…"),
            MenuEntry::submenu(
                "테마",
                PICKER_ORDER
                    .iter()
                    .enumerate()
                    .map(|(i, t)| MenuEntry::checked(CMD_THEME + i as u32, theme::label(*t), *t == self.config.theme))
                    .collect(),
            ),
            MenuEntry::Separator,
            MenuEntry::checked(CMD_MODE_DOCK, "독 모드", dock),
            MenuEntry::checked(CMD_DOCK_EDIT, "    독 수정 모드 (옮기기·크기·순서)", dock && self.dock_editing),
            MenuEntry::checked(CMD_MODE_PANEL, "패널 모드", !dock),
        ];
        if dock {
            entries.extend([
                MenuEntry::checked(CMD_EDGE_BOTTOM, "    독 위치: 아래", edge == DockEdge::Bottom),
                MenuEntry::checked(CMD_EDGE_TOP, "    독 위치: 위", edge == DockEdge::Top),
                MenuEntry::checked(CMD_EDGE_LEFT, "    독 위치: 왼쪽", edge == DockEdge::Left),
                MenuEntry::checked(CMD_EDGE_RIGHT, "    독 위치: 오른쪽", edge == DockEdge::Right),
            ]);
        } else {
            entries.extend([
                MenuEntry::item(CMD_RESET_GEOMETRY, "위치·크기 초기화"),
                MenuEntry::checked(CMD_LOCK, "위치 잠금", self.config.locked),
            ]);
        }
        entries.extend([
            MenuEntry::Separator,
            MenuEntry::checked(CMD_AUTOSTART, "윈도우 시작 시 실행", self.autostart.is_enabled()),
            MenuEntry::item(CMD_CHECK_UPDATE, "업데이트 확인"),
        ]);
        if let Some(r) = &self.release {
            let text = if self.downloading {
                "업데이트 내려받는 중…".to_string()
            } else {
                format!("업데이트 설치 (v{})", r.version)
            };
            entries.push(MenuEntry::Item { id: CMD_INSTALL_UPDATE, text, checked: false, enabled: !self.downloading });
        }
        entries.extend([
            MenuEntry::item(CMD_RELEASES, "릴리스 페이지 열기"),
            MenuEntry::Separator,
            MenuEntry::item(CMD_QUIT, "종료"),
        ]);
        entries
    }

    fn show_menu(&mut self) {
        match show_popup(self.hwnd, &self.menu_entries()) {
            CMD_SHOW => self.panel_send(PanelCmd::Show),
            CMD_ADD => self.add_item(),
            id if (CMD_THEME..CMD_THEME + PICKER_ORDER.len() as u32).contains(&id) => {
                self.set_theme(PICKER_ORDER[(id - CMD_THEME) as usize]);
            }
            CMD_MODE_DOCK => self.set_mode(Mode::Dock, self.config.dock_edge),
            CMD_MODE_PANEL => self.set_mode(Mode::Panel, self.config.dock_edge),
            CMD_DOCK_EDIT => {
                let on = !(self.config.mode == Mode::Dock && self.dock_editing);
                if on && self.config.mode != Mode::Dock {
                    self.set_mode(Mode::Dock, self.config.dock_edge);
                }
                self.panel_send(PanelCmd::SetEditing(on));
            }
            CMD_EDGE_BOTTOM => self.set_edge(DockEdge::Bottom),
            CMD_EDGE_TOP => self.set_edge(DockEdge::Top),
            CMD_EDGE_LEFT => self.set_edge(DockEdge::Left),
            CMD_EDGE_RIGHT => self.set_edge(DockEdge::Right),
            CMD_RESET_GEOMETRY => self.panel_send(PanelCmd::ResetGeometry),
            CMD_LOCK => {
                self.config.locked = !self.config.locked;
                self.save();
                self.panel_send(PanelCmd::SetLocked(self.config.locked));
            }
            CMD_AUTOSTART => self.set_autostart(!self.autostart.is_enabled()),
            CMD_CHECK_UPDATE => self.check_updates(true),
            CMD_INSTALL_UPDATE => self.install_update(),
            CMD_RELEASES => {
                let url = self.release.as_ref().map(|r| r.html_url.as_str()).filter(|u| !u.is_empty());
                if let Err(e) = shell::open(url.unwrap_or(RELEASES_URL)) {
                    logging::warn(&format!("릴리스 페이지 열기 실패: {e}"));
                }
            }
            CMD_QUIT => self.quit(),
            _ => {}
        }
    }

    fn quit(&mut self) {
        self.unwatch_windows();
        if let Some(tray) = self.tray.take() {
            tray.remove();
        }
        self.panel_send(PanelCmd::Close);
        let _ = unsafe { DestroyWindow(self.hwnd) };
        // 핸들러 안에서 자기 창을 없애면 WM_DESTROY가 핸들러에 오지 않으므로 여기서 루프를 끝낸다
        unsafe { PostQuitMessage(0) };
    }
}

impl Handler for App {
    fn handle(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        if self.hwnd.0.is_null() {
            self.start(hwnd);
        }
        match msg {
            WM_APP_TRAY => {
                let event = (lparam.0 & 0xFFFF) as u32;
                if matches!(event, WM_LBUTTONUP | WM_RBUTTONUP | WM_CONTEXTMENU) {
                    self.show_menu();
                }
            }
            WM_APP_EVENT => {
                while let Ok(event) = self.events_rx.try_recv() {
                    match event {
                        Event::Checked { manual, result } => self.on_checked(manual, result),
                        Event::Downloaded(result) => self.on_downloaded(result),
                        Event::Progress(p) if self.downloading => {
                            self.progress = Some(p);
                            self.refresh_tiles();
                        }
                        Event::Progress(_) => {}
                        Event::Ui(ui) => self.on_ui(ui),
                        Event::Opened(chrome::Opened::DefaultBrowser) if !self.config.chrome_notice_shown => {
                            self.config.chrome_notice_shown = true;
                            self.save();
                            self.notify("크롬을 찾지 못해 기본 브라우저로 열었습니다.");
                        }
                        Event::Opened(_) => {}
                        Event::IconsReady => self.refresh_tiles(),
                        Event::LaunchFailed(id) => {
                            self.missing.insert(id);
                            self.refresh_tiles();
                            self.notify("실행할 앱이나 파일을 찾을 수 없습니다. 수정에서 다시 고르세요.");
                        }
                    }
                }
            }
            WM_APP_SHOW => self.panel_send(PanelCmd::Show),
            WM_TIMER => match wparam.0 {
                TIMER_FIRST_CHECK => {
                    let _ = unsafe { KillTimer(Some(hwnd), TIMER_FIRST_CHECK) };
                    unsafe { SetTimer(Some(hwnd), TIMER_DAILY_CHECK, DAILY_CHECK_MS, None) };
                    self.check_updates(false);
                }
                TIMER_DAILY_CHECK => self.check_updates(false),
                TIMER_RUNNING | TIMER_RUNNING_LATE => {
                    let _ = unsafe { KillTimer(Some(hwnd), wparam.0) };
                    self.update_running();
                }
                _ => return None,
            },
            WM_DESTROY => {
                self.unwatch_windows();
                if let Some(tray) = self.tray.take() {
                    tray.remove();
                }
                unsafe { PostQuitMessage(0) };
            }
            m if m == self.taskbar_created && m != 0 => {
                if let Some(tray) = &self.tray {
                    tray.add();
                }
            }
            _ => return None,
        }
        Some(LRESULT(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updater::Asset;

    fn release(version: &str, installable: bool) -> Release {
        let asset = Asset { name: "a".into(), url: "u".into(), size: 1 };
        Release {
            tag: format!("v{version}"),
            version: version.into(),
            html_url: String::new(),
            installer: installable.then(|| asset.clone()),
            sums: Some(asset),
            notes: String::new(),
        }
    }

    #[test]
    fn newer_installable_release_is_available_and_notified_once() {
        let r = Ok(Some(release("0.3.0", true)));
        assert_eq!(decide_update(&r, "0.2.0", ""), UpdateDecision::Available { version: "0.3.0".into(), notify: true });
        assert_eq!(
            decide_update(&r, "0.2.0", "0.3.0"),
            UpdateDecision::Available { version: "0.3.0".into(), notify: false }
        );
    }

    #[test]
    fn same_older_missing_assets_or_no_release_is_up_to_date() {
        assert_eq!(decide_update(&Ok(Some(release("0.2.0", true))), "0.2.0", ""), UpdateDecision::UpToDate);
        assert_eq!(decide_update(&Ok(Some(release("0.1.4", true))), "0.2.0", ""), UpdateDecision::UpToDate);
        assert_eq!(decide_update(&Ok(Some(release("0.3.0", false))), "0.2.0", ""), UpdateDecision::UpToDate);
        assert_eq!(decide_update(&Ok(None), "0.2.0", ""), UpdateDecision::UpToDate);
        assert_eq!(decide_update(&Err("x".into()), "0.2.0", ""), UpdateDecision::Failed);
    }

    #[test]
    fn startup_notices_cover_recovery_and_upgrade() {
        let recovered = LoadReport { recovered: true, migrated: false };
        assert_eq!(startup_notices(LoadReport::default(), "", "0.2.0"), Vec::<String>::new());
        assert_eq!(startup_notices(LoadReport::default(), "0.2.0", "0.2.0"), Vec::<String>::new());
        assert_eq!(
            startup_notices(LoadReport::default(), "0.1.4", "0.2.0"),
            vec!["v0.2.0 버전으로 업데이트되었습니다.".to_string()]
        );
        assert_eq!(startup_notices(recovered, "", "0.2.0").len(), 1);
    }

    #[test]
    fn tile_views_map_kinds_labels_and_badges() {
        let items = vec![
            Item::Youtube { id: "y".into(), query: "로파이 음악".into(), name: "".into() },
            Item::Web { id: "w".into(), url: "https://github.com".into(), name: "".into() },
            Item::App { id: "a".into(), name: "계산기".into(), target: "Calc!App".into(), exe: None },
        ];
        let none = HashSet::new();
        let views = tile_views(&items, &TileStatus { icon_of: &|_| None, running: &none, missing: &none });
        assert_eq!(
            views[0],
            TileView {
                id: "y".into(),
                label: "로파이 음악".into(),
                badge: "로파".into(),
                kind: TileKind::Youtube,
                ..Default::default()
            }
        );
        assert_eq!(
            (views[1].label.as_str(), views[1].badge.as_str(), views[1].kind),
            ("github.com", "G", TileKind::Web)
        );
        assert_eq!((views[2].badge.as_str(), views[2].kind), ("계", TileKind::App));
    }

    #[test]
    fn tile_views_add_icons_running_and_missing_state() {
        let items = vec![
            Item::Youtube { id: "y".into(), query: "음악".into(), name: "".into() },
            Item::App { id: "a".into(), name: "계산기".into(), target: "Calc!App".into(), exe: None },
            Item::Web { id: "w".into(), url: "https://github.com".into(), name: "".into() },
        ];
        let running = HashSet::from(["a".to_string()]);
        let missing = HashSet::from(["w".to_string()]);
        let icon_of = |id: &str| Some(PathBuf::from(format!("{id}.png")));
        let views = tile_views(&items, &TileStatus { icon_of: &icon_of, running: &running, missing: &missing });
        // 유튜브는 직접 그리므로 아이콘 파일을 쓰지 않는다
        assert_eq!(views[0].icon, None);
        assert_eq!(views[1].icon, Some(PathBuf::from("a.png")));
        assert_eq!((views[1].running, views[1].missing), (true, false));
        assert_eq!((views[2].running, views[2].missing), (false, true));
    }

    #[test]
    fn dialog_input_round_trips_to_items() {
        let calc = AppChoice { name: "계산기".into(), target: "Calc!App".into(), exe: None };
        let app = item_from_input("1".into(), ItemInput::App { app: calc.clone(), name: "".into() });
        assert_eq!(app, Item::App { id: "1".into(), name: "계산기".into(), target: "Calc!App".into(), exe: None });
        let named = item_from_input("2".into(), ItemInput::App { app: calc, name: "계산".into() });
        assert_eq!(named.display_name(), "계산");
        let web = item_from_input("3".into(), ItemInput::Web { url: "https://naver.com".into(), name: "".into() });
        assert_eq!(input_of(&web), ItemInput::Web { url: "https://naver.com".into(), name: "".into() });
        let yt = Item::Youtube { id: "4".into(), query: "로파이".into(), name: "공부".into() };
        assert_eq!(item_from_input("4".into(), input_of(&yt)), yt);
        assert_eq!(item_from_input("1".into(), input_of(&app)), app);
    }

    #[test]
    fn icon_is_refetched_only_when_target_changes() {
        let app = |t: &str| Item::App { id: "a".into(), name: "x".into(), target: t.into(), exe: None };
        let web = |u: &str, n: &str| Item::Web { id: "w".into(), url: u.into(), name: n.into() };
        assert!(icon_source_changed(&app("A!App"), &app("B!App")));
        assert!(!icon_source_changed(&app("A!App"), &app("A!App")));
        assert!(!icon_source_changed(&web("https://a.com", ""), &web("https://a.com", "에이")));
        assert!(icon_source_changed(&web("https://a.com", ""), &web("https://b.com", "")));
    }

    #[test]
    fn app_keys_only_for_app_items() {
        let items = vec![
            Item::Youtube { id: "y".into(), query: "음악".into(), name: "".into() },
            Item::App { id: "a".into(), name: "x".into(), target: "T".into(), exe: Some(r"C:\x.exe".into()) },
        ];
        assert_eq!(app_keys(&items), vec![None, Some(AppKey { target: "T".into(), exe: Some(r"C:\x.exe".into()) })]);
    }

    #[test]
    fn update_tile_shows_version_then_progress() {
        let t = update_tile("0.3.0", false, None);
        assert_eq!((t.id.as_str(), t.kind, t.badge.as_str()), (UPDATE_TILE_ID, TileKind::Update, "v0.3.0"));
        assert!(t.label.contains("v0.3.0"));
        let busy = update_tile("0.3.0", true, None);
        assert_eq!(busy.badge, "받는 중");
        let half = update_tile("0.3.0", true, Some(42));
        assert_eq!(half.badge, "42%");
        assert!(half.label.ends_with("42%"));
        assert_eq!((percent(0, 0), percent(50, 200), percent(300, 200)), (None, Some(25), Some(100)));
        assert!(crate::items::new_id() != UPDATE_TILE_ID);
    }

    #[test]
    fn dropped_urls_and_shortcuts_become_web_items() {
        let web = input_from_drop(&Dropped::Url("https://naver.com".into()));
        assert_eq!(web, Some(ItemInput::Web { url: "https://naver.com".into(), name: String::new() }));
        // 없는 경로는 건너뛰고, 있는 파일·폴더는 파일 항목
        assert_eq!(input_from_drop(&Dropped::File("C:/없는/a.txt".into())), None);
        let dir = std::env::temp_dir();
        assert_eq!(
            input_from_drop(&Dropped::File(dir.clone())),
            Some(ItemInput::Path { path: dir.to_string_lossy().into_owned(), name: String::new() })
        );
        let ini = "[InternetShortcut]\r\nURL=https://github.com/ssenu\r\nIconIndex=0";
        assert_eq!(internet_shortcut_url(ini), Some("https://github.com/ssenu".into()));
        assert_eq!(internet_shortcut_url("[InternetShortcut]"), None);
    }

    #[test]
    fn duplicates_are_detected_by_target() {
        let web = |url: &str| Item::Web { id: new_id(), url: url.into(), name: String::new() };
        let app = |t: &str| Item::App { id: new_id(), name: String::new(), target: t.into(), exe: None };
        assert!(same_target(&web("https://a.com/"), &web("https://a.com")));
        assert!(!same_target(&web("https://a.com"), &web("https://b.com")));
        assert!(same_target(&app("C:/X/app.exe"), &app("c:/x/APP.exe")));
        assert!(!same_target(&app("C:/X/app.exe"), &web("C:/X/app.exe")));
        assert_eq!(drop_message(2, 0, 0), "2개 항목을 추가했습니다.");
        assert!(drop_message(0, 1, 1).contains("이미 있는 1개") && drop_message(0, 1, 1).contains("읽을 수 없는 1개"));
        let path = |p: &str| Item::Path { id: new_id(), path: p.into(), name: String::new() };
        assert!(same_target(&path("C:/A/b.txt"), &path("c:/a/B.TXT")));
    }

    #[test]
    fn update_question_lists_user_facing_changes() {
        let mut r = release("0.3.0", true);
        assert_eq!(update_question(&r), "v0.3.0 버전으로 업데이트할까요?\n\n런처가 잠시 꺼졌다 켜집니다.");
        r.notes = "- feat: 끌어다 놓아 추가\n- ci: 빌드\n- fix: 독이 사라지던 문제".into();
        let q = update_question(&r);
        assert!(q.contains("바뀐 점\n • 끌어다 놓아 추가\n • 독이 사라지던 문제\n"));
        assert!(!q.contains("빌드"));
    }

    #[test]
    fn apply_order_follows_ids_and_keeps_unknown() {
        let a = Item::youtube("a", "").unwrap();
        let b = Item::youtube("b", "").unwrap();
        let c = Item::youtube("c", "").unwrap();
        let items = vec![a.clone(), b.clone(), c.clone()];
        let ids = vec![c.id().to_string(), a.id().to_string(), "gone".to_string()];
        assert_eq!(apply_order(&items, &ids), vec![c, a, b]);
    }

    #[test]
    fn debounce_blocks_repeat_within_one_second() {
        let mut d = Debounce::default();
        let t0 = Instant::now();
        assert!(d.allow("x", t0));
        assert!(!d.allow("x", t0 + Duration::from_millis(500)));
        assert!(d.allow("y", t0 + Duration::from_millis(500)));
        assert!(d.allow("x", t0 + Duration::from_millis(1200)));
    }
}
