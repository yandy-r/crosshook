//! Storage history reads and fail-soft cleanup audit writes.

use crosshook_core::metadata::{MetadataStore, PrefixStorageCleanupAuditRow};
use crosshook_core::storage::{PrefixCleanupResult, PrefixCleanupTarget, PrefixCleanupTargetKind};
use uuid::Uuid;

use super::{PrefixStorageHistoryResponse, StorageService};
use crate::services::run_blocking;
use crate::AppError;

impl StorageService {
    /// Returns the latest 50 snapshots and 100 cleanup audit rows.
    pub async fn history(&self) -> Result<PrefixStorageHistoryResponse, AppError> {
        let metadata = self.metadata.clone();
        run_blocking(move || {
            Ok((|| {
                Ok(PrefixStorageHistoryResponse {
                    available: metadata.is_available(),
                    snapshots: metadata.list_latest_prefix_storage_snapshots(50)?,
                    audit: metadata.list_prefix_storage_cleanup_audit(100)?,
                })
            })())
        })
        .await
        .map_err(|error| {
            AppError::new(
                error.kind(),
                format!("prefix storage history task failed: {error}"),
            )
        })?
    }
}

pub(super) fn persist_cleanup_audit(metadata: &MetadataStore, result: &PrefixCleanupResult) {
    let now = chrono::Utc::now().to_rfc3339();
    for target in &result.deleted {
        insert_audit(metadata, target_row(target, "deleted", None, &now));
    }
    for skipped in &result.skipped {
        insert_audit(
            metadata,
            target_row(
                &skipped.target,
                "skipped",
                Some(skipped.reason.clone()),
                &now,
            ),
        );
    }
    if result.reclaimed_bytes > 0 || !result.deleted.is_empty() {
        insert_audit(
            metadata,
            PrefixStorageCleanupAuditRow {
                id: Uuid::new_v4().to_string(),
                target_kind: "summary".into(),
                resolved_prefix_path: String::new(),
                target_path: String::new(),
                result: "deleted".into(),
                reason: None,
                reclaimed_bytes: result.reclaimed_bytes as i64,
                created_at: now,
            },
        );
    }
}

fn target_row(
    target: &PrefixCleanupTarget,
    result: &str,
    reason: Option<String>,
    now: &str,
) -> PrefixStorageCleanupAuditRow {
    PrefixStorageCleanupAuditRow {
        id: Uuid::new_v4().to_string(),
        target_kind: match target.kind {
            PrefixCleanupTargetKind::OrphanPrefix => "orphan_prefix".into(),
            PrefixCleanupTargetKind::StaleStagedTrainer => "stale_staged_trainer".into(),
        },
        resolved_prefix_path: target.resolved_prefix_path.clone(),
        target_path: target.target_path.clone(),
        result: result.into(),
        reason,
        reclaimed_bytes: 0,
        created_at: now.into(),
    }
}

fn insert_audit(metadata: &MetadataStore, row: PrefixStorageCleanupAuditRow) {
    if let Err(error) = metadata.insert_prefix_storage_cleanup_audit(&row) {
        tracing::warn!(%error, path = %row.target_path, result = %row.result, "failed to persist cleanup audit");
    }
}
