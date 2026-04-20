use super::common::{ensure_profile, insert_revision, open_test_db};
use crate::metadata::config_history_store::{
    get_config_revision, insert_config_revision, list_config_revisions,
};
use crate::metadata::ConfigRevisionSource;

#[test]
fn insert_returns_id_and_list_is_newest_first() {
    let conn = open_test_db();
    let id1 = insert_revision(&conn, "profile-1", "hash1");
    let id2 = insert_revision(&conn, "profile-1", "hash2");
    assert!(id2 > id1, "ids must increase monotonically");

    let revisions = list_config_revisions(&conn, "profile-1", None).unwrap();
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].id, id2, "newest revision must be first");
    assert_eq!(revisions[1].id, id1, "oldest revision must be last");
    assert_eq!(revisions[0].profile_id, "profile-1");
    assert_eq!(revisions[0].profile_name_at_write, "Test Profile");
}

#[test]
fn list_returns_empty_for_profile_with_no_revisions() {
    let conn = open_test_db();
    let revisions = list_config_revisions(&conn, "no-such-profile", None).unwrap();
    assert!(revisions.is_empty());
}

#[test]
fn list_respects_custom_limit() {
    let conn = open_test_db();
    insert_revision(&conn, "profile-1", "hash1");
    insert_revision(&conn, "profile-1", "hash2");
    insert_revision(&conn, "profile-1", "hash3");

    let limited = list_config_revisions(&conn, "profile-1", Some(2)).unwrap();
    assert_eq!(limited.len(), 2, "limit parameter must be honoured");
    assert_eq!(
        limited[0].content_hash, "hash3",
        "newest within limit first"
    );
    assert_eq!(limited[1].content_hash, "hash2");
}

#[test]
fn insert_dedup_skips_when_latest_hash_matches() {
    let conn = open_test_db();
    ensure_profile(&conn, "profile-1");
    let id1 = insert_revision(&conn, "profile-1", "hash1");

    let deduped = insert_config_revision(
        &conn,
        "profile-1",
        "Test Profile",
        ConfigRevisionSource::ManualSave,
        "hash1",
        "some toml content",
        None,
    )
    .unwrap();
    assert!(deduped.is_none(), "identical hash must be skipped");

    let revisions = list_config_revisions(&conn, "profile-1", None).unwrap();
    assert_eq!(revisions.len(), 1);
    assert_eq!(revisions[0].id, id1);
}

#[test]
fn insert_dedup_does_not_apply_to_non_latest_hash() {
    // Dedup is only against the single latest row; re-inserting an older
    // hash must create a new row.
    let conn = open_test_db();
    ensure_profile(&conn, "profile-1");
    insert_revision(&conn, "profile-1", "hash1");
    insert_revision(&conn, "profile-1", "hash2");

    let result = insert_config_revision(
        &conn,
        "profile-1",
        "Test Profile",
        ConfigRevisionSource::ManualSave,
        "hash1",
        "some toml content",
        None,
    )
    .unwrap();
    assert!(
        result.is_some(),
        "re-inserting a non-latest hash must not be deduped"
    );

    let revisions = list_config_revisions(&conn, "profile-1", None).unwrap();
    assert_eq!(revisions.len(), 3);
}

#[test]
fn insert_dedup_is_scoped_to_profile() {
    let conn = open_test_db();
    ensure_profile(&conn, "profile-1");
    ensure_profile(&conn, "profile-2");
    insert_revision(&conn, "profile-1", "hash1");

    let result = insert_config_revision(
        &conn,
        "profile-2",
        "Test Profile",
        ConfigRevisionSource::ManualSave,
        "hash1",
        "some toml content",
        None,
    )
    .unwrap();
    assert!(
        result.is_some(),
        "same hash for a different profile must not be deduped"
    );
}

#[test]
fn lineage_source_revision_id_stored_and_retrieved() {
    let conn = open_test_db();
    ensure_profile(&conn, "profile-1");
    let parent_id = insert_revision(&conn, "profile-1", "hash1");
    let child_id = insert_config_revision(
        &conn,
        "profile-1",
        "Test Profile",
        ConfigRevisionSource::RollbackApply,
        "hash2",
        "rollback toml content",
        Some(parent_id),
    )
    .unwrap()
    .expect("child insert should succeed");

    let child = get_config_revision(&conn, "profile-1", child_id)
        .unwrap()
        .expect("child must exist");
    assert_eq!(child.source_revision_id, Some(parent_id));
    assert_eq!(child.source, ConfigRevisionSource::RollbackApply.as_str());
}
