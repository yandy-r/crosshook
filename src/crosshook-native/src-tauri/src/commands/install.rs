use crosshook_core::install::{
    install_default_prefix_path as install_default_prefix_path_core,
    install_game as install_game_core, validate_install_request as validate_install_request_core,
    InstallGameRequest, InstallGameResult,
};

use super::shared::{create_log_path, slugify_target};

#[tauri::command]
pub async fn install_default_prefix_path(profile_name: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        install_default_prefix_path_core(&profile_name)
            .map(|path| path.to_string_lossy().into_owned())
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn validate_install_request(request: InstallGameRequest) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        validate_install_request_core(&request).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn install_game(request: InstallGameRequest) -> Result<InstallGameResult, String> {
    let log_path = create_log_path("install", &slugify_target(&request.profile_name, "install"))?;
    tauri::async_runtime::spawn_blocking(move || {
        install_game_core(&request, &log_path).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}
