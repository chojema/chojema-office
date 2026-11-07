//! Downloading, verifying, installing and opening apps.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

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
pub async fn run_installer(path: &Path, silent: bool) -> Result<()> {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let lower = path.to_string_lossy().to_ascii_lowercase();
    let quoted = |s: &str| s.replace('\'', "''");
    // Start-Process goes through ShellExecute, which raises the UAC prompt for installers that
    // need elevation (CreateProcess would just fail with ERROR_ELEVATION_REQUIRED).
    let script = if lower.ends_with(".msi") {
        let args = if silent { "/passive /norestart" } else { "/norestart" };
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
pub async fn run_installer(path: &Path, _silent: bool) -> Result<()> {
    use tokio::process::Command;
    let mount = std::env::temp_dir().join(format!("chojema-office-{}", std::process::id()));
    let _ = tokio::fs::create_dir_all(&mount).await;
    let st = Command::new("hdiutil")
        .arg("attach")
        .arg(path)
        .args(["-nobrowse", "-readonly", "-noautoopen", "-mountpoint"])
        .arg(&mount)
        .status()
        .await?;
    if !st.success() {
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
        let name = app.file_name().ok_or(Error::NoAsset)?.to_owned();
        let system = PathBuf::from("/Applications");
        let dest_dir = if is_writable(&system) {
            system
        } else {
            let home = std::env::var_os("HOME").map(PathBuf::from).ok_or(Error::Unsupported)?;
            let d = home.join("Applications");
            tokio::fs::create_dir_all(&d).await?;
            d
        };
        let dest = dest_dir.join(name);
        if dest.exists() {
            let _ = tokio::fs::remove_dir_all(&dest).await;
        }
        let st = Command::new("ditto").arg(&app).arg(&dest).status().await?;
        if st.success() {
            Ok(())
        } else {
            Err(Error::InstallerFailed(st.code().unwrap_or(-1)))
        }
    }
    .await;
    let _ = Command::new("hdiutil").arg("detach").arg(&mount).arg("-quiet").status().await;
    let _ = tokio::fs::remove_dir(&mount).await;
    result
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
pub async fn run_installer(_path: &Path, _silent: bool) -> Result<()> {
    Err(Error::Unsupported)
}

/// Starts an installer without waiting (used for Chojema Office's own update, which then quits).
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
