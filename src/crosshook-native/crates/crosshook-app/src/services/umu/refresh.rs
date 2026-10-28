use crosshook_core::umu_database::{Error, UmuDatabaseRefreshStatus};

use super::UmuService;
use crate::{AppError, AppErrorKind};

pub(super) const UMU_REFRESH_FAILED: &str = "failed to refresh the umu database";

impl UmuService {
    /// Refreshes the cached umu database with path-free presenter errors.
    pub async fn refresh(&self) -> Result<UmuDatabaseRefreshStatus, AppError> {
        self.refresher.refresh().await.map_err(|error| {
            tracing::warn!(%error, "umu database refresh failed");
            let kind = match error {
                Error::Network(_) | Error::Metadata(_) => AppErrorKind::Unavailable,
                Error::Io(_) | Error::Base(_) => AppErrorKind::Io,
            };
            AppError::new(kind, UMU_REFRESH_FAILED)
        })
    }
}
