//! Toolkit-independent application services over `crosshook-core` stores.
//!
//! Every public service function is `async`; blocking store I/O runs inside
//! [`tokio::task::spawn_blocking`] so callers never stall an async executor.
//! Task panics and cancellations surface as sanitized [`AppError`]s.

use crate::AppError;

pub mod background;
pub mod catalog;
pub mod diagnostics;
pub mod recent_files;
pub mod settings;

pub use background::{
    BackgroundGrantHandle, BackgroundGrantHolder, BackgroundPortal, BackgroundProtectionState,
    BackgroundRequestError, BackgroundRequestResult, XdgBackgroundPortal,
    BACKGROUND_REQUEST_REASON,
};
pub use catalog::{CatalogService, CommandArgumentCatalogSnapshot, OptimizationCatalogSnapshot};
pub use diagnostics::{
    DiagnosticAttachment, DiagnosticsExport, DiagnosticsRequest, DiagnosticsService,
};
pub use recent_files::{RecentFileKind, RecentFilesService};
pub use settings::{merge_save_request, SettingsSaveRequest, SettingsService, SettingsSnapshot};

/// Runs blocking `work` on Tokio's blocking pool; a panic or cancellation maps
/// to a sanitized [`AppError`] through its `From<JoinError>` impl.
async fn run_blocking<T, F>(work: F) -> Result<T, AppError>
where
    F: FnOnce() -> Result<T, AppError> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work).await?
}
