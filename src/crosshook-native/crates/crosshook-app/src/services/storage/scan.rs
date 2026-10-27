//! Exclusive prefix scan job and take-once result retrieval.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crosshook_core::metadata::{MetadataStore, PrefixStorageSnapshotRow};
use crosshook_core::profile::ProfileStore;
use crosshook_core::storage::{
    collect_profile_prefix_references, scan_prefix_storage, PrefixStorageScanResult,
    ProfilePrefixReferences, DEFAULT_STALE_STAGED_TRAINER_DAYS,
};

use super::{StorageService, STORAGE_SCAN_JOB_KEY, STORAGE_SCAN_JOB_KIND};
use crate::jobs::{
    JobContext, JobEventKind, JobHandle, JobId, JobKey, JobProgress, JobSpec, Outcome,
};
use crate::services::run_blocking;
use crate::{AppError, AppErrorKind};

struct ScanResultGuard {
    results: Arc<Mutex<HashMap<JobId, PrefixStorageScanResult>>>,
    id: JobId,
    abandoned: Arc<AtomicBool>,
}

impl Drop for ScanResultGuard {
    fn drop(&mut self) {
        self.abandoned.store(true, Ordering::Relaxed);
        if let Ok(mut results) = self.results.lock() {
            results.remove(&self.id);
        }
    }
}

impl StorageService {
    /// Starts the exclusive prefix-storage scan job and prunes slots whose job
    /// outcomes have already been evicted from the registry.
    pub fn start_scan(&self) -> Result<JobHandle, AppError> {
        self.start_scan_inner().map(|(handle, _)| handle)
    }

    fn start_scan_inner(&self) -> Result<(JobHandle, Arc<AtomicBool>), AppError> {
        if let Ok(mut results) = self.scan_results.lock() {
            results.retain(|id, _| self.jobs.outcome(id).is_ok());
        }
        let profiles = self.profiles.clone();
        let metadata = self.metadata.clone();
        let results = self.scan_results.clone();
        let abandoned = Arc::new(AtomicBool::new(false));
        let job_abandoned = abandoned.clone();
        let handle = self.jobs.start(
            JobSpec::new(STORAGE_SCAN_JOB_KIND).with_key(JobKey::new(STORAGE_SCAN_JOB_KEY)),
            move |ctx| async move {
                let id = ctx.id().clone();
                let worker_ctx = ctx.clone();
                let result = run_blocking(move || Ok(scan(&profiles, &metadata, &worker_ctx)))
                    .await
                    .map_err(|error| {
                        AppError::new(
                            error.kind(),
                            format!("prefix storage scan task failed: {error}"),
                        )
                    })
                    .and_then(|result| result);
                match result {
                    Ok(result) if !ctx.is_cancelled() => {
                        let mut results = match results.lock() {
                            Ok(results) => results,
                            Err(_) => {
                                return Outcome::Failed(result_slot_error());
                            }
                        };
                        if job_abandoned.load(Ordering::Relaxed) {
                            return Outcome::Succeeded;
                        }
                        results.insert(id, result);
                        Outcome::Succeeded
                    }
                    Ok(_) => Outcome::Cancelled,
                    Err(error) if error.kind() == AppErrorKind::Cancelled => Outcome::Cancelled,
                    Err(error) => Outcome::Failed(error),
                }
            },
        )?;
        Ok((handle, abandoned))
    }

    /// Starts a scan, waits up to five minutes, and takes its result.
    ///
    /// Dropping this future removes any result already stored for its job. On
    /// timeout the scan keeps running and holds the exclusive job key until the
    /// blocking work observes cancellation or finishes (D3).
    pub async fn scan(&self) -> Result<PrefixStorageScanResult, AppError> {
        let (handle, abandoned) = self.start_scan_inner()?;
        let id = handle.id().clone();
        let cancel = handle.cancel_token().clone();
        let mut events = handle.into_events();
        let _result_guard = ScanResultGuard {
            results: self.scan_results.clone(),
            id: id.clone(),
            abandoned,
        };
        let outcome = tokio::time::timeout(Duration::from_secs(300), async {
            while let Some(event) = events.recv().await {
                if let JobEventKind::Finished(outcome) = event.kind {
                    return Ok(outcome);
                }
            }
            Err(AppError::cancelled())
        })
        .await
        .map_err(|_| {
            cancel.cancel();
            AppError::new(AppErrorKind::Io, "prefix storage scan timed out")
        })??;
        match outcome {
            Outcome::Succeeded => self.scan_result(&id)?.ok_or_else(|| {
                AppError::new(
                    AppErrorKind::Internal,
                    "prefix storage scan result is unavailable",
                )
            }),
            Outcome::Failed(error) => Err(error),
            Outcome::Cancelled => Err(AppError::cancelled()),
        }
    }

    /// Takes a finished job's result once; unknown job ids return `NotFound`.
    pub fn scan_result(&self, id: &JobId) -> Result<Option<PrefixStorageScanResult>, AppError> {
        if self.jobs.outcome(id)?.is_none() {
            return Ok(None);
        }
        Ok(self
            .scan_results
            .lock()
            .map_err(|_| result_slot_error())?
            .remove(id))
    }
}

fn result_slot_error() -> AppError {
    AppError::new(
        AppErrorKind::Internal,
        "prefix storage scan result is unavailable",
    )
}

fn scan(
    profiles: &ProfileStore,
    metadata: &MetadataStore,
    ctx: &JobContext,
) -> Result<PrefixStorageScanResult, AppError> {
    let ProfilePrefixReferences {
        references,
        profiles_load_failed,
    } = collect_profile_prefix_references(profiles)
        .map_err(|error| AppError::new(AppErrorKind::Internal, error))?;
    if ctx.is_cancelled() {
        return Err(AppError::cancelled());
    }
    let result = scan_prefix_storage(
        &references,
        DEFAULT_STALE_STAGED_TRAINER_DAYS,
        profiles_load_failed,
    )
    .map_err(|error| AppError::new(AppErrorKind::Internal, error))?;
    let total = result.prefixes.len() as u64;
    for (index, entry) in result.prefixes.iter().enumerate() {
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
        if let Err(error) = metadata.insert_prefix_storage_snapshot(&row) {
            tracing::warn!(%error, path = %entry.resolved_prefix_path, "failed to persist prefix storage snapshot");
        }
        ctx.progress(JobProgress {
            current: index as u64 + 1,
            total: Some(total),
            message: Some("scanned prefix".into()),
        });
    }
    ctx.log(vec![format!("scanned {total} prefixes")]);
    Ok(result)
}
