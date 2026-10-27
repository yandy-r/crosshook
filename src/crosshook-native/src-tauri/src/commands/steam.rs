//! Thin IPC adapters for Steam client discovery and Proton tooling.
//! All logic lives in [`crosshook_app::services::steam`]; the resolver and
//! Proton scan run on the blocking pool inside the service (D11).

use crosshook_app::services::SteamService;
use crosshook_core::steam::{ProtonInstall, SteamAutoPopulateRequest, SteamAutoPopulateResult};
use tauri::State;

#[tauri::command]
pub async fn default_steam_client_install_path(
    service: State<'_, SteamService>,
) -> Result<String, String> {
    Ok(service.default_client_install_path().await)
}

#[tauri::command]
pub async fn list_proton_installs(
    steam_client_install_path: Option<String>,
    service: State<'_, SteamService>,
) -> Result<Vec<ProtonInstall>, String> {
    service
        .list_proton_installs(steam_client_install_path)
        .await
        .map_err(|error| error.message().to_string())
}

#[tauri::command]
pub async fn auto_populate_steam(
    request: SteamAutoPopulateRequest,
    service: State<'_, SteamService>,
) -> Result<SteamAutoPopulateResult, String> {
    service
        .auto_populate(request)
        .await
        .map_err(|error| error.message().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        // Async commands cannot be cast to fn pointers; naming them pins the
        // exported symbol (and thus the IPC command name).
        let _ = default_steam_client_install_path;
        let _ = list_proton_installs;
        let _ = auto_populate_steam;
    }
}
