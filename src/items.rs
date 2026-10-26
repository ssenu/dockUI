//! 독에 놓이는 항목(유튜브 / 앱 / 웹 주소 / 파일·폴더)과 입력 검증.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Item {
    Youtube {
        id: String,
        query: String,
        #[serde(default)]
        name: String,
    },
    App {
        id: String,
        name: String,
        /// shell:AppsFolder 파싱 이름(Store 앱은 AUMID) 또는 직접 고른 exe/lnk 경로
        target: String,
        /// 실행 중 판정에 쓰는 실제 exe 경로(알 수 있을 때만)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exe: Option<String>,
    },
    Web {
        id: String,
        url: String,
        #[serde(default)]
        name: String,
    },
    /// 파일이나 폴더 (셸로 연다: 문서는 연결된 프로그램, 폴더는 탐색기)
    Path {
        id: String,
        path: String,
        #[serde(default)]
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemError {
    #[cfg(test)]
    EmptyQuery,
    InvalidUrl,
}

impl Item {
    pub fn id(&self) -> &str {
        match self {
            Item::Youtube { id, .. } | Item::App { id, .. } | Item::Web { id, .. } | Item::Path { id, .. } => id,
        }
    }

    /// 말풍선에 보이는 이름. 비어 있으면 검색어나 호스트명을 쓴다.
    pub fn display_name(&self) -> String {
        match self {
            Item::Youtube { query, name, .. } => non_empty(name).unwrap_or(query).to_string(),
            Item::App { name, .. } => name.clone(),
            Item::Web { url, name, .. } => {
                non_empty(name).map(str::to_string).unwrap_or_else(|| host_of(url).unwrap_or(url).to_string())
            }
            Item::Path { path, name, .. } => non_empty(name).map(str::to_string).unwrap_or_else(|| path_name(path)),
        }
    }

    /// 테스트용 생성 도우미 (앱에서는 입력 창이 값을 검사한 뒤 `app::item_from_input`으로 만든다)
    #[cfg(test)]
    pub fn youtube(query: &str, name: &str) -> Result<Item, ItemError> {
        let query = query.trim();
        if query.is_empty() {
            return Err(ItemError::EmptyQuery);
        }
        Ok(Item::Youtube { id: new_id(), query: query.to_string(), name: name.trim().to_string() })
    }

    #[cfg(test)]
    pub fn web(url: &str, name: &str) -> Result<Item, ItemError> {
        Ok(Item::Web { id: new_id(), url: normalize_url(url)?, name: name.trim().to_string() })
    }
}

/// 경로의 마지막 이름 (`C:\\` 같은 드라이브는 그대로)
pub fn path_name(path: &str) -> String {
    let trimmed = path.trim_end_matches(['\\', '/']);
    std::path::Path::new(trimmed)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

fn non_empty(s: &str) -> Option<&str> {
    let t = s.trim();
    (!t.is_empty()).then_some(t)
}

/// `naver.com` → `https://naver.com`. http/https가 아니거나 호스트가 없으면 거부.
pub fn normalize_url(input: &str) -> Result<String, ItemError> {
    let input = input.trim();
    if input.is_empty() || input.chars().any(char::is_whitespace) {
        return Err(ItemError::InvalidUrl);
    }
    let url = if input.contains("://") { input.to_string() } else { format!("https://{input}") };
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err(ItemError::InvalidUrl);
    }
    let host_ok = host_of(&url)
        .is_some_and(|h| !h.starts_with('.') && h.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-'));
    let port_ok = port_of(&url).is_none_or(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()));
    if host_ok && port_ok { Ok(url) } else { Err(ItemError::InvalidUrl) }
}

fn authority_of(url: &str) -> Option<&str> {
    let rest = url.split_once("://")?.1;
    let authority = rest.split(['/', '?', '#']).next()?;
    Some(authority.rsplit_once('@').map_or(authority, |(_, h)| h))
}

fn port_of(url: &str) -> Option<&str> {
    authority_of(url)?.split_once(':').map(|(_, port)| port)
}

/// `https://www.github.com:443/a?b` → `www.github.com`
pub fn host_of(url: &str) -> Option<&str> {
    let host = authority_of(url)?.split(':').next()?;
    (!host.is_empty()).then_some(host)
}

/// 32자리 16진수 id (시간 + 프로세스 + 카운터를 SHA-256으로 섞음)
pub fn new_id() -> String {
    use sha2::{Digest, Sha256};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let seed = format!("{nanos}-{}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed));
    let digest = Sha256::digest(seed.as_bytes());
    digest.iter().take(16).map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn youtube_requires_query_and_trims() {
        assert_eq!(Item::youtube("   ", ""), Err(ItemError::EmptyQuery));
        let item = Item::youtube("  로파이 음악 ", " ").unwrap();
        match &item {
            Item::Youtube { query, name, id } => {
                assert_eq!(query, "로파이 음악");
                assert_eq!(name, "");
                assert_eq!(id.len(), 32);
            }
            _ => panic!("wrong kind"),
        }
        assert_eq!(item.display_name(), "로파이 음악");
    }

    #[test]
    fn normalize_url_adds_https_and_rejects_bad_input() {
        assert_eq!(normalize_url("naver.com").unwrap(), "https://naver.com");
        assert_eq!(normalize_url(" http://a.b/c ").unwrap(), "http://a.b/c");
        assert_eq!(normalize_url("HTTPS://GitHub.com").unwrap(), "HTTPS://GitHub.com");
        assert_eq!(normalize_url(""), Err(ItemError::InvalidUrl));
        assert_eq!(normalize_url("ftp://x.com"), Err(ItemError::InvalidUrl));
        assert_eq!(normalize_url("javascript:alert(1)"), Err(ItemError::InvalidUrl));
        assert_eq!(normalize_url("https://"), Err(ItemError::InvalidUrl));
        assert_eq!(normalize_url("has space.com"), Err(ItemError::InvalidUrl));
    }

    #[test]
    fn web_display_name_defaults_to_host() {
        let item = Item::web("https://user@www.github.com:443/x?y", "").unwrap();
        assert_eq!(item.display_name(), "www.github.com");
        let named = Item::web("github.com", "깃허브").unwrap();
        assert_eq!(named.display_name(), "깃허브");
    }

    #[test]
    fn ids_are_unique() {
        assert_ne!(new_id(), new_id());
    }

    #[test]
    fn serde_uses_kind_tag() {
        let item = Item::App { id: "a".into(), name: "계산기".into(), target: "Calc!App".into(), exe: None };
        let json = serde_json::to_string(&item).unwrap();
        assert_eq!(json, r#"{"kind":"app","id":"a","name":"계산기","target":"Calc!App"}"#);
        let back: Item = serde_json::from_str(&json).unwrap();
        assert_eq!(back, item);
    }
}
