use crosshook_core::profile::{GameProfile, ProfileStore, ProfileStoreError};
use serde::{Deserialize, Serialize};
use tauri::State;

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

    std::env::var("HOME").unwrap_or_default()
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
    store: State<'_, ProfileStore>,
) -> Result<(), String> {
    store.save(&name, &data).map_err(map_error)
}

#[tauri::command]
pub fn profile_save_launch_optimizations(
    name: String,
    optimizations: LaunchOptimizationsPayload,
    store: State<'_, ProfileStore>,
) -> Result<(), String> {
    save_launch_optimizations_for_profile(&name, &optimizations, &store)
}

#[tauri::command]
pub fn profile_delete(name: String, store: State<'_, ProfileStore>) -> Result<(), String> {
    // Best-effort launcher cleanup before profile deletion.
    // Profile deletion must succeed even if launcher cleanup fails.
    if let Ok(profile) = store.load(&name) {
        if let Err(error) = cleanup_launchers_for_profile_delete(&name, &profile) {
            tracing::warn!("Launcher cleanup failed for profile {name}: {error}");
        }
    }

    store.delete(&name).map_err(map_error)
}

#[tauri::command]
pub fn profile_rename(
    old_name: String,
    new_name: String,
    store: State<'_, ProfileStore>,
) -> Result<(), String> {
    store.rename(&old_name, &new_name).map_err(map_error)
}

#[tauri::command]
pub fn profile_import_legacy(
    path: String,
    store: State<'_, ProfileStore>,
) -> Result<GameProfile, String> {
    store
        .import_legacy(std::path::Path::new(&path))
        .map_err(map_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crosshook_core::export::check_launcher_exists;
    use crosshook_core::profile::{
        GameSection, LaunchSection, LauncherSection, SteamSection, TrainerSection,
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
