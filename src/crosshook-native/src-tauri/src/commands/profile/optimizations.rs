use crosshook_core::metadata::{
    BundledOptimizationPresetRow, ConfigRevisionSource, MetadataStore, ProfileLaunchPresetOrigin,
};
use crosshook_core::profile::{
    bundled_optimization_preset_toml_key, GamescopeConfig, MangoHudConfig, ProfileStore,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use super::shared::{
    capture_config_revision, emit_profiles_changed, map_error, observe_profile_write_launch_change,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundledOptimizationPresetDto {
    pub preset_id: String,
    pub display_name: String,
    pub vendor: String,
    pub mode: String,
    pub enabled_option_ids: Vec<String>,
    pub catalog_version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LaunchOptimizationsPayload {
    #[serde(
        rename = "enabled_option_ids",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub enabled_option_ids: Vec<String>,
    /// When set, selects that named preset from `launch.presets` and ignores `enabled_option_ids`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub switch_active_preset: Option<String>,
}

fn bundled_row_to_dto(
    row: BundledOptimizationPresetRow,
) -> Result<BundledOptimizationPresetDto, String> {
    let enabled_option_ids: Vec<String> = serde_json::from_str(&row.option_ids_json)
        .map_err(|e| format!("corrupt bundled preset {} option list: {e}", row.preset_id))?;
    Ok(BundledOptimizationPresetDto {
        preset_id: row.preset_id,
        display_name: row.display_name,
        vendor: row.vendor,
        mode: row.mode,
        enabled_option_ids,
        catalog_version: row.catalog_version,
    })
}

pub(super) fn save_launch_optimizations_for_profile(
    name: &str,
    optimizations: &LaunchOptimizationsPayload,
    store: &ProfileStore,
) -> Result<(), String> {
    store
        .save_launch_optimizations(
            name,
            optimizations.enabled_option_ids.clone(),
            optimizations.switch_active_preset.clone(),
        )
        .map_err(map_error)
}

#[tauri::command]
pub fn profile_save_launch_optimizations(
    name: String,
    optimizations: LaunchOptimizationsPayload,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    save_launch_optimizations_for_profile(&name, &optimizations, &store)?;

    if let Ok(updated) = store.load(&name) {
        let profile_path = store.base_path.join(format!("{name}.toml"));
        if let Err(e) = metadata_store.observe_profile_write(
            &name,
            &updated,
            &profile_path,
            crosshook_core::metadata::SyncSource::AppWrite,
            None,
        ) {
            tracing::warn!(
                %e,
                profile_name = %name,
                "metadata sync after save_launch_optimizations failed"
            );
        }
        capture_config_revision(
            &name,
            &updated,
            ConfigRevisionSource::LaunchOptimizationSave,
            None,
            &metadata_store,
        );
    }

    Ok(())
}

#[tauri::command]
pub fn profile_save_mangohud_config(
    name: String,
    config: MangoHudConfig,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    let mut profile = store.load(&name).map_err(|e| e.to_string())?;
    profile.launch.mangohud = config;
    store.save(&name, &profile).map_err(|e| e.to_string())?;
    let profile_path = store.base_path.join(format!("{name}.toml"));
    if let Err(e) = metadata_store.observe_profile_write(
        &name,
        &profile,
        &profile_path,
        crosshook_core::metadata::SyncSource::AppWrite,
        None,
    ) {
        tracing::warn!(%e, profile_name = %name, "metadata sync after profile_save_mangohud_config failed");
    }
    capture_config_revision(
        &name,
        &profile,
        ConfigRevisionSource::ManualSave,
        None,
        &metadata_store,
    );
    Ok(())
}

#[tauri::command]
pub fn profile_save_gamescope_config(
    name: String,
    config: GamescopeConfig,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    let mut profile = store.load(&name).map_err(|e| e.to_string())?;
    profile.launch.gamescope = config;
    store.save(&name, &profile).map_err(|e| e.to_string())?;
    let profile_path = store.base_path.join(format!("{name}.toml"));
    if let Err(e) = metadata_store.observe_profile_write(
        &name,
        &profile,
        &profile_path,
        crosshook_core::metadata::SyncSource::AppWrite,
        None,
    ) {
        tracing::warn!(%e, profile_name = %name, "metadata sync after profile_save_gamescope_config failed");
    }
    capture_config_revision(
        &name,
        &profile,
        ConfigRevisionSource::ManualSave,
        None,
        &metadata_store,
    );
    Ok(())
}

#[tauri::command]
pub fn profile_save_trainer_gamescope_config(
    name: String,
    config: GamescopeConfig,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    let mut profile = store.load(&name).map_err(|e| e.to_string())?;
    profile.launch.trainer_gamescope = config;
    store.save(&name, &profile).map_err(|e| e.to_string())?;
    let profile_path = store.base_path.join(format!("{name}.toml"));
    if let Err(e) = metadata_store.observe_profile_write(
        &name,
        &profile,
        &profile_path,
        crosshook_core::metadata::SyncSource::AppWrite,
        None,
    ) {
        tracing::warn!(%e, profile_name = %name, "metadata sync after profile_save_trainer_gamescope_config failed");
    }
    capture_config_revision(
        &name,
        &profile,
        ConfigRevisionSource::ManualSave,
        None,
        &metadata_store,
    );
    Ok(())
}

#[tauri::command]
pub fn profile_list_bundled_optimization_presets(
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<BundledOptimizationPresetDto>, String> {
    if !metadata_store.is_available() {
        return Ok(Vec::new());
    }
    let rows = metadata_store
        .list_bundled_optimization_presets()
        .map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(bundled_row_to_dto(row)?);
    }
    Ok(out)
}

#[tauri::command]
pub fn profile_apply_bundled_optimization_preset(
    name: String,
    preset_id: String,
    app: AppHandle,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<crosshook_core::profile::GameProfile, String> {
    if !metadata_store.is_available() {
        return Err("metadata store is unavailable — cannot apply bundled presets".to_string());
    }

    let profile_name = name.trim();
    if profile_name.is_empty() {
        return Err("profile name is required".to_string());
    }

    let pid = preset_id.trim();
    if pid.is_empty() {
        return Err("preset_id is required".to_string());
    }

    let row = metadata_store
        .get_bundled_optimization_preset(pid)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("unknown bundled preset: {pid}"))?;

    let enabled_option_ids: Vec<String> =
        serde_json::from_str(&row.option_ids_json).map_err(|e| e.to_string())?;

    let toml_key = bundled_optimization_preset_toml_key(&row.preset_id);
    store
        .materialize_launch_optimization_preset(profile_name, &toml_key, enabled_option_ids, true)
        .map_err(map_error)?;

    let updated = store.load(profile_name).map_err(map_error)?;
    observe_profile_write_launch_change(profile_name, &store, &metadata_store, &updated);
    capture_config_revision(
        profile_name,
        &updated,
        ConfigRevisionSource::PresetApply,
        None,
        &metadata_store,
    );

    if let Ok(Some(profile_id)) = metadata_store.lookup_profile_id(profile_name) {
        if let Err(e) = metadata_store.upsert_profile_launch_preset_metadata(
            &profile_id,
            &toml_key,
            ProfileLaunchPresetOrigin::Bundled,
            Some(row.preset_id.as_str()),
        ) {
            tracing::warn!(
                %e,
                profile_name = %profile_name,
                "failed to upsert bundled preset metadata row"
            );
        }
    }

    emit_profiles_changed(&app, "bundled-optimization-preset");
    Ok(updated)
}

#[tauri::command]
pub fn profile_save_manual_optimization_preset(
    name: String,
    preset_name: String,
    enabled_option_ids: Vec<String>,
    app: AppHandle,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<crosshook_core::profile::GameProfile, String> {
    let profile_name = name.trim();
    if profile_name.is_empty() {
        return Err("profile name is required".to_string());
    }

    let key = preset_name.trim();
    if key.is_empty() {
        return Err("preset name must not be empty".to_string());
    }

    store
        .save_manual_launch_optimization_preset(profile_name, key, enabled_option_ids)
        .map_err(map_error)?;

    let updated = store.load(profile_name).map_err(map_error)?;
    observe_profile_write_launch_change(profile_name, &store, &metadata_store, &updated);
    capture_config_revision(
        profile_name,
        &updated,
        ConfigRevisionSource::PresetApply,
        None,
        &metadata_store,
    );

    if metadata_store.is_available() {
        if let Ok(Some(profile_id)) = metadata_store.lookup_profile_id(profile_name) {
            if let Err(e) = metadata_store.upsert_profile_launch_preset_metadata(
                &profile_id,
                key,
                ProfileLaunchPresetOrigin::User,
                None,
            ) {
                tracing::warn!(
                    %e,
                    profile_name = %profile_name,
                    "failed to upsert user preset metadata row"
                );
            }
        }
    }

    emit_profiles_changed(&app, "manual-optimization-preset");
    Ok(updated)
}
