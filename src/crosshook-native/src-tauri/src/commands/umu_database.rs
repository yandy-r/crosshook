//! Thin IPC adapters over the umu database service.

use crosshook_core::umu_database;

use super::{AppState, IpcResult};
use crate::error_map::ipc;

#[tauri::command]
pub async fn refresh_umu_database(
    app: AppState<'_>,
) -> IpcResult<umu_database::UmuDatabaseRefreshStatus> {
    ipc(app.services().umu.refresh().await)
}

#[tauri::command]
pub async fn check_umu_coverage(
    app_id: Option<String>,
    app: AppState<'_>,
) -> IpcResult<umu_database::CsvCoverage> {
    ipc(app.services().umu.coverage(app_id).await)
}

#[tauri::command]
pub async fn clear_umu_gameid_lookup_cache(app: AppState<'_>) -> IpcResult<usize> {
    ipc(app.services().umu.clear_gameid_cache().await)
}
