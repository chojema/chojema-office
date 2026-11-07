# Changelog

Notable changes to this project. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
versions follow [Semantic Versioning](https://semver.org/). Korean (primary): [CHANGELOG.md](CHANGELOG.md)

## [Unreleased]

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

[Unreleased]: https://github.com/chojema/chojema-office/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/chojema/chojema-office/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/chojema/chojema-office/releases/tag/v0.1.0
