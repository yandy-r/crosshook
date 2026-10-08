use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::{
    export_collection_preset_to_toml, export_community_profile, profile_to_shareable_toml,
    CollectionDefaultsSection, GameProfile, ProfileStore,
};
use std::fs;
use tempfile::tempdir;

const PROFILE: &str = r#"
root_future = 2026-10-08T12:30:00Z
[game]
name = "Game"
executable_path = "/games/game.exe"
future = 1.5
[steam.launcher]
display_name = "Game"
future = { nested = [1, 2] }
[launch]
method = "proton_run"
future = "launch"
[launch.presets.fast]
enabled_option_ids = []
future = 2026-10-08
[local_override.trainer]
path = "/local/trainer.exe"
future = "local"
[[pre_launch_hooks]]
id = "hook"
name = "Pre-launch hook"
path = "/scripts/pre-launch.sh"
stage = "pre-launch"
enabled = false
future = true
"#;

fn store_with_profile() -> (tempfile::TempDir, ProfileStore) {
    let dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(dir.path().into());
    fs::write(dir.path().join("game.toml"), PROFILE).unwrap();
    (dir, store)
}

#[test]
fn profile_store_partial_updates_keep_nested_and_root_extras() {
    let (_dir, store) = store_with_profile();
    store
        .save_launch_optimizations("game", vec![], Some("fast".into()))
        .unwrap();
    store
        .save_command_arguments("game", vec![], vec!["-x".into()], Some("proton_run"))
        .unwrap();
    let saved: toml::Value =
        toml::from_str(&fs::read_to_string(store.base_path.join("game.toml")).unwrap()).unwrap();
    let original: toml::Value = toml::from_str(PROFILE).unwrap();
    assert_eq!(saved["root_future"], original["root_future"]);
    assert_eq!(
        saved["pre_launch_hooks"][0]["future"],
        original["pre_launch_hooks"][0]["future"]
    );
    assert_eq!(saved["game"]["future"], original["game"]["future"]);
    assert_eq!(
        saved["steam"]["launcher"]["future"],
        original["steam"]["launcher"]["future"]
    );
    assert_eq!(saved["launch"]["future"], original["launch"]["future"]);
    assert_eq!(
        saved["launch"]["presets"]["fast"]["future"],
        original["launch"]["presets"]["fast"]["future"]
    );
    assert_eq!(
        saved["local_override"]["trainer"]["future"],
        original["local_override"]["trainer"]["future"]
    );
}

#[test]
fn ipc_profile_merge_ignores_incoming_extras_and_restores_disk_extras() {
    let (_dir, store) = store_with_profile();
    let existing = store.load("game").unwrap();
    let mut incoming = existing.clone();
    incoming.clear_extra();
    assert!(!serde_json::to_string(&incoming).unwrap().contains("future"));
    incoming.extra.insert("injected".into(), true.into());
    incoming.game.extra.insert("injected".into(), true.into());
    incoming.game.name = "Edited".into();
    incoming.clear_extra();
    incoming.preserve_extra_from(&existing);
    store.save("game", &incoming).unwrap();
    let saved = fs::read_to_string(store.base_path.join("game.toml")).unwrap();
    assert!(saved.contains("Edited"));
    assert!(saved.contains("root_future = 2026-10-08T12:30:00Z"));
    assert!(!saved.contains("injected"));
    let saved: toml::Value = toml::from_str(&saved).unwrap();
    let original: toml::Value = toml::from_str(PROFILE).unwrap();
    assert_eq!(
        saved["pre_launch_hooks"][0]["future"],
        original["pre_launch_hooks"][0]["future"]
    );
}

#[test]
fn shareable_and_community_exports_strip_unknown_fields() {
    let (dir, store) = store_with_profile();
    let profile: GameProfile = toml::from_str(PROFILE).unwrap();
    let toml = profile_to_shareable_toml("game", &profile).unwrap();
    assert!(!toml.contains("future"));
    let out = dir.path().join("export.json");
    export_community_profile(&store.base_path, "game", &out).unwrap();
    assert!(!fs::read_to_string(out).unwrap().contains("future"));
}

#[test]
fn collection_defaults_extras_are_strippable_for_export() {
    let mut defaults: CollectionDefaultsSection =
        toml::from_str("future = 1\n[gamescope]\nfuture = 2026-10-08\n").unwrap();
    assert!(!defaults.is_empty());
    defaults.clear_extra();
    assert!(!toml::to_string(&defaults).unwrap().contains("future"));
    assert!(defaults.gamescope.is_some(), "known section remains");
}

#[test]
fn collection_preset_export_strips_default_extras() {
    let (dir, store) = store_with_profile();
    let metadata = MetadataStore::open_in_memory().unwrap();
    let cid = metadata.create_collection("Shared").unwrap();
    let defaults: CollectionDefaultsSection =
        toml::from_str("method = 'native'\nfuture = 1\n[gamescope]\nfuture = 2026-10-08\n")
            .unwrap();
    metadata
        .set_collection_defaults(&cid, Some(&defaults))
        .unwrap();
    let out = dir.path().join("collection.toml");
    export_collection_preset_to_toml(&metadata, &store, &cid, &out).unwrap();
    let exported = fs::read_to_string(out).unwrap();
    assert!(exported.contains("native"));
    assert!(!exported.contains("future"));
}
