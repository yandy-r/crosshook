use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};
use nix::sys::statvfs::statvfs;
use serde::{Deserialize, Serialize};

use crate::launch::runtime_helpers::resolve_wine_prefix_path;
use crate::profile::ProfileStore;

pub const DEFAULT_STALE_STAGED_TRAINER_DAYS: u64 = 14;
pub const DEFAULT_LOW_DISK_WARNING_MB: u64 = 2048;

const STAGED_TRAINERS_RELATIVE: &str = "drive_c/CrossHook/StagedTrainers";
const DRIVE_C_RELATIVE: &str = "drive_c";

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

pub fn collect_profile_prefix_references(store: &ProfileStore) -> Result<Vec<PrefixReference>, String> {
    let names = store
        .list()
        .map_err(|error| format!("failed to list profiles for prefix scan: {error}"))?;

    let mut references = Vec::new();
    for name in names {
        let profile = match store.load(&name) {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(profile = %name, %error, "skipping profile during prefix scan");
                continue;
            }
        };
        let effective = profile.effective_profile();
        let configured = effective.runtime.prefix_path.trim();
        if configured.is_empty() {
            continue;
        }
        let configured_path = PathBuf::from(configured);
        let resolved_path = resolve_wine_prefix_path(&configured_path);
        references.push(PrefixReference {
            profile_name: name,
            configured_prefix_path: configured_path.to_string_lossy().into_owned(),
            resolved_prefix_path: normalized_path_string(&resolved_path),
        });
    }

    Ok(references)
}

pub fn scan_prefix_storage(
    references: &[PrefixReference],
    stale_days: u64,
) -> Result<PrefixStorageScanResult, String> {
    let now = SystemTime::now();
    let stale_threshold = now
        .checked_sub(Duration::from_secs(stale_days.saturating_mul(24 * 60 * 60)))
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let referenced_by_prefix = collect_referenced_profiles(references);
    let discovered_prefixes = discover_candidate_prefixes(&referenced_by_prefix.keys().cloned().collect::<Vec<_>>());

    let mut all_prefixes = BTreeSet::new();
    all_prefixes.extend(referenced_by_prefix.keys().cloned());
    all_prefixes.extend(discovered_prefixes);

    let mut entries = Vec::new();
    let mut orphan_targets = Vec::new();
    let mut stale_targets = Vec::new();

    for resolved_prefix_path in all_prefixes {
        let prefix_path = PathBuf::from(&resolved_prefix_path);
        if !prefix_path.is_dir() || !prefix_path.join(DRIVE_C_RELATIVE).is_dir() {
            continue;
        }

        let referenced_profiles = referenced_by_prefix
            .get(&resolved_prefix_path)
            .cloned()
            .unwrap_or_default();
        let is_crosshook_managed = has_crosshook_managed_marker(&prefix_path);
        let is_orphan = referenced_profiles.is_empty() && is_crosshook_managed;
        let total_bytes = dir_size_bytes(&prefix_path);
        let (staged_trainers_bytes, stale_staged_trainers) =
            staged_trainers_health(&prefix_path, stale_threshold);

        if is_orphan {
            orphan_targets.push(PrefixCleanupTarget {
                kind: PrefixCleanupTargetKind::OrphanPrefix,
                resolved_prefix_path: resolved_prefix_path.clone(),
                target_path: resolved_prefix_path.clone(),
            });
        }

        for stale_entry in &stale_staged_trainers {
            stale_targets.push(PrefixCleanupTarget {
                kind: PrefixCleanupTargetKind::StaleStagedTrainer,
                resolved_prefix_path: stale_entry.resolved_prefix_path.clone(),
                target_path: stale_entry.target_path.clone(),
            });
        }

        entries.push(PrefixStorageEntry {
            resolved_prefix_path,
            total_bytes,
            staged_trainers_bytes,
            is_orphan,
            referenced_by_profiles: referenced_profiles,
            stale_staged_trainers,
        });
    }

    entries.sort_by(|left, right| left.resolved_prefix_path.cmp(&right.resolved_prefix_path));
    orphan_targets.sort_by(|left, right| left.target_path.cmp(&right.target_path));
    stale_targets.sort_by(|left, right| left.target_path.cmp(&right.target_path));

    Ok(PrefixStorageScanResult {
        scanned_at: Utc::now().to_rfc3339(),
        prefixes: entries,
        orphan_targets,
        stale_staged_targets: stale_targets,
    })
}

pub fn cleanup_prefix_storage(
    references: &[PrefixReference],
    targets: &[PrefixCleanupTarget],
) -> PrefixCleanupResult {
    let mut result = PrefixCleanupResult::default();
    let referenced = collect_referenced_profiles(references);

    for target in targets {
        match target.kind {
            PrefixCleanupTargetKind::OrphanPrefix => {
                cleanup_orphan_prefix(&referenced, target, &mut result);
            }
            PrefixCleanupTargetKind::StaleStagedTrainer => {
                cleanup_stale_staged_trainer(target, &mut result);
            }
        }
    }

    result
}

pub fn check_low_disk_warning(prefix_path: &Path, threshold_mb: u64) -> Result<Option<LowDiskWarning>, String> {
    let resolved_prefix = resolve_wine_prefix_path(prefix_path);
    if !resolved_prefix.exists() {
        return Ok(None);
    }

    let stats = statvfs(&resolved_prefix)
        .map_err(|error| format!("failed to query disk usage for {}: {error}", resolved_prefix.display()))?;
    let available_bytes = stats.fragment_size().saturating_mul(stats.blocks_available());
    let threshold_bytes = threshold_mb.saturating_mul(1024 * 1024);

    if available_bytes >= threshold_bytes {
        return Ok(None);
    }

    Ok(Some(LowDiskWarning {
        mount_path: normalized_path_string(&resolved_prefix),
        available_bytes,
        threshold_bytes,
    }))
}

fn collect_referenced_profiles(references: &[PrefixReference]) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for reference in references {
        let key = normalized_path_string(Path::new(&reference.resolved_prefix_path));
        if key.is_empty() {
            continue;
        }
        map.entry(key).or_default().push(reference.profile_name.clone());
    }

    for profile_names in map.values_mut() {
        profile_names.sort();
        profile_names.dedup();
    }

    map
}

fn discover_candidate_prefixes(referenced_prefixes: &[String]) -> BTreeSet<String> {
    let mut candidates = BTreeSet::new();
    for raw in referenced_prefixes {
        let prefix = PathBuf::from(raw);
        if !prefix.is_dir() {
            continue;
        }

        if let Some(parent) = prefix.parent() {
            candidates.extend(discover_prefixes_in_directory(parent));
        }

        if prefix.file_name().and_then(|value| value.to_str()) == Some("pfx") {
            if let Some(compatdata_root) = prefix.parent().and_then(Path::parent) {
                candidates.extend(discover_prefixes_in_compatdata_root(compatdata_root));
            }
        }
    }
    candidates
}

fn discover_prefixes_in_directory(directory: &Path) -> Vec<String> {
    let mut result = Vec::new();
    let entries = match fs::read_dir(directory) {
        Ok(value) => value,
        Err(_) => return result,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.join(DRIVE_C_RELATIVE).is_dir() {
            result.push(normalized_path_string(&path));
        }
    }

    result
}

fn discover_prefixes_in_compatdata_root(compatdata_root: &Path) -> Vec<String> {
    let mut result = Vec::new();
    let entries = match fs::read_dir(compatdata_root) {
        Ok(value) => value,
        Err(_) => return result,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let pfx_path = path.join("pfx");
        if pfx_path.join(DRIVE_C_RELATIVE).is_dir() {
            result.push(normalized_path_string(&pfx_path));
        }
    }

    result
}

fn staged_trainers_health(
    resolved_prefix_path: &Path,
    stale_threshold: SystemTime,
) -> (u64, Vec<StaleStagedTrainerEntry>) {
    let staged_root = resolved_prefix_path.join(STAGED_TRAINERS_RELATIVE);
    if !staged_root.is_dir() {
        return (0, Vec::new());
    }
    if fs::symlink_metadata(&staged_root)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(true)
    {
        tracing::warn!(
            path = %staged_root.display(),
            "skipping staged trainers scan because root is a symlink"
        );
        return (0, Vec::new());
    }

    let mut total_bytes = 0u64;
    let mut stale_entries = Vec::new();
    let entries = match fs::read_dir(&staged_root) {
        Ok(value) => value,
        Err(_) => return (0, Vec::new()),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_symlink() {
            continue;
        }
        let bytes = file_or_dir_size_bytes(&path);
        total_bytes = total_bytes.saturating_add(bytes);

        let modified_at = fs::metadata(&path).ok().and_then(|meta| meta.modified().ok());
        let is_stale = modified_at.map(|time| time <= stale_threshold).unwrap_or(false);
        if !is_stale {
            continue;
        }

        stale_entries.push(StaleStagedTrainerEntry {
            resolved_prefix_path: normalized_path_string(resolved_prefix_path),
            target_path: normalized_path_string(&path),
            entry_name: entry.file_name().to_string_lossy().into_owned(),
            total_bytes: bytes,
            modified_at: modified_at.map(system_time_to_rfc3339),
        });
    }

    stale_entries.sort_by(|left, right| left.target_path.cmp(&right.target_path));
    (total_bytes, stale_entries)
}

fn cleanup_orphan_prefix(
    referenced: &BTreeMap<String, Vec<String>>,
    target: &PrefixCleanupTarget,
    result: &mut PrefixCleanupResult,
) {
    let resolved_prefix_key = normalized_path_string(Path::new(&target.resolved_prefix_path));
    if referenced.contains_key(&resolved_prefix_key) {
        result.skipped.push(PrefixCleanupSkipped {
            target: target.clone(),
            reason: "prefix is still referenced by at least one profile".to_string(),
        });
        return;
    }

    let target_path = PathBuf::from(&target.target_path);
    if !target_path.is_dir() {
        result.skipped.push(PrefixCleanupSkipped {
            target: target.clone(),
            reason: "target no longer exists".to_string(),
        });
        return;
    }
    if !target_path.join(DRIVE_C_RELATIVE).is_dir() {
        result.skipped.push(PrefixCleanupSkipped {
            target: target.clone(),
            reason: "target does not look like a wine prefix (missing drive_c)".to_string(),
        });
        return;
    }
    if !has_crosshook_managed_marker(&target_path) {
        result.skipped.push(PrefixCleanupSkipped {
            target: target.clone(),
            reason: "prefix is not marked as crosshook-managed".to_string(),
        });
        return;
    }

    let canonical_target = match fs::canonicalize(&target_path) {
        Ok(value) => value,
        Err(error) => {
            result.skipped.push(PrefixCleanupSkipped {
                target: target.clone(),
                reason: format!("failed to canonicalize target prefix path: {error}"),
            });
            return;
        }
    };
    if normalized_path_string(&canonical_target) != resolved_prefix_key {
        result.skipped.push(PrefixCleanupSkipped {
            target: target.clone(),
            reason: "target path does not match resolved prefix path".to_string(),
        });
        return;
    }

    let bytes_before = dir_size_bytes(&canonical_target);
    match fs::remove_dir_all(&canonical_target) {
        Ok(()) => {
            result.deleted.push(target.clone());
            result.reclaimed_bytes = result.reclaimed_bytes.saturating_add(bytes_before);
        }
        Err(error) => {
            result.skipped.push(PrefixCleanupSkipped {
                target: target.clone(),
                reason: format!("failed to delete orphan prefix: {error}"),
            });
        }
    }
}

fn cleanup_stale_staged_trainer(target: &PrefixCleanupTarget, result: &mut PrefixCleanupResult) {
    let resolved_prefix_path = PathBuf::from(&target.resolved_prefix_path);
    let staged_root = resolved_prefix_path.join(STAGED_TRAINERS_RELATIVE);
    let target_path = PathBuf::from(&target.target_path);

    if !target_path.exists() {
        result.skipped.push(PrefixCleanupSkipped {
            target: target.clone(),
            reason: "staged trainer entry no longer exists".to_string(),
        });
        return;
    }
    if fs::symlink_metadata(&staged_root)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(true)
    {
        result.skipped.push(PrefixCleanupSkipped {
            target: target.clone(),
            reason: "staged trainers root is a symlink; refusing cleanup".to_string(),
        });
        return;
    }

    let canonical_staged_root = match fs::canonicalize(&staged_root) {
        Ok(value) => value,
        Err(error) => {
            result.skipped.push(PrefixCleanupSkipped {
                target: target.clone(),
                reason: format!("failed to canonicalize staged trainers root: {error}"),
            });
            return;
        }
    };
    let canonical_prefix = match fs::canonicalize(&resolved_prefix_path) {
        Ok(value) => value,
        Err(error) => {
            result.skipped.push(PrefixCleanupSkipped {
                target: target.clone(),
                reason: format!("failed to canonicalize resolved prefix path: {error}"),
            });
            return;
        }
    };
    if !canonical_staged_root.starts_with(&canonical_prefix) {
        result.skipped.push(PrefixCleanupSkipped {
            target: target.clone(),
            reason: "staged trainers root escapes resolved prefix path".to_string(),
        });
        return;
    }
    let canonical_target = match fs::canonicalize(&target_path) {
        Ok(value) => value,
        Err(error) => {
            result.skipped.push(PrefixCleanupSkipped {
                target: target.clone(),
                reason: format!("failed to canonicalize staged trainer entry: {error}"),
            });
            return;
        }
    };

    if !canonical_target.starts_with(&canonical_staged_root) {
        result.skipped.push(PrefixCleanupSkipped {
            target: target.clone(),
            reason: "target path is outside staged trainers root".to_string(),
        });
        return;
    }

    let bytes_before = file_or_dir_size_bytes(&canonical_target);
    let deletion = if canonical_target.is_dir() {
        fs::remove_dir_all(&canonical_target)
    } else {
        fs::remove_file(&canonical_target)
    };

    match deletion {
        Ok(()) => {
            result.deleted.push(target.clone());
            result.reclaimed_bytes = result.reclaimed_bytes.saturating_add(bytes_before);
        }
        Err(error) => {
            result.skipped.push(PrefixCleanupSkipped {
                target: target.clone(),
                reason: format!("failed to delete staged trainer entry: {error}"),
            });
        }
    }
}

fn dir_size_bytes(path: &Path) -> u64 {
    let mut total = 0u64;
    let entries = match fs::read_dir(path) {
        Ok(value) => value,
        Err(_) => return 0,
    };

    for entry in entries.flatten() {
        let child = entry.path();
        total = total.saturating_add(file_or_dir_size_bytes(&child));
    }

    total
}

fn file_or_dir_size_bytes(path: &Path) -> u64 {
    let metadata = match fs::symlink_metadata(path) {
        Ok(value) => value,
        Err(_) => return 0,
    };

    if metadata.file_type().is_symlink() {
        return 0;
    }
    if metadata.is_file() {
        return metadata.len();
    }
    if metadata.is_dir() {
        return dir_size_bytes(path);
    }
    0
}

fn normalized_path_string(path: &Path) -> String {
    match fs::canonicalize(path) {
        Ok(value) => value.to_string_lossy().into_owned(),
        Err(_) => path.to_string_lossy().into_owned(),
    }
}

fn system_time_to_rfc3339(time: SystemTime) -> String {
    let dt: DateTime<Utc> = time.into();
    dt.to_rfc3339()
}

fn has_crosshook_managed_marker(prefix_path: &Path) -> bool {
    prefix_path.join("drive_c/CrossHook").is_dir()
}

