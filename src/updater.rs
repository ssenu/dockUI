//! GitHub Releases 기반 업데이트: 최신 릴리스 확인 → 설치 파일 다운로드·SHA-256 검증 → 조용한 설치 실행.
//!
//! 0.1.x(파이썬) 업데이터와 같은 규칙을 지킨다: 정식 릴리스, `YT-Launcher-Setup-<버전>.exe`,
//! `SHA256SUMS.txt`(`<소문자 해시>  <파일명>`), `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART`.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const REPO: &str = "ssenu/YT-launcher";
pub const SUMS_NAME: &str = "SHA256SUMS.txt";
pub const INSTALLER_PREFIX: &str = "YT-Launcher-Setup-";
pub const SILENT_ARGS: [&str; 3] = ["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"];
const CHUNK: usize = 64 * 1024;
const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub tag: String,
    pub version: String,
    pub html_url: String,
    pub installer: Option<Asset>,
    pub sums: Option<Asset>,
}

impl Release {
    /// 설치 파일과 해시 파일이 둘 다 있어야 업데이트 배지를 띄운다.
    pub fn is_installable(&self) -> bool {
        self.installer.is_some() && self.sums.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateError {
    Network(String),
    BadResponse(String),
    MissingAssets,
    HashMismatch,
    Io(String),
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateError::Network(e) => write!(f, "네트워크 오류: {e}"),
            UpdateError::BadResponse(e) => write!(f, "릴리스 정보를 읽지 못했습니다: {e}"),
            UpdateError::MissingAssets => write!(f, "릴리스에 설치 파일 또는 SHA256SUMS.txt가 없습니다."),
            UpdateError::HashMismatch => write!(f, "설치 파일 해시가 일치하지 않습니다."),
            UpdateError::Io(e) => write!(f, "파일 오류: {e}"),
        }
    }
}

/// HTTP GET 실패 종류. 404는 "릴리스 없음"으로 따로 처리한다.
#[derive(Debug)]
pub enum HttpFailure {
    Status(u16),
    Transport(String),
}

pub trait Http: Send + Sync {
    fn get(&self, url: &str, accept: &str) -> Result<Box<dyn Read + Send>, HttpFailure>;
}

/// 실제 네트워크용. Windows 기본 TLS(schannel)를 쓴다.
pub struct UreqHttp {
    agent: ureq::Agent,
}

impl UreqHttp {
    pub fn new() -> Self {
        let tls = ureq::tls::TlsConfig::builder().provider(ureq::tls::TlsProvider::NativeTls).build();
        let agent = ureq::Agent::config_builder()
            .tls_config(tls)
            .timeout_global(Some(TIMEOUT))
            .user_agent(format!("YT-Launcher/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .into();
        UreqHttp { agent }
    }
}

impl Default for UreqHttp {
    fn default() -> Self {
        Self::new()
    }
}

impl Http for UreqHttp {
    fn get(&self, url: &str, accept: &str) -> Result<Box<dyn Read + Send>, HttpFailure> {
        match self.agent.get(url).header("Accept", accept).call() {
            Ok(response) => Ok(Box::new(response.into_body().into_reader())),
            Err(ureq::Error::StatusCode(code)) => Err(HttpFailure::Status(code)),
            Err(e) => Err(HttpFailure::Transport(e.to_string())),
        }
    }
}

pub fn api_url(repo: &str) -> String {
    format!("https://api.github.com/repos/{repo}/releases/latest")
}

fn is_installer_name(name: &str) -> bool {
    name.strip_prefix(INSTALLER_PREFIX).and_then(|rest| rest.strip_suffix(".exe")).is_some_and(|ver| {
        !ver.is_empty() && ver.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    })
}

pub fn parse_release(data: &Value) -> Option<Release> {
    if data.get("draft").and_then(Value::as_bool).unwrap_or(false)
        || data.get("prerelease").and_then(Value::as_bool).unwrap_or(false)
    {
        return None;
    }
    let tag = data.get("tag_name")?.as_str()?.to_string();
    if tag.is_empty() {
        return None;
    }
    let mut installer = None;
    let mut sums = None;
    for a in data.get("assets").and_then(Value::as_array).into_iter().flatten() {
        let (Some(name), Some(url)) =
            (a.get("name").and_then(Value::as_str), a.get("browser_download_url").and_then(Value::as_str))
        else {
            continue;
        };
        let asset = Asset {
            name: name.to_string(),
            url: url.to_string(),
            size: a.get("size").and_then(Value::as_u64).unwrap_or(0),
        };
        if installer.is_none() && is_installer_name(name) {
            installer = Some(asset);
        } else if name == SUMS_NAME {
            sums = Some(asset);
        }
    }
    Some(Release {
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        tag,
        html_url: data.get("html_url").and_then(Value::as_str).unwrap_or("").to_string(),
        installer,
        sums,
    })
}

fn version_parts(v: &str) -> Option<Vec<u64>> {
    v.trim_start_matches(['v', 'V']).split('.').map(|p| p.parse::<u64>().ok()).collect()
}

/// 숫자 단위로 비교한다 ("0.10.0" > "0.9.0"). 형식이 잘못되면 false.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (version_parts(candidate), version_parts(current)) {
        (Some(mut a), Some(mut b)) => {
            let len = a.len().max(b.len());
            a.resize(len, 0);
            b.resize(len, 0);
            a > b
        }
        _ => false,
    }
}

/// 최신 정식 릴리스. 릴리스가 아직 없으면(404) `Ok(None)`.
pub fn fetch_latest(http: &dyn Http, repo: &str) -> Result<Option<Release>, UpdateError> {
    let mut body = match http.get(&api_url(repo), "application/vnd.github+json") {
        Ok(body) => body,
        Err(HttpFailure::Status(404)) => return Ok(None),
        Err(HttpFailure::Status(code)) => return Err(UpdateError::Network(format!("HTTP {code}"))),
        Err(HttpFailure::Transport(e)) => return Err(UpdateError::Network(e)),
    };
    let mut text = String::new();
    body.read_to_string(&mut text).map_err(|e| UpdateError::Network(e.to_string()))?;
    let value: Value = serde_json::from_str(&text).map_err(|e| UpdateError::BadResponse(e.to_string()))?;
    Ok(parse_release(&value))
}

/// sha256sum 형식(`<hash>  <name>` 또는 `<hash> *<name>`) → {파일명: 소문자 해시}
pub fn parse_sums(text: &str) -> HashMap<String, String> {
    text.trim_start_matches('\u{feff}')
        .lines()
        .filter_map(|line| {
            let (hash, name) = line.trim().split_once(char::is_whitespace)?;
            Some((name.trim().trim_start_matches('*').to_string(), hash.to_ascii_lowercase()))
        })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

/// `.part`로 받으면서 해시를 계산하고, (요청 시) 해시가 맞을 때만 최종 이름으로 바꾼다.
/// 실패하면 `.part`는 항상 지운다.
pub fn download(
    http: &dyn Http,
    asset: &Asset,
    dest: &Path,
    expected_sha256: Option<&str>,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<PathBuf, UpdateError> {
    if let Some(dir) = dest.parent() {
        fs::create_dir_all(dir).map_err(|e| UpdateError::Io(e.to_string()))?;
    }
    let partial =
        dest.with_file_name(format!("{}.part", dest.file_name().and_then(|n| n.to_str()).unwrap_or("download")));
    let result = (|| {
        let mut body = http.get(&asset.url, "application/octet-stream").map_err(|e| match e {
            HttpFailure::Status(code) => UpdateError::Network(format!("HTTP {code}")),
            HttpFailure::Transport(t) => UpdateError::Network(t),
        })?;
        let mut out = File::create(&partial).map_err(|e| UpdateError::Io(e.to_string()))?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; CHUNK];
        let mut done = 0u64;
        loop {
            let n = body.read(&mut buf).map_err(|e| UpdateError::Network(e.to_string()))?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n]).map_err(|e| UpdateError::Io(e.to_string()))?;
            hasher.update(&buf[..n]);
            done += n as u64;
            progress(done, asset.size);
        }
        drop(out);
        if let Some(expected) = expected_sha256
            && hex(&hasher.finalize()) != expected.to_ascii_lowercase()
        {
            return Err(UpdateError::HashMismatch);
        }
        fs::rename(&partial, dest).map_err(|e| UpdateError::Io(e.to_string()))?;
        Ok(dest.to_path_buf())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&partial);
    }
    result
}

pub fn download_and_verify(
    http: &dyn Http,
    release: &Release,
    dir: &Path,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<PathBuf, UpdateError> {
    let (Some(installer), Some(sums)) = (&release.installer, &release.sums) else {
        return Err(UpdateError::MissingAssets);
    };
    let sums_path = download(http, sums, &dir.join(SUMS_NAME), None, &mut |_, _| {})?;
    let bytes = fs::read(&sums_path).map_err(|e| UpdateError::Io(e.to_string()))?;
    let text = String::from_utf8(bytes).map_err(|_| UpdateError::BadResponse("SHA256SUMS 인코딩".into()))?;
    let expected = parse_sums(&text).remove(&installer.name).ok_or(UpdateError::MissingAssets)?;
    let dest = dir.join(&installer.name);
    let _ = fs::remove_file(&dest);
    download(http, installer, &dest, Some(&expected), progress)
}

/// 설치 프로그램을 조용한 모드로 분리 실행한다. 호출한 뒤 앱은 곧바로 종료해야 한다.
pub fn launch_installer(path: &Path) -> io::Result<()> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    std::process::Command::new(path)
        .args(SILENT_ARGS)
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .spawn()
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    const INSTALLER: &[u8] = b"installer-bytes";
    const INSTALLER_SHA: &str = "204676736cea68d6411da9d3aa3fab0a5e70b023ba30cd560cfa9c8e7250f4df";

    enum Reply {
        Body(Vec<u8>),
        Status(u16),
        Fail,
        BreakAfter(Vec<u8>),
    }

    struct FakeHttp {
        replies: HashMap<String, Reply>,
        requested: Mutex<Vec<String>>,
    }

    impl FakeHttp {
        fn new(replies: Vec<(&str, Reply)>) -> Self {
            FakeHttp {
                replies: replies.into_iter().map(|(u, r)| (u.to_string(), r)).collect(),
                requested: Mutex::new(Vec::new()),
            }
        }
    }

    struct Broken(Vec<u8>, bool);
    impl Read for Broken {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.1 {
                return Err(io::Error::other("connection reset"));
            }
            self.1 = true;
            let n = self.0.len().min(buf.len());
            buf[..n].copy_from_slice(&self.0[..n]);
            Ok(n)
        }
    }

    impl Http for FakeHttp {
        fn get(&self, url: &str, _accept: &str) -> Result<Box<dyn Read + Send>, HttpFailure> {
            self.requested.lock().unwrap().push(url.to_string());
            match self.replies.get(url) {
                Some(Reply::Body(b)) => Ok(Box::new(io::Cursor::new(b.clone()))),
                Some(Reply::Status(c)) => Err(HttpFailure::Status(*c)),
                Some(Reply::BreakAfter(b)) => Ok(Box::new(Broken(b.clone(), false))),
                Some(Reply::Fail) | None => Err(HttpFailure::Transport("offline".into())),
            }
        }
    }

    fn release_json() -> Value {
        json!({
            "tag_name": "v0.2.0",
            "html_url": "https://github.com/ssenu/YT-launcher/releases/tag/v0.2.0",
            "draft": false, "prerelease": false,
            "assets": [
                {"name": "YT-Launcher-Setup-0.2.0.exe", "browser_download_url": "https://dl/setup.exe", "size": INSTALLER.len()},
                {"name": "SHA256SUMS.txt", "browser_download_url": "https://dl/sums.txt", "size": 100},
                {"name": "source.zip", "browser_download_url": "https://dl/src.zip", "size": 5}
            ]
        })
    }

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ytl-upd-{}", crate::items::new_id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parses_release_and_assets() {
        let r = parse_release(&release_json()).unwrap();
        assert_eq!(r.version, "0.2.0");
        assert_eq!(r.installer.as_ref().unwrap().name, "YT-Launcher-Setup-0.2.0.exe");
        assert_eq!(r.sums.as_ref().unwrap().url, "https://dl/sums.txt");
        assert!(r.is_installable());
    }

    #[test]
    fn skips_draft_prerelease_and_empty_tag() {
        let mut v = release_json();
        v["draft"] = json!(true);
        assert!(parse_release(&v).is_none());
        let mut v = release_json();
        v["prerelease"] = json!(true);
        assert!(parse_release(&v).is_none());
        let mut v = release_json();
        v["tag_name"] = json!("");
        assert!(parse_release(&v).is_none());
    }

    #[test]
    fn installer_name_rule_matches_v1_updater() {
        assert!(is_installer_name("YT-Launcher-Setup-0.2.0.exe"));
        assert!(is_installer_name("YT-Launcher-Setup-1.0.0-rc_1.exe"));
        assert!(!is_installer_name("YT-Launcher-Setup-.exe"));
        assert!(!is_installer_name("YT-Launcher-Setup-0.2.0.exe.sig"));
        assert!(!is_installer_name("YT-Launcher-Setup-../x.exe"));
    }

    #[test]
    fn compares_versions_numerically() {
        assert!(is_newer("v0.2.0", "0.1.4"));
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(!is_newer("0.1.4", "0.1.4"));
        assert!(!is_newer("0.1.4", "0.2.0"));
        assert!(is_newer("0.2.1", "0.2"));
        assert!(!is_newer("garbage", "0.1.0"));
    }

    #[test]
    fn fetch_latest_handles_ok_404_and_errors() {
        let url = api_url(REPO);
        let ok = FakeHttp::new(vec![(&url, Reply::Body(release_json().to_string().into_bytes()))]);
        assert_eq!(fetch_latest(&ok, REPO).unwrap().unwrap().version, "0.2.0");
        let none = FakeHttp::new(vec![(&url, Reply::Status(404))]);
        assert_eq!(fetch_latest(&none, REPO).unwrap(), None);
        let down = FakeHttp::new(vec![(&url, Reply::Fail)]);
        assert!(matches!(fetch_latest(&down, REPO), Err(UpdateError::Network(_))));
        let bad = FakeHttp::new(vec![(&url, Reply::Body(b"<html>".to_vec()))]);
        assert!(matches!(fetch_latest(&bad, REPO), Err(UpdateError::BadResponse(_))));
    }

    #[test]
    fn parses_sha256sums() {
        let text = format!("\u{feff}{}  YT-Launcher-Setup-0.2.0.exe\nabc *other.exe\n\n", INSTALLER_SHA.to_uppercase());
        let sums = parse_sums(&text);
        assert_eq!(sums["YT-Launcher-Setup-0.2.0.exe"], INSTALLER_SHA);
        assert_eq!(sums["other.exe"], "abc");
    }

    #[test]
    fn download_and_verify_success() {
        let dir = temp_dir();
        let http = FakeHttp::new(vec![
            (
                "https://dl/sums.txt",
                Reply::Body(format!("{INSTALLER_SHA}  YT-Launcher-Setup-0.2.0.exe\n").into_bytes()),
            ),
            ("https://dl/setup.exe", Reply::Body(INSTALLER.to_vec())),
        ]);
        let release = parse_release(&release_json()).unwrap();
        let mut last = (0, 0);
        let path = download_and_verify(&http, &release, &dir, &mut |d, t| last = (d, t)).unwrap();
        assert_eq!(path, dir.join("YT-Launcher-Setup-0.2.0.exe"));
        assert_eq!(fs::read(&path).unwrap(), INSTALLER);
        assert_eq!(sha256_file(&path).unwrap(), INSTALLER_SHA);
        assert_eq!(last, (INSTALLER.len() as u64, INSTALLER.len() as u64));
    }

    #[test]
    fn hash_mismatch_leaves_no_installer_or_part() {
        let dir = temp_dir();
        fs::write(dir.join("YT-Launcher-Setup-0.2.0.exe"), b"stale").unwrap();
        let http = FakeHttp::new(vec![
            (
                "https://dl/sums.txt",
                Reply::Body(format!("{}  YT-Launcher-Setup-0.2.0.exe\n", "0".repeat(64)).into_bytes()),
            ),
            ("https://dl/setup.exe", Reply::Body(INSTALLER.to_vec())),
        ]);
        let release = parse_release(&release_json()).unwrap();
        assert_eq!(download_and_verify(&http, &release, &dir, &mut |_, _| {}), Err(UpdateError::HashMismatch));
        assert!(!dir.join("YT-Launcher-Setup-0.2.0.exe").exists());
        assert!(!dir.join("YT-Launcher-Setup-0.2.0.exe.part").exists());
    }

    #[test]
    fn broken_download_cleans_part() {
        let dir = temp_dir();
        let asset = Asset { name: "x.exe".into(), url: "https://dl/x".into(), size: 10 };
        let http = FakeHttp::new(vec![("https://dl/x", Reply::BreakAfter(b"half".to_vec()))]);
        assert!(matches!(
            download(&http, &asset, &dir.join("x.exe"), None, &mut |_, _| {}),
            Err(UpdateError::Network(_))
        ));
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
    }

    #[test]
    fn missing_assets_or_sums_entry_is_an_error() {
        let dir = temp_dir();
        let mut release = parse_release(&release_json()).unwrap();
        release.sums = None;
        let http = FakeHttp::new(vec![]);
        assert_eq!(download_and_verify(&http, &release, &dir, &mut |_, _| {}), Err(UpdateError::MissingAssets));

        let release = parse_release(&release_json()).unwrap();
        let http = FakeHttp::new(vec![("https://dl/sums.txt", Reply::Body(b"abc  other.exe\n".to_vec()))]);
        assert_eq!(download_and_verify(&http, &release, &dir, &mut |_, _| {}), Err(UpdateError::MissingAssets));
        assert_eq!(*http.requested.lock().unwrap(), vec!["https://dl/sums.txt".to_string()]);
    }
}
