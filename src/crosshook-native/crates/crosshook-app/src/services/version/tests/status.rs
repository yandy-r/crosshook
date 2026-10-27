//! `VersionService` tests: status, snapshot, and write ops.

use crosshook_core::metadata::{hash_trainer_file, MetadataStoreError};

use super::helpers::Harness;
use crate::error::AppErrorKind;

#[tokio::test]
async fn check_status_matched_when_build_and_trainer_match() {
    let harness = Harness::new();
    let profile_id = harness.registered_profile("game");
    harness.write_manifest("100", 4);
    let hash = hash_trainer_file(&harness.trainer_path).unwrap();
    harness
        .metadata
        .upsert_version_snapshot(
            &profile_id,
            "12345",
            Some("100"),
            None,
            Some(&hash),
            None,
            "matched",
        )
        .unwrap();

    let result = harness.service.check_status("game".into()).await.unwrap();
    assert_eq!(result.profile_id, profile_id);
    assert_eq!(result.current_build_id.as_deref(), Some("100"));
    assert_eq!(result.status.as_str(), "matched");
    assert!(!result.update_in_progress);
    let snapshot = result.snapshot.expect("snapshot");
    assert_eq!(snapshot.steam_app_id, "12345");
    assert_eq!(snapshot.steam_build_id.as_deref(), Some("100"));
    assert_eq!(snapshot.status.as_str(), "matched");
}

#[tokio::test]
async fn check_status_update_in_progress_when_state_flags_not_four() {
    let harness = Harness::new();
    let profile_id = harness.registered_profile("game");
    harness.write_manifest("100", 2);
    let hash = hash_trainer_file(&harness.trainer_path).unwrap();
    harness
        .metadata
        .upsert_version_snapshot(
            &profile_id,
            "12345",
            Some("100"),
            None,
            Some(&hash),
            None,
            "matched",
        )
        .unwrap();

    let result = harness.service.check_status("game".into()).await.unwrap();
    assert_eq!(result.status.as_str(), "update_in_progress");
    assert!(result.update_in_progress);
}

#[tokio::test]
async fn check_status_untracked_when_no_manifest() {
    let harness = Harness::new();
    let profile_id = harness.registered_profile("game");
    // No manifest written; a snapshot without a build id keeps it untracked.
    harness
        .metadata
        .upsert_version_snapshot(&profile_id, "12345", None, None, None, None, "untracked")
        .unwrap();

    let result = harness.service.check_status("game".into()).await.unwrap();
    assert_eq!(result.profile_id, profile_id);
    assert_eq!(result.current_build_id, None);
    assert_eq!(result.status.as_str(), "untracked");
    assert!(!result.update_in_progress);
}

#[tokio::test]
async fn check_status_unreadable_trainer_hash_is_none() {
    let harness = Harness::new();
    let profile_id = harness.registered_profile("game");
    harness.write_manifest("100", 4);
    // Snapshot recorded against a trainer hash; the trainer file is now gone,
    // so the current hash reads as None and correlation sees a change.
    harness
        .metadata
        .upsert_version_snapshot(
            &profile_id,
            "12345",
            Some("100"),
            None,
            Some("deadbeef"),
            None,
            "matched",
        )
        .unwrap();
    std::fs::remove_file(&harness.trainer_path).unwrap();

    let result = harness.service.check_status("game".into()).await.unwrap();
    assert_eq!(result.status.as_str(), "trainer_changed");
    let snapshot = result.snapshot.expect("snapshot");
    assert_eq!(snapshot.trainer_file_hash.as_deref(), Some("deadbeef"));
}

#[tokio::test]
async fn check_status_unregistered_profile_has_empty_id_and_no_snapshot() {
    let harness = Harness::new();
    harness.save_profile("game");
    harness.write_manifest("100", 4);

    let result = harness.service.check_status("game".into()).await.unwrap();
    assert_eq!(result.profile_id, "");
    assert!(result.snapshot.is_none());
    assert_eq!(result.current_build_id.as_deref(), Some("100"));
}

#[tokio::test]
async fn snapshot_unknown_id_returns_none() {
    let harness = Harness::new();
    harness.save_profile("game");

    let snapshot = harness.service.snapshot("game".into()).await.unwrap();
    assert!(snapshot.is_none());

    let missing = harness.service.snapshot("nope".into()).await.unwrap();
    assert!(missing.is_none());
}

#[test]
fn status_from_str_unknown_maps_to_unknown() {
    use super::super::status::status_from_str;
    assert_eq!(status_from_str("matched").as_str(), "matched");
    assert_eq!(status_from_str("game_updated").as_str(), "game_updated");
    assert_eq!(
        status_from_str("trainer_changed").as_str(),
        "trainer_changed"
    );
    assert_eq!(status_from_str("both_changed").as_str(), "both_changed");
    assert_eq!(
        status_from_str("update_in_progress").as_str(),
        "update_in_progress"
    );
    assert_eq!(status_from_str("untracked").as_str(), "untracked");
    assert_eq!(status_from_str("bogus").as_str(), "unknown");
    assert_eq!(status_from_str("").as_str(), "unknown");
}

#[tokio::test]
async fn set_trainer_version_preserves_app_id_and_upserts_untracked() {
    let harness = Harness::new();
    let profile_id = harness.registered_profile("game");
    harness
        .metadata
        .upsert_version_snapshot(
            &profile_id,
            "12345",
            Some("100"),
            None,
            None,
            None,
            "matched",
        )
        .unwrap();

    harness
        .service
        .set_trainer_version("game".into(), "2.0".into())
        .await
        .unwrap();

    let snapshot = harness
        .service
        .snapshot("game".into())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.trainer_version.as_deref(), Some("2.0"));
    assert_eq!(snapshot.steam_app_id, "12345");
    assert_eq!(snapshot.status.as_str(), "untracked");
    // The row behind the snapshot is the one just upserted.
    let row = harness
        .metadata
        .lookup_latest_version_snapshot(&profile_id)
        .unwrap()
        .expect("row");
    assert_eq!(row.trainer_version.as_deref(), Some("2.0"));
    assert_eq!(row.steam_app_id, "12345");
    assert_eq!(row.status, "untracked");
}
#[tokio::test]
async fn set_trainer_version_unregistered_is_not_found() {
    let harness = Harness::new();
    harness.save_profile("game");

    let error = harness
        .service
        .set_trainer_version("game".into(), "1.0".into())
        .await
        .unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::NotFound);
    assert_eq!(
        error.message(),
        "profile 'game' is not registered in the metadata store"
    );
}

#[tokio::test]
async fn acknowledge_sets_matched() {
    let harness = Harness::new();
    let profile_id = harness.registered_profile("game");
    harness
        .metadata
        .upsert_version_snapshot(
            &profile_id,
            "12345",
            Some("100"),
            None,
            None,
            None,
            "game_updated",
        )
        .unwrap();

    harness
        .service
        .acknowledge_change("game".into())
        .await
        .unwrap();

    let snapshot = harness
        .service
        .snapshot("game".into())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.status.as_str(), "matched");
}

#[tokio::test]
async fn acknowledge_with_zero_snapshots_is_ok() {
    let harness = Harness::new();
    harness.registered_profile("game");

    harness
        .service
        .acknowledge_change("game".into())
        .await
        .unwrap();
}

#[tokio::test]
async fn disabled_db_writes_return_unavailable_before_not_registered() {
    let harness = Harness::disabled();
    // Profile is not registered (DB disabled), so Unavailable must win.
    let set_error = harness
        .service
        .set_trainer_version("ghost".into(), "1.0".into())
        .await
        .unwrap_err();
    assert_eq!(set_error.kind(), AppErrorKind::Unavailable);

    let ack_error = harness
        .service
        .acknowledge_change("ghost".into())
        .await
        .unwrap_err();
    assert_eq!(ack_error.kind(), AppErrorKind::Unavailable);
}

#[tokio::test]
async fn check_status_with_disabled_db_degrades_to_empty_id() {
    let harness = Harness::disabled();
    harness.save_profile("game");
    harness.write_manifest("100", 4);

    let result = harness.service.check_status("game".into()).await.unwrap();
    assert_eq!(result.profile_id, "");
    assert!(result.snapshot.is_none());
}

#[test]
fn metadata_validation_helper_is_wired_for_version_field() {
    // Guard that the D8 helper remains reachable with the version field;
    // exercised indirectly through store errors in the service paths.
    let error = crate::AppError::from_metadata_validation(
        "version",
        MetadataStoreError::Validation("boom".into()),
    );
    assert_eq!(error.kind(), AppErrorKind::Validation { field: "version" });
    assert_eq!(error.message(), "metadata validation error: boom");
}
