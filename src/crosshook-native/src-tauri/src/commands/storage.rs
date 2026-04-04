use std::collections::BTreeSet;

use crosshook_core::metadata::{
    MetadataStore, PrefixStorageCleanupAuditRow, PrefixStorageSnapshotRow,
};
use crosshook_core::profile::ProfileStore;
use crosshook_core::storage::{
    cleanup_prefix_storage as cleanup_prefix_storage_core, collect_profile_prefix_references,
    scan_prefix_storage as scan_prefix_storage_core, PrefixCleanupResult, PrefixCleanupTarget,
    PrefixCleanupTargetKind, PrefixStorageScanResult, ProfilePrefixReferences,
    DEFAULT_STALE_STAGED_TRAINER_DAYS,
};
use serde::Serialize;
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub async fn scan_prefix_storage(
    profile_store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<PrefixStorageScanResult, String> {
    let profile_store = profile_store.inner().clone();
    let metadata_store = metadata_store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let ProfilePrefixReferences {
            references,
            profiles_load_failed,
        } = collect_profile_prefix_references(&profile_store)?;
        let result =
            scan_prefix_storage_core(&references, DEFAULT_STALE_STAGED_TRAINER_DAYS, profiles_load_failed)?;

        // Persist snapshot rows (fail-soft)
        for entry in &result.prefixes {
            let row = PrefixStorageSnapshotRow {
                id: format!("{}-{}", entry.resolved_prefix_path, result.scanned_at),
                resolved_prefix_path: entry.resolved_prefix_path.clone(),
                total_bytes: entry.total_bytes as i64,
                staged_trainers_bytes: entry.staged_trainers_bytes as i64,
                is_orphan: entry.is_orphan,
                referenced_profiles_json: serde_json::to_string(&entry.referenced_by_profiles)
                    .unwrap_or_default(),
                stale_staged_count: entry.stale_staged_trainers.len() as i64,
                scanned_at: result.scanned_at.clone(),
            };
            if let Err(e) = metadata_store.insert_prefix_storage_snapshot(&row) {
                tracing::warn!(%e, path = %entry.resolved_prefix_path, "failed to persist prefix storage snapshot");
            }
        }

        Ok(result)
    })
    .await
    .map_err(|error| format!("prefix storage scan task failed: {error}"))?
}

#[tauri::command]
pub async fn cleanup_prefix_storage(
    targets: Vec<PrefixCleanupTarget>,
    profile_store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<PrefixCleanupResult, String> {
    let profile_store = profile_store.inner().clone();
    let metadata_store = metadata_store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let ProfilePrefixReferences {
            references,
            profiles_load_failed,
        } = collect_profile_prefix_references(&profile_store)?;
        let scan_result =
            scan_prefix_storage_core(&references, DEFAULT_STALE_STAGED_TRAINER_DAYS, profiles_load_failed)?;

        let allowed_targets = scan_result
            .orphan_targets
            .iter()
            .chain(scan_result.stale_staged_targets.iter())
            .map(target_signature)
            .collect::<BTreeSet<_>>();

        let mut seen_signatures = BTreeSet::new();
        let filtered_targets = targets
            .into_iter()
            .filter(|target| {
                let sig = target_signature(target);
                allowed_targets.contains(&sig) && seen_signatures.insert(sig)
            })
            .collect::<Vec<_>>();

        let result = cleanup_prefix_storage_core(&references, &filtered_targets);
        let now = chrono::Utc::now().to_rfc3339();

        // Persist audit rows (fail-soft)
        for target in &result.deleted {
            let row = PrefixStorageCleanupAuditRow {
                id: Uuid::new_v4().to_string(),
                target_kind: cleanup_target_kind_str(&target.kind),
                resolved_prefix_path: target.resolved_prefix_path.clone(),
                target_path: target.target_path.clone(),
                result: "deleted".into(),
                reason: None,
                reclaimed_bytes: 0,
                created_at: now.clone(),
            };
            if let Err(e) = metadata_store.insert_prefix_storage_cleanup_audit(&row) {
                tracing::warn!(%e, path = %target.target_path, "failed to persist cleanup audit (deleted)");
            }
        }

        for skipped in &result.skipped {
            let row = PrefixStorageCleanupAuditRow {
                id: Uuid::new_v4().to_string(),
                target_kind: cleanup_target_kind_str(&skipped.target.kind),
                resolved_prefix_path: skipped.target.resolved_prefix_path.clone(),
                target_path: skipped.target.target_path.clone(),
                result: "skipped".into(),
                reason: Some(skipped.reason.clone()),
                reclaimed_bytes: 0,
                created_at: now.clone(),
            };
            if let Err(e) = metadata_store.insert_prefix_storage_cleanup_audit(&row) {
                tracing::warn!(%e, path = %skipped.target.target_path, "failed to persist cleanup audit (skipped)");
            }
        }

        // Summary row with total reclaimed bytes
        if result.reclaimed_bytes > 0 || !result.deleted.is_empty() {
            let summary = PrefixStorageCleanupAuditRow {
                id: Uuid::new_v4().to_string(),
                target_kind: "summary".into(),
                resolved_prefix_path: String::new(),
                target_path: String::new(),
                result: "deleted".into(),
                reason: None,
                reclaimed_bytes: result.reclaimed_bytes as i64,
                created_at: now,
            };
            if let Err(e) = metadata_store.insert_prefix_storage_cleanup_audit(&summary) {
                tracing::warn!(%e, "failed to persist cleanup audit summary");
            }
        }

        Ok(result)
    })
    .await
    .map_err(|error| format!("prefix storage cleanup task failed: {error}"))?
}

#[derive(Debug, Clone, Serialize)]
pub struct PrefixStorageHistoryResponse {
    pub available: bool,
    pub snapshots: Vec<PrefixStorageSnapshotRow>,
    pub audit: Vec<PrefixStorageCleanupAuditRow>,
}

#[tauri::command]
pub async fn get_prefix_storage_history(
    metadata_store: State<'_, MetadataStore>,
) -> Result<PrefixStorageHistoryResponse, String> {
    let metadata_store = metadata_store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let available = metadata_store.is_available();
        let snapshots = metadata_store
            .list_latest_prefix_storage_snapshots(50)
            .map_err(|e| e.to_string())?;
        let audit = metadata_store
            .list_prefix_storage_cleanup_audit(100)
            .map_err(|e| e.to_string())?;
        Ok(PrefixStorageHistoryResponse {
            available,
            snapshots,
            audit,
        })
    })
    .await
    .map_err(|error| format!("prefix storage history task failed: {error}"))?
}

fn target_signature(target: &PrefixCleanupTarget) -> (PrefixCleanupTargetKind, String, String) {
    (
        target.kind.clone(),
        target.resolved_prefix_path.clone(),
        target.target_path.clone(),
    )
}

fn cleanup_target_kind_str(kind: &PrefixCleanupTargetKind) -> String {
    match kind {
        PrefixCleanupTargetKind::OrphanPrefix => "orphan_prefix".into(),
        PrefixCleanupTargetKind::StaleStagedTrainer => "stale_staged_trainer".into(),
    }
}
