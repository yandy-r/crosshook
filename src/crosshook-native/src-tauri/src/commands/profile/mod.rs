//! Profile Tauri commands, split into per-domain submodules.
//!
//! - `shared`         — helpers, DTOs, and constants shared across submodules
//! - `lifecycle`      — CRUD and import/export commands
//! - `optimizations`  — launch optimization and preset commands
//! - `favorites`      — favorite profile commands
//! - `config_history` — config revision history, diff, rollback, and mark-known-good

mod command_arguments;
mod config_history;
mod favorites;
mod lifecycle;
mod optimizations;
mod shared;

#[cfg(test)]
mod tests;

pub use command_arguments::profile_save_command_arguments;
pub use config_history::{
    profile_config_diff, profile_config_history, profile_config_rollback, profile_mark_known_good,
};
pub use favorites::{profile_list_favorites, profile_set_favorite};
pub use lifecycle::{
    profile_delete, profile_duplicate, profile_export_toml, profile_import_legacy, profile_list,
    profile_list_summaries, profile_load, profile_rename, profile_save,
};
pub use optimizations::{
    profile_apply_bundled_optimization_preset, profile_list_bundled_optimization_presets,
    profile_save_gamescope_config, profile_save_launch_optimizations, profile_save_mangohud_config,
    profile_save_manual_optimization_preset, profile_save_trainer_gamescope_config,
};
pub use shared::capture_config_revision;

// Re-export Tauri command macros so `generate_handler!` can resolve `commands::profile::<name>`.
pub use command_arguments::__cmd__profile_save_command_arguments;
pub use command_arguments::__tauri_command_name_profile_save_command_arguments;
pub use config_history::__cmd__profile_config_diff;
pub use config_history::__cmd__profile_config_history;
pub use config_history::__cmd__profile_config_rollback;
pub use config_history::__cmd__profile_mark_known_good;
pub use config_history::__tauri_command_name_profile_config_diff;
pub use config_history::__tauri_command_name_profile_config_history;
pub use config_history::__tauri_command_name_profile_config_rollback;
pub use config_history::__tauri_command_name_profile_mark_known_good;
pub use favorites::__cmd__profile_list_favorites;
pub use favorites::__cmd__profile_set_favorite;
pub use favorites::__tauri_command_name_profile_list_favorites;
pub use favorites::__tauri_command_name_profile_set_favorite;
pub use lifecycle::__cmd__profile_delete;
pub use lifecycle::__cmd__profile_duplicate;
pub use lifecycle::__cmd__profile_export_toml;
pub use lifecycle::__cmd__profile_import_legacy;
pub use lifecycle::__cmd__profile_list;
pub use lifecycle::__cmd__profile_list_summaries;
pub use lifecycle::__cmd__profile_load;
pub use lifecycle::__cmd__profile_rename;
pub use lifecycle::__cmd__profile_save;
pub use lifecycle::__tauri_command_name_profile_delete;
pub use lifecycle::__tauri_command_name_profile_duplicate;
pub use lifecycle::__tauri_command_name_profile_export_toml;
pub use lifecycle::__tauri_command_name_profile_import_legacy;
pub use lifecycle::__tauri_command_name_profile_list;
pub use lifecycle::__tauri_command_name_profile_list_summaries;
pub use lifecycle::__tauri_command_name_profile_load;
pub use lifecycle::__tauri_command_name_profile_rename;
pub use lifecycle::__tauri_command_name_profile_save;
pub use optimizations::__cmd__profile_apply_bundled_optimization_preset;
pub use optimizations::__cmd__profile_list_bundled_optimization_presets;
pub use optimizations::__cmd__profile_save_gamescope_config;
pub use optimizations::__cmd__profile_save_launch_optimizations;
pub use optimizations::__cmd__profile_save_mangohud_config;
pub use optimizations::__cmd__profile_save_manual_optimization_preset;
pub use optimizations::__cmd__profile_save_trainer_gamescope_config;
pub use optimizations::__tauri_command_name_profile_apply_bundled_optimization_preset;
pub use optimizations::__tauri_command_name_profile_list_bundled_optimization_presets;
pub use optimizations::__tauri_command_name_profile_save_gamescope_config;
pub use optimizations::__tauri_command_name_profile_save_launch_optimizations;
pub use optimizations::__tauri_command_name_profile_save_mangohud_config;
pub use optimizations::__tauri_command_name_profile_save_manual_optimization_preset;
pub use optimizations::__tauri_command_name_profile_save_trainer_gamescope_config;
