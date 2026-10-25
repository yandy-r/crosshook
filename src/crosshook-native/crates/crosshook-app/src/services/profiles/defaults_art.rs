//! New-profile creation defaults and custom-art auto-import for full saves
//! (moved from the Tauri `profile_save` command).

use crosshook_core::game_images::{
    import_custom_art, import_custom_cover_art, is_in_managed_media_dir, GameImageType,
};
use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::{
    apply_profile_creation_defaults_from_settings, bundled_optimization_preset_toml_key,
    GameProfile, LaunchOptimizationsSection,
};
use crosshook_core::settings::AppSettingsData;

/// Applies settings creation defaults to a **new** profile, then the default
/// bundled optimization preset when the draft has not selected one (an
/// explicit wizard selection is never clobbered). Preset lookup is fail-soft.
pub(super) fn apply_new_profile_defaults(
    profile: &mut GameProfile,
    settings: &AppSettingsData,
    metadata: &MetadataStore,
) {
    apply_profile_creation_defaults_from_settings(profile, settings);

    let pid = settings.default_bundled_optimization_preset_id.trim();
    if pid.is_empty() || !metadata.is_available() || !profile.launch.active_preset.trim().is_empty()
    {
        return;
    }
    match metadata.get_bundled_optimization_preset(pid) {
        Ok(Some(row)) => {
            let enabled_option_ids: Vec<String> =
                serde_json::from_str(&row.option_ids_json).unwrap_or_default();
            let toml_key = bundled_optimization_preset_toml_key(pid);
            profile.launch.presets.insert(
                toml_key.clone(),
                LaunchOptimizationsSection {
                    extra: toml::Table::new(),
                    enabled_option_ids: enabled_option_ids.clone(),
                },
            );
            profile.launch.active_preset = toml_key;
            profile.launch.optimizations = LaunchOptimizationsSection {
                extra: toml::Table::new(),
                enabled_option_ids,
            };
        }
        Ok(None) => {
            tracing::debug!(
                preset_id = %pid,
                "default bundled optimization preset not found in metadata; skipping"
            );
        }
        Err(e) => {
            tracing::warn!(
                %e,
                preset_id = %pid,
                "failed to read default bundled optimization preset from metadata"
            );
        }
    }
}

/// Copies cover, portrait and background art that point outside the managed
/// media directory into it. A failed import keeps the original path.
pub(super) fn auto_import_custom_art(profile_name: &str, profile: &mut GameProfile) {
    let cover = profile.game.custom_cover_art_path.trim().to_string();
    if !cover.is_empty() && !is_in_managed_media_dir(&cover) {
        match import_custom_cover_art(&cover) {
            Ok(imported) => profile.game.custom_cover_art_path = imported,
            Err(e) => {
                tracing::warn!(profile_name = %profile_name, %e, "failed to import custom cover art; keeping original path");
            }
        }
    }
    let portrait = profile.game.custom_portrait_art_path.trim().to_string();
    if !portrait.is_empty() && !is_in_managed_media_dir(&portrait) {
        match import_custom_art(&portrait, GameImageType::Portrait) {
            Ok(imported) => profile.game.custom_portrait_art_path = imported,
            Err(e) => {
                tracing::warn!(profile_name = %profile_name, %e, "failed to auto-import portrait art; keeping original path");
            }
        }
    }
    let background = profile.game.custom_background_art_path.trim().to_string();
    if !background.is_empty() && !is_in_managed_media_dir(&background) {
        match import_custom_art(&background, GameImageType::Background) {
            Ok(imported) => profile.game.custom_background_art_path = imported,
            Err(e) => {
                tracing::warn!(profile_name = %profile_name, %e, "failed to auto-import background art; keeping original path");
            }
        }
    }
}
