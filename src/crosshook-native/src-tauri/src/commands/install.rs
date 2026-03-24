use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crosshook_core::install::{
    install_default_prefix_path as install_default_prefix_path_core,
    install_game as install_game_core, validate_install_request as validate_install_request_core,
    InstallGameRequest, InstallGameResult,
};

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
    let log_path = create_log_path("install", &install_log_target_slug(&request.profile_name))?;
    tauri::async_runtime::spawn_blocking(move || {
        install_game_core(&request, &log_path).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

fn create_log_path(prefix: &str, target_slug: &str) -> Result<PathBuf, String> {
    let log_dir = PathBuf::from("/tmp/crosshook-logs");
    std::fs::create_dir_all(&log_dir).map_err(|error| error.to_string())?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();

    let file_name = format!("{prefix}-{target_slug}-{timestamp}.log");
    let log_path = log_dir.join(file_name);
    std::fs::File::create(&log_path).map_err(|error| error.to_string())?;
    Ok(log_path)
}

fn install_log_target_slug(profile_name: &str) -> String {
    let slug = profile_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();

    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        "install".to_string()
    } else {
        trimmed.to_string()
    }
}
