//! Thin IPC adapters over the discovery service.

use crosshook_core::discovery::{
    CatalogPage, CatalogQuery, ExternalTrainerSearchQuery, ExternalTrainerSearchResponse,
    ExternalTrainerSourceSubscription, VersionMatchResult,
};

use super::{AppState, IpcResult};
use crate::error_map::ipc;

#[tauri::command]
pub async fn discovery_catalog(query: CatalogQuery, app: AppState<'_>) -> IpcResult<CatalogPage> {
    ipc(app.services().discovery.catalog(query).await)
}

#[tauri::command]
pub async fn discovery_search_external(
    query: ExternalTrainerSearchQuery,
    app: AppState<'_>,
) -> IpcResult<ExternalTrainerSearchResponse> {
    ipc(app.services().discovery.search_external(query).await)
}

#[tauri::command]
pub async fn discovery_check_version_compatibility(
    profile_name: String,
    trainer_game_version: Option<String>,
    app: AppState<'_>,
) -> IpcResult<VersionMatchResult> {
    ipc(app
        .services()
        .discovery
        .check_version_compatibility(profile_name, trainer_game_version)
        .await)
}

#[tauri::command]
pub async fn discovery_list_external_sources(
    app: AppState<'_>,
) -> IpcResult<Vec<ExternalTrainerSourceSubscription>> {
    ipc(app.services().discovery.list_external_sources().await)
}

#[tauri::command]
pub async fn discovery_add_external_source(
    source: ExternalTrainerSourceSubscription,
    app: AppState<'_>,
) -> IpcResult<Vec<ExternalTrainerSourceSubscription>> {
    ipc(app.services().discovery.add_external_source(source).await)
}

#[tauri::command]
pub async fn discovery_remove_external_source(
    source_id: String,
    app: AppState<'_>,
) -> IpcResult<Vec<ExternalTrainerSourceSubscription>> {
    ipc(app
        .services()
        .discovery
        .remove_external_source(source_id)
        .await)
}
