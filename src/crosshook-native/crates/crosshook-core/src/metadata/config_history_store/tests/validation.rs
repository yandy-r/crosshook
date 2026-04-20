use super::common::{ensure_profile, insert_revision, open_test_db};
use crate::metadata::config_history_store::{
    get_config_revision, insert_config_revision, set_known_good_revision,
};
use crate::metadata::{
    ConfigRevisionSource, MetadataStore, MetadataStoreError, MAX_SNAPSHOT_TOML_BYTES,
};

#[test]
fn get_revision_enforces_profile_ownership() {
    let conn = open_test_db();
    ensure_profile(&conn, "profile-1");
    let id = insert_revision(&conn, "profile-1", "hash1");

    let result = get_config_revision(&conn, "profile-2", id).unwrap();
    assert!(
        result.is_none(),
        "cross-profile revision access must return None"
    );
}

#[test]
fn oversized_snapshot_toml_is_rejected() {
    let conn = open_test_db();
    ensure_profile(&conn, "profile-1");
    let oversized = "x".repeat(MAX_SNAPSHOT_TOML_BYTES + 1);
    let result = insert_config_revision(
        &conn,
        "profile-1",
        "Test Profile",
        ConfigRevisionSource::ManualSave,
        "hash1",
        &oversized,
        None,
    );
    assert!(
        matches!(result, Err(MetadataStoreError::Validation(_))),
        "oversized payload must be rejected with Validation error"
    );
}

#[test]
fn set_known_good_on_nonexistent_revision_errors() {
    let conn = open_test_db();
    let result = set_known_good_revision(&conn, "profile-1", 9999);
    assert!(
        matches!(result, Err(MetadataStoreError::Corrupt(_))),
        "nonexistent revision must return Corrupt error"
    );
}

#[test]
fn disabled_store_returns_ok_with_defaults() {
    let store = MetadataStore::disabled();

    let insert_result = store
        .insert_config_revision(
            "profile-1",
            "Test Profile",
            ConfigRevisionSource::ManualSave,
            "hash1",
            "some toml",
            None,
        )
        .unwrap();
    assert!(
        insert_result.is_none(),
        "disabled store insert must return None"
    );

    let list_result = store.list_config_revisions("profile-1", None).unwrap();
    assert!(
        list_result.is_empty(),
        "disabled store list must return empty vec"
    );

    let get_result = store.get_config_revision("profile-1", 1).unwrap();
    assert!(get_result.is_none(), "disabled store get must return None");

    assert!(
        store.set_known_good_revision("profile-1", 1).is_ok(),
        "disabled store set_known_good must return Ok"
    );
    assert!(
        store.clear_known_good_revision("profile-1").is_ok(),
        "disabled store clear_known_good must return Ok"
    );
}

#[test]
fn cross_profile_lineage_is_rejected() {
    let conn = open_test_db();
    let rev_a = insert_revision(&conn, "profile-a", "hash-a");
    ensure_profile(&conn, "profile-b");

    let result = insert_config_revision(
        &conn,
        "profile-b",
        "Profile B",
        ConfigRevisionSource::RollbackApply,
        "hash-b",
        "some toml",
        Some(rev_a), // points to profile-a's revision
    );

    assert!(
        matches!(result, Err(MetadataStoreError::Validation(_))),
        "cross-profile source_revision_id must be rejected, got {result:?}"
    );
}
