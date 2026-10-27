//! Launcher validate, export, existence checks, delete and rename.

use std::path::{Component, Path};

use crosshook_core::export::launcher::{resolve_target_home_path, sanitize_launcher_slug};
use crosshook_core::export::{
    check_launcher_exists_for_request, check_launcher_for_profile, delete_launcher_by_slug,
    delete_launcher_files, export_launchers, rename_launcher_files, validate, LauncherDeleteResult,
    LauncherInfo, LauncherRenameResult, LauncherStoreError, SteamExternalLauncherExportError,
    SteamExternalLauncherExportRequest, SteamExternalLauncherExportResult,
    SteamExternalLauncherExportValidationError,
};
use crosshook_core::profile::TrainerLoadingMode;
use crosshook_core::settings::UmuPreference;

use super::{LauncherExportService, LauncherRenameRequest};
use crate::services::run_blocking;
use crate::{AppError, AppErrorKind};

/// Directory (relative to the launcher home) holding exported scripts.
const LAUNCHERS_DIR: &str = ".local/share/crosshook/launchers";
/// Directory (relative to the launcher home) holding exported desktop entries.
const APPLICATIONS_DIR: &str = ".local/share/applications";

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

    /// Rewrites launcher files for a renamed launcher and cleans up the old
    /// paths. D14: the old slug is rejected when it could escape the launcher
    /// directories, sanitized like delete-by-slug, and old/new paths must stay
    /// direct children of the resolved launcher directories before any file
    /// operation. The umu/runtime fields are reset to defaults (D15).
    pub async fn rename(
        &self,
        request: LauncherRenameRequest,
    ) -> Result<LauncherRenameResult, AppError> {
        let metadata = self.metadata.clone();
        run_blocking(move || {
            let loading_mode: TrainerLoadingMode =
                request.trainer_loading_mode.parse().map_err(|_| {
                    AppError::new(
                        AppErrorKind::Validation {
                            field: "trainer_loading_mode",
                        },
                        "invalid trainer loading mode",
                    )
                })?;
            let old_slug = checked_old_slug(&request.old_launcher_slug)?;

            let export_request = SteamExternalLauncherExportRequest {
                method: request.method,
                launcher_name: request.launcher_name,
                trainer_path: request.trainer_path,
                trainer_loading_mode: loading_mode,
                launcher_icon_path: request.new_launcher_icon_path.clone(),
                prefix_path: request.prefix_path,
                proton_path: request.proton_path,
                steam_app_id: request.steam_app_id,
                steam_client_install_path: request.steam_client_install_path.clone(),
                target_home_path: request.target_home_path.clone(),
                profile_name: None,
                runtime_steam_app_id: String::new(),
                umu_game_id: String::new(),
                umu_preference: UmuPreference::Auto,
                network_isolation: request.network_isolation,
                gamescope: request.gamescope,
                custom_env_vars: request.custom_env_vars,
            };
            validate(&export_request).map_err(validation_error)?;

            let new_slug = sanitize_launcher_slug(&request.new_display_name);
            ensure_inside_launcher_dirs(
                &request.target_home_path,
                &request.steam_client_install_path,
                &[&old_slug, &new_slug],
            )?;

            let result = rename_launcher_files(
                &old_slug,
                &request.new_display_name,
                &request.new_launcher_icon_path,
                &request.target_home_path,
                &request.steam_client_install_path,
                &export_request,
            )
            .map_err(store_error)?;

            if let Err(error) = metadata.observe_launcher_renamed(
                &result.old_slug,
                &result.new_slug,
                &request.new_display_name,
                &result.new_script_path,
                &result.new_desktop_entry_path,
            ) {
                tracing::warn!(
                    %error,
                    old_slug = %result.old_slug,
                    new_slug = %result.new_slug,
                    "metadata sync after rename_launcher failed"
                );
            }
            Ok(result)
        })
        .await
    }
}

/// Rejects an old slug that is blank or could escape the launcher directories
/// (path separators, NUL, `..`), then sanitizes it exactly like delete-by-slug.
fn checked_old_slug(raw: &str) -> Result<String, AppError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.contains(['/', '\\', '\0']) || trimmed.contains("..") {
        return Err(AppError::new(
            AppErrorKind::Validation {
                field: "old_launcher_slug",
            },
            "invalid launcher slug",
        ));
    }
    Ok(sanitize_launcher_slug(trimmed))
}

/// Requires the script and desktop entry paths of every slug to be direct
/// children of the resolved launcher directories. Lexical; runs before any
/// file operation. An unresolvable home is left to core (no file op happens).
pub(super) fn ensure_inside_launcher_dirs(
    target_home_path: &str,
    steam_client_install_path: &str,
    slugs: &[&str],
) -> Result<(), AppError> {
    let home = resolve_target_home_path(target_home_path, steam_client_install_path);
    if home.trim().is_empty() {
        return Ok(());
    }
    let home = Path::new(&home);
    let launchers_dir = home.join(LAUNCHERS_DIR);
    let applications_dir = home.join(APPLICATIONS_DIR);

    for slug in slugs {
        let script_name = format!("{slug}-trainer.sh");
        let desktop_name = format!("crosshook-{slug}-trainer.desktop");
        if !is_direct_child(&launchers_dir, &script_name)
            || !is_direct_child(&applications_dir, &desktop_name)
        {
            return Err(AppError::new(
                AppErrorKind::Validation {
                    field: "old_launcher_slug",
                },
                "launcher paths must stay inside the launcher directories",
            ));
        }
    }
    Ok(())
}

fn is_direct_child(dir: &Path, file_name: &str) -> bool {
    let mut components = Path::new(file_name).components();
    let single_normal =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
    single_normal && dir.join(file_name).parent() == Some(dir)
}
