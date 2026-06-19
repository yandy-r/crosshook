use crate::profile::ProfileStoreError;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LutrisImportError {
    Io {
        action: String,
        path: PathBuf,
        message: String,
    },
    Yaml {
        path: PathBuf,
        message: String,
    },
    Pga {
        message: String,
    },
    ProfileStore {
        message: String,
    },
}

impl Display for LutrisImportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                action,
                path,
                message,
            } => write!(f, "failed to {action} '{}': {message}", path.display()),
            Self::Yaml { path, message } => {
                write!(
                    f,
                    "failed to parse Lutris config '{}': {message}",
                    path.display()
                )
            }
            Self::Pga { message } => write!(f, "failed to read Lutris pga.db: {message}"),
            Self::ProfileStore { message } => write!(f, "{message}"),
        }
    }
}

impl Error for LutrisImportError {}

impl From<ProfileStoreError> for LutrisImportError {
    fn from(value: ProfileStoreError) -> Self {
        Self::ProfileStore {
            message: value.to_string(),
        }
    }
}
