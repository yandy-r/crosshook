use super::{AppState, IpcResult};
use crosshook_app::services::{CommandArgumentCatalogSnapshot, OptimizationCatalogSnapshot};
use crosshook_core::launch::mangohud_presets::MangoHudPreset;

#[tauri::command]
pub async fn get_command_argument_catalog(
    app: AppState<'_>,
) -> IpcResult<CommandArgumentCatalogSnapshot> {
    Ok(app.services().catalog.command_argument_catalog().await)
}

#[tauri::command]
pub async fn get_optimization_catalog(app: AppState<'_>) -> IpcResult<OptimizationCatalogSnapshot> {
    Ok(app.services().catalog.optimization_catalog().await)
}

/// Returns the list of built-in MangoHud display presets.
#[tauri::command]
pub async fn get_mangohud_presets(app: AppState<'_>) -> IpcResult<Vec<MangoHudPreset>> {
    Ok(app.services().catalog.mangohud_presets().await)
}
