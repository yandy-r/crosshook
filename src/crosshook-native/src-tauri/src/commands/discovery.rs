//! Thin IPC adapters over the discovery service.

use crosshook_core::discovery::{
    CatalogPage, CatalogQuery, ExternalTrainerSearchQuery, ExternalTrainerSearchResponse,
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
