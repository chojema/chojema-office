//! Chojema Office: one place to install, update and open the Chojema Office apps.

mod catalog;
mod detect;
mod error;
mod github;
mod install;
mod platform;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex as AsyncMutex;

use catalog::{AppEntry, Maker, SELF_APP, SUITE};
use error::{Error, Result};
use github::{Release, Releases};
use install::{Progress, Verified};
use platform::Platform;

struct AppState {
    releases: AsyncMutex<Releases>,
    client: reqwest::Client,
    busy: Mutex<HashSet<String>>,
    downloads: PathBuf,
}

/// Static facts about an app, for the frontend.
#[derive(Serialize)]
struct AppInfo {
    id: &'static str,
    name: &'static str,
    repo: &'static str,
    repo_url: String,
    releases_url: String,
    publisher: &'static str,
    maker: Maker,
}

#[derive(Serialize)]
struct Environment {
    version: &'static str,
    platform: Platform,
    can_install: bool,
    apps: Vec<AppInfo>,
}

/// One app's release as the frontend sees it: the release plus the file picked for this computer.
#[derive(Serialize)]
struct ReleaseView {
    #[serde(flatten)]
    release: Release,
    version: String,
    asset: Option<String>,
    asset_size: Option<u64>,
    has_checksums: bool,
}

/// A cached release shown because GitHub could not be asked (rate limit), with the time the limit
/// resets so the interface can say when a fresh check is possible.
#[derive(Serialize)]
struct StaleView {
    #[serde(flatten)]
    view: ReleaseView,
    reset: Option<u64>,
}

#[derive(Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
enum ReleaseResult {
    Ok(ReleaseView),
    Stale(StaleView),
    Error { error: Error },
}

/// The automatic check at start-up skips apps GitHub confirmed this recently. "Refresh" always asks.
const AUTO_RECHECK_SECS: u64 = 10 * 60;

#[derive(Serialize)]
struct InstallOutcome {
    verified: Verified,
    installed: Option<detect::Installed>,
}

fn view(release: Release, platform: Platform) -> ReleaseView {
    let asset = platform::pick_asset(release.assets.iter().map(|a| a.name.as_str()), platform).map(str::to_string);
    let asset_size =
        asset.as_ref().and_then(|n| release.assets.iter().find(|a| &a.name == n)).map(|a| a.size).filter(|s| *s > 0);
    let has_checksums = release.assets.iter().any(|a| a.name.to_ascii_lowercase().starts_with("sha256sums"));
    let version = platform::parse_version(&release.tag).map(|v| v.to_string()).unwrap_or_else(|| release.tag.clone());
    ReleaseView { release, version, asset, asset_size, has_checksums }
}

#[tauri::command]
fn environment() -> Environment {
    let platform = Platform::current();
    Environment {
        version: env!("CARGO_PKG_VERSION"),
        platform,
        can_install: platform.can_install(),
        apps: SUITE
            .iter()
            .map(|a| AppInfo {
                id: a.id,
                name: a.name,
                repo: a.repo,
                repo_url: a.repo_url(),
                releases_url: a.releases_url(),
                publisher: a.publisher,
                maker: a.maker,
            })
            .collect(),
    }
}

#[tauri::command]
async fn installed_apps() -> HashMap<&'static str, detect::Installed> {
    tauri::async_runtime::spawn_blocking(|| SUITE.iter().filter_map(|a| detect::find(a).map(|i| (a.id, i))).collect())
        .await
        .unwrap_or_default()
}

/// Cached releases, for an instant first paint before the network answers.
#[tauri::command]
async fn cached_releases(state: State<'_, AppState>) -> Result<HashMap<&'static str, ReleaseView>> {
    let platform = Platform::current();
    let releases = state.releases.lock().await;
    Ok(SUITE
        .iter()
        .chain(std::iter::once(&SELF_APP))
        .filter_map(|a| releases.cached(a.repo).map(|r| (a.id, view(r, platform))))
        .collect())
}

/// Latest releases of every app and of Chojema Office itself, fetched in parallel. Without `force`,
/// apps confirmed in the last few minutes are answered from the cache.
#[tauri::command]
async fn refresh_releases(
    state: State<'_, AppState>,
    force: Option<bool>,
) -> Result<HashMap<&'static str, ReleaseResult>> {
    let platform = Platform::current();
    let force = force.unwrap_or(true);
    let apps: Vec<&'static AppEntry> = SUITE.iter().chain(std::iter::once(&SELF_APP)).collect();
    let mut out = HashMap::new();
    let mut to_fetch = Vec::new();
    {
        let releases = state.releases.lock().await;
        for app in &apps {
            match releases.recent(app.repo, AUTO_RECHECK_SECS).filter(|_| !force) {
                Some(r) => {
                    out.insert(app.id, ReleaseResult::Ok(view(r, platform)));
                }
                None => to_fetch.push((*app, releases.entry(app.repo))),
            }
        }
    }
    let tasks: Vec<_> = to_fetch
        .into_iter()
        .map(|(app, prev)| {
            let client = state.client.clone();
            let repo = app.repo;
            (app, tauri::async_runtime::spawn(async move { github::fetch_latest(&client, repo, prev).await }))
        })
        .collect();

    let mut releases = state.releases.lock().await;
    for (app, task) in tasks {
        let result = task.await.map_err(|e| Error::Io(e.to_string()))?;
        let entry = match result {
            Ok((release, etag)) => {
                releases.store(app.repo, etag, release.clone());
                ReleaseResult::Ok(view(release, platform))
            }
            Err(error) if app.id == SELF_APP.id => self_fallback(&mut releases, error, platform).await,
            Err(Error::RateLimited(reset)) => match releases.cached(app.repo) {
                Some(r) => ReleaseResult::Stale(StaleView { view: view(r, platform), reset }),
                None => ReleaseResult::Error { error: Error::RateLimited(reset) },
            },
            Err(error) => ReleaseResult::Error { error },
        };
        out.insert(app.id, entry);
    }
    releases.save();
    Ok(out)
}

/// Chojema Office must notice its own updates even when the API is unavailable, so its check falls
/// back to the release page redirect, which gives the latest tag without using the API limit.
async fn self_fallback(releases: &mut Releases, error: Error, platform: Platform) -> ReleaseResult {
    let cached = releases.cached(SELF_APP.repo);
    let tag = match github::latest_tag_from_web(SELF_APP.repo).await {
        Ok(tag) => tag,
        Err(_) => {
            return match (error, cached) {
                (Error::RateLimited(reset), Some(r)) => {
                    ReleaseResult::Stale(StaleView { view: view(r, platform), reset })
                }
                (error, _) => ReleaseResult::Error { error },
            }
        }
    };
    // Same tag as the cache: the cached release (with its notes) is confirmed current.
    let release = match cached {
        Some(r) if r.tag == tag => r,
        _ => github::self_release_for_tag(SELF_APP.repo, &tag),
    };
    releases.store(SELF_APP.repo, None, release.clone());
    ReleaseResult::Ok(view(release, platform))
}

struct BusyGuard<'a> {
    set: &'a Mutex<HashSet<String>>,
    id: String,
}

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut s) = self.set.lock() {
            s.remove(&self.id);
        }
    }
}

fn claim<'a>(state: &'a AppState, id: &str) -> Result<BusyGuard<'a>> {
    let mut set = state.busy.lock().map_err(|_| Error::Busy)?;
    if !set.insert(id.to_string()) {
        return Err(Error::Busy);
    }
    Ok(BusyGuard { set: &state.busy, id: id.to_string() })
}

async fn fetch_for_install(state: &AppState, app: &AppEntry) -> Result<(Release, github::Asset)> {
    let platform = Platform::current();
    if !platform.can_install() {
        return Err(Error::Unsupported);
    }
    let release = state.releases.lock().await.latest(&state.client, app.repo).await?;
    let name = platform::pick_asset(release.assets.iter().map(|a| a.name.as_str()), platform).ok_or(Error::NoAsset)?;
    let asset = release.assets.iter().find(|a| a.name == name).cloned().ok_or(Error::NoAsset)?;
    Ok((release, asset))
}

/// Downloads, verifies and installs (or updates) one app.
#[tauri::command]
async fn install_app(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    silent: Option<bool>,
) -> Result<InstallOutcome> {
    let entry = AppEntry::by_id(&id).filter(|a| a.id != SELF_APP.id).ok_or(Error::UnknownApp)?;
    let _guard = claim(&state, &id)?;
    let (release, asset) = fetch_for_install(&state, entry).await?;

    let emit = |phase: &'static str, received: u64, total: u64| {
        let _ = app.emit("task-progress", Progress { id: id.clone(), phase, received, total });
    };
    let dir = state.downloads.join(entry.id);
    let (path, verified) = install::download(&state.client, &release, &asset, &dir, emit).await?;
    emit("install", 0, 0);
    let result = install::run_installer(&path, silent.unwrap_or(true)).await;
    let _ = tokio::fs::remove_file(&path).await;
    result?;
    emit("done", 0, 0);

    let installed = tauri::async_runtime::spawn_blocking(move || detect::find(entry)).await.ok().flatten();
    Ok(InstallOutcome { verified, installed })
}

/// Downloads the newest Chojema Office installer, starts it and quits so it can replace this copy.
#[tauri::command]
async fn update_self(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    let _guard = claim(&state, SELF_APP.id)?;
    let (release, asset) = fetch_for_install(&state, &SELF_APP).await?;
    let emit = |phase: &'static str, received: u64, total: u64| {
        let _ = app.emit("task-progress", Progress { id: SELF_APP.id.to_string(), phase, received, total });
    };
    let dir = state.downloads.join(SELF_APP.id);
    let (path, _) = install::download(&state.client, &release, &asset, &dir, emit).await?;
    emit("install", 0, 0);
    // macOS: the new app is copied next to this one now and swapped in after this process exits,
    // then reopened. Windows: the installer replaces the program and offers to start it.
    #[cfg(target_os = "macos")]
    {
        install::stage_self_update(&path).await?;
        let _ = tokio::fs::remove_file(&path).await;
    }
    #[cfg(not(target_os = "macos"))]
    install::spawn_installer(&path)?;
    app.exit(0);
    Ok(())
}

#[tauri::command]
async fn launch_app(id: String) -> Result<()> {
    let entry = AppEntry::by_id(&id).filter(|a| a.id != SELF_APP.id).ok_or(Error::UnknownApp)?;
    let found = tauri::async_runtime::spawn_blocking(move || detect::find(entry)).await.ok().flatten();
    let path = found.and_then(|i| i.path).ok_or(Error::NotInstalled)?;
    install::launch(&path)
}

/// Whether Chojema Office starts when the user signs in to the computer.
#[tauri::command]
fn autostart_enabled(app: AppHandle) -> Result<bool> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(|e| Error::Io(e.to_string()))
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<bool> {
    use tauri_plugin_autostart::ManagerExt;
    let launcher = app.autolaunch();
    let result = if enabled { launcher.enable() } else { launcher.disable() };
    result.map_err(|e| Error::Io(e.to_string()))?;
    launcher.is_enabled().map_err(|e| Error::Io(e.to_string()))
}

/// Opens a web page in the default browser. Only GitHub pages of the suite's repositories and
/// the maker's own pages listed in [`catalog::ABOUT_LINKS`].
#[tauri::command]
fn open_external(app: AppHandle, url: String) -> Result<()> {
    let lower = url.to_ascii_lowercase();
    let allowed = catalog::ABOUT_LINKS.iter().any(|l| lower == l.to_ascii_lowercase())
        || SUITE.iter().chain(std::iter::once(&SELF_APP)).any(|a| {
            let base = format!("{}/", a.repo_url()).to_ascii_lowercase();
            lower == a.repo_url().to_ascii_lowercase() || lower.starts_with(&base)
        });
    if !allowed {
        return Err(Error::Unsupported);
    }
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_url(url, None::<&str>).map_err(|e| Error::Io(e.to_string()))
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .setup(|app| {
            let cache = app.path().app_cache_dir().unwrap_or_else(|_| std::env::temp_dir().join("chojema-office"));
            let client = github::client()?;
            app.manage(AppState {
                releases: AsyncMutex::new(Releases::load(&cache)),
                client,
                busy: Mutex::new(HashSet::new()),
                downloads: cache.join("downloads"),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            environment,
            installed_apps,
            cached_releases,
            refresh_releases,
            install_app,
            update_self,
            launch_app,
            open_external,
            autostart_enabled,
            set_autostart
        ])
        .run(tauri::generate_context!())
        .expect("error while running Chojema Office");
}
