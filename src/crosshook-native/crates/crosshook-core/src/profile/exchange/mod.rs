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
    use crate::profile::{GameProfile, ProfileStore};
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
                umu_preference: None,
            },
            launch: crate::profile::LaunchSection {
                method: "steam_applaunch".to_string(),
                ..Default::default()
            },
            local_override: crate::profile::LocalOverrideSection::default(),
        }
    }

    fn sample_profile_sanitized_for_export() -> GameProfile {
        let mut p = sample_profile();
        p.game.executable_path.clear();
        p.trainer.path.clear();
        p.injection.dll_paths.clear();
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
        assert_eq!(loaded, shareable);
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
}
