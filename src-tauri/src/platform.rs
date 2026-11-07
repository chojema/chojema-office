//! This computer's OS and CPU, the release asset that fits it, and version comparison.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    Windows,
    Macos,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Arch {
    X64,
    Arm64,
    X86,
    Other,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Platform {
    pub os: Os,
    pub arch: Arch,
}

impl Platform {
    pub fn current() -> Self {
        let os = match std::env::consts::OS {
            "windows" => Os::Windows,
            "macos" => Os::Macos,
            _ => Os::Other,
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => Arch::X64,
            "aarch64" => Arch::Arm64,
            "x86" => Arch::X86,
            _ => Arch::Other,
        };
        Self { os, arch }
    }

    /// Whether Chojema Office can install apps here (it can always open release pages).
    pub fn can_install(&self) -> bool {
        matches!(self.os, Os::Windows | Os::Macos) && self.arch != Arch::Other
    }
}

/// Architecture tokens as they appear in asset names, lower-case.
fn arch_tokens(arch: Arch) -> &'static [&'static str] {
    match arch {
        Arch::X64 => &["x64", "x86_64", "amd64"],
        Arch::Arm64 => &["arm64", "aarch64"],
        Arch::X86 => &["x86", "ia32", "i686"],
        Arch::Other => &[],
    }
}

/// Splits a file name into tokens on `-`, `_`, `.` and spaces, keeping `x86_64` whole.
fn tokens(name: &str) -> Vec<String> {
    let lower = name.to_ascii_lowercase().replace("x86_64", "x86~64");
    lower.split(['-', '_', '.', ' ']).filter(|t| !t.is_empty()).map(|t| t.replace("x86~64", "x86_64")).collect()
}

/// Scores an asset for this platform; `None` when it does not fit at all.
fn score(name: &str, platform: Platform) -> Option<i32> {
    let lower = name.to_ascii_lowercase();
    let toks = tokens(name);
    let has = |t: &str| toks.iter().any(|x| x == t);

    let mut score = match platform.os {
        Os::Windows => {
            if lower.ends_with(".exe") {
                // Only installers: "Setup", "setup" or an NSIS-style name.
                if !(lower.contains("setup") || lower.contains("install")) {
                    return None;
                }
                20
            } else if lower.ends_with(".msi") {
                20
            } else {
                return None;
            }
        }
        Os::Macos => {
            if !lower.ends_with(".dmg") {
                return None;
            }
            20
        }
        Os::Other => return None,
    };

    if has("portable") || has("cli") || has("web") || lower.contains("blockmap") || lower.contains("uninstall") {
        return None;
    }

    let mine = arch_tokens(platform.arch);
    let others: Vec<&str> = [Arch::X64, Arch::Arm64, Arch::X86]
        .into_iter()
        .filter(|a| *a != platform.arch)
        .flat_map(arch_tokens)
        .copied()
        .collect();

    if has("universal") {
        score += 8;
    } else if mine.iter().any(|t| has(t)) {
        score += 10;
    } else if others.iter().any(|t| has(t)) {
        return None;
    } else {
        // No architecture in the name: a multi-arch installer, accepted but ranked lower.
        score += 2;
    }
    Some(score)
}

/// Picks the best asset name for `platform` from `names`.
pub fn pick_asset<'a>(names: impl IntoIterator<Item = &'a str>, platform: Platform) -> Option<&'a str> {
    names.into_iter().filter_map(|n| score(n, platform).map(|s| (s, n))).max_by_key(|(s, _)| *s).map(|(_, n)| n)
}

/// Parses a version leniently: `v1.2`, `1.2.3`, `Chojema Dooroo 0.12.2`, `0.20.1.0`.
pub fn parse_version(raw: &str) -> Option<semver::Version> {
    let start = raw.find(|c: char| c.is_ascii_digit())?;
    let rest = &raw[start..];
    let end =
        rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '+')).unwrap_or(rest.len());
    let core = &rest[..end];
    if let Ok(v) = semver::Version::parse(core) {
        return Some(v);
    }
    let (numbers, pre) = match core.split_once('-') {
        Some((n, p)) => (n, Some(p)),
        None => (core, None),
    };
    let mut parts = numbers.split('.').map(|p| p.parse::<u64>().ok());
    let major = parts.next()??;
    let minor = parts.next().flatten().unwrap_or(0);
    let patch = parts.next().flatten().unwrap_or(0);
    let mut v = semver::Version::new(major, minor, patch);
    if let Some(p) = pre {
        v.pre = semver::Prerelease::new(p).unwrap_or(semver::Prerelease::EMPTY);
    }
    Some(v)
}

/// Whether `latest` is newer than `installed`. Unknown versions never ask for an update.
/// (The frontend decides what to show; this is the reference the tests pin down.)
#[cfg(test)]
pub fn is_newer(latest: &str, installed: &str) -> bool {
    match (parse_version(latest), parse_version(installed)) {
        (Some(l), Some(i)) => l > i,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIN_X64: Platform = Platform { os: Os::Windows, arch: Arch::X64 };
    const WIN_ARM: Platform = Platform { os: Os::Windows, arch: Arch::Arm64 };
    const MAC_ARM: Platform = Platform { os: Os::Macos, arch: Arch::Arm64 };
    const MAC_X64: Platform = Platform { os: Os::Macos, arch: Arch::X64 };

    #[test]
    fn picks_electron_nsis_installer() {
        let names = [
            "ChojemaDooroo-0.12.2-arm64.dmg",
            "ChojemaDooroo-0.12.2-arm64.zip",
            "ChojemaDooroo-Setup-0.12.2-x64.exe",
            "ChojemaDooroo-Setup-0.12.2-x64.exe.blockmap",
            "latest.yml",
            "SHA256SUMS.txt",
        ];
        assert_eq!(pick_asset(names, WIN_X64), Some("ChojemaDooroo-Setup-0.12.2-x64.exe"));
        assert_eq!(pick_asset(names, MAC_ARM), Some("ChojemaDooroo-0.12.2-arm64.dmg"));
        assert_eq!(pick_asset(names, MAC_X64), None);
        assert_eq!(pick_asset(names, WIN_ARM), None);
    }

    #[test]
    fn prefers_arch_specific_over_multi_arch() {
        let names = [
            "Chojema.Editor-0.8.1-arm64.dmg",
            "Chojema.Editor-Setup-0.8.1-arm64.exe",
            "Chojema.Editor-Setup-0.8.1-x64.exe",
            "Chojema.Editor-Setup-0.8.1.exe",
        ];
        assert_eq!(pick_asset(names, WIN_X64), Some("Chojema.Editor-Setup-0.8.1-x64.exe"));
        assert_eq!(pick_asset(names, WIN_ARM), Some("Chojema.Editor-Setup-0.8.1-arm64.exe"));
    }

    #[test]
    fn picks_msi_and_universal_dmg() {
        let names = [
            "pdfcraft-0.19.0-macos-universal.dmg",
            "pdfcraft-0.19.0-windows-arm64-portable.zip",
            "pdfcraft-0.19.0-windows-arm64.msi",
            "pdfcraft-0.19.0-windows-x64-portable.zip",
            "pdfcraft-0.19.0-windows-x64.msi",
            "pdfcraft-0.19.0-windows-x86.msi",
            "pdfcraft-cli-0.19.0-macos-universal.zip",
            "pdfcraft-0.19.0-linux-x86_64.deb",
        ];
        assert_eq!(pick_asset(names, WIN_X64), Some("pdfcraft-0.19.0-windows-x64.msi"));
        assert_eq!(pick_asset(names, WIN_ARM), Some("pdfcraft-0.19.0-windows-arm64.msi"));
        assert_eq!(pick_asset(names, MAC_X64), Some("pdfcraft-0.19.0-macos-universal.dmg"));
    }

    #[test]
    fn picks_tauri_installers() {
        let names = ["Chojema-KeyStamp_0.3.0_universal.dmg", "Chojema-KeyStamp_0.3.0_x64-setup.exe"];
        assert_eq!(pick_asset(names, WIN_X64), Some("Chojema-KeyStamp_0.3.0_x64-setup.exe"));
        assert_eq!(pick_asset(names, MAC_ARM), Some("Chojema-KeyStamp_0.3.0_universal.dmg"));
    }

    #[test]
    fn compares_versions_leniently() {
        assert!(is_newer("v0.12.2", "0.12.1"));
        assert!(is_newer("0.20.1", "0.20"));
        assert!(!is_newer("0.8.1", "0.8.1"));
        assert!(!is_newer("1.0.0-beta.1", "1.0.0"));
        assert!(is_newer("Chojema Dooroo 0.13.0", "0.12.9"));
        assert!(!is_newer("unknown", "0.1.0"));
        assert_eq!(parse_version("0.20.1.0").map(|v| v.to_string()), Some("0.20.1".into()));
    }
}
