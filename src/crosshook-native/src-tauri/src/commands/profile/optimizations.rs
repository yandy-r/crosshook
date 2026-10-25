use crosshook_app::services::profiles::{
    BundledOptimizationPresetDto, LaunchOptimizationsPayload, ProfileCommit,
};
use crosshook_app::{AppErrorKind, ProfileWriter};
use crosshook_core::profile::{GameProfile, GamescopeConfig, MangoHudConfig};
use tauri::State;

#[tauri::command]
pub async fn profile_save_launch_optimizations(
    name: String,
    optimizations: LaunchOptimizationsPayload,
    writer: State<'_, ProfileWriter>,
) -> Result<(), String> {
    writer
        .save_launch_optimizations(name, optimizations)
        .await
        .map(ProfileCommit::into_output)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn profile_save_mangohud_config(
    name: String,
    config: MangoHudConfig,
    writer: State<'_, ProfileWriter>,
) -> Result<(), String> {
    writer
        .save_mangohud_config(name, config)
        .await
        .map(ProfileCommit::into_output)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn profile_save_gamescope_config(
    name: String,
    config: GamescopeConfig,
    writer: State<'_, ProfileWriter>,
) -> Result<(), String> {
    writer
        .save_gamescope_config(name, config)
        .await
        .map(ProfileCommit::into_output)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn profile_save_trainer_gamescope_config(
    name: String,
    config: GamescopeConfig,
    writer: State<'_, ProfileWriter>,
) -> Result<(), String> {
    writer
        .save_trainer_gamescope_config(name, config)
        .await
        .map(ProfileCommit::into_output)
        .map_err(|error| error.to_string())
}

/// Lists bundled presets; an unavailable metadata store yields an empty list,
/// as the editor treats "no presets" and "no metadata" alike.
#[tauri::command]
pub async fn profile_list_bundled_optimization_presets(
    writer: State<'_, ProfileWriter>,
) -> Result<Vec<BundledOptimizationPresetDto>, String> {
    match writer.list_bundled_optimization_presets().await {
        Ok(presets) => Ok(presets),
        Err(error) if error.kind() == AppErrorKind::Unavailable => Ok(Vec::new()),
        Err(error) => Err(error.to_string()),
    }
}

#[tauri::command]
pub async fn profile_apply_bundled_optimization_preset(
    name: String,
    preset_id: String,
    writer: State<'_, ProfileWriter>,
) -> Result<GameProfile, String> {
    writer
        .apply_bundled_optimization_preset(name, preset_id)
        .await
        .map(ProfileCommit::into_output)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn profile_save_manual_optimization_preset(
    name: String,
    preset_name: String,
    enabled_option_ids: Vec<String>,
    writer: State<'_, ProfileWriter>,
) -> Result<GameProfile, String> {
    writer
        .save_manual_optimization_preset(name, preset_name, enabled_option_ids)
        .await
        .map(ProfileCommit::into_output)
        .map_err(|error| error.to_string())
}
