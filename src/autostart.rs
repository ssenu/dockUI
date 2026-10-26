//! 윈도우 시작 시 자동 실행 (HKCU Run 키의 `YTLauncher` 값). 설치 프로그램과 같은 값을 쓴다.

use crate::win::reg::{self, HKEY_CURRENT_USER};

pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
pub const VALUE_NAME: &str = "YTLauncher";

pub trait RunBackend {
    fn get(&self) -> Option<String>;
    fn set(&self, value: &str) -> std::io::Result<()>;
    fn delete(&self);
}

pub struct RegistryRun {
    pub value_name: String,
}

impl Default for RegistryRun {
    fn default() -> Self {
        RegistryRun { value_name: VALUE_NAME.to_string() }
    }
}

impl RunBackend for RegistryRun {
    fn get(&self) -> Option<String> {
        reg::get_string(HKEY_CURRENT_USER, RUN_KEY, &self.value_name)
    }
    fn set(&self, value: &str) -> std::io::Result<()> {
        reg::set_string(HKEY_CURRENT_USER, RUN_KEY, &self.value_name, value).map_err(std::io::Error::other)
    }
    fn delete(&self) {
        reg::delete_value(HKEY_CURRENT_USER, RUN_KEY, &self.value_name);
    }
}

/// Run 키에 넣을 명령: 따옴표로 감싼 현재 exe 경로 (설치 프로그램이 쓰는 형식과 같다)
pub fn launch_command() -> String {
    let exe = std::env::current_exe().unwrap_or_default();
    format!("\"{}\"", exe.display())
}

pub struct AutoStart<B: RunBackend> {
    pub backend: B,
}

impl<B: RunBackend> AutoStart<B> {
    pub fn is_enabled(&self) -> bool {
        self.backend.get().is_some()
    }

    pub fn set_enabled(&self, enabled: bool) -> std::io::Result<()> {
        if enabled {
            self.backend.set(&launch_command())
        } else {
            self.backend.delete();
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct Fake(RefCell<Option<String>>);
    impl RunBackend for Fake {
        fn get(&self) -> Option<String> {
            self.0.borrow().clone()
        }
        fn set(&self, value: &str) -> std::io::Result<()> {
            *self.0.borrow_mut() = Some(value.to_string());
            Ok(())
        }
        fn delete(&self) {
            *self.0.borrow_mut() = None;
        }
    }

    #[test]
    fn enable_writes_quoted_exe_and_disable_removes() {
        let auto = AutoStart { backend: Fake::default() };
        assert!(!auto.is_enabled());
        auto.set_enabled(true).unwrap();
        let value = auto.backend.get().unwrap();
        assert!(value.starts_with('"') && value.ends_with(".exe\""), "{value}");
        auto.set_enabled(false).unwrap();
        assert!(!auto.is_enabled());
    }

    #[test]
    fn real_run_key_roundtrip() {
        let backend = RegistryRun { value_name: format!("YTLauncherTest{}", crate::items::new_id()) };
        backend.set("\"C:\\nonexistent\\yt-launcher-test.exe\"").unwrap();
        assert_eq!(backend.get().as_deref(), Some("\"C:\\nonexistent\\yt-launcher-test.exe\""));
        backend.delete();
        assert_eq!(backend.get(), None);
    }
}
