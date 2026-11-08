//! Thin IPC adapters over the install-game service.

use crosshook_core::install::{InstallGameRequest, InstallGameResult};

use super::{AppState, IpcResult};
use crate::error_map::ipc;

#[tauri::command]
pub async fn install_default_prefix_path(
    profile_name: String,
    app: AppState<'_>,
) -> IpcResult<String> {
    ipc(app
        .services()
        .install
        .default_prefix_path(profile_name)
        .await)
}

#[tauri::command]
pub async fn validate_install_request(
    request: InstallGameRequest,
    app: AppState<'_>,
) -> IpcResult<()> {
    ipc(app.services().install.validate(request).await)
}

/// Runs the installer as a supervised job and waits for its result. Install
/// has no IPC log events; the installer log file holds the output.
#[tauri::command]
pub async fn install_game(
    request: InstallGameRequest,
    app: AppState<'_>,
) -> IpcResult<InstallGameResult> {
    ipc(app.services().install.run_to_completion(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = install_default_prefix_path;
        let _ = validate_install_request;
        let _ = install_game;
    }
}
