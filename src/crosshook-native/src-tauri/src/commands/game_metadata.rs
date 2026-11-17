//! Thin IPC adapters for Steam game metadata and cover art.
//! All logic lives in [`crosshook_app::services::steam`].

use crosshook_app::ipc::unavailable_or;
use crosshook_core::steam_metadata::SteamMetadataLookupResult;

use super::{AppState, IpcResult};
use crate::error_map::ipc;

#[tauri::command]
pub async fn fetch_game_metadata(
    app_id: String,
    force_refresh: Option<bool>,
    app: AppState<'_>,
) -> IpcResult<SteamMetadataLookupResult> {
    ipc(app
        .services()
        .steam
        .fetch_game_metadata(app_id, force_refresh)
        .await)
}

/// Cover-art lookup for a Steam app id. A service `Unavailable` (core found no
/// usable art: no cache and the network fetch failed) maps to `Ok(None)` per
/// the frozen IPC contract (D12); cached art is always returned first.
#[tauri::command]
pub async fn fetch_game_cover_art(
    app_id: String,
    image_type: Option<String>,
    app: AppState<'_>,
) -> IpcResult<Option<String>> {
    ipc(unavailable_or(
        app.services()
            .steam
            .fetch_cover_art(app_id, image_type)
            .await,
        None,
    ))
}

#[tauri::command]
pub async fn import_custom_art(
    source_path: String,
    art_type: Option<String>,
    app: AppState<'_>,
) -> IpcResult<String> {
    ipc(app
        .services()
        .steam
        .import_custom_art(source_path, art_type)
        .await)
}
