//! The apps Chojema Office knows about.
//!
//! The catalog is plain data. The frontend owns every user-facing text (both languages), so this
//! file only carries what the backend needs: where releases live and how to find an installed copy.

use serde::Serialize;

/// Who makes an app. Partner apps are made by other developers and are always shown with their
/// publisher and a link to their own repository.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Maker {
    Chojema,
    Partner,
}

/// How to find an installed copy on Windows.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(not(windows), allow(dead_code))]
pub struct WinProbe {
    /// Exact `DisplayName` values (or prefixes followed by a space and a version) in the
    /// Uninstall registry keys.
    pub display_names: &'static [&'static str],
    /// Executable file name of the app (also looked up under `App Paths`).
    pub exe: &'static str,
    /// Folder names to try under Program Files and `%LOCALAPPDATA%\Programs` / `%LOCALAPPDATA%`.
    pub folders: &'static [&'static str],
}

/// One app in the suite.
#[derive(Clone, Copy, Debug)]
pub struct AppEntry {
    /// Machine id used by the frontend (`dooroo`).
    pub id: &'static str,
    /// Display name, kept the same in every language (`Chojema Dooroo`).
    pub name: &'static str,
    /// GitHub `owner/repo`.
    pub repo: &'static str,
    /// Who publishes it, as shown on the card.
    pub publisher: &'static str,
    pub maker: Maker,
    #[cfg_attr(not(windows), allow(dead_code))]
    pub windows: WinProbe,
    /// `.app` bundle names to look for in `/Applications` and `~/Applications`.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub mac_bundles: &'static [&'static str],
}

impl AppEntry {
    pub fn by_id(id: &str) -> Option<&'static AppEntry> {
        SUITE.iter().chain(std::iter::once(&SELF_APP)).find(|a| a.id == id)
    }

    pub fn repo_url(&self) -> String {
        format!("https://github.com/{}", self.repo)
    }

    pub fn releases_url(&self) -> String {
        format!("https://github.com/{}/releases", self.repo)
    }
}

/// The suite, in display order.
pub const SUITE: [AppEntry; 8] = [
    AppEntry {
        id: "dooroo",
        name: "Chojema Dooroo",
        repo: "chojema/ChojemaDooroo",
        publisher: "Chulwoo Park",
        maker: Maker::Chojema,
        windows: WinProbe {
            display_names: &["Chojema Dooroo"],
            exe: "ChojemaDooroo.exe",
            folders: &["ChojemaDooroo", "Chojema Dooroo"],
        },
        mac_bundles: &["Chojema Dooroo", "ChojemaDooroo"],
    },
    AppEntry {
        id: "wordcraft",
        name: "WordCraft",
        repo: "storytold/wordcraft",
        publisher: "ArtCraft Team",
        maker: Maker::Partner,
        windows: WinProbe {
            display_names: &["WordCraft"],
            exe: "wordcraft.exe",
            folders: &["WordCraft", "ArtCraft\\WordCraft"],
        },
        mac_bundles: &["WordCraft"],
    },
    AppEntry {
        id: "gridcraft",
        name: "GridCraft",
        repo: "storytold/gridcraft",
        publisher: "ArtCraft Team",
        maker: Maker::Partner,
        windows: WinProbe {
            display_names: &["GridCraft"],
            exe: "gridcraft.exe",
            folders: &["GridCraft", "ArtCraft\\GridCraft"],
        },
        mac_bundles: &["GridCraft"],
    },
    AppEntry {
        id: "deckcraft",
        name: "DeckCraft",
        repo: "storytold/deckcraft",
        publisher: "ArtCraft Team",
        maker: Maker::Partner,
        windows: WinProbe {
            display_names: &["DeckCraft"],
            exe: "deckcraft.exe",
            folders: &["DeckCraft", "ArtCraft\\DeckCraft"],
        },
        mac_bundles: &["DeckCraft"],
    },
    AppEntry {
        id: "pdfcraft",
        name: "PdfCraft",
        repo: "storytold/pdfcraft",
        publisher: "ArtCraft Team",
        maker: Maker::Partner,
        windows: WinProbe {
            display_names: &["PdfCraft", "PrintCraft"],
            exe: "pdfcraft.exe",
            folders: &["PdfCraft", "ArtCraft\\PdfCraft"],
        },
        mac_bundles: &["PdfCraft"],
    },
    AppEntry {
        id: "editor",
        name: "Chojema Editor",
        repo: "chojema/chojema-editor",
        publisher: "Chulwoo Park",
        maker: Maker::Chojema,
        windows: WinProbe {
            display_names: &["Chojema Editor"],
            exe: "Chojema Editor.exe",
            folders: &["Chojema Editor"],
        },
        mac_bundles: &["Chojema Editor"],
    },
    AppEntry {
        id: "imageplus",
        name: "Chojema Image+",
        repo: "chojema/ChojemaImagePlus",
        publisher: "Chulwoo Park",
        maker: Maker::Chojema,
        windows: WinProbe {
            display_names: &["Chojema Image+"],
            exe: "chojema-image-plus.exe",
            folders: &["Chojema Image+"],
        },
        mac_bundles: &["Chojema Image+"],
    },
    AppEntry {
        id: "keystamp",
        name: "Chojema KeyStamp",
        repo: "chojema/chojema-keystamp",
        publisher: "Chulwoo Park",
        maker: Maker::Chojema,
        windows: WinProbe {
            display_names: &["Chojema KeyStamp"],
            exe: "chojema-keystamp.exe",
            folders: &["Chojema KeyStamp"],
        },
        mac_bundles: &["Chojema KeyStamp"],
    },
];

/// Chojema Office itself, used for its own update check. Not shown as a card.
pub const SELF_APP: AppEntry = AppEntry {
    id: "chojema-office",
    name: "Chojema Office",
    repo: "chojema/chojema-office",
    publisher: "Chulwoo Park",
    maker: Maker::Chojema,
    windows: WinProbe { display_names: &["Chojema Office"], exe: "chojema-office.exe", folders: &["Chojema Office"] },
    mac_bundles: &["Chojema Office"],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_resolvable() {
        let mut ids: Vec<_> = SUITE.iter().map(|a| a.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), SUITE.len());
        for app in &SUITE {
            assert_eq!(AppEntry::by_id(app.id).map(|a| a.repo), Some(app.repo));
        }
        assert!(AppEntry::by_id("chojema-office").is_some());
        assert!(AppEntry::by_id("nope").is_none());
    }

    #[test]
    fn partner_apps_name_their_publisher() {
        for app in SUITE.iter().filter(|a| a.maker == Maker::Partner) {
            assert_ne!(app.publisher, "Chulwoo Park");
        }
    }
}
