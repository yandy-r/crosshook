use std::path::PathBuf;

use crosshook_app::services::profiles::ProfileCommit;
use crosshook_app::ProfileWriter;
use crosshook_core::profile::lutris_import::preview_lutris_import;
use crosshook_core::profile::{
    LutrisImportEntry, LutrisImportPreview, LutrisImportResult, ProfileStore,
};
use tauri::State;

fn map_error(error: impl ToString) -> String {
    error.to_string()
}

#[tauri::command]
pub fn lutris_prepare_import(
    directory: Option<String>,
    profile_store: State<'_, ProfileStore>,
) -> Result<LutrisImportPreview, String> {
    let root = directory.map(PathBuf::from);
    preview_lutris_import(root, Some(&profile_store)).map_err(map_error)
}

#[tauri::command]
pub async fn lutris_import_profiles(
    entries: Vec<LutrisImportEntry>,
    writer: State<'_, ProfileWriter>,
) -> Result<LutrisImportResult, String> {
    writer
        .import_lutris(entries)
        .await
        .map(ProfileCommit::into_output)
        .map_err(map_error)
}
