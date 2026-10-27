//! Launcher export service: validate, export, preview, list, rename and delete
//! external launcher scripts and desktop entries.

#![expect(
    dead_code,
    reason = "B2 fills the methods; B4/B5 removes this once everything is wired"
)]

mod export;
mod list;
mod preview;

use std::collections::BTreeMap;

use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::{GamescopeConfig, ProfileStore};
use crosshook_core::settings::SettingsStore;

/// Async application service over launcher export, preview, list and rename.
#[derive(Clone)]
pub struct LauncherExportService {
    profiles: ProfileStore,
    settings: SettingsStore,
    metadata: MetadataStore,
}

impl LauncherExportService {
    /// Builds the service from shared stores.
    pub fn new(profiles: ProfileStore, settings: SettingsStore, metadata: MetadataStore) -> Self {
        Self {
            profiles,
            settings,
            metadata,
        }
    }
}

/// Inputs of a launcher rename. The IPC command keeps its 15 flat arguments;
/// the adapter packs them into this struct.
#[derive(Debug, Clone)]
pub struct LauncherRenameRequest {
    /// Slug of the launcher being renamed.
    pub old_launcher_slug: String,
    /// New display name.
    pub new_display_name: String,
    /// New launcher icon path.
    pub new_launcher_icon_path: String,
    /// Home directory that receives the launcher files.
    pub target_home_path: String,
    /// Steam client install path.
    pub steam_client_install_path: String,
    /// Launch method (`steam_applaunch` or `proton_run`).
    pub method: String,
    /// Trainer path.
    pub trainer_path: String,
    /// Trainer loading mode text.
    pub trainer_loading_mode: String,
    /// Proton prefix path.
    pub prefix_path: String,
    /// Proton executable path.
    pub proton_path: String,
    /// Steam app id.
    pub steam_app_id: String,
    /// Launcher name.
    pub launcher_name: String,
    /// Whether network isolation is enabled.
    pub network_isolation: bool,
    /// Gamescope wrapper configuration.
    pub gamescope: GamescopeConfig,
    /// Custom environment variables (ordered).
    pub custom_env_vars: BTreeMap<String, String>,
}

static_assertions::assert_impl_all!(LauncherExportService: Send, Sync);

#[cfg(test)]
mod tests;
