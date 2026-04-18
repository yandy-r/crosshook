#![cfg(test)]

use rusqlite::params;

use super::test_support::{connection, insert_test_profile_row};
use super::{MetadataStore, MAX_VERSION_SNAPSHOTS_PER_PROFILE};

#[test]
fn test_version_snapshot_upsert_and_lookup_lifecycle() {
    let store = MetadataStore::open_in_memory().unwrap();
    {
        let conn = connection(&store);
        insert_test_profile_row(&conn, "lifecycle-profile");
    }

    store
        .upsert_version_snapshot(
            "lifecycle-profile",
            "99999",
            Some("build-abc"),
            Some("v1.2.3"),
            Some("deadbeef01234567deadbeef01234567deadbeef01234567deadbeef01234567"),
            Some("1.2.3"),
            "matched",
        )
        .unwrap();

    let snapshot = store
        .lookup_latest_version_snapshot("lifecycle-profile")
        .unwrap()
        .expect("snapshot should be present after upsert");

    assert_eq!(snapshot.profile_id, "lifecycle-profile");
    assert_eq!(snapshot.steam_app_id, "99999");
    assert_eq!(snapshot.steam_build_id.as_deref(), Some("build-abc"));
    assert_eq!(snapshot.trainer_version.as_deref(), Some("v1.2.3"));
    assert_eq!(snapshot.status, "matched");
    assert!(!snapshot.checked_at.is_empty());
}

#[test]
fn test_version_snapshot_lookup_returns_latest() {
    let store = MetadataStore::open_in_memory().unwrap();
    {
        let conn = connection(&store);
        insert_test_profile_row(&conn, "latest-profile");
    }

    // Insert two snapshots with distinct checked_at values via raw SQL
    // so we can control ordering.
    {
        let conn = connection(&store);
        conn.execute(
            "INSERT INTO version_snapshots
             (profile_id, steam_app_id, steam_build_id, status, checked_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                "latest-profile",
                "11111",
                "build-old",
                "untracked",
                "2024-01-01T00:00:00+00:00",
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO version_snapshots
             (profile_id, steam_app_id, steam_build_id, status, checked_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                "latest-profile",
                "11111",
                "build-new",
                "matched",
                "2024-06-01T00:00:00+00:00",
            ],
        )
        .unwrap();
    }

    let snapshot = store
        .lookup_latest_version_snapshot("latest-profile")
        .unwrap()
        .expect("snapshot should be present");

    assert_eq!(snapshot.steam_build_id.as_deref(), Some("build-new"));
    assert_eq!(snapshot.status, "matched");
}

#[test]
fn test_version_snapshot_pruning_at_max_limit() {
    let store = MetadataStore::open_in_memory().unwrap();
    {
        let conn = connection(&store);
        insert_test_profile_row(&conn, "prune-profile");
    }

    // Insert MAX+1 rows — the prune step must keep exactly MAX.
    for i in 0..=MAX_VERSION_SNAPSHOTS_PER_PROFILE {
        store
            .upsert_version_snapshot(
                "prune-profile",
                "55555",
                Some(&format!("build-{i:04}")),
                None,
                None,
                None,
                "untracked",
            )
            .unwrap();
    }

    let conn = connection(&store);
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM version_snapshots WHERE profile_id = 'prune-profile'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(
        count, MAX_VERSION_SNAPSHOTS_PER_PROFILE as i64,
        "row count must be exactly MAX after pruning"
    );
}

#[test]
fn test_acknowledge_version_change_sets_matched() {
    let store = MetadataStore::open_in_memory().unwrap();
    {
        let conn = connection(&store);
        insert_test_profile_row(&conn, "ack-profile");
    }

    store
        .upsert_version_snapshot(
            "ack-profile",
            "77777",
            None,
            None,
            None,
            None,
            "game_updated",
        )
        .unwrap();

    // Confirm initial status is game_updated.
    let before = store
        .lookup_latest_version_snapshot("ack-profile")
        .unwrap()
        .unwrap();
    assert_eq!(before.status, "game_updated");

    store.acknowledge_version_change("ack-profile").unwrap();

    let after = store
        .lookup_latest_version_snapshot("ack-profile")
        .unwrap()
        .unwrap();
    assert_eq!(after.status, "matched");
}

#[test]
fn test_load_version_snapshots_for_profiles_returns_latest_per_profile() {
    let store = MetadataStore::open_in_memory().unwrap();
    {
        let conn = connection(&store);
        insert_test_profile_row(&conn, "bulk-profile-a");
        insert_test_profile_row(&conn, "bulk-profile-b");
    }

    // Profile A: two snapshots — the second (game_updated) should win.
    {
        let conn = connection(&store);
        conn.execute(
            "INSERT INTO version_snapshots (profile_id, steam_app_id, status, checked_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                "bulk-profile-a",
                "10001",
                "untracked",
                "2024-01-01T00:00:00+00:00",
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO version_snapshots (profile_id, steam_app_id, status, checked_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                "bulk-profile-a",
                "10001",
                "game_updated",
                "2024-06-01T00:00:00+00:00",
            ],
        )
        .unwrap();
    }

    // Profile B: one snapshot.
    {
        let conn = connection(&store);
        conn.execute(
            "INSERT INTO version_snapshots (profile_id, steam_app_id, status, checked_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                "bulk-profile-b",
                "20002",
                "matched",
                "2024-03-01T00:00:00+00:00",
            ],
        )
        .unwrap();
    }

    let snapshots = store.load_version_snapshots_for_profiles().unwrap();

    assert_eq!(snapshots.len(), 2, "should return one row per profile");

    let snap_a = snapshots
        .iter()
        .find(|s| s.profile_id == "bulk-profile-a")
        .expect("profile-a snapshot must be present");
    let snap_b = snapshots
        .iter()
        .find(|s| s.profile_id == "bulk-profile-b")
        .expect("profile-b snapshot must be present");

    // MAX(id) picks the last-inserted row for profile-a, which is game_updated.
    assert_eq!(snap_a.status, "game_updated");
    assert_eq!(snap_b.status, "matched");
}

#[test]
fn test_version_store_disabled_store_noop() {
    let store = MetadataStore::disabled();

    assert!(store
        .upsert_version_snapshot("any-profile", "12345", None, None, None, None, "untracked")
        .is_ok());
    let snapshot = store.lookup_latest_version_snapshot("any-profile").unwrap();
    assert!(snapshot.is_none());
    let snapshots = store.load_version_snapshots_for_profiles().unwrap();
    assert!(snapshots.is_empty());
    assert!(store.acknowledge_version_change("any-profile").is_ok());
}
