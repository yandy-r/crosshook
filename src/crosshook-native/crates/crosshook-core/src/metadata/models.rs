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

/// Maps to the `launch_operations.status` TEXT column.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchOutcome {
    Started,
    Succeeded,
    Failed,
    Abandoned,
}

impl LaunchOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Abandoned => "abandoned",
        }
    }
}

/// Maps to the `launchers.drift_state` TEXT column.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftState {
    Unknown,
    Aligned,
    Missing,
    Moved,
    Stale,
}

impl DriftState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Aligned => "aligned",
            Self::Missing => "missing",
            Self::Moved => "moved",
            Self::Stale => "stale",
        }
    }
}

/// Defensive storage cap: persist at most 4 KiB of diagnostic JSON to keep
/// metadata rows bounded and reduce risk from oversized untrusted log-derived payloads.
pub const MAX_DIAGNOSTIC_JSON_BYTES: usize = 4_096;

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

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct LauncherRow {
    pub launcher_id: String,
    pub profile_id: Option<String>,
    pub launcher_slug: String,
    pub display_name: String,
    pub script_path: String,
    pub desktop_entry_path: String,
    pub drift_state: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct LaunchOperationRow {
    pub operation_id: String,
    pub profile_id: Option<String>,
    pub profile_name: Option<String>,
    pub launch_method: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub log_path: Option<String>,
    pub diagnostic_json: Option<String>,
    pub severity: Option<String>,
    pub failure_mode: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
}
