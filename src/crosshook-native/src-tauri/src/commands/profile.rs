use crosshook_core::metadata::{MetadataStore, SyncSource};
use crosshook_core::profile::{
    DuplicateProfileResult, GameProfile, ProfileStore, ProfileStoreError,
};
use crosshook_core::settings::SettingsStore;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::{AppHandle, Emitter, State};

const STEAM_COMPATDATA_MARKER: &str = "/steamapps/compatdata/";
const STEAM_ROOT_SUFFIXES: [&str; 2] = ["/.local/share/Steam", "/.steam/root"];

fn map_error(error: ProfileStoreError) -> String {
    error.to_string()
}

fn derive_steam_client_install_path(profile: &GameProfile) -> String {
    let compatdata_path = profile.steam.compatdata_path.trim().replace('\\', "/");
    compatdata_path
        .split_once(STEAM_COMPATDATA_MARKER)
        .map(|(steam_root, _)| steam_root.to_string())
        .unwrap_or_default()
}

fn derive_target_home_path(steam_client_install_path: &str) -> String {
    let normalized = steam_client_install_path.trim().replace('\\', "/");

    for suffix in STEAM_ROOT_SUFFIXES {
        if let Some(home_path) = normalized.strip_suffix(suffix) {
            return home_path.to_string();
        }
    }

    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() => home,
        _ => {
            tracing::warn!(
                "HOME is unset or empty and Steam client path did not match known patterns; derived home for launcher cleanup will be empty"
            );
            String::new()
        }
    }
}

fn cleanup_launchers_for_profile_delete(
    profile_name: &str,
    profile: &GameProfile,
) -> Result<Option<crosshook_core::export::LauncherDeleteResult>, String> {
    if profile.launch.method == "native" {
        tracing::debug!(
            profile_name,
            "skipping launcher cleanup for native profile delete"
        );
        return Ok(None);
    }

    let steam_client_install_path = derive_steam_client_install_path(profile);
    let target_home_path = derive_target_home_path(&steam_client_install_path);

    crosshook_core::export::delete_launcher_for_profile(
        profile,
        &target_home_path,
        &steam_client_install_path,
    )
    .map(Some)
    .map_err(|error| error.to_string())
}

fn save_launch_optimizations_for_profile(
    name: &str,
    optimizations: &LaunchOptimizationsPayload,
    store: &ProfileStore,
) -> Result<(), String> {
    store
        .save_launch_optimizations(name, optimizations.enabled_option_ids.clone())
        .map_err(map_error)
}

fn emit_profiles_changed(app: &AppHandle, reason: &str) {
    if let Err(error) = app.emit("profiles-changed", reason.to_string()) {
        tracing::warn!(%error, reason, "failed to emit profiles-changed event");
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LaunchOptimizationsPayload {
    #[serde(
        rename = "enabled_option_ids",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub enabled_option_ids: Vec<String>,
}

#[tauri::command]
pub fn profile_list(store: State<'_, ProfileStore>) -> Result<Vec<String>, String> {
    store.list().map_err(map_error)
}

#[tauri::command]
pub fn profile_load(name: String, store: State<'_, ProfileStore>) -> Result<GameProfile, String> {
    store.load(&name).map_err(map_error)
}

#[tauri::command]
pub fn profile_save(
    name: String,
    data: GameProfile,
    app: AppHandle,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
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

    emit_profiles_changed(&app, "saved");
    Ok(())
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
            SyncSource::AppWrite,
            None,
        ) {
            tracing::warn!(
                %e,
                profile_name = %name,
                "metadata sync after save_launch_optimizations failed"
            );
        }
    }

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

#[tauri::command]
pub fn profile_set_favorite(
    name: String,
    favorite: bool,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    metadata_store
        .set_profile_favorite(&name, favorite)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn profile_list_favorites(
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<String>, String> {
    metadata_store
        .list_favorite_profiles()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crosshook_core::export::check_launcher_exists;
    use crosshook_core::profile::{
        GameSection, LaunchSection, LauncherSection, SteamSection, TrainerLoadingMode,
        TrainerSection,
    };
    use std::fs;
    use tempfile::tempdir;

    fn steam_profile(home: &str) -> GameProfile {
        GameProfile {
            game: GameSection {
                name: "Test Game".to_string(),
                executable_path: String::new(),
            },
            trainer: TrainerSection {
                path: "/tmp/trainers/test.exe".to_string(),
                kind: String::new(),
                loading_mode: TrainerLoadingMode::SourceDirectory,
            },
            steam: SteamSection {
                app_id: "12345".to_string(),
                compatdata_path: format!("{home}/.local/share/Steam/steamapps/compatdata/12345"),
                launcher: LauncherSection {
                    display_name: "Test Game".to_string(),
                    icon_path: String::new(),
                },
                ..Default::default()
            },
            launch: LaunchSection {
                method: "steam_applaunch".to_string(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn create_watermarked_launcher_files(script_path: &str, desktop_path: &str) {
        fs::create_dir_all(
            std::path::Path::new(script_path)
                .parent()
                .expect("script parent"),
        )
        .expect("script dirs");
        fs::create_dir_all(
            std::path::Path::new(desktop_path)
                .parent()
                .expect("desktop parent"),
        )
        .expect("desktop dirs");
        fs::write(
            script_path,
            "#!/usr/bin/env bash\n# Generated by CrossHook\n",
        )
        .expect("write script");
        fs::write(
            desktop_path,
            "[Desktop Entry]\nName=Test Game - Trainer\nComment=Generated by CrossHook\n",
        )
        .expect("write desktop");
    }

    #[test]
    fn cleanup_launchers_for_profile_delete_uses_derived_steam_paths() {
        let temp = tempdir().expect("temp dir");
        let home = temp.path().to_string_lossy().into_owned();
        let profile = steam_profile(&home);
        let steam_root = format!("{home}/.local/share/Steam");

        let info = check_launcher_exists(
            &profile.steam.launcher.display_name,
            &profile.steam.app_id,
            &profile.trainer.path,
            &home,
            &steam_root,
        )
        .expect("check launcher exists");
        create_watermarked_launcher_files(&info.script_path, &info.desktop_entry_path);

        let result = cleanup_launchers_for_profile_delete("test-profile", &profile)
            .expect("cleanup should succeed");

        assert!(result.is_some());
        assert!(!std::path::Path::new(&info.script_path).exists());
        assert!(!std::path::Path::new(&info.desktop_entry_path).exists());
    }

    #[test]
    fn cleanup_launchers_for_profile_delete_skips_native_profiles() {
        let profile = GameProfile {
            launch: LaunchSection {
                method: "native".to_string(),
                ..Default::default()
            },
            ..GameProfile::default()
        };

        let result = cleanup_launchers_for_profile_delete("native-profile", &profile)
            .expect("native cleanup should not fail");

        assert!(result.is_none());
    }

    #[test]
    fn save_launch_optimizations_for_profile_updates_only_launch_section() {
        let temp = tempdir().expect("temp dir");
        let store = ProfileStore::with_base_path(temp.path().join("profiles"));
        let home = temp.path().to_string_lossy().into_owned();
        let profile = steam_profile(&home);

        store.save("test-profile", &profile).expect("save profile");

        let optimizations = LaunchOptimizationsPayload {
            enabled_option_ids: vec![
                "disable_steam_input".to_string(),
                "use_gamemode".to_string(),
            ],
        };

        save_launch_optimizations_for_profile("test-profile", &optimizations, &store)
            .expect("save launch optimizations");

        let loaded = store.load("test-profile").expect("load profile");
        assert_eq!(loaded.game, profile.game);
        assert_eq!(loaded.trainer, profile.trainer);
        assert_eq!(loaded.injection, profile.injection);
        assert_eq!(loaded.steam, profile.steam);
        assert_eq!(loaded.runtime, profile.runtime);
        assert_eq!(loaded.launch.method, profile.launch.method);
        assert_eq!(
            loaded.launch.optimizations.enabled_option_ids,
            optimizations.enabled_option_ids
        );
    }

    #[test]
    fn save_launch_optimizations_for_profile_rejects_missing_profiles() {
        let temp = tempdir().expect("temp dir");
        let store = ProfileStore::with_base_path(temp.path().join("profiles"));

        let error = save_launch_optimizations_for_profile(
            "missing-profile",
            &LaunchOptimizationsPayload {
                enabled_option_ids: vec!["use_gamemode".to_string()],
            },
            &store,
        )
        .expect_err("missing profile should fail");

        assert!(error.contains("profile file not found"));
    }
}
