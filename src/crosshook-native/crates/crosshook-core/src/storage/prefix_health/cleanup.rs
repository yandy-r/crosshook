use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::constants::{DRIVE_C_RELATIVE, STAGED_TRAINERS_RELATIVE};
use super::discovery::collect_referenced_profiles;
use super::types::{
    PrefixCleanupResult, PrefixCleanupSkipped, PrefixCleanupTarget, PrefixCleanupTargetKind,
    PrefixReference,
};
use super::utils::{
    dir_size_bytes, file_or_dir_size_bytes, has_crosshook_managed_marker, normalized_path_string,
};

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
