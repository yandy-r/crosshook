//! Toolkit-independent application services over `crosshook-core` stores.
//!
//! Every public service function is `async` and blocking store I/O runs inside
//! [`tokio::task::spawn_blocking`] so callers never stall an async executor;
//! task panics and cancellations surface as sanitized [`AppError`]s. Services
//! with typed getters and setters over in-memory snapshots (for example
//! [`settings::UiSettingsService`]) may expose them as plain synchronous
//! methods — only the throttled persistence behind them stays async.

use crate::jobs::{JobContext, JobRegistry, JobSpec, Outcome};
use crate::{AppError, AppErrorKind};

pub mod background;
pub mod catalog;
pub mod collections;
pub mod community;
pub mod diagnostics;
pub mod discovery;
pub mod health;
pub mod host_tools;
pub mod launch;
pub mod launcher_export;
pub mod lutris;
pub mod mods;
pub mod offline;
pub mod onboarding;
pub mod profiles;
pub mod protondb;
pub mod recent_files;
pub mod settings;
pub mod startup_scans;
pub mod steam;
pub mod storage;
pub mod version;

pub use background::{
    BackgroundGrantHandle, BackgroundGrantHolder, BackgroundPortal, BackgroundProtectionState,
    BackgroundRequestError, BackgroundRequestResult, XdgBackgroundPortal,
    BACKGROUND_REQUEST_REASON,
};
pub use catalog::{CatalogService, CommandArgumentCatalogSnapshot, OptimizationCatalogSnapshot};
pub use collections::CollectionsService;
pub use community::CommunityService;
pub use diagnostics::{
    DiagnosticAttachment, DiagnosticsExport, DiagnosticsRequest, DiagnosticsService,
};
pub use discovery::DiscoveryService;
pub use health::HealthService;
pub use host_tools::{CoreHostProbe, HostProbe, HostToolsService};
pub use launch::LaunchService;
pub use launcher_export::{LauncherExportService, LauncherRenameRequest};
pub use lutris::LutrisService;
pub use mods::{ModsService, ProfileModsResponse};
pub use offline::OfflineService;
pub use onboarding::OnboardingService;
pub use profiles::ProfileWriter;
pub use protondb::ProtonDbService;
pub use recent_files::{RecentFileKind, RecentFilesService};
pub use settings::{
    merge_save_request, SettingsSaveRequest, SettingsService, SettingsSnapshot, UiSettingsService,
};
pub use startup_scans::StartupScansService;
pub use steam::SteamService;
pub use storage::{PrefixStorageHistoryResponse, StorageService};
pub use version::{VersionCheckResult, VersionService, VersionSnapshotInfo};

/// Runs blocking `work` on Tokio's blocking pool; a panic or cancellation maps
/// to a sanitized [`AppError`] through its `From<JoinError>` impl.
async fn run_blocking<T, F>(work: F) -> Result<T, AppError>
where
    F: FnOnce() -> Result<T, AppError> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work).await?
}

/// Runs blocking `work` as a supervised [`JobRegistry`] job and returns its
/// value.
///
/// The job holds `spec.key` (if any) while it runs, so a concurrent request
/// for the same key fails fast with `Conflict`. A job cancelled before it
/// starts never runs `work`. Cancellation through the registry (app shutdown)
/// or the job token while `work` runs resolves to `Cancelled`; the blocking
/// closure itself cannot be interrupted and finishes in the background. When
/// `work` completes in the same poll as cancellation, its result wins, so a
/// caller never sees `Cancelled` for work whose side effects already landed.
/// The finished job is reaped before returning. Dropping the returned future
/// leaves the job to finish; the registry evicts it.
async fn run_job<T, F>(jobs: &JobRegistry, spec: JobSpec, work: F) -> Result<T, AppError>
where
    F: FnOnce() -> Result<T, AppError> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = tokio::sync::oneshot::channel();
    let handle = jobs.start(spec, move |ctx: JobContext| async move {
        if ctx.is_cancelled() {
            return Outcome::Cancelled;
        }
        let task = tokio::task::spawn_blocking(work);
        tokio::select! {
            biased;
            joined = task => {
                let result: Result<T, AppError> = joined.map_err(AppError::from).and_then(|r| r);
                let outcome = match &result {
                    Ok(_) => Outcome::Succeeded,
                    Err(error) if error.kind() == AppErrorKind::Cancelled => Outcome::Cancelled,
                    Err(error) => Outcome::Failed(error.clone()),
                };
                let _ = tx.send(result);
                outcome
            }
            () = ctx.cancelled() => Outcome::Cancelled,
        }
    })?;
    let id = handle.id().clone();
    let mut events = handle.into_events();
    while events.recv().await.is_some() {}
    let _ = jobs.reap(&id);
    rx.await
        .unwrap_or_else(|_| Err(AppError::new(AppErrorKind::Cancelled, "job was cancelled")))
}
