use std::path::PathBuf;

use crosshook_core::profile::{
    export_community_profile, preview_community_profile_import, CommunityExportResult,
    CommunityImportPreview, CommunityImportResult,
};

use super::containment::{contain_in_taps, require_regular_json};
use super::taps::current_workspaces;
use super::CommunityService;
use crate::services::{profiles::ProfileCommit, run_blocking};
use crate::{AppError, AppErrorKind};

impl CommunityService {
    /// Exports a shareable profile to the user's selected destination.
    pub async fn export_profile(
        &self,
        profile_name: String,
        output_path: String,
    ) -> Result<CommunityExportResult, AppError> {
        let profiles = self.profiles.clone();
        run_blocking(move || {
            Ok(export_community_profile(
                &profiles.base_path,
                profile_name.trim(),
                &PathBuf::from(output_path),
            )?)
        })
        .await
    }

    /// Previews any user-selected local JSON file, reading its canonical path.
    pub async fn prepare_import(&self, path: PathBuf) -> Result<CommunityImportPreview, AppError> {
        let settings = self.settings.clone();
        let taps = self.taps.clone();
        run_blocking(move || {
            let canonical = path.canonicalize().map_err(|error| {
                AppError::new(
                    if error.kind() == std::io::ErrorKind::NotFound {
                        AppErrorKind::NotFound
                    } else {
                        AppErrorKind::Io
                    },
                    "failed to resolve community profile path",
                )
            })?;
            require_regular_json(&canonical)?;
            // Classification is diagnostic only: the user-selected preview must
            // remain available even when settings or a stored tap are invalid.
            if let Ok(workspaces) = settings
                .load()
                .map_err(AppError::from)
                .and_then(|settings| current_workspaces(&taps, &settings.community_taps))
            {
                let classification = contain_in_taps(&workspaces, &path);
                tracing::debug!(
                    inside_tap_workspace = classification.is_ok(),
                    "classified community import preview"
                );
            } else {
                tracing::debug!("community import preview classification unavailable");
            }
            Ok(preview_community_profile_import(&canonical)?)
        })
        .await
    }

    /// Imports a regular JSON file authorized by a subscribed tap workspace.
    pub async fn import_profile(&self, path: PathBuf) -> Result<CommunityImportResult, AppError> {
        let settings = self.settings.clone();
        let taps = self.taps.clone();
        let canonical = run_blocking(move || {
            let workspaces = current_workspaces(&taps, &settings.load()?.community_taps)?;
            let canonical = contain_in_taps(&workspaces, &path)?;
            require_regular_json(&canonical)?;
            Ok(canonical)
        })
        .await?;
        self.writer
            .import_community(canonical)
            .await
            .map(ProfileCommit::into_output)
    }
}
