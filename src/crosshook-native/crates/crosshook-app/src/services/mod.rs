//! Toolkit-independent application services over `crosshook-core` stores.
//!
//! Every public service function is `async` and blocking store I/O runs inside
//! [`tokio::task::spawn_blocking`] so callers never stall an async executor;
//! task panics and cancellations surface as sanitized [`AppError`]s. Services
//! with typed getters and setters over in-memory snapshots (for example
//! [`settings::UiSettingsService`]) may expose them as plain synchronous
//! methods — only the throttled persistence behind them stays async.

use crate::AppError;

pub mod background;
pub mod catalog;
pub mod collections;
pub mod diagnostics;
pub mod health;
pub mod launch;
pub mod launcher_export;
pub mod mods;
pub mod profiles;
pub mod recent_files;
pub mod settings;
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
pub use diagnostics::{
    DiagnosticAttachment, DiagnosticsExport, DiagnosticsRequest, DiagnosticsService,
};
pub use health::HealthService;
pub use launch::LaunchService;
pub use launcher_export::{LauncherExportService, LauncherRenameRequest};
pub use mods::{ModsService, ProfileModsResponse};
pub use profiles::ProfileWriter;
pub use recent_files::{RecentFileKind, RecentFilesService};
pub use settings::{
    merge_save_request, SettingsSaveRequest, SettingsService, SettingsSnapshot, UiSettingsService,
};
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
