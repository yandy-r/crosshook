//! Version snapshot and known-good tag tests (Task 2.4).
//!
//! These call `record_success_effects` directly (the success gate lives in the
//! finalizer): a profile present in metadata gets a version snapshot and its
//! latest config revision tagged known-good under the conditions moved from
//! the old stream finalizer (I-48/I-49).

use std::fs;
use std::sync::Arc;

use crosshook_core::launch::{
    LaunchHookExecutionContext, LaunchSessionRegistry, SessionKind, WatchdogOutcome,
};
use crosshook_core::metadata::{
    ConfigRevisionSource, MetadataStore, SyncSource, MAX_CONFIG_REVISIONS_PER_PROFILE,
};
use crosshook_core::profile::GameProfile;

use super::super::snapshot::record_success_effects;
use super::super::types::SessionContext;
use super::helpers::{Harness, Metadata};

const PROFILE: &str = "snapshot-profile";

fn seed_profile(metadata: &MetadataStore, name: &str) -> String {
    metadata
        .observe_profile_write(
            name,
            &GameProfile::default(),
            std::path::Path::new("/tmp/snapshot-profile.toml"),
            SyncSource::AppWrite,
            None,
        )
        .unwrap();
    metadata.lookup_profile_id(name).unwrap().unwrap()
}

fn seed_revision(metadata: &MetadataStore, profile_id: &str, name: &str, hash: &str) -> i64 {
    metadata
        .insert_config_revision(
            profile_id,
            name,
            ConfigRevisionSource::ManualSave,
            hash,
            "toml = true",
            None,
            MAX_CONFIG_REVISIONS_PER_PROFILE,
        )
        .unwrap()
        .unwrap()
}

fn make_ctx(
    harness: &Harness,
    profile_name: Option<String>,
    steam_app_id: String,
    steam_client_path: String,
    trainer_host_path: Option<String>,
) -> SessionContext {
    let registry = Arc::new(LaunchSessionRegistry::new());
    let (session_id, _rx) = registry.register(SessionKind::Game, "snapshot-test");
    SessionContext {
        session_id,
        kind: SessionKind::Game,
        method: "proton_run",
        profile_name,
        steam_app_id,
        steam_client_path,
        trainer_host_path,
        log_path: harness.root.path().join("launch.log"),
        operation_id: None,
        watchdog_outcome: WatchdogOutcome::new(),
        coexistence: Vec::new(),
        post_exit_hooks: Vec::new(),
        hook_context: LaunchHookExecutionContext::default(),
        portal_guard: None,
        metadata: harness.metadata.clone(),
        registry,
    }
}

fn write_manifest(steam_root: &std::path::Path, app_id: &str, state_flags: u32) {
    let steamapps = steam_root.join("steamapps");
    fs::create_dir_all(&steamapps).unwrap();
    fs::write(
        steamapps.join(format!("appmanifest_{app_id}.acf")),
        format!(
            "\"AppState\"\n{{\n  \"appid\" \"{app_id}\"\n  \"installdir\" \"Game\"\n  \"buildid\" \"12345\"\n  \"StateFlags\" \"{state_flags}\"\n}}\n"
        ),
    )
    .unwrap();
}

#[tokio::test]
async fn first_success_writes_untracked_snapshot_even_with_empty_app_id() {
    let harness = Harness::new(Metadata::Enabled);
    let profile_id = seed_profile(&harness.metadata, PROFILE);
    let revision_id = seed_revision(&harness.metadata, &profile_id, PROFILE, "hash-1");

    let ctx = make_ctx(
        &harness,
        Some(PROFILE.to_string()),
        String::new(),
        String::new(),
        None,
    );
    record_success_effects(&ctx).await;

    let snapshot = harness
        .metadata
        .lookup_latest_version_snapshot(&profile_id)
        .unwrap()
        .expect("first success writes a snapshot");
    assert_eq!(snapshot.status, "untracked");
    assert_eq!(snapshot.steam_app_id, "");
    assert_eq!(snapshot.steam_build_id, None);

    let revisions = harness
        .metadata
        .list_config_revisions(&profile_id, None)
        .unwrap();
    assert_eq!(revisions.len(), 1);
    assert_eq!(revisions[0].id, revision_id);
    assert!(
        revisions[0].is_last_known_working,
        "latest revision is tagged known-good"
    );
}

#[tokio::test]
async fn second_success_with_changed_trainer_hash_records_trainer_changed() {
    let harness = Harness::new(Metadata::Enabled);
    let profile_id = seed_profile(&harness.metadata, PROFILE);

    let trainer_path = harness.root.path().join("trainer.exe");
    fs::write(&trainer_path, b"trainer-v1").unwrap();
    let trainer = trainer_path.to_string_lossy().into_owned();

    // Baseline with an empty build id (no Steam manifest), as a first launch
    // with an empty app id would record. Only this row exists when
    // `record_success_effects` looks up the prior snapshot, so the
    // `checked_at` second-resolution cannot tie there.
    harness
        .metadata
        .upsert_version_snapshot(
            &profile_id,
            "",
            Some(""),
            None,
            crosshook_core::metadata::hash_trainer_file(&trainer_path).as_deref(),
            None,
            "untracked",
        )
        .unwrap();

    fs::write(&trainer_path, b"trainer-v2").unwrap();

    let ctx = make_ctx(
        &harness,
        Some(PROFILE.to_string()),
        String::new(),
        String::new(),
        Some(trainer),
    );
    record_success_effects(&ctx).await;

    // `load_version_snapshots_for_profiles` picks MAX(id) per profile, so it
    // is deterministic even when two rows share a `checked_at` second.
    let snapshots = harness
        .metadata
        .load_version_snapshots_for_profiles()
        .unwrap();
    let snapshot = snapshots
        .iter()
        .find(|row| row.profile_id == profile_id)
        .expect("second success writes a snapshot");
    assert_eq!(snapshot.status, "trainer_changed");
}

#[tokio::test]
async fn update_in_progress_skips_snapshot_but_still_tags_known_good() {
    let harness = Harness::new(Metadata::Enabled);
    let profile_id = seed_profile(&harness.metadata, PROFILE);
    let revision_id = seed_revision(&harness.metadata, &profile_id, PROFILE, "hash-1");

    // Steam root with steamapps/ so discovery accepts it, plus a manifest
    // whose StateFlags != 4 (update in progress).
    let steam_root = harness.root.path().join("steam");
    write_manifest(&steam_root, "12345", 6);

    let ctx = make_ctx(
        &harness,
        Some(PROFILE.to_string()),
        "12345".to_string(),
        steam_root.to_string_lossy().into_owned(),
        None,
    );
    record_success_effects(&ctx).await;

    assert!(
        harness
            .metadata
            .lookup_latest_version_snapshot(&profile_id)
            .unwrap()
            .is_none(),
        "UpdateInProgress skips the snapshot"
    );
    let revisions = harness
        .metadata
        .list_config_revisions(&profile_id, None)
        .unwrap();
    assert_eq!(revisions[0].id, revision_id);
    assert!(
        revisions[0].is_last_known_working,
        "known-good tag still runs when the snapshot is skipped"
    );
}

#[tokio::test]
async fn no_profile_id_skips_both_effects() {
    let harness = Harness::new(Metadata::Enabled);
    // No profile seeded: lookup_profile_id returns None.
    let ctx = make_ctx(
        &harness,
        Some("missing-profile".to_string()),
        String::new(),
        String::new(),
        None,
    );
    record_success_effects(&ctx).await;

    assert!(harness
        .metadata
        .load_version_snapshots_for_profiles()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn disabled_metadata_is_silent_noop() {
    let harness = Harness::new(Metadata::Disabled);
    let ctx = make_ctx(
        &harness,
        Some(PROFILE.to_string()),
        String::new(),
        String::new(),
        None,
    );
    // Must not panic and must not write anything observable.
    record_success_effects(&ctx).await;
    assert!(harness
        .metadata
        .load_version_snapshots_for_profiles()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn no_profile_name_is_noop() {
    let harness = Harness::new(Metadata::Enabled);
    let ctx = make_ctx(&harness, None, String::new(), String::new(), None);
    record_success_effects(&ctx).await;
    assert!(harness
        .metadata
        .load_version_snapshots_for_profiles()
        .unwrap()
        .is_empty());
}
