use std::path::PathBuf;

use crosshook_core::steam::{
    attempt_auto_populate, discover_compat_tools, discover_steam_root_candidates, ProtonInstall,
    SteamAutoPopulateRequest, SteamAutoPopulateResult,
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
pub fn list_proton_installs(
    steam_client_install_path: Option<String>,
) -> Result<Vec<ProtonInstall>, String> {
    let configured_path =
        steam_client_install_path.unwrap_or_else(default_steam_client_install_path);
    let mut diagnostics = Vec::new();
    let steam_root_candidates = discover_steam_root_candidates(configured_path, &mut diagnostics);
    let installs = discover_compat_tools(&steam_root_candidates, &mut diagnostics);

    for entry in &diagnostics {
        tracing::debug!(entry, "proton discovery diagnostic");
    }

    Ok(installs)
}

#[tauri::command]
pub async fn auto_populate_steam(
    request: SteamAutoPopulateRequest,
) -> Result<SteamAutoPopulateResult, String> {
    tauri::async_runtime::spawn_blocking(move || attempt_auto_populate(&request))
        .await
        .map_err(|error| error.to_string())
}
