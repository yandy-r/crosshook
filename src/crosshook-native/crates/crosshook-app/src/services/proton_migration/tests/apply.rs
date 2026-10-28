//! `apply` / `apply_batch` tests: delegation to the shared writer.

use std::fs;

use crosshook_core::profile::migration::{
    ApplyMigrationRequest, BatchMigrationRequest, MigrationOutcome, ProtonPathField,
};

use super::{proton_file, Harness};
use crate::{AppErrorKind, AppEvent, ProfileChangeReason};

fn request(name: &str, new_path: &str) -> ApplyMigrationRequest {
    ApplyMigrationRequest {
        profile_name: name.to_string(),
        field: ProtonPathField::RuntimeProtonPath,
        new_path: new_path.to_string(),
    }
}

#[tokio::test]
async fn single_apply_writes_profile_and_emits_one_migrated_event() {
    let mut harness = Harness::new();
    harness.seed_proton_run("Alpha", "/stale/GE-Proton9-20/proton");
    let new_path = proton_file(harness.root.path(), "GE-Proton9-21");
    let new_path = new_path.to_string_lossy().into_owned();

    let result = harness
        .service
        .apply(request("Alpha", &new_path))
        .await
        .unwrap();

    assert_eq!(result.profile_name, "Alpha");
    assert_eq!(result.outcome, MigrationOutcome::Applied);
    assert_eq!(result.error, None);
    assert_eq!(
        harness.profiles.load("Alpha").unwrap().runtime.proton_path,
        new_path
    );
    assert_eq!(
        harness.next_event().await,
        Some(AppEvent::ProfilesChanged {
            reason: ProfileChangeReason::Migrated,
            names: vec!["Alpha".to_string()],
        })
    );
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn single_apply_with_invalid_profile_name_is_a_validation_error() {
    let mut harness = Harness::new();
    let new_path = proton_file(harness.root.path(), "GE-Proton9-21");

    let error = harness
        .service
        .apply(request("../escape", &new_path.to_string_lossy()))
        .await
        .unwrap_err();

    assert!(matches!(error.kind(), AppErrorKind::Validation { .. }));
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn single_apply_with_vanished_replacement_errors_without_writing() {
    let mut harness = Harness::new();
    harness.seed_proton_run("Alpha", "/stale/GE-Proton9-20/proton");
    let before = fs::read(harness.profiles.base_path.join("Alpha.toml")).unwrap();
    let gone = harness.root.path().join("gone/proton");

    let error = harness
        .service
        .apply(request("Alpha", &gone.to_string_lossy()))
        .await
        .unwrap_err();

    assert_eq!(error.kind(), AppErrorKind::Validation { field: "new_path" });
    assert_eq!(
        fs::read(harness.profiles.base_path.join("Alpha.toml")).unwrap(),
        before
    );
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn batch_apply_reports_per_item_outcomes_and_emits_one_event() {
    let mut harness = Harness::new();
    let current = proton_file(harness.root.path(), "GE-Proton9-21");
    let current = current.to_string_lossy().into_owned();
    harness.seed_proton_run("Alpha", "/stale/GE-Proton9-20/proton");
    harness.seed_proton_run("Beta", "/stale/GE-Proton9-19/proton");
    harness.seed_proton_run("Gamma", &current);

    let result = harness
        .service
        .apply_batch(BatchMigrationRequest {
            migrations: vec![
                request("Alpha", &current),
                request("Beta", &current),
                request("Gamma", &current),
            ],
        })
        .await
        .unwrap();

    let outcomes: Vec<_> = result.results.iter().map(|r| r.outcome).collect();
    assert_eq!(
        outcomes,
        [
            MigrationOutcome::Applied,
            MigrationOutcome::Applied,
            MigrationOutcome::AlreadyValid
        ]
    );
    assert_eq!(
        (
            result.applied_count,
            result.skipped_count,
            result.failed_count
        ),
        (2, 1, 0)
    );
    assert_eq!(
        harness.next_event().await,
        Some(AppEvent::ProfilesChanged {
            reason: ProfileChangeReason::Migrated,
            names: vec!["Alpha".to_string(), "Beta".to_string()],
        })
    );
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn batch_apply_preflight_failure_writes_nothing() {
    let mut harness = Harness::new();
    harness.seed_proton_run("Alpha", "/stale/GE-Proton9-20/proton");
    let before = fs::read(harness.profiles.base_path.join("Alpha.toml")).unwrap();
    let good = proton_file(harness.root.path(), "GE-Proton9-21");
    let gone = harness.root.path().join("gone/proton");

    let error = harness
        .service
        .apply_batch(BatchMigrationRequest {
            migrations: vec![
                request("Alpha", &good.to_string_lossy()),
                request("Alpha", &gone.to_string_lossy()),
            ],
        })
        .await
        .unwrap_err();

    assert_eq!(error.kind(), AppErrorKind::Validation { field: "new_path" });
    assert_eq!(
        fs::read(harness.profiles.base_path.join("Alpha.toml")).unwrap(),
        before
    );
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn batch_apply_with_invalid_profile_name_is_a_validation_error() {
    let harness = Harness::new();
    let good = proton_file(harness.root.path(), "GE-Proton9-21");

    let error = harness
        .service
        .apply_batch(BatchMigrationRequest {
            migrations: vec![request("bad/name", &good.to_string_lossy())],
        })
        .await
        .unwrap_err();

    assert!(matches!(error.kind(), AppErrorKind::Validation { .. }));
}
