//! Finds installed copies of the suite's apps and their versions.
//!
//! Windows: the Uninstall registry keys (machine, 32-bit view and user), then `App Paths`, then the
//! usual install folders. macOS: `.app` bundles in `/Applications` and `~/Applications`.

use std::path::PathBuf;

use serde::Serialize;

use crate::catalog::AppEntry;

#[derive(Clone, Debug, Serialize)]
pub struct Installed {
    /// Version the installer registered, when known.
    pub version: Option<String>,
    /// What to start to open the app (an `.exe` on Windows, an `.app` bundle on macOS).
    pub path: Option<String>,
    /// Windows registry scope, kept internally so MSI updates retain their installation context.
    #[serde(skip)]
    #[cfg_attr(not(windows), allow(dead_code))]
    pub machine: bool,
}

/// Whether a registry `DisplayName` belongs to an app: an exact name or the name followed by a
/// space and a version (`Chojema Dooroo 0.12.1`).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn display_name_matches(display: &str, names: &[&str]) -> bool {
    let display = display.trim();
    names.iter().any(|n| {
        if display.eq_ignore_ascii_case(n) {
            return true;
        }
        match display.get(..n.len()) {
            Some(head) if head.eq_ignore_ascii_case(n) => {
                let tail = display[n.len()..].trim_start();
                display[n.len()..].starts_with(' ')
                    && tail.chars().next().is_some_and(|c| c.is_ascii_digit() || c == 'v')
            }
            _ => false,
        }
    })
}

/// Pulls the executable out of a `DisplayIcon` value such as `"C:\App\app.exe",0`.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn exe_from_display_icon(icon: &str) -> Option<PathBuf> {
    let s = icon.trim();
    let s = match s.rfind(',') {
        Some(i) if s[i + 1..].trim().trim_start_matches('-').chars().all(|c| c.is_ascii_digit()) => &s[..i],
        _ => s,
    };
    let s = s.trim().trim_matches('"');
    s.to_ascii_lowercase().ends_with(".exe").then(|| PathBuf::from(s))
}

#[cfg(windows)]
pub fn find(app: &AppEntry) -> Option<Installed> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};
    use winreg::RegKey;

    const UNINSTALL: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";
    let probe = &app.windows;
    let views = [
        (HKEY_LOCAL_MACHINE, KEY_READ | KEY_WOW64_64KEY),
        (HKEY_LOCAL_MACHINE, KEY_READ | KEY_WOW64_32KEY),
        (HKEY_CURRENT_USER, KEY_READ),
    ];

    let mut found: Option<(Option<String>, Option<PathBuf>, Option<PathBuf>)> = None;
    let mut machine = false;
    'outer: for (hive, flags) in views {
        let Ok(root) = RegKey::predef(hive).open_subkey_with_flags(UNINSTALL, flags) else { continue };
        for name in root.enum_keys().flatten() {
            let Ok(key) = root.open_subkey_with_flags(&name, flags) else { continue };
            let display: String = key.get_value("DisplayName").unwrap_or_default();
            if !display_name_matches(&display, probe.display_names) {
                continue;
            }
            let version: Option<String> =
                key.get_value("DisplayVersion").ok().filter(|v: &String| !v.trim().is_empty());
            let icon_exe = key
                .get_value::<String, _>("DisplayIcon")
                .ok()
                .and_then(|i| exe_from_display_icon(&i))
                .filter(|p| p.is_file());
            let location = key
                .get_value::<String, _>("InstallLocation")
                .ok()
                .map(|l| PathBuf::from(l.trim().trim_matches('"')))
                .filter(|p| !p.as_os_str().is_empty());
            found = Some((version, icon_exe, location));
            machine = hive == HKEY_LOCAL_MACHINE;
            break 'outer;
        }
    }

    let app_paths_exe = || -> Option<PathBuf> {
        let sub = format!(r"Software\Microsoft\Windows\CurrentVersion\App Paths\{}", probe.exe);
        [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER].into_iter().find_map(|hive| {
            let key = RegKey::predef(hive).open_subkey_with_flags(&sub, KEY_READ).ok()?;
            let v: String = key.get_value("").ok()?;
            Some(PathBuf::from(v.trim().trim_matches('"'))).filter(|p| p.is_file())
        })
    };

    let folder_exe = || -> Option<PathBuf> {
        let mut roots: Vec<PathBuf> = Vec::new();
        for var in ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(v) = std::env::var_os(var) {
                roots.push(PathBuf::from(v));
            }
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let local = PathBuf::from(local);
            roots.push(local.join("Programs"));
            roots.push(local);
        }
        roots.iter().find_map(|r| probe.folders.iter().map(|f| r.join(f).join(probe.exe)).find(|p| p.is_file()))
    };

    let (version, icon_exe, location) = match found {
        Some(f) => f,
        None => {
            // Not registered (for example unzipped by hand): still openable if the program is there.
            let exe = app_paths_exe().or_else(folder_exe)?;
            return Some(Installed { version: None, path: Some(exe.to_string_lossy().into_owned()), machine: false });
        }
    };

    let exe = icon_exe
        .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(probe.exe)))
        .or_else(|| location.as_ref().map(|l| l.join(probe.exe)).filter(|p| p.is_file()))
        .or_else(app_paths_exe)
        .or_else(folder_exe);

    Some(Installed { version, path: exe.map(|p| p.to_string_lossy().into_owned()), machine })
}

#[cfg(target_os = "macos")]
pub fn find(app: &AppEntry) -> Option<Installed> {
    let mut roots = vec![PathBuf::from("/Applications")];
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(home).join("Applications"));
    }
    let bundle = roots.iter().find_map(|r| {
        app.mac_bundles.iter().map(|b| r.join(format!("{b}.app"))).find(|p| p.join("Contents/Info.plist").is_file())
    })?;
    from_bundle(&bundle)
}

#[cfg(target_os = "macos")]
pub fn from_bundle(bundle: &std::path::Path) -> Option<Installed> {
    let dict = plist::Value::from_file(bundle.join("Contents/Info.plist")).ok()?.into_dictionary()?;
    let executable = dict.get("CFBundleExecutable")?.as_string()?;
    if !bundle.join("Contents/MacOS").join(executable).is_file() {
        return None;
    }
    let version = dict
        .get("CFBundleShortVersionString")
        .or_else(|| dict.get("CFBundleVersion"))
        .and_then(|s| s.as_string().map(str::to_string));
    Some(Installed { version, path: Some(bundle.to_string_lossy().into_owned()), machine: false })
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn find(_app: &AppEntry) -> Option<Installed> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_display_names() {
        assert!(display_name_matches("Chojema Dooroo 0.12.1", &["Chojema Dooroo"]));
        assert!(display_name_matches("WordCraft", &["WordCraft"]));
        assert!(display_name_matches("Chojema Image+", &["Chojema Image+"]));
        assert!(!display_name_matches("DesignCraft (Craft Apps)", &["DesignCraft"]));
        assert!(!display_name_matches("WordCraft Helper", &["WordCraft"]));
        assert!(!display_name_matches("Chojema Editor Pro", &["Chojema Editor"]));
    }

    #[test]
    fn parses_display_icon() {
        assert_eq!(
            exe_from_display_icon(r"d:\Program Files\ChojemaDooroo\ChojemaDooroo.exe,0"),
            Some(PathBuf::from(r"d:\Program Files\ChojemaDooroo\ChojemaDooroo.exe"))
        );
        assert_eq!(
            exe_from_display_icon(r#""C:\Program Files\App\app.exe",0"#),
            Some(PathBuf::from(r"C:\Program Files\App\app.exe"))
        );
        assert_eq!(exe_from_display_icon(r"d:\Program Files\Chojema Editor\uninstallerIcon.ico"), None);
    }
}
