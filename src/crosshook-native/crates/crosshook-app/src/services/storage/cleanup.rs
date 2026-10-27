//! Fresh-scan cleanup authorization and fail-soft audit persistence.

use std::collections::BTreeSet;

use crosshook_core::storage::{
    cleanup_prefix_storage, collect_profile_prefix_references, scan_prefix_storage,
    PrefixCleanupResult, PrefixCleanupSkipped, PrefixCleanupTarget, PrefixCleanupTargetKind,
    ProfilePrefixReferences, DEFAULT_STALE_STAGED_TRAINER_DAYS,
};

use super::audit::persist_cleanup_audit;
use super::StorageService;
use crate::services::run_blocking;
use crate::{AppError, AppErrorKind};

impl StorageService {
    /// Reloads the inventory and authorizes exact fresh-scan tuples before cleanup.
    pub async fn cleanup(
        &self,
        targets: Vec<PrefixCleanupTarget>,
    ) -> Result<PrefixCleanupResult, AppError> {
        let profiles = self.profiles.clone();
        let metadata = self.metadata.clone();
        // The nested Result distinguishes worker failures from scanner errors.
        run_blocking(move || {
            Ok((|| {
                let ProfilePrefixReferences {
                    references,
                    profiles_load_failed,
                } = collect_profile_prefix_references(&profiles)
                    .map_err(|error| AppError::new(AppErrorKind::Internal, error))?;
                let scan_result = scan_prefix_storage(
                    &references,
                    DEFAULT_STALE_STAGED_TRAINER_DAYS,
                    profiles_load_failed,
                )
                .map_err(|error| AppError::new(AppErrorKind::Internal, error))?;
                let allowed_targets = scan_result
                    .orphan_targets
                    .iter()
                    .chain(scan_result.stale_staged_targets.iter())
                    .map(target_signature)
                    .collect::<BTreeSet<_>>();
                let mut seen_signatures = BTreeSet::new();
                let mut filtered_targets = Vec::new();
                let mut rejected = Vec::new();
                for target in targets {
                    let signature = target_signature(&target);
                    if !seen_signatures.insert(signature.clone()) {
                        continue;
                    }
                    if let Some((kind, resolved_prefix_path, target_path)) =
                        allowed_targets.get(&signature)
                    {
                        // Rebuild from the server scan, never forward the request object.
                        filtered_targets.push(PrefixCleanupTarget {
                            kind: kind.clone(),
                            resolved_prefix_path: resolved_prefix_path.clone(),
                            target_path: target_path.clone(),
                        });
                    } else {
                        rejected.push(PrefixCleanupSkipped {
                            target,
                            reason: "not present in a fresh storage scan".into(),
                        });
                    }
                }
                let mut result = cleanup_prefix_storage(&references, &filtered_targets);
                result.skipped.extend(rejected);
                persist_cleanup_audit(&metadata, &result);
                Ok(result)
            })())
        })
        .await
        .map_err(|error| {
            AppError::new(
                error.kind(),
                format!("prefix storage cleanup task failed: {error}"),
            )
        })?
    }
}

fn target_signature(target: &PrefixCleanupTarget) -> (PrefixCleanupTargetKind, String, String) {
    (
        target.kind.clone(),
        target.resolved_prefix_path.clone(),
        target.target_path.clone(),
    )
}
