use crosshook_core::game_images::{
    import_custom_art, import_custom_cover_art, is_in_managed_media_dir, GameImageType,
};
use crosshook_core::metadata::{ConfigRevisionSource, MetadataStore, SyncSource};
use crosshook_core::profile::{
    apply_profile_creation_defaults_from_settings, bundled_optimization_preset_toml_key,
    resolve_art_app_id, validate_steam_app_id, DuplicateProfileResult, GameProfile,
    LaunchOptimizationsSection, ProfileStore,
};
use crosshook_core::settings::SettingsStore;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::{AppHandle, State};

use super::shared::{
    apply_collection_defaults, capture_config_revision, cleanup_launchers_for_profile_delete,
    emit_profiles_changed, map_error,
};

#[tauri::command]
pub fn profile_list(store: State<'_, ProfileStore>) -> Result<Vec<String>, String> {
    store.list().map_err(map_error)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    pub name: String,
    pub game_name: String,
    pub steam_app_id: String,
    pub custom_cover_art_path: Option<String>,
    pub custom_portrait_art_path: Option<String>,
    /// Effective `launch.network_isolation` (default true) for Flatpak capability badges.
    pub network_isolation: bool,
}

#[tauri::command]
pub fn profile_load(
    name: String,
    collection_id: Option<String>,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<GameProfile, String> {
    let profile = store.load(&name).map_err(map_error)?;

    // When a collection context is provided, merge the collection's defaults
    // into the profile via `effective_profile_with`. The returned profile still
    // reflects the machine-specific `local_override` layer that `ProfileStore::load`
    // baked into layer 1, so collection defaults can never clobber portable paths.
    apply_collection_defaults(profile, metadata_store.inner(), collection_id.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn profile_list_summaries(
    collection_id: Option<String>,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<ProfileSummary>, String> {
    let names = store.list().map_err(map_error)?;
    let mut summaries = Vec::with_capacity(names.len());
    for name in names {
        match store.load(&name) {
            Ok(profile) => {
                let profile = apply_collection_defaults(
                    profile,
                    metadata_store.inner(),
                    collection_id.as_deref(),
                )
                .map_err(|e| e.to_string())?;
                let effective = profile.effective_profile();
                let cover_art = effective.game.custom_cover_art_path.trim();
                let portrait_art = effective.game.custom_portrait_art_path.trim();
                summaries.push(ProfileSummary {
                    name,
                    game_name: effective.game.name.clone(),
                    steam_app_id: resolve_art_app_id(&effective).to_string(),
                    custom_cover_art_path: if cover_art.is_empty() {
                        None
                    } else {
                        Some(cover_art.to_string())
                    },
                    custom_portrait_art_path: if portrait_art.is_empty() {
                        None
                    } else {
                        Some(portrait_art.to_string())
                    },
                    network_isolation: effective.launch.network_isolation,
                });
            }
            Err(e) => {
                tracing::warn!(profile_name = %name, %e, "skipping profile in summaries");
            }
        }
    }
    Ok(summaries)
}

#[tauri::command]
pub fn profile_save(
    name: String,
    mut data: GameProfile,
    app: AppHandle,
    store: State<'_, ProfileStore>,
    settings_store: State<'_, SettingsStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    // Validate runtime.steam_app_id before writing to disk (BR-4).
    if let Err(e) = validate_steam_app_id(data.runtime.steam_app_id.trim()) {
        return Err(format!("Invalid Steam App ID in runtime section: {e}"));
    }

    let is_new = !store.profile_exists(&name);
    if is_new {
        let app_settings = settings_store.load().map_err(|e| e.to_string())?;
        apply_profile_creation_defaults_from_settings(&mut data, &app_settings);

        // Only apply the app-settings default bundled preset when the incoming
        // draft has not already selected one — otherwise an explicit wizard
        // selection would be silently clobbered by the user's default.
        let pid = app_settings.default_bundled_optimization_preset_id.trim();
        if !pid.is_empty()
            && metadata_store.is_available()
            && data.launch.active_preset.trim().is_empty()
        {
            match metadata_store.get_bundled_optimization_preset(pid) {
                Ok(Some(row)) => {
                    let enabled_option_ids: Vec<String> =
                        serde_json::from_str(&row.option_ids_json).unwrap_or_default();
                    let toml_key = bundled_optimization_preset_toml_key(pid);
                    data.launch.presets.insert(
                        toml_key.clone(),
                        LaunchOptimizationsSection {
                            enabled_option_ids: enabled_option_ids.clone(),
                        },
                    );
                    data.launch.active_preset = toml_key;
                    data.launch.optimizations = LaunchOptimizationsSection { enabled_option_ids };
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
    }

    // Auto-import custom cover art into the managed media directory when the
    // source path points outside it (e.g. a user-typed filesystem path).
    let cover = data.game.custom_cover_art_path.trim().to_string();
    if !cover.is_empty() && !is_in_managed_media_dir(&cover) {
        match import_custom_cover_art(&cover) {
            Ok(imported) => data.game.custom_cover_art_path = imported,
            Err(e) => {
                tracing::warn!(profile_name = %name, %e, "failed to import custom cover art; keeping original path");
            }
        }
    }
    // Portrait auto-import
    let portrait = data.game.custom_portrait_art_path.trim().to_string();
    if !portrait.is_empty() && !is_in_managed_media_dir(&portrait) {
        match import_custom_art(&portrait, GameImageType::Portrait) {
            Ok(imported) => data.game.custom_portrait_art_path = imported,
            Err(e) => {
                tracing::warn!(profile_name = %name, %e, "failed to auto-import portrait art; keeping original path");
            }
        }
    }
    // Background auto-import
    let background = data.game.custom_background_art_path.trim().to_string();
    if !background.is_empty() && !is_in_managed_media_dir(&background) {
        match import_custom_art(&background, GameImageType::Background) {
            Ok(imported) => data.game.custom_background_art_path = imported,
            Err(e) => {
                tracing::warn!(profile_name = %name, %e, "failed to auto-import background art; keeping original path");
            }
        }
    }

    store.save(&name, &data).map_err(map_error)?;

    let profile_path = store.base_path.join(format!("{name}.toml"));
    if let Err(e) = metadata_store.observe_profile_write(
        &name,
        &data,
        &profile_path,
        SyncSource::AppWrite,
        None,
    ) {
        tracing::warn!(%e, profile_name = %name, "metadata sync after profile_save failed");
    }

    capture_config_revision(
        &name,
        &data,
        ConfigRevisionSource::ManualSave,
        None,
        &metadata_store,
    );

    emit_profiles_changed(&app, "saved");
    Ok(())
}

#[tauri::command]
pub fn profile_delete(
    name: String,
    app: AppHandle,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    // Best-effort launcher cleanup before profile deletion.
    // Profile deletion must succeed even if launcher cleanup fails.
    if let Ok(profile) = store.load(&name) {
        if let Err(error) = cleanup_launchers_for_profile_delete(&name, &profile) {
            tracing::warn!("Launcher cleanup failed for profile {name}: {error}");
        }
    }

    store.delete(&name).map_err(map_error)?;

    if let Err(e) = metadata_store.observe_profile_delete(&name) {
        tracing::warn!(%e, profile_name = %name, "metadata sync after profile_delete failed");
    }

    emit_profiles_changed(&app, "deleted");
    Ok(())
}

/// Duplicates an existing profile under a unique copy name.
///
/// Delegates to [`ProfileStore::duplicate`] which handles name generation, collision
/// avoidance, and persistence. The returned [`DuplicateProfileResult`] is serialized
/// to the frontend where it drives profile list refresh and auto-selection of the copy.
///
/// # Frontend invocation
/// ```ts
/// const result = await invoke<DuplicateProfileResult>('profile_duplicate', { name });
/// ```
///
/// # Errors
/// Returns a stringified error when the source profile does not exist or if the
/// generated copy name cannot pass filesystem validation.
#[tauri::command]
pub fn profile_duplicate(
    name: String,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<DuplicateProfileResult, String> {
    let source_profile_id = metadata_store.lookup_profile_id(&name).ok().flatten();

    let result = store.duplicate(&name).map_err(map_error)?;

    let copy_path = store.base_path.join(format!("{}.toml", result.name));
    if let Err(e) = metadata_store.observe_profile_write(
        &result.name,
        &result.profile,
        &copy_path,
        SyncSource::AppDuplicate,
        source_profile_id.as_deref(),
    ) {
        tracing::warn!(%e, name = %result.name, "metadata sync after profile_duplicate failed");
    }

    Ok(result)
}

#[tauri::command]
pub fn profile_rename(
    old_name: String,
    new_name: String,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
    settings_store: State<'_, SettingsStore>,
) -> Result<bool, String> {
    // Load profile BEFORE rename for launcher cleanup and display_name update.
    let old_profile = store.load(&old_name).ok();

    store.rename(&old_name, &new_name).map_err(map_error)?;

    let old_path = store.base_path.join(format!("{old_name}.toml"));
    let new_path = store.base_path.join(format!("{new_name}.toml"));
    if let Err(e) =
        metadata_store.observe_profile_rename(&old_name, &new_name, &old_path, &new_path)
    {
        tracing::warn!(%e, %old_name, %new_name, "metadata sync after profile_rename failed");
    }

    // Best-effort: delete old launcher files so the frontend can re-export with correct paths.
    let had_launcher = if let Some(ref profile) = old_profile {
        match cleanup_launchers_for_profile_delete(&old_name, profile) {
            Ok(Some(result)) => result.script_deleted || result.desktop_entry_deleted,
            Ok(None) => false,
            Err(error) => {
                tracing::warn!(%error, %old_name, %new_name, "launcher cleanup during profile rename failed");
                false
            }
        }
    } else {
        false
    };

    // Best-effort: update display_name inside the renamed profile so future exports use the new name.
    if old_profile.is_some() {
        if let Ok(mut profile) = store.load(&new_name) {
            profile.steam.launcher.display_name = new_name.trim().to_string();
            if let Err(err) = store.save(&new_name, &profile) {
                tracing::warn!(%err, %new_name, "display_name update after profile rename failed");
            }
        }
    }

    if let Ok(mut settings) = settings_store.load() {
        if settings.last_used_profile.trim() == old_name.trim() {
            settings.last_used_profile = new_name.trim().to_string();
            if let Err(err) = settings_store.save(&settings) {
                tracing::warn!(%err, %old_name, %new_name, "settings update after profile rename failed");
            }
        }
    }

    Ok(had_launcher)
}

#[tauri::command]
pub fn profile_import_legacy(
    path: String,
    app: AppHandle,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<GameProfile, String> {
    let profile = store.import_legacy(Path::new(&path)).map_err(map_error)?;

    let stem = Path::new(&path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("imported");
    let import_path = store.base_path.join(format!("{stem}.toml"));
    if let Err(e) =
        metadata_store.observe_profile_write(stem, &profile, &import_path, SyncSource::Import, None)
    {
        tracing::warn!(%e, profile_name = %stem, "metadata sync after import_legacy failed");
    }

    capture_config_revision(
        stem,
        &profile,
        ConfigRevisionSource::Import,
        None,
        &metadata_store,
    );

    emit_profiles_changed(&app, "imported-legacy");
    Ok(profile)
}

/// Serializes the provided in-memory profile to a shareable TOML string
/// with comment headers indicating the save location.
#[tauri::command]
pub fn profile_export_toml(name: String, data: GameProfile) -> Result<String, String> {
    crosshook_core::profile::profile_to_shareable_toml(&name, &data)
        .map_err(|error| error.to_string())
}
