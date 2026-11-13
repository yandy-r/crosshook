//! Launcher validate, export, existence checks and delete.

use crosshook_core::export::launcher::sanitize_launcher_slug;
use crosshook_core::export::{
    check_launcher_exists_for_request, check_launcher_for_profile, delete_launcher_by_slug,
    delete_launcher_files, export_launchers, validate, LauncherDeleteResult, LauncherInfo,
    LauncherStoreError, SteamExternalLauncherExportError, SteamExternalLauncherExportRequest,
    SteamExternalLauncherExportResult, SteamExternalLauncherExportValidationError,
};

use super::LauncherExportService;
use crate::services::run_blocking;
use crate::{AppError, AppErrorKind};

/// Maps a request validation failure to `Validation { field: "request" }`.
pub(super) fn validation_error(error: SteamExternalLauncherExportValidationError) -> AppError {
    AppError::new(
        AppErrorKind::Validation { field: "request" },
        error.to_string(),
    )
}

/// Maps a core export error keeping its Display text; request validation is a
/// `Validation`, everything else `Io`.
pub(super) fn export_error(error: SteamExternalLauncherExportError) -> AppError {
    match error {
        SteamExternalLauncherExportError::InvalidRequest(validation) => {
            validation_error(validation)
        }
        other => AppError::new(AppErrorKind::Io, other.to_string()),
    }
}

/// Maps a core launcher-store error keeping its Display text.
pub(super) fn store_error(error: LauncherStoreError) -> AppError {
    AppError::new(AppErrorKind::Io, error.to_string())
}

impl LauncherExportService {
    /// Validates whether a launcher export request has the required
    /// trainer/runtime inputs.
    pub async fn validate(
        &self,
        request: SteamExternalLauncherExportRequest,
    ) -> Result<(), AppError> {
        run_blocking(move || validate(&request).map_err(validation_error)).await
    }

    /// Writes the launcher script and desktop entry for `request`, then records
    /// the export in metadata (fail-soft). Resolves only after both finish.
    pub async fn export(
        &self,
        request: SteamExternalLauncherExportRequest,
    ) -> Result<SteamExternalLauncherExportResult, AppError> {
        let metadata = self.metadata.clone();
        run_blocking(move || {
            let result = export_launchers(&request).map_err(export_error)?;
            if let Err(error) = metadata.observe_launcher_exported(
                request.profile_name.as_deref(),
                &result.launcher_slug,
                &result.display_name,
                &result.script_path,
                &result.desktop_entry_path,
            ) {
                tracing::warn!(
                    %error,
                    launcher_slug = %result.launcher_slug,
                    "metadata sync after export_launchers failed"
                );
            }
            Ok(result)
        })
        .await
    }

    /// Checks whether the launcher files derived from `request` exist on disk.
    pub async fn check_exists(
        &self,
        request: SteamExternalLauncherExportRequest,
    ) -> Result<LauncherInfo, AppError> {
        run_blocking(move || {
            check_launcher_exists_for_request(&request.launcher_name, &request).map_err(store_error)
        })
        .await
    }

    /// Loads a saved profile and checks whether its exported launcher files
    /// exist. A settings load failure is a hard error here (preserved
    /// inconsistency with the soft fallback used for stale detection).
    pub async fn check_for_profile(&self, name: String) -> Result<LauncherInfo, AppError> {
        let profiles = self.profiles.clone();
        let settings = self.settings.clone();
        run_blocking(move || {
            let profile = profiles.load(&name)?;
            let global_umu_preference = settings.load()?.umu_preference;
            check_launcher_for_profile(&profile, "", "", global_umu_preference).map_err(store_error)
        })
        .await
    }

    /// Deletes the launcher files derived from the supplied profile fields.
    pub async fn delete(
        &self,
        display_name: String,
        steam_app_id: String,
        trainer_path: String,
        target_home_path: String,
        steam_client_install_path: String,
    ) -> Result<LauncherDeleteResult, AppError> {
        let metadata = self.metadata.clone();
        run_blocking(move || {
            let result = delete_launcher_files(
                &display_name,
                &steam_app_id,
                &trainer_path,
                &target_home_path,
                &steam_client_install_path,
            )
            .map_err(store_error)?;

            let slug = sanitize_launcher_slug(&display_name);
            if let Err(error) = metadata.observe_launcher_deleted(&slug) {
                tracing::warn!(%error, launcher_slug = %slug, "metadata sync after delete_launcher failed");
            }
            Ok(result)
        })
        .await
    }

    /// Deletes launcher files directly from a known launcher slug. The metadata
    /// observation uses the raw slug.
    pub async fn delete_by_slug(
        &self,
        launcher_slug: String,
        target_home_path: String,
        steam_client_install_path: String,
    ) -> Result<LauncherDeleteResult, AppError> {
        let metadata = self.metadata.clone();
        run_blocking(move || {
            let result = delete_launcher_by_slug(
                &launcher_slug,
                &target_home_path,
                &steam_client_install_path,
            )
            .map_err(store_error)?;

            if let Err(error) = metadata.observe_launcher_deleted(&launcher_slug) {
                tracing::warn!(%error, launcher_slug = %launcher_slug, "metadata sync after delete_launcher_by_slug failed");
            }
            Ok(result)
        })
        .await
    }
}
