use super::types::{CachedHealthSnapshot, CachedOfflineReadinessSnapshot};
use crosshook_core::metadata::MetadataStore;
use tauri::State;

/// Returns the cached health snapshots from the last batch validation run.
///
/// Called on frontend mount to display instant badge status before the live scan
/// completes. Only returns rows for non-deleted profiles (enforced by the JOIN in
/// `load_health_snapshots`). Returns an empty list when MetadataStore is unavailable.
#[tauri::command]
pub fn get_cached_health_snapshots(
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<CachedHealthSnapshot>, String> {
    let snapshots = metadata_store
        .load_health_snapshots()
        .map_err(|e| e.to_string())?;

    Ok(snapshots
        .into_iter()
        .map(CachedHealthSnapshot::from)
        .collect())
}

#[tauri::command]
pub fn get_cached_offline_readiness_snapshots(
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<CachedOfflineReadinessSnapshot>, String> {
    let rows = metadata_store
        .load_offline_readiness_snapshot_rows()
        .map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(CachedOfflineReadinessSnapshot::from)
        .collect())
}
