//! Launcher script and desktop entry previews (clipboard copy; nothing is written).

use crosshook_core::export::{
    preview_desktop_entry_content, preview_trainer_script_content,
    SteamExternalLauncherExportRequest,
};

use super::export::export_error;
use super::LauncherExportService;
use crate::services::run_blocking;
use crate::AppError;

impl LauncherExportService {
    /// Generates the trainer launcher script with placement headers. Does NOT
    /// write to disk.
    pub async fn preview_script(
        &self,
        request: SteamExternalLauncherExportRequest,
    ) -> Result<String, AppError> {
        run_blocking(move || preview_trainer_script_content(&request).map_err(export_error)).await
    }

    /// Generates the desktop entry with placement headers. Does NOT write to
    /// disk.
    pub async fn preview_desktop(
        &self,
        request: SteamExternalLauncherExportRequest,
    ) -> Result<String, AppError> {
        run_blocking(move || preview_desktop_entry_content(&request).map_err(export_error)).await
    }
}
