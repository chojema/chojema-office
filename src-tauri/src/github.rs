//! Latest-release lookups on GitHub, with an on-disk ETag cache so repeated checks stay well under
//! the anonymous API rate limit.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,
    pub size: u64,
    pub url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Release {
    pub tag: String,
    pub name: String,
    pub published_at: String,
    pub body: String,
    pub html_url: String,
    pub assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    size: u64,
    browser_download_url: String,
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    name: Option<String>,
    published_at: Option<String>,
    body: Option<String>,
    html_url: String,
    assets: Vec<ApiAsset>,
}

#[derive(Default, Serialize, Deserialize)]
struct CacheEntry {
    etag: Option<String>,
    release: Option<Release>,
    /// When GitHub last confirmed this release (Unix seconds).
    #[serde(default)]
    checked_at: Option<u64>,
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[derive(Default, Serialize, Deserialize)]
struct Cache {
    entries: HashMap<String, CacheEntry>,
}

pub fn user_agent() -> String {
    format!("ChojemaOffice/{} (+https://github.com/chojema/chojema-office)", env!("CARGO_PKG_VERSION"))
}

pub fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(user_agent())
        .connect_timeout(Duration::from_secs(15))
        .build()
        .map_err(Error::from)
}

pub struct Releases {
    cache_file: PathBuf,
    cache: Cache,
}

impl Releases {
    pub fn load(cache_dir: &Path) -> Self {
        let cache_file = cache_dir.join("releases.json");
        let cache = std::fs::read(&cache_file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        Self { cache_file, cache }
    }

    pub fn cached(&self, repo: &str) -> Option<Release> {
        self.cache.entries.get(repo).and_then(|e| e.release.clone())
    }

    /// The cached ETag and release for `repo`, for a conditional request.
    pub fn entry(&self, repo: &str) -> Option<(Option<String>, Release)> {
        self.cache.entries.get(repo).and_then(|e| e.release.clone().map(|r| (e.etag.clone(), r)))
    }

    /// The cached release when GitHub confirmed it less than `max_age_secs` ago.
    pub fn recent(&self, repo: &str, max_age_secs: u64) -> Option<Release> {
        let e = self.cache.entries.get(repo)?;
        let age = now_secs().saturating_sub(e.checked_at?);
        (age < max_age_secs).then(|| e.release.clone()).flatten()
    }

    pub fn store(&mut self, repo: &str, etag: Option<String>, release: Release) {
        let entry = CacheEntry { etag, release: Some(release), checked_at: Some(now_secs()) };
        self.cache.entries.insert(repo.to_string(), entry);
    }

    pub fn save(&self) {
        if let Some(dir) = self.cache_file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(bytes) = serde_json::to_vec(&self.cache) {
            let _ = std::fs::write(&self.cache_file, bytes);
        }
    }

    /// Fetches `repo`'s latest release and updates the cache. When the rate limit is hit, the cached
    /// release is used instead: its download links do not count against the API limit.
    pub async fn latest(&mut self, client: &reqwest::Client, repo: &str) -> Result<Release> {
        match fetch_latest(client, repo, self.entry(repo)).await {
            Ok((release, etag)) => {
                self.store(repo, etag, release.clone());
                self.save();
                Ok(release)
            }
            Err(Error::RateLimited(reset)) => self.cached(repo).ok_or(Error::RateLimited(reset)),
            Err(e) => Err(e),
        }
    }
}

/// Fetches `repo`'s latest release. With `previous` (ETag and release) the request is conditional
/// and the previous release is returned when GitHub says it is unchanged. Hitting the rate limit
/// is an error (with the reset time) so callers can say the data is not current.
pub async fn fetch_latest(
    client: &reqwest::Client,
    repo: &str,
    previous: Option<(Option<String>, Release)>,
) -> Result<(Release, Option<String>)> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let mut req = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .timeout(Duration::from_secs(30));
    if let Ok(token) = std::env::var("CHOJEMA_OFFICE_GITHUB_TOKEN").or_else(|_| std::env::var("GITHUB_TOKEN")) {
        if !token.trim().is_empty() {
            req = req.bearer_auth(token.trim());
        }
    }
    if let Some((Some(etag), _)) = &previous {
        req = req.header("If-None-Match", etag.as_str());
    }

    let resp = req.send().await?;
    let status = resp.status();
    if status == reqwest::StatusCode::NOT_MODIFIED {
        if let Some((etag, r)) = previous {
            return Ok((r, etag));
        }
    }
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(Error::NoRelease);
    }
    let header = |name: &str| resp.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_string);
    let exhausted = header("x-ratelimit-remaining").as_deref() == Some("0");
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS || (status == reqwest::StatusCode::FORBIDDEN && exhausted) {
        let reset = header("x-ratelimit-reset").and_then(|v| v.parse().ok());
        return Err(Error::RateLimited(reset));
    }
    if !status.is_success() {
        return Err(Error::Http(status.as_u16()));
    }
    let etag = resp.headers().get("etag").and_then(|v| v.to_str().ok()).map(str::to_string);
    let api: ApiRelease = resp.json().await?;
    let prefix = format!("https://github.com/{repo}/releases/download/").to_ascii_lowercase();
    let release = Release {
        name: api.name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| api.tag_name.clone()),
        tag: api.tag_name,
        published_at: api.published_at.unwrap_or_default(),
        body: api.body.unwrap_or_default(),
        html_url: api.html_url,
        assets: api
            .assets
            .into_iter()
            // Only files hosted on the app's own release page are ever downloaded.
            .filter(|a| a.browser_download_url.to_ascii_lowercase().starts_with(&prefix))
            .map(|a| Asset { name: a.name, size: a.size, url: a.browser_download_url })
            .collect(),
    };
    Ok((release, etag))
}

/// Reads the latest release tag from the `github.com/<repo>/releases/latest` redirect. This is a
/// web page, not the REST API, so it still answers when the API rate limit is used up.
pub async fn latest_tag_from_web(repo: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .user_agent(user_agent())
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(15))
        .build()?;
    let resp = client
        .get(format!("https://github.com/{repo}/releases/latest"))
        .timeout(Duration::from_secs(20))
        .send()
        .await?;
    if !resp.status().is_redirection() {
        return Err(Error::Http(resp.status().as_u16()));
    }
    resp.headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .and_then(tag_from_location)
        .ok_or(Error::NoRelease)
}

/// `https://github.com/o/r/releases/tag/v1.2.3` -> `v1.2.3`.
fn tag_from_location(location: &str) -> Option<String> {
    let (_, rest) = location.split_once("/releases/tag/")?;
    let tag = rest.split(['?', '#', '/']).next()?.trim();
    (!tag.is_empty()).then(|| tag.to_string())
}

/// Chojema Office's own release for `tag`, built from the file names the release workflow always
/// publishes (`.github/workflows/release.yml`). Used when only the tag could be read.
pub fn self_release_for_tag(repo: &str, tag: &str) -> Release {
    let version = tag.trim_start_matches('v');
    let names = [
        format!("ChojemaOffice-{version}-windows-x64-setup.exe"),
        format!("ChojemaOffice-{version}-macos-universal.dmg"),
        "SHA256SUMS.txt".to_string(),
    ];
    Release {
        tag: tag.to_string(),
        name: format!("Chojema Office {version}"),
        published_at: String::new(),
        body: String::new(),
        html_url: format!("https://github.com/{repo}/releases/tag/{tag}"),
        assets: names
            .into_iter()
            .map(|name| Asset {
                url: format!("https://github.com/{repo}/releases/download/{tag}/{name}"),
                size: 0,
                name,
            })
            .collect(),
    }
}

/// Finds the SHA-256 for `file` in a `SHA256SUMS` style text (`<hex>  <name>` or `<hex> *<name>`).
pub fn find_checksum(sums: &str, file: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let name = parts.next()?.trim_start_matches('*');
        let name = name.rsplit(['/', '\\']).next().unwrap_or(name);
        (hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()) && name == file)
            .then(|| hash.to_ascii_lowercase())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_sha256sums() {
        let sums = "\
0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  wordcraft-0.16.0-windows-x64.msi
ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789 *dist/Setup.exe
";
        assert_eq!(
            find_checksum(sums, "wordcraft-0.16.0-windows-x64.msi").as_deref(),
            Some("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
        );
        assert!(find_checksum(sums, "Setup.exe").unwrap().starts_with("abcdef"));
        assert_eq!(find_checksum(sums, "missing.msi"), None);
    }

    #[test]
    fn reads_tag_from_redirect() {
        let loc = "https://github.com/chojema/chojema-office/releases/tag/v0.2.1";
        assert_eq!(tag_from_location(loc).as_deref(), Some("v0.2.1"));
        assert_eq!(tag_from_location("https://github.com/o/r/releases/tag/1.0.0?x=1").as_deref(), Some("1.0.0"));
        assert_eq!(tag_from_location("https://github.com/o/r/releases"), None);
        assert_eq!(tag_from_location("https://github.com/o/r/releases/tag/"), None);
    }

    #[test]
    fn self_release_has_installers_for_both_systems() {
        use crate::platform::{pick_asset, Arch, Os, Platform};
        let r = self_release_for_tag("chojema/chojema-office", "v0.2.1");
        let names = || r.assets.iter().map(|a| a.name.as_str());
        let mac = Platform { os: Os::Macos, arch: Arch::Arm64 };
        let win = Platform { os: Os::Windows, arch: Arch::X64 };
        assert_eq!(pick_asset(names(), mac), Some("ChojemaOffice-0.2.1-macos-universal.dmg"));
        assert_eq!(pick_asset(names(), win), Some("ChojemaOffice-0.2.1-windows-x64-setup.exe"));
        assert!(r
            .assets
            .iter()
            .all(|a| a.url.starts_with("https://github.com/chojema/chojema-office/releases/download/v0.2.1/")));
        assert!(r.assets.iter().any(|a| a.name == "SHA256SUMS.txt"));
    }

    #[test]
    fn recent_respects_age() {
        let dir = std::env::temp_dir().join(format!("co-test-{}", std::process::id()));
        let mut rel = Releases { cache_file: dir.join("releases.json"), cache: Cache::default() };
        assert!(rel.recent("o/r", 600).is_none());
        rel.store("o/r", None, self_release_for_tag("o/r", "v1.0.0"));
        assert!(rel.recent("o/r", 600).is_some());
        assert!(rel.recent("o/r", 0).is_none());
    }

    /// Hits github.com; run by hand with `cargo test -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn web_tag_lookup_works() {
        let tag = latest_tag_from_web("chojema/chojema-office").await.unwrap();
        assert!(tag.starts_with('v'), "{tag}");
    }
}
