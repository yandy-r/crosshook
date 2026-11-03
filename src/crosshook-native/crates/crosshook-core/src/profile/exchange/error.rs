use crate::profile::ProfileStoreError;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommunityExchangeError {
    Io {
        action: String,
        path: PathBuf,
        message: String,
    },
    Json {
        path: PathBuf,
        message: String,
    },
    InvalidManifest {
        message: String,
    },
    UnsupportedSchemaVersion {
        version: u32,
        supported: u32,
    },
    /// A profile store write found the target name already taken.
    AlreadyExists {
        name: String,
    },
    /// No free profile name was left for the import: `<base>` and every
    /// `<base>-2` … `<base>-<max_suffix>` are taken.
    NoFreeName {
        base: String,
        max_suffix: u32,
    },
    ProfileStore {
        message: String,
    },
}

impl Display for CommunityExchangeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                action,
                path,
                message,
            } => write!(f, "failed to {action} '{}': {message}", path.display()),
            Self::Json { path, message } => {
                write!(
                    f,
                    "failed to parse community profile '{}': {message}",
                    path.display()
                )
            }
            Self::InvalidManifest { message } => write!(f, "invalid community profile: {message}"),
            Self::UnsupportedSchemaVersion { version, supported } => {
                write!(
                    f,
                    "unsupported community profile schema version {version}; supported version is {supported}"
                )
            }
            Self::AlreadyExists { name } => write!(f, "a profile named '{name}' already exists"),
            Self::NoFreeName { base, max_suffix } => write!(
                f,
                "no free profile name for '{base}' (tried up to '{base}-{max_suffix}')"
            ),
            Self::ProfileStore { message } => write!(f, "{message}"),
        }
    }
}

impl Error for CommunityExchangeError {}

impl From<ProfileStoreError> for CommunityExchangeError {
    fn from(value: ProfileStoreError) -> Self {
        match value {
            ProfileStoreError::AlreadyExists(name) => Self::AlreadyExists { name },
            other => Self::ProfileStore {
                message: other.to_string(),
            },
        }
    }
}
