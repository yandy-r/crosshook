//! Toolkit-independent application services over `crosshook-core` stores.
//!
//! Every public service function is `async` and blocking store I/O runs inside
//! [`tokio::task::spawn_blocking`] so callers never stall an async executor;
//! task panics and cancellations surface as sanitized [`AppError`]s. Services

use std::future::Future;
use std::pin::Pin;

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
pub mod install;
pub mod launch;
pub mod launcher_export;
pub mod lutris;
pub mod mods;
pub mod offline;
pub mod onboarding;
pub mod prefix_deps;
pub(crate) mod process;
pub mod profiles;
pub mod proton_migration;
pub mod protondb;
pub mod protonup;
pub mod recent_files;
pub mod run_executable;
pub mod settings;
pub mod startup_scans;
pub mod steam;
pub mod storage;
pub mod umu;
pub mod update;
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
pub use install::InstallService;
pub use launch::LaunchService;
pub use launcher_export::LauncherExportService;
pub use lutris::LutrisService;
pub use mods::{ModsService, ProfileModsResponse};
pub use offline::OfflineService;
pub use onboarding::OnboardingService;
pub use prefix_deps::PrefixDepsService;
pub use process::{JobOwner, ProcessRun, RunEvent};
pub use profiles::ProfileWriter;
pub use proton_migration::ProtonMigrationService;
pub use protondb::ProtonDbService;
pub use protonup::ProtonUpService;
pub use recent_files::{RecentFileKind, RecentFilesService};
pub use run_executable::{
    RunCompletion, RunExecutableResponse, RunExecutableService, RunExecutableStartError,
    RUN_EXECUTABLE_JOB_KIND,
};
pub use settings::{merge_save_request, SettingsSaveRequest, SettingsService, SettingsSnapshot};
pub use startup_scans::StartupScansService;
pub use steam::SteamService;
pub use storage::{PrefixStorageHistoryResponse, StorageService};
pub use umu::UmuService;
pub use update::{UpdateCompletion, UpdateGameResponse, UpdateService, UPDATE_GAME_JOB_KIND};
pub use version::{VersionCheckResult, VersionService, VersionSnapshotInfo};

/// Boxed `Send` future returned by the object-safe async seams of the
/// services (for example the protonup catalog/installer and the umu refresher).
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn run_blocking_returns_ok_and_executes_off_the_caller_thread() {
        let caller = std::thread::current().id();
        let worker = run_blocking(move || Ok(std::thread::current().id()))
            .await
            .expect("blocking task succeeds");
        assert_ne!(
            worker, caller,
            "run_blocking must run on Tokio's blocking pool"
        );
    }

    #[tokio::test]
    async fn run_blocking_propagates_service_errors_unchanged() {
        let error = run_blocking::<(), _>(|| Err(AppError::validation("name", "invalid profile")))
            .await
            .unwrap_err();
        assert_eq!(error.kind(), AppErrorKind::Validation { field: "name" });
        assert_eq!(error.message(), "invalid profile");
    }

    #[tokio::test]
    async fn run_blocking_maps_panic_to_sanitized_internal_error() {
        let error = run_blocking::<(), _>(|| {
            panic!("sensitive /home/alice/secret payload");
        })
        .await
        .unwrap_err();
        assert_eq!(error.kind(), AppErrorKind::Internal);
        assert_eq!(error.message(), "background task failed");
        assert!(
            !error.message().contains("alice"),
            "panic payload must never leak through run_blocking"
        );
    }
}
