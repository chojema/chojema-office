//! Downloading, verifying, installing and opening apps.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::detect::Installed;
use crate::error::{Error, Result};
use crate::github::{find_checksum, Asset, Release};

/// Progress reported to the frontend while a task runs.
#[derive(Clone, Debug, Serialize)]
pub struct Progress {
    pub id: String,
    /// `download`, `verify`, `install` or `done`.
    pub phase: &'static str,
    pub received: u64,
    pub total: u64,
}

/// How a finished download was checked.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Verified {
    /// Matched the release's published SHA-256.
    Checksum,
    /// The release publishes no checksum for this file.
    Unchecked,
}

pub fn verify_installed(installed: Option<Installed>, expected: &str) -> Result<Installed> {
    let installed = installed.ok_or(Error::InstallUnverified)?;
    let actual = installed.version.as_deref().and_then(crate::platform::parse_version);
    let expected = crate::platform::parse_version(expected);
    let matches = matches!((actual, expected), (Some(a), Some(e)) if a.cmp_precedence(&e).is_ge());
    if !matches || !installed.path.as_deref().is_some_and(|p| Path::new(p).exists()) {
        return Err(Error::InstallUnverified);
    }
    Ok(installed)
}

#[cfg(any(windows, test))]
fn msi_arguments(silent: bool, machine: bool) -> String {
    format!("{} /norestart{}", if silent { "/passive" } else { "" }, if machine { " ALLUSERS=1" } else { "" })
        .trim()
        .to_string()
}

/// Downloads `asset` into `dir`, reporting progress, and checks it against the release's checksum
/// file when there is one.
pub async fn download(
    client: &reqwest::Client,
    release: &Release,
    asset: &Asset,
    dir: &Path,
    mut on_progress: impl FnMut(&'static str, u64, u64),
) -> Result<(PathBuf, Verified)> {
    tokio::fs::create_dir_all(dir).await?;
    // Names come from GitHub; keep only the final path component.
    let file_name = Path::new(&asset.name).file_name().ok_or(Error::NoAsset)?;
    let target = dir.join(file_name);
    let partial = dir.join(format!("{}.part", file_name.to_string_lossy()));

    let resp = client.get(&asset.url).send().await?;
    if !resp.status().is_success() {
        return Err(Error::Http(resp.status().as_u16()));
    }
    let total = resp.content_length().unwrap_or(asset.size);
    let mut file = tokio::fs::File::create(&partial).await?;
    let mut hasher = Sha256::new();
    let mut received = 0u64;
    let mut stream = resp.bytes_stream();
    let mut last_report = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
        received += chunk.len() as u64;
        if received - last_report >= 256 * 1024 || received == total {
            last_report = received;
            on_progress("download", received, total);
        }
    }
    file.flush().await?;
    drop(file);
    let digest = hex::encode(hasher.finalize());

    on_progress("verify", received, total);
    let sums: Vec<&Asset> = release
        .assets
        .iter()
        .filter(|a| {
            let n = a.name.to_ascii_lowercase();
            n.starts_with("sha256sums") && n.ends_with(".txt")
        })
        .collect();
    let mut verified = Verified::Unchecked;
    for sum in sums {
        let text = client.get(&sum.url).send().await?.error_for_status()?.text().await?;
        if let Some(expected) = find_checksum(&text, &asset.name) {
            if expected != digest {
                let _ = tokio::fs::remove_file(&partial).await;
                return Err(Error::Checksum);
            }
            verified = Verified::Checksum;
            break;
        }
    }

    let _ = tokio::fs::remove_file(&target).await;
    tokio::fs::rename(&partial, &target).await?;
    Ok((target, verified))
}

/// Runs an installer and waits for it. `silent` asks for an unattended install; the system still
/// shows its own permission prompt when the installer needs administrator rights.
#[cfg(windows)]
pub async fn run_installer(path: &Path, silent: bool, previous: Option<&Installed>) -> Result<()> {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let lower = path.to_string_lossy().to_ascii_lowercase();
    let quoted = |s: &str| s.replace('\'', "''");
    // Start-Process goes through ShellExecute, which raises the UAC prompt for installers that
    // need elevation (CreateProcess would just fail with ERROR_ELEVATION_REQUIRED).
    let script = if lower.ends_with(".msi") {
        let args = msi_arguments(silent, previous.is_some_and(|i| i.machine));
        format!(
            "$p = Start-Process -FilePath 'msiexec.exe' -ArgumentList @('/i', '\"{}\"', '{}') -Wait -PassThru; exit $p.ExitCode",
            quoted(&path.to_string_lossy()),
            args
        )
    } else {
        let args = if silent { "-ArgumentList '/S' " } else { "" };
        format!(
            "$p = Start-Process -FilePath '{}' {}-Wait -PassThru; exit $p.ExitCode",
            quoted(&path.to_string_lossy()),
            args
        )
    };
    let status = tokio::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .await?;
    match status.code().unwrap_or(-1) {
        0 | 3010 | 1641 => Ok(()),
        // 1602: user cancelled (MSI). 1223: UAC prompt declined. 1: our script when Start-Process
        // threw (most often a declined prompt).
        1602 | 1223 | 1 => Err(Error::Cancelled),
        code => Err(Error::InstallerFailed(code)),
    }
}

/// Mounts the disk image, copies the app into `/Applications` (or `~/Applications` when that is
/// not writable) and unmounts it.
#[cfg(target_os = "macos")]
pub async fn run_installer(path: &Path, _silent: bool, previous: Option<&Installed>) -> Result<()> {
    let system = PathBuf::from("/Applications");
    let dest_dir = if let Some(parent) = previous.and_then(|i| i.path.as_deref()).and_then(|p| Path::new(p).parent()) {
        parent.to_path_buf()
    } else if is_writable(&system) {
        system
    } else {
        let home = std::env::var_os("HOME").map(PathBuf::from).ok_or(Error::Unsupported)?;
        let d = home.join("Applications");
        tokio::fs::create_dir_all(&d).await?;
        d
    };
    copy_app_from_dmg(path, |name| dest_dir.join(name)).await.map(|_| ())
}

/// Mounts `dmg`, copies the `.app` inside to `dest(app name)` (replacing what is there) and
/// unmounts. Returns the copied path.
#[cfg(target_os = "macos")]
async fn copy_app_from_dmg(dmg: &Path, dest: impl FnOnce(&std::ffi::OsStr) -> PathBuf) -> Result<PathBuf> {
    use tokio::process::Command;
    let mount = work_dir(&std::env::temp_dir())?;
    let st = Command::new("hdiutil")
        .arg("attach")
        .arg(dmg)
        .args(["-nobrowse", "-readonly", "-noautoopen", "-mountpoint"])
        .arg(&mount)
        .status()
        .await?;
    if !st.success() {
        let _ = tokio::fs::remove_dir(&mount).await;
        return Err(Error::InstallerFailed(st.code().unwrap_or(-1)));
    }
    let result = async {
        let mut entries = tokio::fs::read_dir(&mount).await?;
        let mut app = None;
        while let Some(e) = entries.next_entry().await? {
            if e.path().extension().is_some_and(|x| x == "app") {
                app = Some(e.path());
                break;
            }
        }
        let app = app.ok_or(Error::NoAsset)?;
        let dest = dest(app.file_name().ok_or(Error::NoAsset)?);
        replace_app(&app, &dest).await?;
        Ok(dest)
    }
    .await;
    let _ = Command::new("hdiutil").arg("detach").arg(&mount).arg("-quiet").status().await;
    let _ = tokio::fs::remove_dir(&mount).await;
    result
}

#[cfg(target_os = "macos")]
fn work_dir(parent: &Path) -> Result<PathBuf> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    loop {
        let dir =
            parent.join(format!(".chojema-office-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
}

#[cfg(target_os = "macos")]
async fn replace_app(source: &Path, dest: &Path) -> Result<()> {
    let work = work_dir(dest.parent().ok_or(Error::Unsupported)?)?;
    let staged = work.join("new.app");
    let backup = work.join("previous.app");
    let result = async {
        let st = tokio::process::Command::new("ditto").arg(source).arg(&staged).status().await?;
        if !st.success() {
            return Err(Error::InstallerFailed(st.code().unwrap_or(-1)));
        }
        let expected = crate::detect::from_bundle(source).and_then(|i| i.version).ok_or(Error::InstallUnverified)?;
        verify_installed(crate::detect::from_bundle(&staged), &expected)?;
        commit_app(&staged, dest, &backup)
    }
    .await;
    // A failed rollback must retain the backup for manual recovery.
    if result.is_ok() || !backup.exists() {
        let _ = tokio::fs::remove_dir_all(&work).await;
    }
    result
}

#[cfg(target_os = "macos")]
fn commit_app(staged: &Path, dest: &Path, backup: &Path) -> Result<()> {
    let had_previous = dest.try_exists()?;
    if had_previous {
        std::fs::rename(dest, backup)?;
    }
    if let Err(e) = std::fs::rename(staged, dest) {
        if had_previous {
            std::fs::rename(backup, dest).map_err(|restore| {
                Error::Io(format!(
                    "{e}; restoring the old app failed: {restore}; backup retained at {}",
                    backup.display()
                ))
            })?;
        }
        return Err(e.into());
    }
    Ok(())
}

/// Prepares Chojema Office's own update: copies the new app next to the running one, then starts a
/// small detached script that waits for this process to exit, swaps the bundles (restoring the old
/// one if the swap fails) and reopens the app. The caller exits right after.
///
/// When the running copy cannot be replaced in place (read-only disk image, App Translocation, a
/// folder without write access) the new version is installed into Applications instead.
#[cfg(target_os = "macos")]
pub async fn stage_self_update(dmg: &Path) -> Result<()> {
    let exe = std::env::current_exe()?;
    let bundle = exe
        .ancestors()
        .find(|p| p.extension().is_some_and(|x| x == "app"))
        .map(Path::to_path_buf)
        .ok_or(Error::Unsupported)?;
    let parent = bundle.parent().ok_or(Error::Unsupported)?.to_path_buf();
    let in_place = is_writable(&parent)
        && !bundle.to_string_lossy().contains("/AppTranslocation/")
        && !bundle.starts_with("/Volumes");

    if !in_place {
        run_installer(dmg, true, None).await?;
        let name = bundle.file_name().ok_or(Error::Unsupported)?;
        let installed = [
            PathBuf::from("/Applications"),
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Applications"),
        ]
        .into_iter()
        .map(|d| d.join(name))
        .find(|p| p.exists())
        .ok_or(Error::NotInstalled)?;
        return relaunch_after_exit(&installed, None);
    }

    let name = bundle.file_name().ok_or(Error::Unsupported)?.to_string_lossy().into_owned();
    let staged = parent.join(format!(".{name}.update"));
    copy_app_from_dmg(dmg, |_| staged.clone()).await?;
    relaunch_after_exit(&bundle, Some(&staged))
}

/// Spawns the detached helper: wait for this process, optionally move `staged` over `bundle`
/// (keeping a backup until the move succeeds), then open `bundle`.
#[cfg(target_os = "macos")]
fn relaunch_after_exit(bundle: &Path, staged: Option<&Path>) -> Result<()> {
    const SCRIPT: &str = r#"
while kill -0 "$1" 2>/dev/null; do sleep 0.2; done
if [ -n "$3" ]; then
  backup="$2.previous"
  rm -rf "$backup"
  if mv "$2" "$backup"; then
    if mv "$3" "$2"; then rm -rf "$backup"; else mv "$backup" "$2"; rm -rf "$3"; fi
  fi
fi
open "$2"
"#;
    let staged = staged.map(|p| p.as_os_str().to_owned()).unwrap_or_default();
    std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(SCRIPT)
        .arg("chojema-office-update")
        .arg(std::process::id().to_string())
        .arg(bundle)
        .arg(staged)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn is_writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".chojema-office-{}", std::process::id()));
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(probe);
            true
        }
        Err(_) => false,
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
pub async fn run_installer(_path: &Path, _silent: bool, _previous: Option<&Installed>) -> Result<()> {
    Err(Error::Unsupported)
}

/// Starts an installer without waiting (used for Chojema Office's own update, which then quits).
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub fn spawn_installer(path: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let script = format!("Start-Process -FilePath '{}'", path.to_string_lossy().replace('\'', "''"));
        std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(path).spawn()?;
        Ok(())
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = path;
        Err(Error::Unsupported)
    }
}

/// Opens an installed app.
pub fn launch(path: &str) -> Result<()> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(Error::NotInstalled);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        let mut cmd = std::process::Command::new(p);
        if let Some(dir) = p.parent() {
            cmd.current_dir(dir);
        }
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP).spawn()?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(p).spawn()?;
        Ok(())
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn installed(version: Option<&str>) -> Installed {
        Installed {
            version: version.map(str::to_string),
            path: Some(std::env::current_exe().unwrap().to_string_lossy().into_owned()),
            machine: false,
        }
    }

    #[test]
    fn verifies_installed_version_and_path_before_success() {
        assert!(verify_installed(Some(installed(Some("0.20.0"))), "v0.20.0").is_ok());
        assert!(verify_installed(Some(installed(Some("0.21.0"))), "v0.20.0").is_ok());
        assert!(verify_installed(Some(installed(Some("0.20.0.0"))), "v0.20.0").is_ok());
        for version in [None, Some("unknown"), Some("0.19.0"), Some("0.20.0-beta.1")] {
            assert!(matches!(verify_installed(Some(installed(version)), "v0.20.0"), Err(Error::InstallUnverified)));
        }
        assert!(matches!(verify_installed(None, "v0.20.0"), Err(Error::InstallUnverified)));
        let mut no_path = installed(Some("0.20.0"));
        no_path.path = None;
        assert!(verify_installed(Some(no_path), "v0.20.0").is_err());
        let mut missing = installed(Some("0.20.0"));
        missing.path = Some(std::env::current_exe().unwrap().join("missing-app.exe").to_string_lossy().into_owned());
        assert!(verify_installed(Some(missing), "v0.20.0").is_err());
    }

    #[test]
    fn msi_updates_retain_machine_scope_in_passive_and_interactive_modes() {
        for silent in [false, true] {
            let machine = msi_arguments(silent, true);
            assert!(machine.split_whitespace().any(|a| a == "ALLUSERS=1"));
            assert!(!msi_arguments(silent, false).contains("ALLUSERS"));
            assert_eq!(machine.contains("/passive"), silent);
            assert!(machine.contains("/norestart"));
        }
    }

    #[cfg(target_os = "macos")]
    fn bundle(path: &Path, version: &str) {
        std::fs::create_dir_all(path.join("Contents/MacOS")).unwrap();
        std::fs::write(path.join("Contents/MacOS/test-app"), b"test executable").unwrap();
        let mut dict = plist::Dictionary::new();
        dict.insert("CFBundleExecutable".into(), "test-app".into());
        dict.insert("CFBundleShortVersionString".into(), version.into());
        plist::Value::Dictionary(dict).to_file_xml(path.join("Contents/Info.plist")).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn replaces_only_after_a_complete_bundle_is_ready() {
        let dir = work_dir(&std::env::temp_dir()).unwrap();
        let source = dir.join("source.app");
        let dest = dir.join("existing.app");
        bundle(&source, "0.20.0");
        bundle(&dest, "0.19.0");
        std::fs::write(dest.join("old-only"), b"old").unwrap();
        replace_app(&source, &dest).await.unwrap();
        assert_eq!(crate::detect::from_bundle(&dest).unwrap().version.as_deref(), Some("0.20.0"));
        assert!(!dest.join("old-only").exists());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn copy_or_validation_failure_keeps_the_old_app() {
        let dir = work_dir(&std::env::temp_dir()).unwrap();
        let dest = dir.join("existing.app");
        bundle(&dest, "0.19.0");
        assert!(replace_app(&dir.join("missing.app"), &dest).await.is_err());
        let invalid = dir.join("invalid.app");
        bundle(&invalid, "0.20.0");
        std::fs::remove_file(invalid.join("Contents/MacOS/test-app")).unwrap();
        assert!(matches!(replace_app(&invalid, &dest).await, Err(Error::InstallUnverified)));
        assert_eq!(crate::detect::from_bundle(&dest).unwrap().version.as_deref(), Some("0.19.0"));
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn failed_swap_restores_the_previous_app() {
        let dir = work_dir(&std::env::temp_dir()).unwrap();
        let dest = dir.join("existing.app");
        bundle(&dest, "0.19.0");
        let backup = dir.join("backup.app");
        assert!(commit_app(&dir.join("missing-staged.app"), &dest, &backup).is_err());
        assert_eq!(crate::detect::from_bundle(&dest).unwrap().version.as_deref(), Some("0.19.0"));
        assert!(!backup.exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
