use super::common::{ensure_profile, insert_revision, open_test_db};
use crate::metadata::config_history_store::{
    clear_known_good_revision, get_config_revision, set_known_good_revision,
};

#[test]
fn known_good_supersede_clears_previous_marker() {
    let conn = open_test_db();
    ensure_profile(&conn, "profile-1");
    let id1 = insert_revision(&conn, "profile-1", "hash1");
    let id2 = insert_revision(&conn, "profile-1", "hash2");

    set_known_good_revision(&conn, "profile-1", id1).unwrap();
    let r1 = get_config_revision(&conn, "profile-1", id1)
        .unwrap()
        .unwrap();
    assert!(
        r1.is_last_known_working,
        "initial known-good marker must be set"
    );

    // Supersede: mark id2 as known-good
    set_known_good_revision(&conn, "profile-1", id2).unwrap();

    let r1 = get_config_revision(&conn, "profile-1", id1)
        .unwrap()
        .unwrap();
    let r2 = get_config_revision(&conn, "profile-1", id2)
        .unwrap()
        .unwrap();
    assert!(
        !r1.is_last_known_working,
        "previous known-good marker must be cleared on supersede"
    );
    assert!(
        r2.is_last_known_working,
        "new known-good marker must be set"
    );
}

#[test]
fn known_good_is_isolated_per_profile() {
    let conn = open_test_db();
    ensure_profile(&conn, "profile-1");
    ensure_profile(&conn, "profile-2");
    let id_p1 = insert_revision(&conn, "profile-1", "hash1");
    let id_p2 = insert_revision(&conn, "profile-2", "hash2");

    set_known_good_revision(&conn, "profile-1", id_p1).unwrap();

    let r_p2 = get_config_revision(&conn, "profile-2", id_p2)
        .unwrap()
        .unwrap();
    assert!(
        !r_p2.is_last_known_working,
        "profile-2 must be unaffected by profile-1 known-good change"
    );
}

#[test]
fn clear_known_good_removes_all_markers_for_profile() {
    let conn = open_test_db();
    ensure_profile(&conn, "profile-1");
    let id = insert_revision(&conn, "profile-1", "hash1");
    set_known_good_revision(&conn, "profile-1", id).unwrap();

    clear_known_good_revision(&conn, "profile-1").unwrap();

    let r = get_config_revision(&conn, "profile-1", id)
        .unwrap()
        .unwrap();
    assert!(
        !r.is_last_known_working,
        "known-good marker must be cleared"
    );
}
