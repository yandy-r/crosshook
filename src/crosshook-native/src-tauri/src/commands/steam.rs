use std::path::PathBuf;

use crosshook_core::steam::{
    attempt_auto_populate, SteamAutoPopulateRequest, SteamAutoPopulateResult,
};

#[tauri::command]
pub fn default_steam_client_install_path() -> String {
    if let Ok(value) = std::env::var("STEAM_COMPAT_CLIENT_INSTALL_PATH") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return String::new();
    };

    for candidate in [
        home.join(".local/share/Steam"),
        home.join(".steam/root"),
        home.join(".var/app/com.valvesoftware.Steam/data/Steam"),
    ] {
        if candidate.join("steamapps").is_dir() {
            return candidate.to_string_lossy().into_owned();
        }
    }

    String::new()
}

#[tauri::command]
pub async fn auto_populate_steam(
    request: SteamAutoPopulateRequest,
) -> Result<SteamAutoPopulateResult, String> {
    tauri::async_runtime::spawn_blocking(move || attempt_auto_populate(&request))
        .await
        .map_err(|error| error.to_string())
}
