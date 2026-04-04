use std::collections::BTreeSet;

use crosshook_core::profile::ProfileStore;
use crosshook_core::storage::{
    cleanup_prefix_storage as cleanup_prefix_storage_core,
    collect_profile_prefix_references,
    scan_prefix_storage as scan_prefix_storage_core,
    PrefixCleanupTargetKind,
    PrefixCleanupResult,
    PrefixCleanupTarget,
    PrefixStorageScanResult,
    DEFAULT_STALE_STAGED_TRAINER_DAYS,
};
use tauri::State;

#[tauri::command]
pub async fn scan_prefix_storage(
    profile_store: State<'_, ProfileStore>,
) -> Result<PrefixStorageScanResult, String> {
    let profile_store = profile_store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let references = collect_profile_prefix_references(&profile_store)?;
        scan_prefix_storage_core(&references, DEFAULT_STALE_STAGED_TRAINER_DAYS)
    })
    .await
    .map_err(|error| format!("prefix storage scan task failed: {error}"))?
}

#[tauri::command]
pub async fn cleanup_prefix_storage(
    targets: Vec<PrefixCleanupTarget>,
    profile_store: State<'_, ProfileStore>,
) -> Result<PrefixCleanupResult, String> {
    let profile_store = profile_store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let references = collect_profile_prefix_references(&profile_store)?;
        let scan_result = scan_prefix_storage_core(&references, DEFAULT_STALE_STAGED_TRAINER_DAYS)?;

        let allowed_targets = scan_result
            .orphan_targets
            .iter()
            .chain(scan_result.stale_staged_targets.iter())
            .map(target_signature)
            .collect::<BTreeSet<_>>();

        let filtered_targets = targets
            .into_iter()
            .filter(|target| allowed_targets.contains(&target_signature(target)))
            .collect::<Vec<_>>();

        Ok(cleanup_prefix_storage_core(&references, &filtered_targets))
    })
    .await
    .map_err(|error| format!("prefix storage cleanup task failed: {error}"))?
}

fn target_signature(target: &PrefixCleanupTarget) -> (PrefixCleanupTargetKind, String, String) {
    (
        target.kind.clone(),
        target.resolved_prefix_path.clone(),
        target.target_path.clone(),
    )
}

