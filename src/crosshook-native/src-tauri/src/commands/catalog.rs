use crosshook_core::launch::catalog::{global_catalog, OptimizationEntry};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct OptimizationCatalogPayload {
    pub catalog_version: u32,
    pub entries: Vec<OptimizationEntry>,
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
