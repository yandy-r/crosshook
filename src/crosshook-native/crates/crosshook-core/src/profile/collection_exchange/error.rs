//! Error type for collection preset export/import.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::metadata::MetadataStoreError;
use crate::profile::ProfileStoreError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollectionExchangeError {
    Io {
        action: String,
        path: PathBuf,
        message: String,
    },
    Toml {
        path: PathBuf,
        message: String,
    },
    InvalidManifest {
        message: String,
    },
    UnsupportedSchemaVersion {
        version: String,
        supported: String,
    },
    Metadata {
        message: String,
    },
    ProfileStore {
        message: String,
    },
}

impl Display for CollectionExchangeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                action,
                path,
                message,
            } => write!(f, "failed to {action} '{}': {message}", path.display()),
            Self::Toml { path, message } => {
                write!(
                    f,
                    "failed to parse collection preset '{}': {message}",
                    path.display()
                )
            }
            Self::InvalidManifest { message } => {
                write!(f, "invalid collection preset: {message}")
            }
            Self::UnsupportedSchemaVersion { version, supported } => write!(
                f,
                "unsupported collection preset schema version {version:?}; supported version is {supported:?}"
            ),
            Self::Metadata { message } => write!(f, "{message}"),
            Self::ProfileStore { message } => write!(f, "{message}"),
        }
    }
}

impl Error for CollectionExchangeError {}

impl From<ProfileStoreError> for CollectionExchangeError {
    fn from(value: ProfileStoreError) -> Self {
        Self::ProfileStore {
            message: value.to_string(),
        }
    }
}

impl From<MetadataStoreError> for CollectionExchangeError {
    fn from(value: MetadataStoreError) -> Self {
        Self::Metadata {
            message: value.to_string(),
        }
    }
}
