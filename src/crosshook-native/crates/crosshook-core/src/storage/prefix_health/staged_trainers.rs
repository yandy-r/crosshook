use std::fs;
use std::path::Path;
use std::time::SystemTime;

use super::constants::STAGED_TRAINERS_RELATIVE;
use super::types::StaleStagedTrainerEntry;
use super::utils::{file_or_dir_size_bytes, normalized_path_string, system_time_to_rfc3339};

pub(super) fn staged_trainers_health(
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

        let modified_at = fs::metadata(&path)
            .ok()
            .and_then(|meta| meta.modified().ok());
        let is_stale = modified_at
            .map(|time| time <= stale_threshold)
            .unwrap_or(false);
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
