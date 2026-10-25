//! History, diff and rollback tests: SHA-256 revision dedupe, rollback
//! lineage, integrity failures that leave disk untouched, and diff caps with
//! semantic fallback.

use std::collections::BTreeMap;
use std::fs;

use crosshook_core::metadata::{
    sha256_hex, ConfigRevisionSource, MetadataStore, SyncSource, MAX_CONFIG_REVISIONS_PER_PROFILE,
};
use crosshook_core::profile::{
    GameProfile, ProfileStore, MAX_DIFF_OUTPUT_BYTES, MAX_SEMANTIC_CHANGES,
};
use crosshook_core::settings::SettingsStore;

use super::helpers::{Harness, Metadata};
use crate::services::profiles::types::{Effect, Persisted, ProfileMutation};
use crate::services::profiles::writer::CommitContext;
use crate::services::profiles::{ConfigDiffMode, ProfileWriter};
use crate::{AppError, AppErrorKind, AppEvent, EventBus, ProfileChangeReason};

const NAME: &str = "Alpha";

/// Saves a profile and records the standard save revision (test seed only).
struct SeedSave {
    profile: GameProfile,
}

impl ProfileMutation for SeedSave {
    type Output = ();

    fn accepts(&self, reason: &ProfileChangeReason) -> bool {
        matches!(reason, ProfileChangeReason::Saved)
    }

    fn persist(self, ctx: &CommitContext<'_>) -> Result<Persisted<()>, AppError> {
        ctx.profiles.save(NAME, &self.profile)?;
        Ok(Persisted {
            output: (),
            effects: vec![
                Effect::ObserveWrite {
                    name: NAME.to_string(),
                    profile: self.profile.clone(),
                    source: SyncSource::AppWrite,
                    source_profile_id: None,
                },
                Effect::Revision {
                    name: NAME.to_string(),
                    profile: self.profile,
                    source: ConfigRevisionSource::ManualSave,
                    source_revision_id: None,
                },
            ],
            names: Some(vec![NAME.to_string()]),
        })
    }
}

fn profile(game_name: &str) -> GameProfile {
    let mut profile = GameProfile::default();
    profile.game.name = game_name.to_string();
    profile
}

/// Saves `game_name` as [`NAME`] and returns the inserted revision id.
async fn seed(harness: &mut Harness, game_name: &str) -> i64 {
    seed_profile(harness, profile(game_name)).await
}

/// Saves `profile` as [`NAME`] and returns the inserted revision id.
async fn seed_profile(harness: &mut Harness, profile: GameProfile) -> i64 {
    let commit = harness
        .writer
        .commit(SeedSave { profile }, ProfileChangeReason::Saved)
        .await
        .unwrap();
    while harness.next_event().await.is_some() {}
    commit.new_revision_id.expect("seed revision inserted")
}

/// A profile with `count` custom env vars whose values are `value`.
fn env_profile(count: usize, value: &str) -> GameProfile {
    let mut profile = profile("Env");
    profile.launch.custom_env_vars = (0..count)
        .map(|i| (format!("VAR_{i:05}"), value.to_string()))
        .collect::<BTreeMap<_, _>>();
    profile
}

/// A harness over a file-backed metadata store, so revisions carry HMAC
/// signatures (ADR-0005).
fn signed_harness() -> Harness {
    let root = tempfile::tempdir().unwrap();
    let profiles = ProfileStore::with_base_path(root.path().join("profiles"));
    let settings = SettingsStore::with_base_path(root.path().join("config"));
    let metadata = MetadataStore::with_path(&root.path().join("metadata.db")).unwrap();
    let bus = EventBus::default();
    let events = bus.subscribe();
    let writer = ProfileWriter::new(profiles.clone(), metadata.clone(), settings.clone(), bus);
    Harness {
        root,
        profiles,
        settings,
        metadata,
        writer,
        events,
    }
}

fn profile_id(harness: &Harness) -> String {
    harness.metadata.lookup_profile_id(NAME).unwrap().unwrap()
}

fn revision_count(harness: &Harness) -> usize {
    harness
        .metadata
        .list_config_revisions(&profile_id(harness), None)
        .unwrap()
        .len()
}

/// Simulates a database-only edit of a revision row behind CrossHook's back.
fn tamper(harness: &Harness, revision_id: i64, snapshot: &str, content_hash: Option<&str>) {
    harness
        .metadata
        .with_sqlite_conn("tamper with a revision", |conn| {
            match content_hash {
                Some(hash) => conn.execute(
                    "UPDATE config_revisions SET snapshot_toml = ?1, content_hash = ?2 WHERE id = ?3",
                    (snapshot, hash, revision_id),
                )?,
                None => conn.execute(
                    "UPDATE config_revisions SET snapshot_toml = ?1 WHERE id = ?2",
                    (snapshot, revision_id),
                )?,
            };
            Ok(())
        })
        .unwrap();
}

fn disk_bytes(harness: &Harness) -> Vec<u8> {
    fs::read(harness.profiles.base_path.join(format!("{NAME}.toml"))).unwrap()
}

/// Asserts a rejected rollback wrote nothing, recorded nothing and published nothing.
async fn assert_rollback_rejected(harness: &mut Harness, revision_id: i64, field: &'static str) {
    let before = disk_bytes(harness);
    let revisions_before = revision_count(harness);

    let error = harness
        .writer
        .config_rollback(NAME.to_string(), revision_id)
        .await
        .unwrap_err();

    assert_eq!(error.kind(), AppErrorKind::Validation { field });
    assert_eq!(disk_bytes(harness), before);
    assert_eq!(revision_count(harness), revisions_before);
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn rollback_dedupes_against_latest_and_records_lineage_for_older() {
    let mut harness = Harness::new(Metadata::Enabled);
    let first = seed(&mut harness, "First").await;
    let second = seed(&mut harness, "Second").await;
    let pid = profile_id(&harness);
    harness
        .metadata
        .upsert_health_snapshot(&pid, "healthy", 0, "2026-01-01T00:00:00Z")
        .unwrap();

    // Same SHA-256 as the latest revision: restored, but no new revision row.
    let same = harness
        .writer
        .config_rollback(NAME.to_string(), second)
        .await
        .unwrap();
    assert_eq!(same.new_revision_id, None);
    assert_eq!(same.output.new_revision_id, None);
    assert!(same.warnings.is_empty(), "{:?}", same.warnings);
    assert_eq!(revision_count(&harness), 2);
    assert_eq!(
        harness.next_event().await,
        Some(AppEvent::ProfilesChanged {
            reason: ProfileChangeReason::RolledBack,
            names: vec![NAME.to_string()],
        })
    );
    assert_eq!(harness.next_event().await, None);

    // Returning to an older snapshot differs from the latest: new lineage row.
    let older = harness
        .writer
        .config_rollback(NAME.to_string(), first)
        .await
        .unwrap();
    let new_id = older.new_revision_id.expect("rollback revision inserted");
    assert_eq!(older.output.restored_revision_id, first);
    assert_eq!(older.output.new_revision_id, Some(new_id));
    assert_eq!(older.output.profile.game.name, "First");
    assert_eq!(harness.profiles.load(NAME).unwrap().game.name, "First");

    let latest = harness
        .metadata
        .get_config_revision(&pid, new_id)
        .unwrap()
        .unwrap();
    assert_eq!(latest.source, ConfigRevisionSource::RollbackApply.as_str());
    assert_eq!(latest.source_revision_id, Some(first));
    assert_eq!(revision_count(&harness), 3);
    assert!(harness
        .metadata
        .lookup_health_snapshot(&pid)
        .unwrap()
        .is_none());
    assert_eq!(
        harness.next_event().await,
        Some(AppEvent::ProfilesChanged {
            reason: ProfileChangeReason::RolledBack,
            names: vec![NAME.to_string()],
        })
    );
}

#[tokio::test]
async fn rollback_with_content_hash_mismatch_leaves_disk_unchanged() {
    let mut harness = Harness::new(Metadata::Enabled);
    let first = seed(&mut harness, "First").await;
    seed(&mut harness, "Second").await;
    let forged = toml::to_string_pretty(&profile("Forged")).unwrap();
    tamper(&harness, first, &forged, None);

    assert_rollback_rejected(&mut harness, first, "revision").await;
    assert_eq!(harness.profiles.load(NAME).unwrap().game.name, "Second");
}

#[tokio::test]
async fn rollback_with_forged_signed_revision_leaves_disk_unchanged() {
    let mut harness = signed_harness();
    let first = seed(&mut harness, "First").await;
    seed(&mut harness, "Second").await;
    // The attacker recomputes the SHA-256, so only the HMAC can catch it.
    let forged = toml::to_string_pretty(&profile("Forged")).unwrap();
    tamper(
        &harness,
        first,
        &forged,
        Some(&sha256_hex(forged.as_bytes())),
    );

    assert_rollback_rejected(&mut harness, first, "revision").await;
    assert_eq!(harness.profiles.load(NAME).unwrap().game.name, "Second");
}

#[tokio::test]
async fn rollback_rejects_revisions_of_other_profiles_and_disabled_metadata() {
    let mut harness = Harness::new(Metadata::Enabled);
    seed(&mut harness, "First").await;

    let error = harness
        .writer
        .config_rollback("Other".to_string(), 1)
        .await
        .unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::NotFound);

    let disabled = Harness::new(Metadata::Disabled);
    let error = disabled
        .writer
        .config_rollback(NAME.to_string(), 1)
        .await
        .unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Unavailable);
}

#[tokio::test]
async fn diff_caps_output_and_keeps_semantic_changes() {
    let mut harness = Harness::new(Metadata::Enabled);
    // 1000 short values: well under the snapshot cap, so it is recorded.
    let small = seed_profile(&mut harness, env_profile(1_000, "a")).await;
    // Written outside the writer: far above the snapshot cap, so it is only
    // reachable as the live right-hand side of the diff. Every shared key
    // changes and 1500 are added, well over the semantic change cap.
    let large = env_profile(2_500, &"x".repeat(300));
    harness.profiles.save(NAME, &large).unwrap();

    let semantic = harness
        .writer
        .config_diff(NAME.to_string(), small, None, Some("semantic".to_string()))
        .await
        .unwrap();
    assert!(semantic.truncated);
    assert!(semantic.diff_text.len() <= MAX_DIFF_OUTPUT_BYTES);
    assert!(semantic.diff_text.len() > MAX_DIFF_OUTPUT_BYTES - 1_024);
    assert!(semantic
        .diff_text
        .starts_with(&format!("--- revision/{small}")));
    assert!(semantic.added_lines > 0);
    assert_eq!(semantic.mode, ConfigDiffMode::Semantic);
    assert!(!semantic.semantic_parse_failed);
    assert_eq!(
        semantic.semantic_changes.map(|changes| changes.len()),
        Some(MAX_SEMANTIC_CHANGES)
    );

    let unified = harness
        .writer
        .config_diff(NAME.to_string(), small, None, None)
        .await
        .unwrap();
    assert_eq!(unified.mode, ConfigDiffMode::Unified);
    assert!(unified.semantic_changes.is_none());
    assert_eq!(unified.diff_text, semantic.diff_text);
}

#[tokio::test]
async fn invalid_snapshot_falls_back_to_unified_diff_and_aborts_rollback() {
    let mut harness = Harness::new(Metadata::Enabled);
    let valid = seed(&mut harness, "Valid").await;
    let invalid_toml = "[game\nname = \"broken\n";
    let invalid = harness
        .metadata
        .insert_config_revision(
            &profile_id(&harness),
            NAME,
            ConfigRevisionSource::ManualSave,
            &sha256_hex(invalid_toml.as_bytes()),
            invalid_toml,
            None,
            MAX_CONFIG_REVISIONS_PER_PROFILE,
        )
        .unwrap()
        .expect("invalid revision inserted");

    let diff = harness
        .writer
        .config_diff(
            NAME.to_string(),
            invalid,
            Some(valid),
            Some("semantic".to_string()),
        )
        .await
        .unwrap();
    assert_eq!(diff.mode, ConfigDiffMode::Unified);
    assert!(diff.semantic_parse_failed);
    assert!(diff.semantic_changes.is_none());
    assert!(!diff.diff_text.is_empty());

    assert_rollback_rejected(&mut harness, invalid, "revision").await;
}
