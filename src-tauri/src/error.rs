//! Errors returned to the frontend as a stable code plus a detail string.

use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("network error: {0}")]
    Network(String),
    #[error("GitHub answered HTTP {0}")]
    Http(u16),
    #[error("GitHub rate limit reached")]
    RateLimited,
    #[error("no published release")]
    NoRelease,
    #[error("unknown app")]
    UnknownApp,
    #[error("no installer for this computer")]
    NoAsset,
    #[error("checksum mismatch")]
    Checksum,
    #[error("the installer was cancelled")]
    #[cfg_attr(not(windows), allow(dead_code))]
    Cancelled,
    #[error("the installer failed (exit code {0})")]
    InstallerFailed(i32),
    #[error("not installed")]
    NotInstalled,
    #[error("busy with another task")]
    Busy,
    #[error("not supported on this system")]
    Unsupported,
    #[error("{0}")]
    Io(String),
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Error::Network(_) => "network",
            Error::Http(_) => "http",
            Error::RateLimited => "rate_limited",
            Error::NoRelease => "no_release",
            Error::UnknownApp => "unknown_app",
            Error::NoAsset => "no_asset",
            Error::Checksum => "checksum",
            Error::Cancelled => "cancelled",
            Error::InstallerFailed(_) => "installer_failed",
            Error::NotInstalled => "not_installed",
            Error::Busy => "busy",
            Error::Unsupported => "unsupported",
            Error::Io(_) => "io",
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Network(e.without_url().to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e.to_string())
    }
}

impl Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("Error", 2)?;
        st.serialize_field("code", self.code())?;
        st.serialize_field("detail", &self.to_string())?;
        st.end()
    }
}

pub type Result<T> = std::result::Result<T, Error>;
