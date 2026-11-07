//! Latest-release lookups on GitHub, with an on-disk ETag cache so repeated checks stay well under
//! the anonymous API rate limit.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

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

    pub fn store(&mut self, repo: &str, etag: Option<String>, release: Release) {
        self.cache.entries.insert(repo.to_string(), CacheEntry { etag, release: Some(release) });
    }

    pub fn save(&self) {
        if let Some(dir) = self.cache_file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(bytes) = serde_json::to_vec(&self.cache) {
            let _ = std::fs::write(&self.cache_file, bytes);
        }
    }

    /// Fetches `repo`'s latest release and updates the cache.
    pub async fn latest(&mut self, client: &reqwest::Client, repo: &str) -> Result<Release> {
        let (release, etag) = fetch_latest(client, repo, self.entry(repo)).await?;
        self.store(repo, etag, release.clone());
        self.save();
        Ok(release)
    }
}

/// Fetches `repo`'s latest release. With `previous` (ETag and release) the request is conditional
/// and the previous release is returned when GitHub says it is unchanged or the rate limit is hit.
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
    if status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return match previous {
            Some((etag, r)) => Ok((r, etag)),
            None => Err(Error::RateLimited),
        };
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
}
