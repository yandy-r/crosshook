use crosshook_core::metadata::CollectionRow;
use crosshook_core::profile::{
    CollectionDefaultsSection, CollectionExportResult, CollectionImportPreview,
};

use super::{AppState, IpcResult};
use crate::error_map::ipc;

#[tauri::command]
pub async fn collection_list(app: AppState<'_>) -> IpcResult<Vec<CollectionRow>> {
    ipc(app.services().collections.list().await)
}

#[tauri::command]
pub async fn collection_create(name: String, app: AppState<'_>) -> IpcResult<String> {
    ipc(app.services().collections.create(name).await)
}

#[tauri::command]
pub async fn collection_delete(collection_id: String, app: AppState<'_>) -> IpcResult<()> {
    ipc(app.services().collections.delete(collection_id).await)
}

#[tauri::command]
pub async fn collection_add_profile(
    collection_id: String,
    profile_name: String,
    app: AppState<'_>,
) -> IpcResult<()> {
    ipc(app
        .services()
        .collections
        .add_profile(collection_id, profile_name)
        .await)
}

#[tauri::command]
pub async fn collection_remove_profile(
    collection_id: String,
    profile_name: String,
    app: AppState<'_>,
) -> IpcResult<()> {
    ipc(app
        .services()
        .collections
        .remove_profile(collection_id, profile_name)
        .await)
}

#[tauri::command]
pub async fn collection_list_profiles(
    collection_id: String,
    app: AppState<'_>,
) -> IpcResult<Vec<String>> {
    ipc(app
        .services()
        .collections
        .list_profiles(collection_id)
        .await)
}

#[tauri::command]
pub async fn collection_rename(
    collection_id: String,
    new_name: String,
    app: AppState<'_>,
) -> IpcResult<()> {
    ipc(app
        .services()
        .collections
        .rename(collection_id, new_name)
        .await)
}

#[tauri::command]
pub async fn collection_update_description(
    collection_id: String,
    description: Option<String>,
    app: AppState<'_>,
) -> IpcResult<()> {
    ipc(app
        .services()
        .collections
        .update_description(collection_id, description)
        .await)
}

#[tauri::command]
pub async fn collections_for_profile(
    profile_name: String,
    app: AppState<'_>,
) -> IpcResult<Vec<CollectionRow>> {
    ipc(app
        .services()
        .collections
        .collections_for_profile(profile_name)
        .await)
}

#[tauri::command]
pub async fn collection_get_defaults(
    collection_id: String,
    app: AppState<'_>,
) -> IpcResult<Option<CollectionDefaultsSection>> {
    ipc(app.services().collections.get_defaults(collection_id).await)
}

#[tauri::command]
pub async fn collection_set_defaults(
    collection_id: String,
    defaults: Option<CollectionDefaultsSection>,
    app: AppState<'_>,
) -> IpcResult<()> {
    ipc(app
        .services()
        .collections
        .set_defaults(collection_id, defaults)
        .await)
}

#[tauri::command]
pub async fn collection_export_to_toml(
    collection_id: String,
    output_path: String,
    app: AppState<'_>,
) -> IpcResult<CollectionExportResult> {
    ipc(app
        .services()
        .collections
        .export_to_toml(collection_id, output_path)
        .await)
}

#[tauri::command]
pub async fn collection_import_from_toml(
    path: String,
    app: AppState<'_>,
) -> IpcResult<CollectionImportPreview> {
    ipc(app.services().collections.import_from_toml(path).await)
}
