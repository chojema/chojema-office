# Changelog

Notable changes to this project. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
versions follow [Semantic Versioning](https://semver.org/). Korean (primary): [CHANGELOG.md](CHANGELOG.md)

## [Unreleased]

## [0.2.2] - 2026-11-10

### Fixed

- macOS app updates stage the new bundle before replacement, restore the previous app if the swap fails, and retain the existing installation location
- Windows MSI updates preserve the scope of existing all-users installations
- Installation and update completion is reported only after verifying the installed app and version

## [0.2.1] - 2026-11-07

### Fixed

- Hitting the GitHub request limit silently showed old information; a notice now says so and when you can check again
- Chojema Office notices and installs its own new version even when the request limit is used up (it reads the latest tag from the GitHub release page)
- On macOS, "Update and restart" only opened the disk image; it now replaces the app and reopens it

### Changed

- The automatic check at start-up does not ask again about apps checked in the last 10 minutes, to save requests ("Refresh" always checks)

## [0.2.0] - 2026-11-07

### Added

- "Open at sign-in" setting (bottom left, off by default; Windows startup apps, macOS login items)
- The "About & links" dialog links to the GitHub repository, Chojema App Park and channels (blog, YouTube, X, Instagram, GitHub, homepage)

### Changed

- Redesigned the "About & links" dialog (project, channels, partner apps and credits)
- Refined the Chojema Editor and DeckCraft descriptions

### Fixed

- "1 updates" in the English summary when exactly one update is available

## [0.1.0] - 2026-11-07

First public release.

### Added

- Manage Chojema Dooroo, WordCraft, GridCraft, DeckCraft, PdfCraft, Chojema Editor, Chojema Image+ and Chojema KeyStamp from one window
- Latest GitHub release of each app (ETag cache to stay under the rate limit; last known data when offline)
- Detects installed apps and their versions (Windows registry, macOS Applications folders)
- Picks the installer for this computer's OS and CPU, verifies its SHA-256 checksum, then installs or updates
- "Update all" for every app with an update
- Drawers (Writing, Spreadsheets, Presenting, PDF, Images & tools) and views (Installed, Updates, Not installed)
- Details panel: versions, file to download, integrity check, install location and release notes
- `Ctrl K` quick-launch palette to find, open or install an app
- Korean (default) and English interface; light, dark or system theme
- Update notice and installer for Chojema Office itself

[Unreleased]: https://github.com/chojema/chojema-office/compare/v0.2.2...HEAD
[0.2.2]: https://github.com/chojema/chojema-office/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/chojema/chojema-office/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/chojema/chojema-office/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/chojema/chojema-office/releases/tag/v0.1.0
