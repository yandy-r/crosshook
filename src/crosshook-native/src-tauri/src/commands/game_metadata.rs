//! Thin IPC adapters for Steam game metadata and cover art.
//! All logic lives in [`crosshook_app::services::steam`].

use crosshook_app::services::SteamService;
use crosshook_app::{AppError, AppErrorKind};
use crosshook_core::steam_metadata::SteamMetadataLookupResult;
use tauri::State;

fn map_error(error: AppError) -> String {
    error.message().to_string()
}

#[tauri::command]
pub async fn fetch_game_metadata(
    app_id: String,
    force_refresh: Option<bool>,
    service: State<'_, SteamService>,
) -> Result<SteamMetadataLookupResult, String> {
    service
        .fetch_game_metadata(app_id, force_refresh)
        .await
        .map_err(map_error)
}

/// Cover-art lookup for a Steam app id. A service `Unavailable` (core found no
/// usable art: no cache and the network fetch failed) maps to `Ok(None)` per
/// the frozen IPC contract (D12); cached art is always returned first.
#[tauri::command]
pub async fn fetch_game_cover_art(
    app_id: String,
    image_type: Option<String>,
    service: State<'_, SteamService>,
) -> Result<Option<String>, String> {
    match service.fetch_cover_art(app_id, image_type).await {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == AppErrorKind::Unavailable => Ok(None),
        Err(error) => Err(error.message().to_string()),
    }
}

#[tauri::command]
pub async fn import_custom_cover_art(
    source_path: String,
    service: State<'_, SteamService>,
) -> Result<String, String> {
    service
        .import_custom_cover_art(source_path)
        .await
        .map_err(map_error)
}

#[tauri::command]
pub async fn import_custom_art(
    source_path: String,
    art_type: Option<String>,
    service: State<'_, SteamService>,
) -> Result<String, String> {
    service
        .import_custom_art(source_path, art_type)
        .await
        .map_err(map_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        // Async commands cannot be cast to fn pointers; naming them pins the
        // exported symbol (and thus the IPC command name).
        let _ = fetch_game_metadata;
        let _ = fetch_game_cover_art;
        let _ = import_custom_cover_art;
        let _ = import_custom_art;
    }
}
