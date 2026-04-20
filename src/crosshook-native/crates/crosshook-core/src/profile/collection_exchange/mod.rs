//! Export and import-preview for `*.crosshook-collection.toml` collection presets.

mod error;
mod export;
mod import;
mod matching;
mod types;

pub use error::CollectionExchangeError;
pub use export::export_collection_preset_to_toml;
pub use import::preview_collection_preset_import;
pub use types::{
    CollectionExportResult, CollectionImportPreview, CollectionPresetAmbiguousEntry,
    CollectionPresetMatchCandidate, CollectionPresetMatchedEntry,
};

#[cfg(test)]
mod tests {
    use super::export::write_preset_toml;
    use super::*;
    use crate::metadata::{MetadataStore, SyncSource};
    use crate::profile::collection_schema::{
        CollectionPresetManifest, CollectionPresetProfileDescriptor,
        COLLECTION_PRESET_SCHEMA_VERSION,
    };
    use crate::profile::{CollectionDefaultsSection, GameProfile, ProfileStore};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn sample_profile_named(name: &str, steam_app_id: &str) -> GameProfile {
        let mut p = GameProfile::default();
        p.game.name = name.to_string();
        p.steam.app_id = steam_app_id.to_string();
        p
    }

    fn register_profile(
        metadata: &MetadataStore,
        store: &ProfileStore,
        filename: &str,
        profile: &GameProfile,
    ) {
        store.save(filename, profile).unwrap();
        metadata
            .observe_profile_write(
                filename,
                profile,
                &PathBuf::from("/profiles").join(format!("{filename}.toml")),
                SyncSource::AppWrite,
                None,
            )
            .unwrap();
    }

    #[test]
    fn parse_rejects_future_schema_version() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad.toml");
        fs::write(
            &path,
            r#"
schema_version = "2"
name = "X"
profiles = []
"#,
        )
        .unwrap();

        let store = ProfileStore::with_base_path(dir.path().join("profiles"));
        let err = preview_collection_preset_import(&store, &path).unwrap_err();
        assert!(matches!(
            err,
            CollectionExchangeError::UnsupportedSchemaVersion { .. }
        ));
    }

    #[test]
    fn parse_rejects_malformed_toml() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad.toml");
        fs::write(&path, "not toml [[[").unwrap();
        let store = ProfileStore::with_base_path(dir.path().join("profiles"));
        let err = preview_collection_preset_import(&store, &path).unwrap_err();
        assert!(matches!(err, CollectionExchangeError::Toml { .. }));
    }

    #[test]
    fn export_preview_roundtrip_with_effective_app_id() {
        let dir = tempdir().unwrap();
        let profiles_dir = dir.path().join("profiles");
        let store = ProfileStore::with_base_path(profiles_dir.clone());
        let metadata = MetadataStore::open_in_memory().unwrap();

        let mut p = GameProfile::default();
        p.game.name = "Elden Ring".to_string();
        p.steam.app_id = String::new();
        p.runtime.steam_app_id = "1245620".to_string();
        p.trainer.community_trainer_sha256 = "ab".repeat(32);

        register_profile(&metadata, &store, "elden-ring", &p);

        let cid = metadata.create_collection("Action").unwrap();
        metadata
            .add_profile_to_collection(&cid, "elden-ring")
            .unwrap();

        let defaults = CollectionDefaultsSection {
            method: Some("proton_run".to_string()),
            ..CollectionDefaultsSection::default()
        };
        metadata
            .set_collection_defaults(&cid, Some(&defaults))
            .unwrap();

        let out = dir.path().join("out.crosshook-collection.toml");
        export_collection_preset_to_toml(&metadata, &store, &cid, &out).unwrap();

        let preview = preview_collection_preset_import(&store, &out).unwrap();
        assert_eq!(preview.manifest.name, "Action");
        assert_eq!(preview.matched.len(), 1);
        assert_eq!(preview.matched[0].local_profile_name, "elden-ring");
        assert!(preview.ambiguous.is_empty());
        assert!(preview.unmatched.is_empty());
        assert_eq!(
            preview
                .manifest
                .defaults
                .as_ref()
                .and_then(|d| d.method.as_deref()),
            Some("proton_run")
        );
    }

    #[test]
    fn steam_match_ambiguous() {
        let dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(dir.path().join("profiles"));
        let metadata = MetadataStore::open_in_memory().unwrap();

        let a = sample_profile_named("A", "1");
        let b = sample_profile_named("B", "1");
        register_profile(&metadata, &store, "pa", &a);
        register_profile(&metadata, &store, "pb", &b);

        let manifest = CollectionPresetManifest {
            schema_version: COLLECTION_PRESET_SCHEMA_VERSION.to_string(),
            name: "Import".to_string(),
            description: None,
            defaults: None,
            profiles: vec![CollectionPresetProfileDescriptor {
                steam_app_id: "1".to_string(),
                game_name: String::new(),
                trainer_community_trainer_sha256: String::new(),
            }],
        };

        let path = dir.path().join("p.toml");
        write_preset_toml(&path, &manifest).unwrap();
        let preview = preview_collection_preset_import(&store, &path).unwrap();
        assert_eq!(preview.ambiguous.len(), 1);
        assert_eq!(preview.ambiguous[0].candidates.len(), 2);
    }

    #[test]
    fn steam_tie_broken_by_unique_pair() {
        let dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(dir.path().join("profiles"));
        let metadata = MetadataStore::open_in_memory().unwrap();

        let mut a = sample_profile_named("Alpha", "100");
        a.trainer.community_trainer_sha256 = "aa".repeat(32);
        let mut b = sample_profile_named("Bravo", "100");
        b.trainer.community_trainer_sha256 = "bb".repeat(32);
        register_profile(&metadata, &store, "pa", &a);
        register_profile(&metadata, &store, "pb", &b);

        let manifest = CollectionPresetManifest {
            schema_version: COLLECTION_PRESET_SCHEMA_VERSION.to_string(),
            name: "Import".to_string(),
            description: None,
            defaults: None,
            profiles: vec![CollectionPresetProfileDescriptor {
                steam_app_id: "100".to_string(),
                game_name: "Bravo".to_string(),
                trainer_community_trainer_sha256: "bb".repeat(32),
            }],
        };

        let path = dir.path().join("p.toml");
        write_preset_toml(&path, &manifest).unwrap();
        let preview = preview_collection_preset_import(&store, &path).unwrap();
        assert!(preview.ambiguous.is_empty());
        assert_eq!(preview.matched.len(), 1);
        assert_eq!(preview.matched[0].local_profile_name, "pb");
    }

    #[test]
    fn pair_fallback_match() {
        let dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(dir.path().join("profiles"));
        let metadata = MetadataStore::open_in_memory().unwrap();

        let mut p = GameProfile::default();
        p.game.name = "Game".to_string();
        p.steam.app_id = String::new();
        p.trainer.community_trainer_sha256 = "cd".repeat(32);

        register_profile(&metadata, &store, "g1", &p);

        let manifest = CollectionPresetManifest {
            schema_version: COLLECTION_PRESET_SCHEMA_VERSION.to_string(),
            name: "X".to_string(),
            description: None,
            defaults: None,
            profiles: vec![CollectionPresetProfileDescriptor {
                steam_app_id: String::new(),
                game_name: "Game".to_string(),
                trainer_community_trainer_sha256: "cd".repeat(32),
            }],
        };

        let path = dir.path().join("p.toml");
        write_preset_toml(&path, &manifest).unwrap();
        let preview = preview_collection_preset_import(&store, &path).unwrap();
        assert_eq!(preview.matched.len(), 1);
        assert_eq!(preview.matched[0].local_profile_name, "g1");
    }

    #[test]
    fn unmatched_descriptor() {
        let dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(dir.path().join("profiles"));
        let _metadata = MetadataStore::open_in_memory().unwrap();

        let manifest = CollectionPresetManifest {
            schema_version: COLLECTION_PRESET_SCHEMA_VERSION.to_string(),
            name: "X".to_string(),
            description: None,
            defaults: None,
            profiles: vec![CollectionPresetProfileDescriptor {
                steam_app_id: "999".to_string(),
                game_name: "Nope".to_string(),
                trainer_community_trainer_sha256: String::new(),
            }],
        };

        let path = dir.path().join("p.toml");
        write_preset_toml(&path, &manifest).unwrap();
        let preview = preview_collection_preset_import(&store, &path).unwrap();
        assert_eq!(preview.unmatched.len(), 1);
    }

    #[test]
    fn export_fails_when_member_profile_missing() {
        let dir = tempdir().unwrap();
        let profiles_dir = dir.path().join("profiles");
        let store = ProfileStore::with_base_path(profiles_dir.clone());
        let metadata = MetadataStore::open_in_memory().unwrap();

        let p = sample_profile_named("X", "1");
        register_profile(&metadata, &store, "gone", &p);

        let cid = metadata.create_collection("C").unwrap();
        metadata.add_profile_to_collection(&cid, "gone").unwrap();

        fs::remove_file(profiles_dir.join("gone.toml")).unwrap();

        let out = dir.path().join("out.toml");
        let err = export_collection_preset_to_toml(&metadata, &store, &cid, &out).unwrap_err();
        assert!(matches!(
            err,
            CollectionExchangeError::InvalidManifest { .. }
                | CollectionExchangeError::ProfileStore { .. }
        ));
    }
}
