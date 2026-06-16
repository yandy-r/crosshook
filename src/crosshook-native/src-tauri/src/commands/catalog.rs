use crosshook_core::launch::catalog::{global_catalog, OptimizationEntry};
use crosshook_core::launch::command_arguments::{
    global_catalog as global_command_argument_catalog, CommandArgumentEntry,
};
use crosshook_core::launch::mangohud_presets::{global_mangohud_presets, MangoHudPreset};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct OptimizationCatalogPayload {
    pub catalog_version: u32,
    pub entries: Vec<OptimizationEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandArgumentCatalogPayload {
    pub catalog_version: u32,
    pub entries: Vec<CommandArgumentEntry>,
}

/// Returns the active command-argument catalog (merged default + user overrides).
#[tauri::command]
pub fn get_command_argument_catalog() -> CommandArgumentCatalogPayload {
    let catalog = global_command_argument_catalog();
    CommandArgumentCatalogPayload {
        catalog_version: catalog.catalog_version,
        entries: catalog.entries.clone(),
    }
}

/// Returns the active optimization catalog (merged default + user overrides).
#[tauri::command]
pub fn get_optimization_catalog() -> OptimizationCatalogPayload {
    let catalog = global_catalog();
    OptimizationCatalogPayload {
        catalog_version: catalog.catalog_version,
        entries: catalog.entries.clone(),
    }
}

/// Returns the list of built-in MangoHud display presets.
#[tauri::command]
pub fn get_mangohud_presets() -> Vec<MangoHudPreset> {
    let catalog = global_mangohud_presets();
    catalog.preset.clone()
}
