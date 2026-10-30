//! Thin IPC adapters over the install-game service.

use crosshook_app::{
    services::{InstallService, RunEvent},
    AppError,
};
use crosshook_core::install::{InstallGameRequest, InstallGameResult};
use tauri::State;

fn map_app_error(error: AppError) -> String {
    error.message().to_string()
}

/// Text returned when the install job ended without a completion.
const MSG_INSTALL_INTERRUPTED: &str = "the installer run ended before reporting a result";

#[tauri::command]
pub async fn install_default_prefix_path(
    profile_name: String,
    service: State<'_, InstallService>,
) -> Result<String, String> {
    service
        .default_prefix_path(profile_name)
        .await
        .map_err(map_app_error)
}

#[tauri::command]
pub async fn validate_install_request(
    request: InstallGameRequest,
    service: State<'_, InstallService>,
) -> Result<(), String> {
    service.validate(request).await.map_err(map_app_error)
}

/// Runs the installer as a supervised job and waits for its result.
#[tauri::command]
pub async fn install_game(
    request: InstallGameRequest,
    service: State<'_, InstallService>,
) -> Result<InstallGameResult, String> {
    let mut run = service.start(request).await.map_err(map_app_error)?;
    // Install has no IPC log events; the installer log file holds the output.
    while let Some(event) = run.next().await {
        if let RunEvent::Finished(completion) = event {
            return match completion {
                Some(result) => result.map_err(map_app_error),
                None => Err(MSG_INSTALL_INTERRUPTED.to_string()),
            };
        }
    }
    Err(MSG_INSTALL_INTERRUPTED.to_string())
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

    #[test]
    fn app_error_maps_to_its_plain_message() {
        let error = AppError::validation("installer_path", "installer path is required");
        assert_eq!(map_app_error(error), "installer path is required");
    }
}
