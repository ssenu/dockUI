//! 설정 파일(`%APPDATA%\YT-Launcher\config.json`) 읽기·쓰기와 0.1.x(schema 1) 변환.

use crate::items::Item;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const SCHEMA: u32 = 2;
pub const CONFIG_NAME: &str = "config.json";
pub const BACKUP_NAME: &str = "config.json.bak";
pub const V1_BACKUP_NAME: &str = "config.v1.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Dock,
    Panel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DockEdge {
    #[default]
    Bottom,
    Top,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    Sepia,
    #[default]
    Glass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub schema: u32,
    pub items: Vec<Item>,
    #[serde(deserialize_with = "lenient")]
    pub mode: Mode,
    #[serde(deserialize_with = "lenient")]
    pub dock_edge: DockEdge,
    pub panel_window: Option<Rect>,
    #[serde(deserialize_with = "lenient")]
    pub theme: Theme,
    pub locked: bool,
    pub autostart: Option<bool>,
    pub last_run_version: String,
    pub notified_version: String,
    pub chrome_notice_shown: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            schema: SCHEMA,
            items: Vec::new(),
            mode: Mode::Dock,
            dock_edge: DockEdge::Bottom,
            panel_window: None,
            theme: Theme::Glass,
            locked: false,
            autostart: None,
            last_run_version: String::new(),
            notified_version: String::new(),
            chrome_notice_shown: false,
        }
    }
}

/// 알 수 없는 enum 값은 오류 대신 기본값으로 읽는다.
fn lenient<'de, D, T>(de: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    let value = Value::deserialize(de)?;
    Ok(T::deserialize(value).unwrap_or_default())
}

/// 불러오기 결과에 덧붙는 정보 (알림을 띄울지 결정)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LoadReport {
    /// 파일이 깨져서 .bak으로 옮기고 기본값으로 시작함
    pub recovered: bool,
    /// 0.1.x 설정을 변환함
    pub migrated: bool,
}

pub fn default_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("YT-Launcher")
}

pub struct Storage {
    pub dir: PathBuf,
}

impl Storage {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Storage { dir: dir.into() }
    }

    pub fn path(&self) -> PathBuf {
        self.dir.join(CONFIG_NAME)
    }

    pub fn load(&self) -> (Config, LoadReport) {
        let path = self.path();
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return (Config::default(), LoadReport::default()),
            Err(_) => return self.recover(&path),
        };
        let parsed: Option<Value> = std::str::from_utf8(&bytes)
            .ok()
            .map(|s| s.trim_start_matches('\u{feff}'))
            .and_then(|s| serde_json::from_str(s).ok());
        let Some(value) = parsed else { return self.recover(&path) };
        if value.get("schema").and_then(Value::as_u64) == Some(1) {
            return match migrate_v1(&value) {
                Some(config) => {
                    let _ = fs::copy(&path, self.dir.join(V1_BACKUP_NAME));
                    let _ = self.save(&config);
                    (config, LoadReport { recovered: false, migrated: true })
                }
                None => self.recover(&path),
            };
        }
        match serde_json::from_value::<Config>(value) {
            Ok(mut config) => {
                config.schema = SCHEMA;
                (config, LoadReport::default())
            }
            Err(_) => self.recover(&path),
        }
    }

    fn recover(&self, path: &Path) -> (Config, LoadReport) {
        let _ = fs::rename(path, self.dir.join(BACKUP_NAME));
        (Config::default(), LoadReport { recovered: true, migrated: false })
    }

    /// 임시 파일에 다 쓴 뒤 교체한다 (저장 중 꺼져도 파일이 깨지지 않음).
    pub fn save(&self, config: &Config) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let tmp = self.dir.join("config.json.tmp");
        let json = serde_json::to_string_pretty(config).map_err(io::Error::other)?;
        fs::write(&tmp, json)?;
        fs::rename(&tmp, self.path())
    }
}

/// 0.1.x 설정 → schema 2. 타일은 유튜브 항목으로, 창 위치는 패널 위치로, 모드는 패널.
pub fn migrate_v1(v1: &Value) -> Option<Config> {
    let tiles = v1.get("tiles")?.as_array()?;
    let mut items = Vec::with_capacity(tiles.len());
    for tile in tiles {
        let query = tile.get("query")?.as_str()?.trim().to_string();
        if query.is_empty() {
            continue;
        }
        let id = tile
            .get("id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(crate::items::new_id);
        let name = tile.get("name").and_then(Value::as_str).unwrap_or("").to_string();
        items.push(Item::Youtube { id, query, name });
    }
    let panel_window = v1.get("window").and_then(|w| {
        Some(Rect {
            x: i32::try_from(w.get("x")?.as_i64()?).ok()?,
            y: i32::try_from(w.get("y")?.as_i64()?).ok()?,
            w: i32::try_from(w.get("w")?.as_i64()?).ok()?,
            h: i32::try_from(w.get("h")?.as_i64()?).ok()?,
        })
    });
    let theme = v1.get("theme").and_then(|t| serde_json::from_value::<Theme>(t.clone()).ok()).unwrap_or(Theme::Dark);
    let text = |key: &str| v1.get(key).and_then(Value::as_str).unwrap_or("").to_string();
    Some(Config {
        schema: SCHEMA,
        items,
        mode: Mode::Panel,
        dock_edge: DockEdge::Bottom,
        panel_window,
        theme,
        locked: v1.get("locked").and_then(Value::as_bool).unwrap_or(false),
        autostart: v1.get("autostart").and_then(Value::as_bool),
        last_run_version: text("last_run_version"),
        notified_version: text("notified_version"),
        chrome_notice_shown: v1.get("chrome_notice_shown").and_then(Value::as_bool).unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ytl-test-{name}-{}", crate::items::new_id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_file_gives_defaults() {
        let storage = Storage::new(temp_dir("missing"));
        let (config, report) = storage.load();
        assert_eq!(config, Config::default());
        assert_eq!(config.mode, Mode::Dock);
        assert_eq!(config.theme, Theme::Glass);
        assert_eq!(report, LoadReport::default());
    }

    #[test]
    fn roundtrip() {
        let storage = Storage::new(temp_dir("roundtrip"));
        let config = Config {
            items: vec![Item::youtube("로파이", "").unwrap(), Item::web("github.com", "").unwrap()],
            mode: Mode::Panel,
            dock_edge: DockEdge::Left,
            panel_window: Some(Rect { x: 1, y: 2, w: 300, h: 400 }),
            theme: Theme::Sepia,
            locked: true,
            autostart: Some(false),
            last_run_version: "0.2.0".into(),
            ..Config::default()
        };
        storage.save(&config).unwrap();
        let (loaded, report) = storage.load();
        assert_eq!(loaded, config);
        assert_eq!(report, LoadReport::default());
        assert!(!storage.dir.join("config.json.tmp").exists());
    }

    #[test]
    fn migrates_real_v1_config_and_backs_it_up() {
        let dir = temp_dir("v1");
        let v1 = include_str!("../testdata/config_v1.json");
        fs::write(dir.join(CONFIG_NAME), v1).unwrap();
        let storage = Storage::new(&dir);
        let (config, report) = storage.load();
        assert!(report.migrated);
        assert_eq!(config.schema, 2);
        assert_eq!(config.mode, Mode::Panel);
        assert_eq!(config.theme, Theme::Sepia);
        assert!(config.locked);
        assert_eq!(config.autostart, Some(false));
        assert_eq!(config.last_run_version, "0.1.4");
        assert!(config.chrome_notice_shown);
        assert_eq!(config.panel_window, Some(Rect { x: 1544, y: 16, w: 360, h: 480 }));
        assert_eq!(
            config.items,
            vec![
                Item::Youtube {
                    id: "0f3c9a8e2b7d4c1e9a6b5d4c3b2a1f0e".into(),
                    query: "로파이 음악".into(),
                    name: "".into()
                },
                Item::Youtube {
                    id: "1a2b3c4d5e6f708192a3b4c5d6e7f809".into(),
                    query: "뉴스".into(),
                    name: "아침 뉴스".into()
                },
            ]
        );
        assert_eq!(fs::read_to_string(dir.join(V1_BACKUP_NAME)).unwrap(), v1);
        // 변환 결과가 schema 2로 저장되어 다음 실행에는 변환하지 않는다
        let (again, report2) = storage.load();
        assert_eq!(again, config);
        assert!(!report2.migrated);
    }

    #[test]
    fn corrupt_file_is_backed_up() {
        let dir = temp_dir("corrupt");
        fs::write(dir.join(CONFIG_NAME), b"{not json").unwrap();
        let (config, report) = Storage::new(&dir).load();
        assert_eq!(config, Config::default());
        assert!(report.recovered);
        assert_eq!(fs::read(dir.join(BACKUP_NAME)).unwrap(), b"{not json");
    }

    #[test]
    fn non_utf8_file_is_backed_up() {
        let dir = temp_dir("utf8");
        fs::write(dir.join(CONFIG_NAME), b"\xff\xfe\x00bad").unwrap();
        let (_, report) = Storage::new(&dir).load();
        assert!(report.recovered);
    }

    #[test]
    fn unknown_enum_values_fall_back_to_defaults() {
        let dir = temp_dir("enums");
        fs::write(dir.join(CONFIG_NAME), r#"{"schema":2,"mode":"floating","theme":"neon","dock_edge":"middle"}"#)
            .unwrap();
        let (config, report) = Storage::new(&dir).load();
        assert!(!report.recovered);
        assert_eq!((config.mode, config.theme, config.dock_edge), (Mode::Dock, Theme::Glass, DockEdge::Bottom));
    }

    #[test]
    fn utf8_bom_is_accepted() {
        let dir = temp_dir("bom");
        fs::write(dir.join(CONFIG_NAME), "\u{feff}{\"schema\":2,\"locked\":true}").unwrap();
        let (config, report) = Storage::new(&dir).load();
        assert!(!report.recovered);
        assert!(config.locked);
    }
}
