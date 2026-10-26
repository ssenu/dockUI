//! `%APPDATA%\YT-Launcher\logs\launcher.log` 파일 로그. 1MB가 넘으면 `.1`~`.3`으로 돌린다.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const MAX_BYTES: u64 = 1_000_000;
const BACKUPS: u32 = 3;

static LOG_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn init(dir: &Path) {
    let _ = fs::create_dir_all(dir);
    *LOG_PATH.lock().unwrap_or_else(|e| e.into_inner()) = Some(dir.join("launcher.log"));
    std::panic::set_hook(Box::new(|info| error(&format!("패닉: {info}"))));
}

fn rotate(path: &Path) {
    if fs::metadata(path).map(|m| m.len()).unwrap_or(0) < MAX_BYTES {
        return;
    }
    for i in (1..BACKUPS).rev() {
        let _ = fs::rename(path.with_extension(format!("log.{i}")), path.with_extension(format!("log.{}", i + 1)));
    }
    let _ = fs::rename(path, path.with_extension("log.1"));
}

fn timestamp() -> String {
    let st = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond)
}

fn write(level: &str, message: &str) {
    let guard = LOG_PATH.lock().unwrap_or_else(|e| e.into_inner());
    let Some(path) = guard.as_ref() else { return };
    rotate(path);
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{} {level} {message}", timestamp());
    }
}

pub fn info(message: &str) {
    write("INFO", message);
}

pub fn warn(message: &str) {
    write("WARN", message);
}

pub fn error(message: &str) {
    write("ERROR", message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_keeps_three_backups() {
        let dir = std::env::temp_dir().join(format!("ytl-log-{}", crate::items::new_id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("launcher.log");
        for round in 0..5 {
            fs::write(&path, vec![b'x'; MAX_BYTES as usize + round]).unwrap();
            rotate(&path);
            assert!(!path.exists());
        }
        assert!(dir.join("launcher.log.1").exists());
        assert!(dir.join("launcher.log.3").exists());
        assert!(!dir.join("launcher.log.4").exists());
    }
}
