use rusqlite::Error as SqlError;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;

#[derive(Debug)]
pub enum MetadataStoreError {
    HomeDirectoryUnavailable,
    Database {
        action: &'static str,
        source: SqlError,
    },
    Io {
        action: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    Corrupt(String),
    SymlinkDetected(PathBuf),
}

impl Display for MetadataStoreError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::HomeDirectoryUnavailable => {
                write!(
                    f,
                    "home directory not found — CrossHook requires a user home directory"
                )
            }
            Self::Database { action, source } => write!(f, "failed to {action}: {source}"),
            Self::Io {
                action,
                path,
                source,
            } => write!(f, "failed to {action} '{}': {source}", path.display()),
            Self::Corrupt(message) => write!(f, "metadata database is corrupt: {message}"),
            Self::SymlinkDetected(path) => {
                write!(
                    f,
                    "refusing to open metadata database symlink: {}",
                    path.display()
                )
            }
        }
    }
}

impl Error for MetadataStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database { source, .. } => Some(source),
            Self::Io { source, .. } => Some(source),
            Self::HomeDirectoryUnavailable | Self::Corrupt(_) | Self::SymlinkDetected(_) => None,
        }
    }
}

impl From<SqlError> for MetadataStoreError {
    fn from(source: SqlError) -> Self {
        Self::Database {
            action: "run a database operation",
            source,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncSource {
    AppWrite,
    AppRename,
    AppDuplicate,
    AppDelete,
    FilesystemScan,
    Import,
    InitialCensus,
}

impl SyncSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AppWrite => "app_write",
            Self::AppRename => "app_rename",
            Self::AppDuplicate => "app_duplicate",
            Self::AppDelete => "app_delete",
            Self::FilesystemScan => "filesystem_scan",
            Self::Import => "import",
            Self::InitialCensus => "initial_census",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncReport {
    pub profiles_seen: usize,
    pub created: usize,
    pub updated: usize,
    pub deleted: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct ProfileRow {
    pub profile_id: String,
    pub current_filename: String,
    pub current_path: String,
    pub game_name: Option<String>,
    pub launch_method: Option<String>,
    pub source: Option<String>,
    pub is_favorite: bool,
    pub is_pinned: bool,
    pub created_at: String,
    pub updated_at: String,
}
