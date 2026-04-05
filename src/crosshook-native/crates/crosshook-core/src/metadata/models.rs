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
    Validation(String),
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
            Self::Validation(msg) => write!(f, "metadata validation error: {msg}"),
        }
    }
}

impl Error for MetadataStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database { source, .. } => Some(source),
            Self::Io { source, .. } => Some(source),
            Self::HomeDirectoryUnavailable
            | Self::Corrupt(_)
            | Self::SymlinkDetected(_)
            | Self::Validation(_) => None,
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
    AppMigration,
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
            Self::AppMigration => "app_migration",
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

/// Defensive storage cap: persist at most 512 KiB of external cache payload JSON to keep
/// cache rows bounded and reduce risk from oversized untrusted remote payloads.
pub const MAX_CACHE_PAYLOAD_BYTES: usize = 524_288;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncReport {
    pub profiles_seen: usize,
    pub created: usize,
    pub updated: usize,
    pub deleted: usize,
    pub errors: Vec<String>,
}

/// Row from `bundled_optimization_presets` (GPU vendor presets shipped with the app).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundledOptimizationPresetRow {
    pub preset_id: String,
    pub display_name: String,
    pub vendor: String,
    pub mode: String,
    /// JSON array of launch optimization option ids, e.g. `["enable_nvapi"]`.
    pub option_ids_json: String,
    pub catalog_version: i64,
}

/// Origin of a named `[launch.presets]` entry tracked in `profile_launch_preset_metadata`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileLaunchPresetOrigin {
    Bundled,
    User,
}

impl ProfileLaunchPresetOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bundled => "bundled",
            Self::User => "user",
        }
    }
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

/// Maps to the `external_cache_entries` status classification.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheEntryStatus {
    Valid,
    Stale,
    Oversized,
    Corrupt,
}

impl CacheEntryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::Stale => "stale",
            Self::Oversized => "oversized",
            Self::Corrupt => "corrupt",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct CommunityTapRow {
    pub tap_id: String,
    pub tap_url: String,
    pub tap_branch: String,
    pub local_path: String,
    pub last_head_commit: Option<String>,
    pub profile_count: i64,
    pub last_indexed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct CommunityProfileRow {
    pub id: i64,
    pub tap_id: String,
    /// Denormalized for IPC convenience — populated via JOIN in queries.
    pub tap_url: String,
    pub relative_path: String,
    pub manifest_path: String,
    pub game_name: Option<String>,
    pub game_version: Option<String>,
    pub trainer_name: Option<String>,
    pub trainer_version: Option<String>,
    pub proton_version: Option<String>,
    pub compatibility_rating: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub platform_tags: Option<String>,
    pub schema_version: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct CollectionRow {
    pub collection_id: String,
    pub name: String,
    pub description: Option<String>,
    pub profile_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct FailureTrendRow {
    pub profile_name: String,
    pub successes: i64,
    pub failures: i64,
    pub failure_modes: Option<String>,
}

/// Maximum number of version snapshot rows retained per profile.
/// Older rows beyond this limit are pruned after each insert.
pub const MAX_VERSION_SNAPSHOTS_PER_PROFILE: usize = 20;

/// Maximum number of config revision rows retained per profile.
/// Older rows beyond this limit are pruned in the same transaction as each insert.
pub const MAX_CONFIG_REVISIONS_PER_PROFILE: usize = 20;

/// Defensive storage cap: persist at most 256 KiB of TOML snapshot content per revision row.
/// Rejects oversized profiles before they reach the database.
pub const MAX_SNAPSHOT_TOML_BYTES: usize = 262_144;

/// Maximum number of revision rows returned per a single history list request.
/// Caps the `limit` parameter on `profile_config_history` to prevent large IPC payloads.
pub const MAX_HISTORY_LIST_LIMIT: usize = 50;

/// Maps to a row in the `version_snapshots` multi-row history table.
#[derive(Debug, Clone)]
pub struct VersionSnapshotRow {
    pub id: i64,
    pub profile_id: String,
    pub steam_app_id: String,
    pub steam_build_id: Option<String>,
    pub trainer_version: Option<String>,
    pub trainer_file_hash: Option<String>,
    pub human_game_ver: Option<String>,
    pub status: String,
    pub checked_at: String,
}

/// Version correlation status between a game build and its trainer binary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionCorrelationStatus {
    /// No baseline snapshot recorded yet.
    Untracked,
    /// Game build ID and trainer hash match the last recorded snapshot.
    Matched,
    /// Game build ID changed since last snapshot; trainer hash unchanged.
    GameUpdated,
    /// Trainer binary hash changed since last snapshot; game build unchanged.
    TrainerChanged,
    /// Both game build ID and trainer hash changed since last snapshot.
    BothChanged,
    /// Status cannot be determined (e.g., missing manifest or hash).
    Unknown,
    /// Steam is currently updating the game (`StateFlags != 4`).
    UpdateInProgress,
}

impl VersionCorrelationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Untracked => "untracked",
            Self::Matched => "matched",
            Self::GameUpdated => "game_updated",
            Self::TrainerChanged => "trainer_changed",
            Self::BothChanged => "both_changed",
            Self::Unknown => "unknown",
            Self::UpdateInProgress => "update_in_progress",
        }
    }
}

/// Maps to the `config_revisions.source` TEXT column.
/// Identifies which write path produced the revision snapshot.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigRevisionSource {
    ManualSave,
    LaunchOptimizationSave,
    PresetApply,
    RollbackApply,
    Import,
    Migration,
    ProtonDbSuggestionApply,
}

impl ConfigRevisionSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ManualSave => "manual_save",
            Self::LaunchOptimizationSave => "launch_optimization_save",
            Self::PresetApply => "preset_apply",
            Self::RollbackApply => "rollback_apply",
            Self::Import => "import",
            Self::Migration => "migration",
            Self::ProtonDbSuggestionApply => "protondb_suggestion_apply",
        }
    }
}

/// Maps to a row in the `prefix_storage_snapshots` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefixStorageSnapshotRow {
    pub id: String,
    pub resolved_prefix_path: String,
    pub total_bytes: i64,
    pub staged_trainers_bytes: i64,
    pub is_orphan: bool,
    pub referenced_profiles_json: String,
    pub stale_staged_count: i64,
    pub scanned_at: String,
}

/// Maps to a row in the `prefix_storage_cleanup_audit` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefixStorageCleanupAuditRow {
    pub id: String,
    pub target_kind: String,
    pub resolved_prefix_path: String,
    pub target_path: String,
    pub result: String,
    pub reason: Option<String>,
    pub reclaimed_bytes: i64,
    pub created_at: String,
}

/// Maps to a row in the `prefix_dependency_state` table.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrefixDependencyStateRow {
    pub id: i64,
    pub profile_id: String,
    pub package_name: String,
    pub prefix_path: String,
    pub state: String,
    pub checked_at: Option<String>,
    pub installed_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Maps to a row in the `config_revisions` append-only history table.
#[derive(Debug, Clone)]
pub struct ConfigRevisionRow {
    pub id: i64,
    pub profile_id: String,
    pub profile_name_at_write: String,
    pub source: String,
    pub content_hash: String,
    pub snapshot_toml: String,
    pub source_revision_id: Option<i64>,
    pub is_last_known_working: bool,
    pub created_at: String,
}
