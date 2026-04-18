#![cfg(test)]

use std::fs;

use tempfile::tempdir;

use super::test_support::{connection, insert_test_profile_row};
use super::MetadataStore;

#[test]
fn verify_trainer_hash_second_hit_uses_cache() {
    let store = MetadataStore::open_in_memory().unwrap();
    {
        let conn = connection(&store);
        insert_test_profile_row(&conn, "profile-1");
    }
    let dir = tempdir().unwrap();
    let path = dir.path().join("trainer.exe");
    fs::write(&path, b"fake-trainer-bytes").unwrap();

    let first = store
        .verify_trainer_hash_for_profile_path("profile-1", &path)
        .unwrap()
        .expect("hash");
    assert!(!first.from_cache);

    let second = store
        .verify_trainer_hash_for_profile_path("profile-1", &path)
        .unwrap()
        .expect("hash");
    assert!(second.from_cache);
    assert_eq!(first.hash, second.hash);
}

#[test]
fn trainer_hash_launch_check_first_baseline() {
    let store = MetadataStore::open_in_memory().unwrap();
    {
        let conn = connection(&store);
        insert_test_profile_row(&conn, "launch-pid-1");
    }
    let dir = tempdir().unwrap();
    let path = dir.path().join("trainer.exe");
    fs::write(&path, b"v1-bytes").unwrap();
    let out = store
        .with_sqlite_conn("trainer hash launch test", |conn| {
            crate::offline::trainer_hash_launch_check(conn, "launch-pid-1", &path, None)
        })
        .unwrap();
    assert!(matches!(
        out.baseline,
        crate::offline::TrainerHashBaselineResult::FirstBaselineRecorded
    ));
}

#[test]
fn trainer_hash_launch_check_mismatch_after_content_change() {
    let store = MetadataStore::open_in_memory().unwrap();
    {
        let conn = connection(&store);
        insert_test_profile_row(&conn, "launch-pid-2");
    }
    let dir = tempdir().unwrap();
    let path = dir.path().join("trainer.exe");
    fs::write(&path, b"v1").unwrap();
    store
        .with_sqlite_conn("seed baseline", |conn| {
            crate::offline::trainer_hash_launch_check(conn, "launch-pid-2", &path, None)
        })
        .unwrap();
    fs::write(&path, b"v2-different").unwrap();
    let out = store
        .with_sqlite_conn("detect mismatch", |conn| {
            crate::offline::trainer_hash_launch_check(conn, "launch-pid-2", &path, None)
        })
        .unwrap();
    assert!(matches!(
        out.baseline,
        crate::offline::TrainerHashBaselineResult::Mismatch { .. }
    ));
}

#[test]
fn launch_issues_from_trainer_hash_maps_mismatch_and_community() {
    use crate::launch::launch_issues_from_trainer_hash_outcome;
    use crate::offline::{
        TrainerHashBaselineResult, TrainerHashCommunityAdvisory, TrainerHashLaunchOutcome,
    };

    let out = TrainerHashLaunchOutcome {
        baseline: TrainerHashBaselineResult::Mismatch {
            stored_hash: "aa".repeat(32),
            current_hash: "bb".repeat(32),
        },
        community_advisory: Some(TrainerHashCommunityAdvisory {
            expected: "cc".repeat(32),
            current: "dd".repeat(32),
        }),
    };
    let issues = launch_issues_from_trainer_hash_outcome(out);
    assert_eq!(issues.len(), 2);
    assert_eq!(issues[0].code.as_deref(), Some("trainer_hash_mismatch"));
    assert_eq!(
        issues[1].code.as_deref(),
        Some("trainer_hash_community_mismatch")
    );
}
