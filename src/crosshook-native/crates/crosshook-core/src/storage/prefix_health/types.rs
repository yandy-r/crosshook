use serde::{Deserialize, Serialize};

pub const DEFAULT_STALE_STAGED_TRAINER_DAYS: u64 = 14;
pub const DEFAULT_LOW_DISK_WARNING_MB: u64 = 2048;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefixReference {
    pub profile_name: String,
    pub configured_prefix_path: String,
    pub resolved_prefix_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaleStagedTrainerEntry {
    pub resolved_prefix_path: String,
    pub target_path: String,
    pub entry_name: String,
    pub total_bytes: u64,
    pub modified_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefixStorageEntry {
    pub resolved_prefix_path: String,
    pub total_bytes: u64,
    pub staged_trainers_bytes: u64,
    pub is_orphan: bool,
    pub referenced_by_profiles: Vec<String>,
    pub stale_staged_trainers: Vec<StaleStagedTrainerEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefixStorageScanResult {
    pub scanned_at: String,
    pub prefixes: Vec<PrefixStorageEntry>,
    pub orphan_targets: Vec<PrefixCleanupTarget>,
    pub stale_staged_targets: Vec<PrefixCleanupTarget>,
    pub inventory_incomplete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PrefixCleanupTargetKind {
    OrphanPrefix,
    StaleStagedTrainer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrefixCleanupTarget {
    pub kind: PrefixCleanupTargetKind,
    pub resolved_prefix_path: String,
    pub target_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefixCleanupSkipped {
    pub target: PrefixCleanupTarget,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PrefixCleanupResult {
    pub deleted: Vec<PrefixCleanupTarget>,
    pub skipped: Vec<PrefixCleanupSkipped>,
    pub reclaimed_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LowDiskWarning {
    pub mount_path: String,
    pub available_bytes: u64,
    pub threshold_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct ProfilePrefixReferences {
    pub references: Vec<PrefixReference>,
    pub profiles_load_failed: bool,
}
