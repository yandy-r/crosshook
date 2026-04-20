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
            Self::ProfileStore { message } => write!(f, "{message}"),
        }
    }
}

impl Error for CommunityExchangeError {}

impl From<ProfileStoreError> for CommunityExchangeError {
    fn from(value: ProfileStoreError) -> Self {
        Self::ProfileStore {
            message: value.to_string(),
        }
    }
}
