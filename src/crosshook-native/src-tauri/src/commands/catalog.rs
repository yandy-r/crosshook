use crosshook_app::services::{
    CatalogService, CommandArgumentCatalogSnapshot, OptimizationCatalogSnapshot,
};
use crosshook_core::launch::mangohud_presets::MangoHudPreset;

/// Returns the active command-argument catalog (merged default + user overrides).
#[tauri::command]
pub async fn get_command_argument_catalog() -> CommandArgumentCatalogSnapshot {
    CatalogService.command_argument_catalog().await
}

/// Returns the active optimization catalog (merged default + user overrides).
#[tauri::command]
pub async fn get_optimization_catalog() -> OptimizationCatalogSnapshot {
    CatalogService.optimization_catalog().await
}

/// Returns the list of built-in MangoHud display presets.
#[tauri::command]
pub async fn get_mangohud_presets() -> Vec<MangoHudPreset> {
    CatalogService.mangohud_presets().await
}
