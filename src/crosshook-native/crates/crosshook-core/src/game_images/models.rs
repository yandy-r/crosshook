use std::fmt;

use serde::{Deserialize, Serialize};

/// Supported image types for a game entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameImageType {
    Cover,
    Hero,
    Capsule,
}

impl fmt::Display for GameImageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cover => write!(f, "cover"),
            Self::Hero => write!(f, "hero"),
            Self::Capsule => write!(f, "capsule"),
        }
    }
}

/// The upstream source that supplied an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameImageSource {
    SteamCdn,
    SteamGridDb,
}

impl fmt::Display for GameImageSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SteamCdn => write!(f, "steam_cdn"),
            Self::SteamGridDb => write!(f, "steamgriddb"),
        }
    }
}

/// Errors that can occur during image download and cache operations.
#[derive(Debug)]
pub enum GameImageError {
    /// The `app_id` contains characters other than ASCII digits.
    InvalidAppId,
    /// The filename component contains path separators.
    InvalidFilename,
    /// The resolved path escaped the expected cache base directory.
    PathEscaped,
    /// `canonicalize` or a path operation produced no valid parent.
    InvalidPath,
    /// The downloaded content exceeds the 5 MB size limit.
    TooLarge,
    /// The magic bytes indicate a format outside the allow-list.
    ForbiddenFormat(String),
    /// An I/O error occurred (directory creation, file write, etc.).
    Io(std::io::Error),
    /// An HTTP / network error occurred.
    Network(reqwest::Error),
    /// The HTTP client could not be constructed.
    ClientBuild(reqwest::Error),
    /// The metadata store operation failed.
    Store(String),
}

impl fmt::Display for GameImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAppId => write!(f, "app_id must be a non-empty decimal integer"),
            Self::InvalidFilename => write!(f, "filename must be a plain basename with no path separators"),
            Self::PathEscaped => write!(f, "constructed cache path escaped the expected base directory"),
            Self::InvalidPath => write!(f, "could not compute parent of the constructed cache path"),
            Self::TooLarge => write!(f, "image response exceeds the 5 MB size limit"),
            Self::ForbiddenFormat(mime) => write!(f, "image format '{mime}' is not permitted (allowed: jpeg, png, webp)"),
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::Network(error) => write!(f, "network error: {error}"),
            Self::ClientBuild(error) => write!(f, "failed to build HTTP client: {error}"),
            Self::Store(msg) => write!(f, "metadata store error: {msg}"),
        }
    }
}

impl std::error::Error for GameImageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Network(error) => Some(error),
            Self::ClientBuild(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for GameImageError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<reqwest::Error> for GameImageError {
    fn from(error: reqwest::Error) -> Self {
        Self::Network(error)
    }
}
