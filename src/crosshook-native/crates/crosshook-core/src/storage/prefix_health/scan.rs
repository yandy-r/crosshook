use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use chrono::Utc;

use super::constants::DRIVE_C_RELATIVE;
use super::discovery::{collect_referenced_profiles, discover_candidate_prefixes};
use super::staged_trainers::staged_trainers_health;
use super::types::{
    PrefixCleanupTarget, PrefixCleanupTargetKind, PrefixReference, PrefixStorageEntry,
    PrefixStorageScanResult,
};
use super::utils::{dir_size_bytes, has_crosshook_managed_marker};

pub fn scan_prefix_storage(
    references: &[PrefixReference],
    stale_days: u64,
    inventory_incomplete: bool,
) -> Result<PrefixStorageScanResult, String> {
    let now = SystemTime::now();
    let stale_threshold = now
        .checked_sub(Duration::from_secs(stale_days.saturating_mul(24 * 60 * 60)))
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let referenced_by_prefix = collect_referenced_profiles(references);
    let discovered_prefixes =
        discover_candidate_prefixes(&referenced_by_prefix.keys().cloned().collect::<Vec<_>>());

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
        let is_orphan =
            !inventory_incomplete && referenced_profiles.is_empty() && is_crosshook_managed;
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
        inventory_incomplete,
    })
}
