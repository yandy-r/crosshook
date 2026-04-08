use crosshook_core::metadata::{CollectionRow, MetadataStore};
use tauri::State;

fn map_error(e: impl ToString) -> String {
    e.to_string()
}

#[tauri::command]
pub fn collection_list(
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<CollectionRow>, String> {
    metadata_store.list_collections().map_err(map_error)
}

#[tauri::command]
pub fn collection_create(
    name: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<String, String> {
    metadata_store.create_collection(&name).map_err(map_error)
}

#[tauri::command]
pub fn collection_delete(
    collection_id: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    metadata_store
        .delete_collection(&collection_id)
        .map_err(map_error)
}

#[tauri::command]
pub fn collection_add_profile(
    collection_id: String,
    profile_name: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    metadata_store
        .add_profile_to_collection(&collection_id, &profile_name)
        .map_err(map_error)
}

#[tauri::command]
pub fn collection_remove_profile(
    collection_id: String,
    profile_name: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    metadata_store
        .remove_profile_from_collection(&collection_id, &profile_name)
        .map_err(map_error)
}

#[tauri::command]
pub fn collection_list_profiles(
    collection_id: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<String>, String> {
    metadata_store
        .list_profiles_in_collection(&collection_id)
        .map_err(map_error)
}

#[tauri::command]
pub fn collection_rename(
    collection_id: String,
    new_name: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    metadata_store
        .rename_collection(&collection_id, &new_name)
        .map_err(map_error)
}

#[tauri::command]
pub fn collection_update_description(
    collection_id: String,
    description: Option<String>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    metadata_store
        .update_collection_description(&collection_id, description.as_deref())
        .map_err(map_error)
}

#[tauri::command]
pub fn collections_for_profile(
    profile_name: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<CollectionRow>, String> {
    metadata_store
        .collections_for_profile(&profile_name)
        .map_err(map_error)
}
