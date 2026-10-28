//! Thin IPC adapters over the discovery service.

use crosshook_app::services::DiscoveryService;
use crosshook_app::AppError;
use crosshook_core::discovery::{
    CatalogPage, CatalogQuery, ExternalTrainerSearchQuery, ExternalTrainerSearchResponse,
    ExternalTrainerSourceSubscription, VersionMatchResult,
};
use tauri::State;

fn map_app_error(error: AppError) -> String {
    error.message().to_string()
}

#[tauri::command]
pub async fn discovery_catalog(
    query: CatalogQuery,
    service: State<'_, DiscoveryService>,
) -> Result<CatalogPage, String> {
    service.catalog(query).await.map_err(map_app_error)
}

#[tauri::command]
pub async fn discovery_search_external(
    query: ExternalTrainerSearchQuery,
    service: State<'_, DiscoveryService>,
) -> Result<ExternalTrainerSearchResponse, String> {
    service.search_external(query).await.map_err(map_app_error)
}

#[tauri::command]
pub async fn discovery_check_version_compatibility(
    profile_name: String,
    trainer_game_version: Option<String>,
    service: State<'_, DiscoveryService>,
) -> Result<VersionMatchResult, String> {
    service
        .check_version_compatibility(profile_name, trainer_game_version)
        .await
        .map_err(map_app_error)
}

#[tauri::command]
pub async fn discovery_list_external_sources(
    service: State<'_, DiscoveryService>,
) -> Result<Vec<ExternalTrainerSourceSubscription>, String> {
    service.list_external_sources().await.map_err(map_app_error)
}

#[tauri::command]
pub async fn discovery_add_external_source(
    source: ExternalTrainerSourceSubscription,
    service: State<'_, DiscoveryService>,
) -> Result<Vec<ExternalTrainerSourceSubscription>, String> {
    service
        .add_external_source(source)
        .await
        .map_err(map_app_error)
}

#[tauri::command]
pub async fn discovery_remove_external_source(
    source_id: String,
    service: State<'_, DiscoveryService>,
) -> Result<Vec<ExternalTrainerSourceSubscription>, String> {
    service
        .remove_external_source(source_id)
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
        let _ = discovery_catalog;
        let _ = discovery_search_external;
        let _ = discovery_check_version_compatibility;
        let _ = discovery_list_external_sources;
        let _ = discovery_add_external_source;
        let _ = discovery_remove_external_source;
    }
}
