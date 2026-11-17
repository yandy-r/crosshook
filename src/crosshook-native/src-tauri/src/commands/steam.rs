//! Thin IPC adapters for Steam client discovery and Proton tooling.

use crosshook_core::steam::{ProtonInstall, SteamAutoPopulateRequest, SteamAutoPopulateResult};

use super::{AppState, IpcResult};
use crate::error_map::ipc;

#[tauri::command]
pub async fn default_steam_client_install_path(app: AppState<'_>) -> IpcResult<String> {
    Ok(app.services().steam.default_client_install_path().await)
}

#[tauri::command]
pub async fn list_proton_installs(
    steam_client_install_path: Option<String>,
    app: AppState<'_>,
) -> IpcResult<Vec<ProtonInstall>> {
    ipc(app
        .services()
        .steam
        .list_proton_installs(steam_client_install_path)
        .await)
}

#[tauri::command]
pub async fn auto_populate_steam(
    request: SteamAutoPopulateRequest,
    app: AppState<'_>,
) -> IpcResult<SteamAutoPopulateResult> {
    ipc(app.services().steam.auto_populate(request).await)
}
