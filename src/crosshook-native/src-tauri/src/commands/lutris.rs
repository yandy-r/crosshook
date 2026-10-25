use std::path::PathBuf;

use crosshook_app::services::profiles::ImportedProfileWrite;
use crosshook_app::ProfileWriter;
use crosshook_core::profile::lutris_import::{apply_lutris_import, preview_lutris_import};
use crosshook_core::profile::{
    LutrisImportEntry, LutrisImportOutcome, LutrisImportPreview, LutrisImportResult, ProfileStore,
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
    profile_store: State<'_, ProfileStore>,
    writer: State<'_, ProfileWriter>,
) -> Result<LutrisImportResult, String> {
    let store = profile_store.inner().clone();
    let result = tauri::async_runtime::spawn_blocking(move || apply_lutris_import(&store, entries))
        .await
        .map_err(map_error)?;

    let imports: Vec<ImportedProfileWrite> = result
        .results
        .iter()
        .filter(|entry| entry.outcome == LutrisImportOutcome::Imported)
        .filter_map(|entry| {
            Some(ImportedProfileWrite {
                name: entry.profile_name.clone()?,
                profile: entry.entry.mapped.clone(),
                path: entry.profile_path.clone()?,
            })
        })
        .collect();

    writer.record_imports(imports).await.map_err(map_error)?;

    Ok(result)
}
