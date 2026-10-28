//! Thin IPC adapters over the Lutris import service.

use std::path::PathBuf;

use crosshook_app::services::LutrisService;
use crosshook_app::AppError;
use crosshook_core::profile::{LutrisImportEntry, LutrisImportPreview, LutrisImportResult};
use tauri::State;

fn map_app_error(error: AppError) -> String {
    error.message().to_string()
}

#[tauri::command]
pub async fn lutris_prepare_import(
    directory: Option<String>,
    service: State<'_, LutrisService>,
) -> Result<LutrisImportPreview, String> {
    service
        .prepare_import(directory.map(PathBuf::from))
        .await
        .map_err(map_app_error)
}

#[tauri::command]
pub async fn lutris_import_profiles(
    entries: Vec<LutrisImportEntry>,
    service: State<'_, LutrisService>,
) -> Result<LutrisImportResult, String> {
    service
        .import_profiles(entries)
        .await
        .map_err(map_app_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        // Async commands cannot be cast to fn pointers; naming them pins the
        // exported symbol (and thus the IPC command name).
        let _ = lutris_prepare_import;
        let _ = lutris_import_profiles;
    }
}
