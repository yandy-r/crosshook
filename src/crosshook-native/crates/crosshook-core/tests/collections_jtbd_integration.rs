//! Integration test exercising the full JTBD (Jobs-To-Be-Done) path for profile
//! collections: seed profiles → create collections → assign members →
//! filter/query → set defaults → effective merge → export → reset → re-import
//! preview → apply.

use crosshook_core::metadata::{CollectionRow, MetadataStore, SyncSource};
use crosshook_core::profile::{
    export_collection_preset_to_toml, preview_collection_preset_import, CollectionDefaultsSection,
    CollectionPresetManifest, GameProfile, ProfileStore, COLLECTION_PRESET_SCHEMA_VERSION,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use tempfile::tempdir;

// ── helpers ──────────────────────────────────────────────────────────────────

fn sample_profile_named(name: &str, steam_app_id: &str, trainer_sha: &str) -> GameProfile {
    let mut p = GameProfile::default();
    p.game.name = name.to_string();
    p.steam.app_id = steam_app_id.to_string();
    p.trainer.community_trainer_sha256 = trainer_sha.to_string();
    p.launch
        .custom_env_vars
        .insert("PRE_EXISTING".to_string(), "1".to_string());
    p
}

fn register_profile(
    metadata: &MetadataStore,
    store: &ProfileStore,
    filename: &str,
    profile: &GameProfile,
) {
    store.save(filename, profile).unwrap();
    // Synthetic path — the AppWrite sync source does not call fs::metadata,
    // so no real file is needed; the string is only persisted in metadata.
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

// ── JTBD end-to-end test ────────────────────────────────────────────────────

#[test]
fn end_to_end_collections_jtbd() {
    // ── Step 1: Setup ───────────────────────────────────────────────────────
    let dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(dir.path().join("profiles"));
    let metadata = MetadataStore::open_in_memory().unwrap();

    // ── Step 2: Seed 50 profiles ────────────────────────────────────────────
    for i in 0..50 {
        let name = format!("fixture-{i:02}");
        let app_id = (1_000_000 + i as u64).to_string();
        let sha = format!("{i:064x}");
        let profile = sample_profile_named(&name, &app_id, &sha);
        register_profile(&metadata, &store, &name, &profile);
    }

    // ── Step 3: Create 3 collections ────────────────────────────────────────
    let action_cid = metadata.create_collection("Action").unwrap();
    let stable_cid = metadata.create_collection("Stable").unwrap();
    let wip_cid = metadata.create_collection("WIP").unwrap();

    // ── Step 4: Assign 10 profiles each (some multi-membership) ─────────────
    // Action: fixture-00 through fixture-09
    for i in 0..10 {
        let name = format!("fixture-{i:02}");
        metadata
            .add_profile_to_collection(&action_cid, &name)
            .unwrap();
    }
    // Stable: fixture-05 through fixture-14 (overlaps Action at 05-09)
    for i in 5..15 {
        let name = format!("fixture-{i:02}");
        metadata
            .add_profile_to_collection(&stable_cid, &name)
            .unwrap();
    }
    // WIP: fixture-10 through fixture-19
    for i in 10..20 {
        let name = format!("fixture-{i:02}");
        metadata.add_profile_to_collection(&wip_cid, &name).unwrap();
    }

    // ── Step 5: Filter assertions ───────────────────────────────────────────
    let action_members = metadata.list_profiles_in_collection(&action_cid).unwrap();
    assert_eq!(
        action_members.len(),
        10,
        "Action collection must contain exactly 10 profiles"
    );

    let collections_for_05: Vec<CollectionRow> =
        metadata.collections_for_profile("fixture-05").unwrap();
    assert!(
        collections_for_05.len() >= 2,
        "fixture-05 must belong to at least Action and Stable, got {}",
        collections_for_05.len()
    );

    // ── Step 6: Set collection defaults ─────────────────────────────────────
    let defaults = CollectionDefaultsSection {
        custom_env_vars: BTreeMap::from([("DXVK_HUD".to_string(), "fps".to_string())]),
        ..CollectionDefaultsSection::default()
    };
    metadata
        .set_collection_defaults(&action_cid, Some(&defaults))
        .unwrap();

    // ── Step 7: Effective profile WITH context ──────────────────────────────
    let profile_05 = store.load("fixture-05").unwrap();
    let merged = profile_05.effective_profile_with(Some(&defaults));
    assert_eq!(
        merged.launch.custom_env_vars.get("PRE_EXISTING"),
        Some(&"1".to_string()),
        "effective merge must preserve pre-existing launch env vars from the base profile"
    );
    assert_eq!(
        merged.launch.custom_env_vars.get("DXVK_HUD"),
        Some(&"fps".to_string()),
        "effective profile with collection defaults must contain DXVK_HUD=fps"
    );

    // ── Step 8: Effective profile WITHOUT context ───────────────────────────
    let no_merge = profile_05.effective_profile_with(None);
    assert_eq!(
        no_merge.launch.custom_env_vars.get("PRE_EXISTING"),
        Some(&"1".to_string()),
        "base profile env vars must survive when collection defaults are not applied"
    );
    assert!(
        !no_merge.launch.custom_env_vars.contains_key("DXVK_HUD"),
        "effective profile without collection defaults must NOT contain DXVK_HUD"
    );

    // ── Step 9: Export ──────────────────────────────────────────────────────
    let exported_path = dir.path().join("action.crosshook-collection.toml");
    let export_result =
        export_collection_preset_to_toml(&metadata, &store, &action_cid, &exported_path).unwrap();
    assert!(
        exported_path.exists(),
        "exported collection preset file must exist on disk"
    );

    // Verify the exported file parses as valid TOML with schema_version = "1"
    let exported_content = std::fs::read_to_string(&exported_path).unwrap();
    let parsed_manifest: CollectionPresetManifest = toml::from_str(&exported_content).unwrap();
    assert_eq!(
        parsed_manifest.schema_version, COLLECTION_PRESET_SCHEMA_VERSION,
        "exported preset must have schema_version = \"1\""
    );
    assert_eq!(
        export_result.manifest.name, "Action",
        "exported manifest name must be 'Action'"
    );

    // ── Step 10: Reset — new in-memory metadata store ───────────────────────
    // Profiles remain on disk; only the metadata database is fresh.
    let metadata2 = MetadataStore::open_in_memory().unwrap();

    // ── Step 11: Re-import preview ──────────────────────────────────────────
    let preview = preview_collection_preset_import(&store, &exported_path).unwrap();
    assert_eq!(
        preview.matched.len(),
        10,
        "all 10 exported profiles must match against on-disk profile store"
    );
    assert!(
        preview.ambiguous.is_empty(),
        "no ambiguous matches expected for unique steam_app_id profiles"
    );
    assert!(
        preview.unmatched.is_empty(),
        "no unmatched descriptors expected when profiles are still on disk"
    );
    assert_eq!(
        preview.manifest.name, "Action",
        "preview manifest name must be 'Action'"
    );
    assert_eq!(
        preview.manifest.defaults.as_ref().unwrap().custom_env_vars["DXVK_HUD"],
        "fps",
        "preview manifest defaults must carry DXVK_HUD=fps"
    );

    // ── Step 12: Simulate fresh-store re-import via individual building blocks
    //    (no high-level apply-import function exists yet) ─────────────────
    let new_action_cid = metadata2.create_collection("Action").unwrap();
    for entry in &preview.matched {
        // Re-register profiles in the new metadata store so FK constraints pass
        let profile = store.load(&entry.local_profile_name).unwrap();
        // Synthetic path (same as `register_profile`): `AppWrite` does not touch the filesystem.
        metadata2
            .observe_profile_write(
                &entry.local_profile_name,
                &profile,
                &PathBuf::from("/profiles").join(format!("{}.toml", entry.local_profile_name)),
                SyncSource::AppWrite,
                None,
            )
            .unwrap();
        metadata2
            .add_profile_to_collection(&new_action_cid, &entry.local_profile_name)
            .unwrap();
    }

    // Restore collection defaults from the preview manifest
    if let Some(ref imported_defaults) = preview.manifest.defaults {
        metadata2
            .set_collection_defaults(&new_action_cid, Some(imported_defaults))
            .unwrap();
    }

    // Verify the re-created collection matches the original
    let new_members = metadata2
        .list_profiles_in_collection(&new_action_cid)
        .unwrap();
    let expected: BTreeSet<_> = action_members.iter().cloned().collect();
    let got: BTreeSet<_> = new_members.iter().cloned().collect();
    assert_eq!(
        got, expected,
        "re-imported collection membership must match the original (set equality, not just length)"
    );

    let new_defaults = metadata2
        .get_collection_defaults(&new_action_cid)
        .unwrap()
        .expect("re-imported collection must have defaults");
    assert_eq!(
        new_defaults.custom_env_vars.get("DXVK_HUD"),
        Some(&"fps".to_string()),
        "re-imported collection defaults must match the original DXVK_HUD=fps"
    );
    assert_eq!(
        new_defaults, defaults,
        "re-imported collection defaults must equal the original defaults"
    );
}
