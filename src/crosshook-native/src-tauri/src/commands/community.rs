//! Thin IPC adapters over the community service.
//!
//! Tap, import, export and supervised sync logic lives in
//! `crosshook_app::services::community`.

use std::path::PathBuf;

use crosshook_core::community::{
    CommunityProfileIndex, CommunityTapSubscription, CommunityTapSyncResult,
};
use crosshook_core::metadata::CommunityProfileRow;
use crosshook_core::profile::{
    CommunityExportResult, CommunityImportPreview, CommunityImportResult,
};

use super::{AppState, IpcResult};
use crate::error_map::ipc;

#[tauri::command]
pub async fn community_add_tap(
    tap: CommunityTapSubscription,
    app: AppState<'_>,
) -> IpcResult<Vec<CommunityTapSubscription>> {
    ipc(app.services().community.add_tap(tap).await)
}

#[tauri::command]
pub async fn community_list_profiles(app: AppState<'_>) -> IpcResult<CommunityProfileIndex> {
    ipc(app.services().community.list_profiles().await)
}

#[tauri::command]
pub async fn community_export_profile(
    profile_name: String,
    output_path: String,
    app: AppState<'_>,
) -> IpcResult<CommunityExportResult> {
    ipc(app
        .services()
        .community
        .export_profile(profile_name, output_path)
        .await)
}

#[tauri::command]
pub async fn community_import_profile(
    path: String,
    app: AppState<'_>,
) -> IpcResult<CommunityImportResult> {
    ipc(app
        .services()
        .community
        .import_profile(PathBuf::from(path))
        .await)
}

#[tauri::command]
pub async fn community_prepare_import(
    path: String,
    app: AppState<'_>,
) -> IpcResult<CommunityImportPreview> {
    ipc(app
        .services()
        .community
        .prepare_import(PathBuf::from(path))
        .await)
}

#[tauri::command]
pub async fn community_sync(app: AppState<'_>) -> IpcResult<Vec<CommunityTapSyncResult>> {
    ipc(app.services().community.sync().await)
}

#[tauri::command]
pub async fn community_list_indexed_profiles(
    app: AppState<'_>,
) -> IpcResult<Vec<CommunityProfileRow>> {
    ipc(app.services().community.list_indexed_profiles().await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = community_add_tap;
        let _ = community_list_profiles;
        let _ = community_export_profile;
        let _ = community_import_profile;
        let _ = community_prepare_import;
        let _ = community_sync;
        let _ = community_list_indexed_profiles;
    }
}
