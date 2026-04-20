use super::common::{ensure_profile, insert_revision, open_test_db};
use crate::metadata::config_history_store::{
    get_config_revision, insert_config_revision, list_config_revisions,
};
use crate::metadata::{ConfigRevisionSource, MAX_CONFIG_REVISIONS_PER_PROFILE};

#[test]
fn pruning_respects_max_revisions_per_profile() {
    let conn = open_test_db();
    ensure_profile(&conn, "profile-1");
    let over_limit = MAX_CONFIG_REVISIONS_PER_PROFILE + 1;
    for i in 0..over_limit {
        insert_config_revision(
            &conn,
            "profile-1",
            "Test Profile",
            ConfigRevisionSource::ManualSave,
            &format!("hash-{i}"),
            "some toml content",
            None,
        )
        .expect("insert must not fail")
        .expect("each unique-hash insert should succeed");
    }

    let revisions = list_config_revisions(&conn, "profile-1", None).unwrap();
    assert_eq!(
        revisions.len(),
        MAX_CONFIG_REVISIONS_PER_PROFILE,
        "revision count must not exceed retention limit"
    );
    assert!(
        revisions.iter().all(|r| r.content_hash != "hash-0"),
        "oldest revision must be pruned"
    );
    assert!(
        revisions
            .iter()
            .any(|r| r.content_hash == format!("hash-{}", over_limit - 1)),
        "newest revision must be retained"
    );
}

#[test]
fn pruning_does_not_affect_other_profiles() {
    let conn = open_test_db();
    ensure_profile(&conn, "profile-1");
    ensure_profile(&conn, "profile-2");
    let over_limit = MAX_CONFIG_REVISIONS_PER_PROFILE + 1;
    for i in 0..over_limit {
        insert_config_revision(
            &conn,
            "profile-1",
            "Profile One",
            ConfigRevisionSource::ManualSave,
            &format!("p1-hash-{i}"),
            "toml",
            None,
        )
        .unwrap();
    }
    insert_revision(&conn, "profile-2", "p2-hash-1");

    let p2_revisions = list_config_revisions(&conn, "profile-2", None).unwrap();
    assert_eq!(
        p2_revisions.len(),
        1,
        "profile-2 must be unaffected by profile-1 pruning"
    );
}

#[test]
fn pruning_retains_revisions_referenced_by_source_revision_id() {
    let conn = open_test_db();
    ensure_profile(&conn, "profile-1");
    let mut oldest_id: i64 = 0;
    for i in 0..MAX_CONFIG_REVISIONS_PER_PROFILE {
        let id = insert_config_revision(
            &conn,
            "profile-1",
            "Test Profile",
            ConfigRevisionSource::ManualSave,
            &format!("chain-hash-{i}"),
            "toml",
            None,
        )
        .expect("insert must not fail")
        .expect("each insert must create a row");
        if i == 0 {
            oldest_id = id;
        }
    }
    assert!(oldest_id > 0, "oldest revision id must be set");

    // New rollback row points at the oldest revision; pruning must not delete that parent
    // (would violate FK on config_revisions.source_revision_id).
    insert_config_revision(
        &conn,
        "profile-1",
        "Test Profile",
        ConfigRevisionSource::RollbackApply,
        "rollback-child-hash",
        "rollback toml",
        Some(oldest_id),
    )
    .expect("insert with parent reference must succeed after FK-safe pruning");

    let parent = get_config_revision(&conn, "profile-1", oldest_id)
        .unwrap()
        .expect("referenced parent revision must still exist");
    assert_eq!(parent.id, oldest_id);
}
