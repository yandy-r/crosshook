//! Integration tests for config revision history.
//!
//! These tests exercise the end-to-end flow through the public `MetadataStore` API:
//! revision capture, diff validation, rollback lineage, rename continuity, and
//! graceful degradation when the metadata store is unavailable.

use crosshook_core::metadata::{sha256_hex, ConfigRevisionSource, MetadataStore, SyncSource};
use crosshook_core::profile::GameProfile;
use std::path::Path;

// ── helpers ──────────────────────────────────────────────────────────────────

/// Serialize a `GameProfile` to TOML. Panics on serialization failure (tests only).
fn profile_to_toml(profile: &GameProfile) -> String {
    toml::to_string_pretty(profile).expect("GameProfile must serialize to TOML")
}

/// Register a profile row in the metadata store so FK constraints are satisfied.
fn observe_write(store: &MetadataStore, name: &str, profile: &GameProfile) {
    store
        .observe_profile_write(
            name,
            profile,
            Path::new(&format!("/test/profiles/{name}.toml")),
            SyncSource::AppWrite,
            None,
        )
        .unwrap_or_else(|e| panic!("observe_profile_write('{name}') failed: {e}"));
}

/// Look up the stable `profile_id` assigned by the metadata store for a profile name.
fn lookup_id(store: &MetadataStore, name: &str) -> String {
    store
        .lookup_profile_id(name)
        .unwrap_or_else(|e| panic!("lookup_profile_id('{name}') failed: {e}"))
        .unwrap_or_else(|| panic!("profile row for '{name}' must exist after observe_write"))
}

// ── Test 1: save → revision append → diff → rollback → lineage append ────────

/// Verifies the full happy-path flow:
/// save profile → insert two revisions → confirm list order and diff-relevant
/// differences → simulate rollback → confirm lineage pointer on the rollback row.
#[test]
fn end_to_end_save_revision_rollback_lineage() {
    let store = MetadataStore::open_in_memory().expect("open in-memory MetadataStore");

    // ── setup: register profile row ──────────────────────────────────────────
    let mut profile_v1 = GameProfile::default();
    profile_v1.game.name = "Elden Ring".to_string();
    observe_write(&store, "elden-ring", &profile_v1);
    let profile_id = lookup_id(&store, "elden-ring");

    // ── revision 1: initial save ─────────────────────────────────────────────
    let toml_v1 = profile_to_toml(&profile_v1);
    let hash_v1 = sha256_hex(toml_v1.as_bytes());

    let rev1_id = store
        .insert_config_revision(
            &profile_id,
            "elden-ring",
            ConfigRevisionSource::ManualSave,
            &hash_v1,
            &toml_v1,
            None,
        )
        .expect("insert revision 1 must succeed")
        .expect("revision 1 must not be deduped on first insert");

    // ── revision 2: profile modified ─────────────────────────────────────────
    let mut profile_v2 = profile_v1.clone();
    profile_v2.game.name = "Elden Ring — trainer path updated".to_string();
    let toml_v2 = profile_to_toml(&profile_v2);
    let hash_v2 = sha256_hex(toml_v2.as_bytes());

    let rev2_id = store
        .insert_config_revision(
            &profile_id,
            "elden-ring",
            ConfigRevisionSource::ManualSave,
            &hash_v2,
            &toml_v2,
            None,
        )
        .expect("insert revision 2 must succeed")
        .expect("revision 2 must not be deduped (different hash)");

    assert!(
        rev2_id > rev1_id,
        "revision ids must increase monotonically"
    );

    // ── list: newest first, both present ─────────────────────────────────────
    let list = store
        .list_config_revisions(&profile_id, None)
        .expect("list must succeed");

    assert_eq!(list.len(), 2, "both revisions must be present in the list");
    assert_eq!(list[0].id, rev2_id, "newest revision must be first");
    assert_eq!(list[1].id, rev1_id, "oldest revision must be last");

    // ── diff-relevant checks: hashes and snapshot content differ ─────────────
    assert_ne!(
        list[0].content_hash, list[1].content_hash,
        "revisions must have distinct content hashes"
    );
    assert_ne!(
        list[0].snapshot_toml, list[1].snapshot_toml,
        "snapshot content must differ between revisions"
    );

    // ── rollback: fetch target revision, verify parseability ─────────────────
    let rollback_target = store
        .get_config_revision(&profile_id, rev1_id)
        .expect("get_config_revision must succeed")
        .expect("revision 1 must exist when fetched by id");

    // The stored snapshot must round-trip through the GameProfile deserializer.
    let restored: GameProfile = toml::from_str(&rollback_target.snapshot_toml)
        .expect("stored snapshot must be parseable as GameProfile");

    assert_eq!(
        restored.game.name, profile_v1.game.name,
        "restored snapshot must match the original profile content"
    );

    // ── rollback lineage row: RollbackApply with source_revision_id ──────────
    // The latest revision is v2 (hash_v2), so restoring v1's content (hash_v1)
    // is NOT deduped — a new row is inserted to record the rollback.
    let rollback_id = store
        .insert_config_revision(
            &profile_id,
            "elden-ring",
            ConfigRevisionSource::RollbackApply,
            &hash_v1, // same content as rev1 (intentional restore)
            &toml_v1,
            Some(rev1_id), // lineage pointer back to the revision being restored
        )
        .expect("insert rollback revision must succeed")
        .expect("rollback revision must be inserted (latest hash is v2, not v1)");

    // ── lineage verification ──────────────────────────────────────────────────
    let rollback_row = store
        .get_config_revision(&profile_id, rollback_id)
        .expect("get rollback revision must succeed")
        .expect("rollback revision must exist");

    assert_eq!(
        rollback_row.source,
        ConfigRevisionSource::RollbackApply.as_str(),
        "rollback revision source field must be 'rollback_apply'"
    );
    assert_eq!(
        rollback_row.source_revision_id,
        Some(rev1_id),
        "rollback revision must carry a lineage pointer to the restored revision"
    );

    // ── final list: three revisions in newest-first order ────────────────────
    let final_list = store
        .list_config_revisions(&profile_id, None)
        .expect("final list must succeed");

    assert_eq!(final_list.len(), 3, "rollback append must bring total to 3");
    assert_eq!(final_list[0].id, rollback_id, "rollback row is newest");
    assert_eq!(final_list[1].id, rev2_id);
    assert_eq!(final_list[2].id, rev1_id, "original first save is oldest");
}

// ── Test 2: revision history survives a profile rename ───────────────────────

/// Verifies that the stable `profile_id` key preserves history through a rename:
/// revisions inserted before and after the rename are all retrievable via the
/// same `profile_id`, and `profile_name_at_write` reflects the name at write time.
#[test]
fn rename_continuity_via_profile_id() {
    let store = MetadataStore::open_in_memory().expect("open in-memory MetadataStore");

    // ── setup: register profile under original name ───────────────────────────
    let mut profile = GameProfile::default();
    profile.game.name = "Hollow Knight".to_string();
    observe_write(&store, "hollow-knight", &profile);
    let profile_id = lookup_id(&store, "hollow-knight");

    // ── revisions before the rename ───────────────────────────────────────────
    let toml_a = profile_to_toml(&profile);
    let rev_a = store
        .insert_config_revision(
            &profile_id,
            "hollow-knight",
            ConfigRevisionSource::ManualSave,
            "hash-before-rename-a",
            &toml_a,
            None,
        )
        .expect("insert rev_a must succeed")
        .expect("rev_a must be inserted");

    let mut profile2 = profile.clone();
    profile2.game.name = "Hollow Knight (pre-rename edit)".to_string();
    let toml_b = profile_to_toml(&profile2);
    let rev_b = store
        .insert_config_revision(
            &profile_id,
            "hollow-knight",
            ConfigRevisionSource::ManualSave,
            "hash-before-rename-b",
            &toml_b,
            None,
        )
        .expect("insert rev_b must succeed")
        .expect("rev_b must be inserted");

    // ── rename the profile ────────────────────────────────────────────────────
    store
        .observe_profile_rename(
            "hollow-knight",
            "hollow-knight-v2",
            Path::new("/test/profiles/hollow-knight.toml"),
            Path::new("/test/profiles/hollow-knight-v2.toml"),
        )
        .expect("observe_profile_rename must succeed");

    // ── the new name must resolve to the same stable profile_id ───────────────
    let renamed_id = lookup_id(&store, "hollow-knight-v2");
    assert_eq!(
        renamed_id, profile_id,
        "observe_profile_rename must preserve the stable profile_id"
    );

    // ── revision after the rename (new name, same profile_id) ────────────────
    let mut profile3 = profile2.clone();
    profile3.game.name = "Hollow Knight (post-rename edit)".to_string();
    let toml_c = profile_to_toml(&profile3);
    let rev_c = store
        .insert_config_revision(
            &profile_id,
            "hollow-knight-v2", // profile_name_at_write reflects new name
            ConfigRevisionSource::ManualSave,
            "hash-after-rename-c",
            &toml_c,
            None,
        )
        .expect("insert rev_c must succeed")
        .expect("rev_c must be inserted");

    // ── all three revisions accessible via the same profile_id ───────────────
    let all = store
        .list_config_revisions(&profile_id, None)
        .expect("list must succeed");

    assert_eq!(
        all.len(),
        3,
        "all revisions before and after rename must be accessible via stable profile_id"
    );

    // ── correct ordering: newest first ───────────────────────────────────────
    assert_eq!(all[0].id, rev_c, "post-rename revision is newest");
    assert_eq!(all[1].id, rev_b);
    assert_eq!(all[2].id, rev_a, "pre-rename first revision is oldest");

    // ── profile_name_at_write reflects the name active at insertion time ──────
    assert_eq!(
        all[0].profile_name_at_write, "hollow-knight-v2",
        "post-rename revision must carry the new name"
    );
    assert_eq!(
        all[1].profile_name_at_write, "hollow-knight",
        "pre-rename revision must retain the original name"
    );
    assert_eq!(
        all[2].profile_name_at_write, "hollow-knight",
        "earliest pre-rename revision must retain the original name"
    );
}

// ── Test 3: metadata unavailable — all history operations degrade gracefully ─

/// Verifies that when the metadata store is disabled (e.g., database unavailable),
/// all config history operations return safe defaults (`Ok(None)`, `Ok([])`, `Ok(())`)
/// without panicking or returning errors.
#[test]
fn disabled_store_history_operations_return_safe_defaults() {
    let store = MetadataStore::disabled();

    // insert → Ok(None): no-op, does not error
    let insert_result = store
        .insert_config_revision(
            "some-profile",
            "Some Profile",
            ConfigRevisionSource::ManualSave,
            "any-hash",
            "[game]\nname = \"test\"\n",
            None,
        )
        .expect("disabled store insert_config_revision must not return Err");
    assert!(
        insert_result.is_none(),
        "disabled store insert must return None (no revision stored)"
    );

    // list → Ok([]): empty result, no error
    let list_result = store
        .list_config_revisions("some-profile", None)
        .expect("disabled store list_config_revisions must not return Err");
    assert!(
        list_result.is_empty(),
        "disabled store list must return an empty vec"
    );

    // get → Ok(None): no revision found, no error
    let get_result = store
        .get_config_revision("some-profile", 99)
        .expect("disabled store get_config_revision must not return Err");
    assert!(get_result.is_none(), "disabled store get must return None");

    // set_known_good → Ok(()): no-op, no error
    store
        .set_known_good_revision("some-profile", 1)
        .expect("disabled store set_known_good_revision must not return Err");

    // clear_known_good → Ok(()): no-op, no error
    store
        .clear_known_good_revision("some-profile")
        .expect("disabled store clear_known_good_revision must not return Err");
}
