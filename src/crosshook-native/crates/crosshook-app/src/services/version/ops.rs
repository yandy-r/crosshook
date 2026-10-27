//! Version write operations: trainer version and acknowledge.

use super::VersionService;
use crate::error::{AppError, AppErrorKind};
use crate::services::run_blocking;

impl VersionService {
    /// Records a manual trainer version hint for a named profile.
    ///
    /// Upserts a new snapshot row with `status = "untracked"` and the provided
    /// version string. The `steam_app_id` is preserved from the latest existing
    /// snapshot, or left empty if no snapshot has been recorded yet.
    pub async fn set_trainer_version(&self, name: String, version: String) -> Result<(), AppError> {
        if !self.metadata.is_available() {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "metadata database is unavailable",
            ));
        }
        let metadata = self.metadata.clone();
        run_blocking(move || {
            let profile_id = metadata
                .lookup_profile_id(&name)
                .map_err(|e| AppError::from_metadata_validation("version", e))?
                .ok_or_else(|| {
                    AppError::new(
                        AppErrorKind::NotFound,
                        format!("profile '{name}' is not registered in the metadata store"),
                    )
                })?;

            let steam_app_id = metadata
                .lookup_latest_version_snapshot(&profile_id)
                .ok()
                .flatten()
                .map(|row| row.steam_app_id)
                .unwrap_or_default();

            metadata
                .upsert_version_snapshot(
                    &profile_id,
                    &steam_app_id,
                    None,
                    Some(&version),
                    None,
                    None,
                    "untracked",
                )
                .map_err(|e| AppError::from_metadata_validation("version", e))
        })
        .await
    }

    /// Marks the latest version change for a named profile as acknowledged.
    ///
    /// Sets the latest snapshot's status to `"matched"`, indicating the user has
    /// verified the trainer still works with the updated game version.
    pub async fn acknowledge_change(&self, name: String) -> Result<(), AppError> {
        if !self.metadata.is_available() {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "metadata database is unavailable",
            ));
        }
        let metadata = self.metadata.clone();
        run_blocking(move || {
            let profile_id = metadata
                .lookup_profile_id(&name)
                .map_err(|e| AppError::from_metadata_validation("version", e))?
                .ok_or_else(|| {
                    AppError::new(
                        AppErrorKind::NotFound,
                        format!("profile '{name}' is not registered in the metadata store"),
                    )
                })?;

            metadata
                .acknowledge_version_change(&profile_id)
                .map_err(|e| AppError::from_metadata_validation("version", e))
        })
        .await
    }
}
