use crosshook_core::metadata::MetadataStore;
use tauri::{AppHandle, State};

use super::shared::emit_profiles_changed;

#[tauri::command]
pub fn profile_set_favorite(
    name: String,
    favorite: bool,
    app: AppHandle,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    metadata_store
        .set_profile_favorite(&name, favorite)
        .map_err(|e| e.to_string())?;
    emit_profiles_changed(&app, "favorite-updated");
    Ok(())
}

#[tauri::command]
pub fn profile_list_favorites(
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<String>, String> {
    metadata_store
        .list_favorite_profiles()
        .map_err(|e| e.to_string())
}
