use crosshook_app::{AppError, CollectionsService};
use crosshook_core::metadata::CollectionRow;
use crosshook_core::profile::{
    CollectionDefaultsSection, CollectionExportResult, CollectionImportPreview,
};
use tauri::State;

fn map_app_error(error: AppError) -> String {
    error.message().to_string()
}

#[tauri::command]
pub async fn collection_list(
    service: State<'_, CollectionsService>,
) -> Result<Vec<CollectionRow>, String> {
    service.list().await.map_err(map_app_error)
}

#[tauri::command]
pub async fn collection_create(
    name: String,
    service: State<'_, CollectionsService>,
) -> Result<String, String> {
    service.create(name).await.map_err(map_app_error)
}

#[tauri::command]
pub async fn collection_delete(
    collection_id: String,
    service: State<'_, CollectionsService>,
) -> Result<(), String> {
    service.delete(collection_id).await.map_err(map_app_error)
}

#[tauri::command]
pub async fn collection_add_profile(
    collection_id: String,
    profile_name: String,
    service: State<'_, CollectionsService>,
) -> Result<(), String> {
    service
        .add_profile(collection_id, profile_name)
        .await
        .map_err(map_app_error)
}

#[tauri::command]
pub async fn collection_remove_profile(
    collection_id: String,
    profile_name: String,
    service: State<'_, CollectionsService>,
) -> Result<(), String> {
    service
        .remove_profile(collection_id, profile_name)
        .await
        .map_err(map_app_error)
}

#[tauri::command]
pub async fn collection_list_profiles(
    collection_id: String,
    service: State<'_, CollectionsService>,
) -> Result<Vec<String>, String> {
    service
        .list_profiles(collection_id)
        .await
        .map_err(map_app_error)
}

#[tauri::command]
pub async fn collection_rename(
    collection_id: String,
    new_name: String,
    service: State<'_, CollectionsService>,
) -> Result<(), String> {
    service
        .rename(collection_id, new_name)
        .await
        .map_err(map_app_error)
}

#[tauri::command]
pub async fn collection_update_description(
    collection_id: String,
    description: Option<String>,
    service: State<'_, CollectionsService>,
) -> Result<(), String> {
    service
        .update_description(collection_id, description)
        .await
        .map_err(map_app_error)
}

#[tauri::command]
pub async fn collections_for_profile(
    profile_name: String,
    service: State<'_, CollectionsService>,
) -> Result<Vec<CollectionRow>, String> {
    service
        .collections_for_profile(profile_name)
        .await
        .map_err(map_app_error)
}

#[tauri::command]
pub async fn collection_get_defaults(
    collection_id: String,
    service: State<'_, CollectionsService>,
) -> Result<Option<CollectionDefaultsSection>, String> {
    service
        .get_defaults(collection_id)
        .await
        .map_err(map_app_error)
}

#[tauri::command]
pub async fn collection_set_defaults(
    collection_id: String,
    defaults: Option<CollectionDefaultsSection>,
    service: State<'_, CollectionsService>,
) -> Result<(), String> {
    service
        .set_defaults(collection_id, defaults)
        .await
        .map_err(map_app_error)
}

#[tauri::command]
pub async fn collection_export_to_toml(
    collection_id: String,
    output_path: String,
    service: State<'_, CollectionsService>,
) -> Result<CollectionExportResult, String> {
    service
        .export_to_toml(collection_id, output_path)
        .await
        .map_err(map_app_error)
}

#[tauri::command]
pub async fn collection_import_from_toml(
    path: String,
    service: State<'_, CollectionsService>,
) -> Result<CollectionImportPreview, String> {
    service.import_from_toml(path).await.map_err(map_app_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        // Async commands cannot be cast to function pointers; pin their exported names.
        let _ = collection_list;
        let _ = collection_create;
        let _ = collection_delete;
        let _ = collection_add_profile;
        let _ = collection_remove_profile;
        let _ = collection_list_profiles;
        let _ = collection_rename;
        let _ = collection_update_description;
        let _ = collections_for_profile;
        let _ = collection_get_defaults;
        let _ = collection_set_defaults;
        let _ = collection_export_to_toml;
        let _ = collection_import_from_toml;
    }
}
