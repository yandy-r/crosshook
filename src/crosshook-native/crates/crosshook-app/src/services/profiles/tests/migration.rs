//! Proton migration apply tests.
//!
//! Owned by YAN-811 Task 1.5.

use std::fs;
use std::path::Path;

use crosshook_core::profile::migration::{
    ApplyMigrationRequest, BatchMigrationRequest, MigrationOutcome, ProtonPathField,
};

use super::helpers::{steam_profile, Harness, Metadata};
use crate::{AppErrorKind, AppEvent, ProfileChangeReason};

/// Saves a Steam profile whose `steam.proton_path` is `proton_path`.
fn seed(harness: &Harness, name: &str, proton_path: &str) {
    let mut profile = steam_profile("/home/tester");
    profile.steam.proton_path = proton_path.to_string();
    harness.profiles.save(name, &profile).unwrap();
}

/// Creates `<root>/<dir>/proton` and returns its path.
fn proton_file(root: &Path, dir: &str) -> String {
    let proton_dir = root.join(dir);
    fs::create_dir_all(&proton_dir).unwrap();
    let path = proton_dir.join("proton");
    fs::write(&path, b"#!/bin/sh\n").unwrap();
    path.to_string_lossy().into_owned()
}

fn request(name: &str, new_path: &str) -> ApplyMigrationRequest {
    ApplyMigrationRequest {
        profile_name: name.to_string(),
        field: ProtonPathField::SteamProtonPath,
        new_path: new_path.to_string(),
    }
}

fn toml_bytes(harness: &Harness, name: &str) -> Vec<u8> {
    fs::read(harness.profiles.base_path.join(format!("{name}.toml"))).unwrap()
}

#[tokio::test]
async fn applied_single_migration_observes_and_publishes_migrated() {
    let mut harness = Harness::new(Metadata::Enabled);
    seed(&harness, "Alpha", "/stale/GE-Proton9-4/proton");
    let new_path = proton_file(harness.root.path(), "GE-Proton9-7");

    let commit = harness
        .writer
        .apply_proton_migration(request("Alpha", &new_path))
        .await
        .unwrap();

    assert_eq!(commit.output.outcome, MigrationOutcome::Applied);
    assert!(commit.warnings.is_empty(), "{:?}", commit.warnings);
    assert_eq!(
        harness.profiles.load("Alpha").unwrap().steam.proton_path,
        new_path
    );
    assert!(harness
        .metadata
        .lookup_profile_id("Alpha")
        .unwrap()
        .is_some());
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
async fn already_valid_and_failed_single_migrations_emit_nothing() {
    let mut harness = Harness::new(Metadata::Enabled);
    let valid_old = proton_file(harness.root.path(), "GE-Proton9-4");
    let new_path = proton_file(harness.root.path(), "GE-Proton9-7");
    seed(&harness, "Valid", &valid_old);
    let before = toml_bytes(&harness, "Valid");

    let already_valid = harness
        .writer
        .apply_proton_migration(request("Valid", &new_path))
        .await
        .unwrap();
    // A replacement that exists but is a directory fails after the TOCTOU check.
    let failed = harness
        .writer
        .apply_proton_migration(request("Valid", &harness.root.path().to_string_lossy()))
        .await
        .unwrap();
    let missing_profile = harness
        .writer
        .apply_proton_migration(request("Missing", &new_path))
        .await
        .unwrap();

    assert_eq!(already_valid.output.outcome, MigrationOutcome::AlreadyValid);
    assert_eq!(failed.output.outcome, MigrationOutcome::Failed);
    assert_eq!(missing_profile.output.outcome, MigrationOutcome::Failed);
    assert_eq!(toml_bytes(&harness, "Valid"), before);
    assert!(harness
        .metadata
        .lookup_profile_id("Valid")
        .unwrap()
        .is_none());
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn vanished_replacement_is_an_error_without_writes() {
    let mut harness = Harness::new(Metadata::Enabled);
    seed(&harness, "Alpha", "/stale/proton");
    let before = toml_bytes(&harness, "Alpha");

    let error = harness
        .writer
        .apply_proton_migration(request("Alpha", "/nonexistent/GE-Proton9-7/proton"))
        .await
        .unwrap_err();

    assert_eq!(error.kind(), AppErrorKind::Validation { field: "new_path" });
    assert_eq!(toml_bytes(&harness, "Alpha"), before);
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn batch_preflight_rejection_writes_nothing_and_emits_nothing() {
    let mut harness = Harness::new(Metadata::Enabled);
    seed(&harness, "Alpha", "/stale/alpha/proton");
    seed(&harness, "Beta", "/stale/beta/proton");
    let new_path = proton_file(harness.root.path(), "GE-Proton9-7");
    let alpha_before = toml_bytes(&harness, "Alpha");
    let beta_before = toml_bytes(&harness, "Beta");

    // Missing replacement on the last item: the first item must not be written.
    let missing_path = harness
        .writer
        .apply_batch_migration(BatchMigrationRequest {
            migrations: vec![
                request("Alpha", &new_path),
                request("Beta", "/nonexistent/proton"),
            ],
        })
        .await
        .unwrap_err();
    // Unloadable profile on the last item.
    let missing_profile = harness
        .writer
        .apply_batch_migration(BatchMigrationRequest {
            migrations: vec![request("Alpha", &new_path), request("Ghost", &new_path)],
        })
        .await
        .unwrap_err();

    assert_eq!(
        missing_path.kind(),
        AppErrorKind::Validation { field: "new_path" }
    );
    assert!(missing_path
        .message()
        .starts_with("Pre-flight failed for 'Beta'"));
    assert_eq!(missing_profile.kind(), AppErrorKind::NotFound);
    assert!(missing_profile
        .message()
        .starts_with("Pre-flight failed for 'Ghost': could not load profile"));
    assert_eq!(toml_bytes(&harness, "Alpha"), alpha_before);
    assert_eq!(toml_bytes(&harness, "Beta"), beta_before);
    assert!(harness
        .metadata
        .lookup_profile_id("Alpha")
        .unwrap()
        .is_none());
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn batch_counts_outcomes_and_publishes_only_applied_names() {
    let mut harness = Harness::new(Metadata::Enabled);
    let valid_old = proton_file(harness.root.path(), "GE-Proton9-4");
    let new_path = proton_file(harness.root.path(), "GE-Proton9-7");
    seed(&harness, "Applied", "/stale/applied/proton");
    seed(&harness, "Skipped", &valid_old);
    seed(&harness, "Failed", "/stale/failed/proton");
    let skipped_before = toml_bytes(&harness, "Skipped");
    let failed_before = toml_bytes(&harness, "Failed");
    let directory = harness.root.path().to_string_lossy().into_owned();

    let commit = harness
        .writer
        .apply_batch_migration(BatchMigrationRequest {
            migrations: vec![
                request("Applied", &new_path),
                request("Skipped", &new_path),
                request("Failed", &directory),
            ],
        })
        .await
        .unwrap();

    let result = commit.output;
    assert_eq!(
        (
            result.applied_count,
            result.skipped_count,
            result.failed_count
        ),
        (1, 1, 1)
    );
    let outcomes: Vec<_> = result.results.iter().map(|r| r.outcome).collect();
    assert_eq!(
        outcomes,
        vec![
            MigrationOutcome::Applied,
            MigrationOutcome::AlreadyValid,
            MigrationOutcome::Failed,
        ]
    );
    assert!(result.results[2].error.is_some());
    assert_eq!(
        harness.profiles.load("Applied").unwrap().steam.proton_path,
        new_path
    );
    assert_eq!(toml_bytes(&harness, "Skipped"), skipped_before);
    assert_eq!(toml_bytes(&harness, "Failed"), failed_before);
    assert!(harness
        .metadata
        .lookup_profile_id("Applied")
        .unwrap()
        .is_some());
    assert!(harness
        .metadata
        .lookup_profile_id("Skipped")
        .unwrap()
        .is_none());
    assert!(harness
        .metadata
        .lookup_profile_id("Failed")
        .unwrap()
        .is_none());
    assert_eq!(
        harness.next_event().await,
        Some(AppEvent::ProfilesChanged {
            reason: ProfileChangeReason::Migrated,
            names: vec!["Applied".to_string()],
        })
    );
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn batch_with_nothing_applied_emits_nothing() {
    let mut harness = Harness::new(Metadata::Enabled);
    let valid_old = proton_file(harness.root.path(), "GE-Proton9-4");
    let new_path = proton_file(harness.root.path(), "GE-Proton9-7");
    seed(&harness, "Skipped", &valid_old);

    let commit = harness
        .writer
        .apply_batch_migration(BatchMigrationRequest {
            migrations: vec![request("Skipped", &new_path)],
        })
        .await
        .unwrap();

    assert_eq!(commit.output.skipped_count, 1);
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn migration_does_not_rewrite_mangohud_companion() {
    let harness = Harness::new(Metadata::Disabled);
    seed(&harness, "Alpha", "/stale/proton");
    // MangoHud is disabled, so `ProfileStore::save` would delete this file.
    let companion = harness.profiles.base_path.join("Alpha.mangohud.conf");
    fs::write(&companion, b"user-edited\n").unwrap();
    let new_path = proton_file(harness.root.path(), "GE-Proton9-7");

    let commit = harness
        .writer
        .apply_proton_migration(request("Alpha", &new_path))
        .await
        .unwrap();

    assert_eq!(commit.output.outcome, MigrationOutcome::Applied);
    assert_eq!(fs::read(&companion).unwrap(), b"user-edited\n");
}
