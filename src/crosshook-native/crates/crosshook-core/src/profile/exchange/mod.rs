mod error;
mod export;
mod import;
mod types;
mod utils;
mod validation;

pub use error::CommunityExchangeError;
pub use export::export_community_profile;
pub use import::{import_community_profile, preview_community_profile_import};
pub use types::{CommunityExportResult, CommunityImportPreview, CommunityImportResult};

// Re-export validation functions for testing
#[cfg(test)]
pub use validation::validate_manifest_value;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::community_schema::COMMUNITY_PROFILE_SCHEMA_VERSION;
    use crate::profile::{GameProfile, HookStage, LaunchHook, ProfileStore};
    use serde_json::Value;
    use std::fs;
    use tempfile::tempdir;

    fn sample_profile() -> GameProfile {
        GameProfile {
            game: crate::profile::GameSection {
                name: "Elden Ring".to_string(),
                executable_path: "/games/elden-ring/eldenring.exe".to_string(),
                custom_cover_art_path: String::new(),
                custom_portrait_art_path: String::new(),
                custom_background_art_path: String::new(),
            },
            trainer: crate::profile::TrainerSection {
                path: "/trainers/elden-ring.exe".to_string(),
                kind: "fling".to_string(),
                loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
                trainer_type: "unknown".to_string(),
                required_protontricks: Vec::new(),
                community_trainer_sha256: String::new(),
            },
            injection: crate::profile::InjectionSection {
                dll_paths: vec!["/dlls/a.dll".to_string(), "/dlls/b.dll".to_string()],
                inject_on_launch: vec![true, false],
                ..Default::default()
            },
            steam: crate::profile::SteamSection {
                enabled: true,
                app_id: "1245620".to_string(),
                compatdata_path: "/steam/compatdata/1245620".to_string(),
                proton_path: "/steam/proton/proton".to_string(),
                launcher: crate::profile::LauncherSection {
                    icon_path: "/icons/elden-ring.png".to_string(),
                    display_name: "Elden Ring".to_string(),
                },
            },
            runtime: crate::profile::RuntimeSection {
                prefix_path: String::new(),
                proton_path: String::new(),
                working_directory: String::new(),
                steam_app_id: String::new(),
                umu_game_id: String::new(),
                umu_store: String::new(),
                umu_codename: String::new(),
                umu_preference: None,
            },
            launch: crate::profile::LaunchSection {
                method: "steam_applaunch".to_string(),
                ..Default::default()
            },
            local_override: crate::profile::LocalOverrideSection::default(),
            pre_launch_hooks: Vec::new(),
            post_exit_hooks: Vec::new(),
        }
    }

    fn sample_profile_sanitized_for_export() -> GameProfile {
        let mut p = sample_profile();
        p.normalize_injection();
        p.game.executable_path.clear();
        p.trainer.path.clear();
        for hook in &mut p.injection.loaded_hooks {
            hook.path.clear();
            hook.enabled = false;
        }
        p.injection.dll_paths.clear();
        p.injection.inject_on_launch.clear();
        p.steam.compatdata_path.clear();
        p.steam.proton_path.clear();
        p.steam.launcher.icon_path.clear();
        p.runtime.prefix_path.clear();
        p.runtime.proton_path.clear();
        p.runtime.working_directory.clear();
        p
    }

    #[test]
    fn export_strips_machine_specific_paths() {
        let temp_dir = tempdir().unwrap();
        let profiles_dir = temp_dir.path().join("profiles");
        let export_path = temp_dir.path().join("exports").join("elden-ring.json");
        let store = ProfileStore::with_base_path(profiles_dir.clone());
        let profile = sample_profile();
        let expected_shareable = sample_profile_sanitized_for_export();

        store.save("elden-ring", &profile).unwrap();

        let exported = export_community_profile(&profiles_dir, "elden-ring", &export_path).unwrap();
        assert_eq!(exported.manifest.profile, expected_shareable);
        assert_ne!(exported.manifest.profile, profile);
        assert_eq!(exported.manifest.metadata.trainer_name, "elden-ring");

        let json = fs::read_to_string(&export_path).unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();
        let prof = value.get("profile").and_then(Value::as_object).unwrap();
        let game = prof.get("game").and_then(Value::as_object).unwrap();
        assert_eq!(
            game.get("executable_path").and_then(Value::as_str),
            Some("")
        );
        let trainer = prof.get("trainer").and_then(Value::as_object).unwrap();
        assert_eq!(trainer.get("path").and_then(Value::as_str), Some(""));
        let injection = prof.get("injection").and_then(Value::as_object).unwrap();
        let loaded_hooks = injection
            .get("loaded_hooks")
            .and_then(Value::as_array)
            .unwrap();
        assert_eq!(loaded_hooks.len(), 2);
        assert!(loaded_hooks.iter().all(|hook| {
            hook.get("path")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
                && hook.get("enabled").and_then(Value::as_bool) == Some(false)
        }));
        assert!(injection
            .get("dll_paths")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty));
        assert!(injection
            .get("inject_on_launch")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty));
        let steam = prof.get("steam").and_then(Value::as_object).unwrap();
        assert_eq!(
            steam.get("compatdata_path").and_then(Value::as_str),
            Some("")
        );
        assert_eq!(steam.get("proton_path").and_then(Value::as_str), Some(""));
    }

    #[test]
    fn export_and_import_round_trip_profile() {
        let temp_dir = tempdir().unwrap();
        let profiles_dir = temp_dir.path().join("profiles");
        let export_path = temp_dir.path().join("exports").join("elden-ring.json");
        let store = ProfileStore::with_base_path(profiles_dir.clone());
        let profile = sample_profile();
        let shareable = sample_profile_sanitized_for_export();

        store.save("elden-ring", &profile).unwrap();

        let exported = export_community_profile(&profiles_dir, "elden-ring", &export_path).unwrap();
        assert_eq!(exported.profile_name, "elden-ring");
        assert_eq!(exported.manifest.profile, shareable);
        assert_eq!(exported.manifest.metadata.game_name, "Elden Ring");
        assert_eq!(exported.manifest.metadata.trainer_name, "elden-ring");

        let imported_profiles_dir = temp_dir.path().join("imported-profiles");
        let imported = import_community_profile(&export_path, &imported_profiles_dir).unwrap();
        assert_eq!(imported.profile_name, "elden-ring");
        assert_eq!(imported.manifest.profile, shareable);

        let imported_store = ProfileStore::with_base_path(imported_profiles_dir);
        let mut loaded = imported_store.load("elden-ring").unwrap();
        // Import may hydrate steam.proton_path from a local Steam install when app_id is set;
        // compare the portable shape expected from the exported manifest.
        loaded.steam.proton_path.clear();
        let mut expected_imported = shareable;
        expected_imported.normalize_injection();
        assert_eq!(loaded, expected_imported);
    }

    #[test]
    fn rejects_future_schema_versions() {
        let value = serde_json::json!({
            "schema_version": COMMUNITY_PROFILE_SCHEMA_VERSION + 1,
            "metadata": {
                "game_name": "Elden Ring",
                "game_version": "",
                "trainer_name": "",
                "trainer_version": "",
                "proton_version": "",
                "platform_tags": [],
                "compatibility_rating": "unknown",
                "author": "",
                "description": ""
            },
            "profile": sample_profile(),
        });

        let error = validate_manifest_value(&value).unwrap_err();
        assert!(matches!(
            error,
            CommunityExchangeError::UnsupportedSchemaVersion { .. }
        ));
    }

    #[test]
    fn rejects_missing_required_manifest_sections() {
        let value = serde_json::json!({
            "schema_version": COMMUNITY_PROFILE_SCHEMA_VERSION,
            "metadata": {
                "game_name": "Elden Ring",
                "game_version": "",
                "trainer_name": "",
                "trainer_version": "",
                "proton_version": "",
                "platform_tags": [],
                "compatibility_rating": "unknown",
                "author": "",
                "description": ""
            }
        });

        let error = validate_manifest_value(&value).unwrap_err();
        assert!(matches!(
            error,
            CommunityExchangeError::InvalidManifest { .. }
        ));
    }

    #[test]
    fn export_clears_all_custom_art_paths_and_preserves_steam_app_id() {
        let temp_dir = tempdir().unwrap();
        let profiles_dir = temp_dir.path().join("profiles");
        let export_path = temp_dir.path().join("exports").join("elden-ring.json");
        let store = ProfileStore::with_base_path(profiles_dir.clone());

        let mut profile = sample_profile();
        profile.game.custom_cover_art_path = "/home/user/.local/cover.png".to_string();
        profile.game.custom_portrait_art_path = "/home/user/.local/portrait.png".to_string();
        profile.game.custom_background_art_path = "/home/user/.local/background.png".to_string();
        profile.local_override.game.custom_cover_art_path =
            "/home/user/.local/cover-override.png".to_string();
        profile.local_override.game.custom_portrait_art_path =
            "/home/user/.local/portrait-override.png".to_string();
        profile.local_override.game.custom_background_art_path =
            "/home/user/.local/background-override.png".to_string();

        store.save("elden-ring", &profile).unwrap();

        let exported = export_community_profile(&profiles_dir, "elden-ring", &export_path).unwrap();
        let exported_profile = &exported.manifest.profile;

        assert!(
            exported_profile.game.custom_cover_art_path.is_empty(),
            "custom_cover_art_path must be cleared on export"
        );
        assert!(
            exported_profile.game.custom_portrait_art_path.is_empty(),
            "custom_portrait_art_path must be cleared on export"
        );
        assert!(
            exported_profile.game.custom_background_art_path.is_empty(),
            "custom_background_art_path must be cleared on export"
        );

        assert_eq!(
            exported_profile.steam.app_id, "1245620",
            "steam.app_id must survive community export"
        );
    }

    fn sample_hook_enabled(id: &str, stage: HookStage) -> LaunchHook {
        LaunchHook {
            id: id.to_string(),
            name: format!("Hook {id}"),
            path: format!("/scripts/{id}.sh"),
            stage,
            enabled: true,
        }
    }

    #[test]
    fn community_export_strips_launch_hooks() {
        let temp_dir = tempdir().unwrap();
        let profiles_dir = temp_dir.path().join("profiles");
        let export_path = temp_dir.path().join("exports").join("elden-ring.json");
        let store = ProfileStore::with_base_path(profiles_dir.clone());

        let mut profile = sample_profile();
        profile.pre_launch_hooks = vec![
            sample_hook_enabled("pre-a", HookStage::PreLaunch),
            sample_hook_enabled("pre-b", HookStage::PreLaunch),
        ];
        profile.post_exit_hooks = vec![sample_hook_enabled("post-a", HookStage::PostExit)];

        store.save("elden-ring", &profile).unwrap();

        let exported = export_community_profile(&profiles_dir, "elden-ring", &export_path).unwrap();
        let exported_profile = &exported.manifest.profile;

        assert!(
            exported_profile.pre_launch_hooks.is_empty(),
            "pre_launch_hooks must be stripped on community export"
        );
        assert!(
            exported_profile.post_exit_hooks.is_empty(),
            "post_exit_hooks must be stripped on community export"
        );

        // Also verify no hook paths appear in the raw JSON output.
        let json = fs::read_to_string(&export_path).unwrap();
        assert!(
            !json.contains("/scripts/pre-a.sh"),
            "hook path must not appear in exported JSON: {json}"
        );
        assert!(
            !json.contains("/scripts/post-a.sh"),
            "hook path must not appear in exported JSON: {json}"
        );
    }

    #[test]
    fn community_import_force_disables_launch_hooks() {
        let temp_dir = tempdir().unwrap();
        let profiles_dir = temp_dir.path().join("profiles");
        let export_path = temp_dir.path().join("exports").join("elden-ring.json");
        let import_dir = temp_dir.path().join("imported");
        let store = ProfileStore::with_base_path(profiles_dir.clone());

        // Build a profile with hooks and export it, then manually re-inject enabled hooks
        // into the JSON so the importer sees `enabled = true` in the manifest.
        let profile = sample_profile();
        store.save("elden-ring", &profile).unwrap();
        export_community_profile(&profiles_dir, "elden-ring", &export_path).unwrap();

        // Patch the exported JSON to embed enabled hooks directly in the manifest profile.
        let content = fs::read_to_string(&export_path).unwrap();
        let mut value: Value = serde_json::from_str(&content).unwrap();
        let hooks_json = serde_json::json!([
            { "id": "pre-a", "name": "Hook pre-a", "path": "/scripts/pre-a.sh", "stage": "pre-launch", "enabled": true },
            { "id": "pre-b", "name": "Hook pre-b", "path": "/scripts/pre-b.sh", "stage": "pre-launch", "enabled": true },
        ]);
        let post_hooks_json = serde_json::json!([
            { "id": "post-a", "name": "Hook post-a", "path": "/scripts/post-a.sh", "stage": "post-exit", "enabled": true },
        ]);
        value["profile"]["pre_launch_hooks"] = hooks_json;
        value["profile"]["post_exit_hooks"] = post_hooks_json;
        value["profile"]["injection"]["loaded_hooks"] = serde_json::json!([
            { "id": "dll-a", "name": "Injected A", "path": "/dlls/a.dll", "enabled": true },
            { "id": "dll-b", "name": "Injected B", "path": "/dlls/b.dll", "enabled": true },
        ]);
        value["profile"]["injection"]["dll_paths"] =
            serde_json::json!(["/dlls/a.dll", "/dlls/b.dll"]);
        value["profile"]["injection"]["inject_on_launch"] = serde_json::json!([true, true]);
        fs::write(&export_path, serde_json::to_string_pretty(&value).unwrap()).unwrap();

        let imported = import_community_profile(&export_path, &import_dir).unwrap();

        // All hooks must be force-disabled; entries and paths must be retained.
        for hook in &imported.profile.pre_launch_hooks {
            assert!(
                !hook.enabled,
                "pre_launch hook '{}' must be disabled after import, got enabled=true",
                hook.id
            );
        }
        for hook in &imported.profile.post_exit_hooks {
            assert!(
                !hook.enabled,
                "post_exit hook '{}' must be disabled after import, got enabled=true",
                hook.id
            );
        }

        // Entries and identifying data must survive (only `enabled` is forced off).
        assert_eq!(imported.profile.pre_launch_hooks.len(), 2);
        assert_eq!(imported.profile.post_exit_hooks.len(), 1);
        assert_eq!(imported.profile.pre_launch_hooks[0].id, "pre-a");
        assert_eq!(
            imported.profile.pre_launch_hooks[0].path,
            "/scripts/pre-a.sh"
        );
        assert_eq!(imported.profile.post_exit_hooks[0].id, "post-a");
        assert_eq!(imported.profile.injection.loaded_hooks.len(), 2);
        assert!(imported
            .profile
            .injection
            .loaded_hooks
            .iter()
            .all(|hook| !hook.enabled));
        assert_eq!(
            imported.profile.injection.inject_on_launch,
            vec![false, false]
        );
    }
}
