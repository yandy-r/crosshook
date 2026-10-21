//! Catalog service: read-only snapshots of the process-global catalogs.
//!
//! Catalogs live in in-memory `OnceLock`s initialized at startup, so reads do
//! no I/O and need no blocking pool.

use crosshook_core::launch::catalog::{global_catalog, OptimizationEntry};
use crosshook_core::launch::command_arguments::{
    global_catalog as global_command_argument_catalog, CommandArgumentEntry,
};
use crosshook_core::launch::mangohud_presets::{global_mangohud_presets, MangoHudPreset};
use serde::Serialize;

/// Active optimization catalog (merged default + user overrides).
#[derive(Debug, Clone, Serialize)]
pub struct OptimizationCatalogSnapshot {
    /// Catalog schema version.
    pub catalog_version: u32,
    /// Catalog entries in catalog order.
    pub entries: Vec<OptimizationEntry>,
}

/// Active command-argument catalog (merged default + user overrides).
#[derive(Debug, Clone, Serialize)]
pub struct CommandArgumentCatalogSnapshot {
    /// Catalog schema version.
    pub catalog_version: u32,
    /// Catalog entries in catalog order.
    pub entries: Vec<CommandArgumentEntry>,
}

/// Catalog service. Stateless; reads the process-global catalogs.
#[derive(Debug, Clone, Copy, Default)]
pub struct CatalogService;

impl CatalogService {
    /// Returns the active optimization catalog.
    pub async fn optimization_catalog(&self) -> OptimizationCatalogSnapshot {
        let catalog = global_catalog();
        OptimizationCatalogSnapshot {
            catalog_version: catalog.catalog_version,
            entries: catalog.entries.clone(),
        }
    }

    /// Returns the active command-argument catalog.
    pub async fn command_argument_catalog(&self) -> CommandArgumentCatalogSnapshot {
        let catalog = global_command_argument_catalog();
        CommandArgumentCatalogSnapshot {
            catalog_version: catalog.catalog_version,
            entries: catalog.entries.clone(),
        }
    }

    /// Returns the built-in MangoHud display presets.
    pub async fn mangohud_presets(&self) -> Vec<MangoHudPreset> {
        global_mangohud_presets().preset.clone()
    }
}
