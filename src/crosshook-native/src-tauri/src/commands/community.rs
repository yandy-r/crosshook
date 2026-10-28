//! Thin IPC adapters over the community service.
//!
//! Tap, import, export and supervised sync logic lives in
//! `crosshook_app::services::community`; each command only extracts the
//! managed service, awaits the call and maps the sanitized `AppError`
//! message to the IPC error string.

use std::path::PathBuf;

use crosshook_app::services::CommunityService;
use crosshook_app::AppError;
use crosshook_core::community::{
    CommunityProfileIndex, CommunityTapSubscription, CommunityTapSyncResult,
};
use crosshook_core::metadata::CommunityProfileRow;
use crosshook_core::profile::{
    CommunityExportResult, CommunityImportPreview, CommunityImportResult,
};
use tauri::State;

fn map_app_error(error: AppError) -> String {
    error.message().to_string()
}

#[tauri::command]
pub async fn community_add_tap(
    tap: CommunityTapSubscription,
    service: State<'_, CommunityService>,
) -> Result<Vec<CommunityTapSubscription>, String> {
    service.add_tap(tap).await.map_err(map_app_error)
}

#[tauri::command]
pub async fn community_list_profiles(
    service: State<'_, CommunityService>,
) -> Result<CommunityProfileIndex, String> {
    service.list_profiles().await.map_err(map_app_error)
}

#[tauri::command]
pub async fn community_export_profile(
    profile_name: String,
    output_path: String,
    service: State<'_, CommunityService>,
) -> Result<CommunityExportResult, String> {
    service
        .export_profile(profile_name, output_path)
        .await
        .map_err(map_app_error)
}

#[tauri::command]
pub async fn community_import_profile(
    path: String,
    service: State<'_, CommunityService>,
) -> Result<CommunityImportResult, String> {
    service
        .import_profile(PathBuf::from(path))
        .await
        .map_err(map_app_error)
}

#[tauri::command]
pub async fn community_prepare_import(
    path: String,
    service: State<'_, CommunityService>,
) -> Result<CommunityImportPreview, String> {
    service
        .prepare_import(PathBuf::from(path))
        .await
        .map_err(map_app_error)
}

#[tauri::command]
pub async fn community_sync(
    service: State<'_, CommunityService>,
) -> Result<Vec<CommunityTapSyncResult>, String> {
    service.sync().await.map_err(map_app_error)
}

#[tauri::command]
pub async fn community_list_indexed_profiles(
    service: State<'_, CommunityService>,
) -> Result<Vec<CommunityProfileRow>, String> {
    service.list_indexed_profiles().await.map_err(map_app_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        // Async commands cannot be cast to fn pointers; naming them pins the
        // exported symbol (and thus the IPC command name).
        let _ = community_add_tap;
        let _ = community_list_profiles;
        let _ = community_export_profile;
        let _ = community_import_profile;
        let _ = community_prepare_import;
        let _ = community_sync;
        let _ = community_list_indexed_profiles;
    }

    #[test]
    fn app_error_maps_to_its_plain_message() {
        let error = AppError::validation("path", "community profile path is invalid");
        assert_eq!(map_app_error(error), "community profile path is invalid");
    }
}
