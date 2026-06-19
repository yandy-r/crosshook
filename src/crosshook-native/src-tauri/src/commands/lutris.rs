use std::path::PathBuf;

use crosshook_core::metadata::{MetadataStore, SyncSource};
use crosshook_core::profile::lutris_import::{apply_lutris_import, preview_lutris_import};
use crosshook_core::profile::{
    LutrisImportEntry, LutrisImportOutcome, LutrisImportPreview, LutrisImportResult, ProfileStore,
};
use tauri::{AppHandle, State};

use super::profile::emit_profiles_changed;

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
pub fn lutris_import_profiles(
    entries: Vec<LutrisImportEntry>,
    profile_store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
    app: AppHandle,
) -> Result<LutrisImportResult, String> {
    let result = apply_lutris_import(&profile_store.base_path, entries);

    for entry_result in &result.results {
        if entry_result.outcome != LutrisImportOutcome::Imported {
            continue;
        }

        let Some(profile_name) = entry_result.profile_name.as_deref() else {
            continue;
        };
        let Some(profile_path) = entry_result.profile_path.as_deref() else {
            continue;
        };

        if let Err(error) = metadata_store.observe_profile_write(
            profile_name,
            &entry_result.entry.mapped,
            profile_path,
            SyncSource::LutrisImport,
            None,
        ) {
            tracing::warn!(
                %error,
                profile_name,
                "metadata sync after lutris_import_profiles failed"
            );
        }
    }

    if result.imported_count > 0 {
        emit_profiles_changed(&app, "lutris-import");
    }

    Ok(result)
}
