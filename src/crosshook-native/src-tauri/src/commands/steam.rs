use crosshook_core::steam::{
    attempt_auto_populate, SteamAutoPopulateRequest, SteamAutoPopulateResult,
};

#[tauri::command]
pub async fn auto_populate_steam(
    request: SteamAutoPopulateRequest,
) -> Result<SteamAutoPopulateResult, String> {
    tauri::async_runtime::spawn_blocking(move || attempt_auto_populate(&request))
        .await
        .map_err(|error| error.to_string())
}
