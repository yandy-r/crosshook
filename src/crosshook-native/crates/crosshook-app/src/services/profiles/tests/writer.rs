//! Commit pipeline tests: reason gate, publish-on-persist, fail-soft
//! advisory effects, degraded metadata warnings and lock poison recovery.

use std::fs;

use crosshook_core::metadata::{ConfigRevisionSource, SyncSource};
use crosshook_core::profile::GameProfile;

use super::helpers::{Harness, Metadata};
use crate::services::profiles::types::{Effect, Persisted, ProfileMutation};
use crate::services::profiles::writer::CommitContext;
use crate::services::profiles::CommitWarning;
use crate::{AppError, AppErrorKind, AppEvent, ProfileChangeReason};

/// Saves `profile` as `name` and requests the standard save effects.
struct TestSave {
    name: String,
    profile: GameProfile,
    fail: bool,
}

impl TestSave {
    fn new(name: &str) -> Self {
        let mut profile = GameProfile::default();
        profile.game.name = name.to_string();
        Self {
            name: name.to_string(),
            profile,
            fail: false,
        }
    }
}

impl ProfileMutation for TestSave {
    type Output = String;

    fn accepts(&self, reason: &ProfileChangeReason) -> bool {
        matches!(reason, ProfileChangeReason::Saved)
    }

    fn persist(self, ctx: &CommitContext<'_>) -> Result<Persisted<String>, AppError> {
        if self.fail {
            return Err(AppError::validation("profile", "rejected"));
        }
        ctx.profiles.save(&self.name, &self.profile)?;
        Ok(Persisted {
            output: self.name.clone(),
            effects: vec![
                Effect::ObserveWrite {
                    name: self.name.clone(),
                    profile: self.profile.clone(),
                    source: SyncSource::AppWrite,
                    source_profile_id: None,
                },
                Effect::Revision {
                    name: self.name.clone(),
                    profile: self.profile,
                    source: ConfigRevisionSource::ManualSave,
                    source_revision_id: None,
                },
                Effect::InvalidateHealth {
                    name: self.name.clone(),
                },
            ],
            names: Some(vec![self.name]),
        })
    }
}

fn saved(name: &str) -> AppEvent {
    AppEvent::ProfilesChanged {
        reason: ProfileChangeReason::Saved,
        names: vec![name.to_string()],
    }
}

#[tokio::test]
async fn commit_persists_runs_effects_and_publishes_once() {
    let mut harness = Harness::new(Metadata::Enabled);

    let commit = harness
        .writer
        .commit(TestSave::new("Alpha"), ProfileChangeReason::Saved)
        .await
        .unwrap();

    assert_eq!(commit.output, "Alpha");
    assert!(commit.warnings.is_empty(), "{:?}", commit.warnings);
    assert!(commit.new_revision_id.is_some());
    assert!(harness.profiles.profile_exists("Alpha"));
    assert_eq!(harness.next_event().await, Some(saved("Alpha")));
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn identical_commit_dedupes_revision_without_warning() {
    let mut harness = Harness::new(Metadata::Enabled);
    let writer = harness.writer.clone();
    writer
        .commit(TestSave::new("Alpha"), ProfileChangeReason::Saved)
        .await
        .unwrap();

    let second = writer
        .commit(TestSave::new("Alpha"), ProfileChangeReason::Saved)
        .await
        .unwrap();

    assert_eq!(second.new_revision_id, None);
    assert!(second.warnings.is_empty(), "{:?}", second.warnings);
    assert_eq!(harness.next_event().await, Some(saved("Alpha")));
    assert_eq!(harness.next_event().await, Some(saved("Alpha")));
}

#[tokio::test]
async fn mismatched_reason_is_rejected_before_any_side_effect() {
    let mut harness = Harness::new(Metadata::Enabled);

    let error = harness
        .writer
        .commit(TestSave::new("Alpha"), ProfileChangeReason::FavoriteToggled)
        .await
        .unwrap_err();

    assert_eq!(error.kind(), AppErrorKind::Internal);
    assert!(!harness.profiles.profile_exists("Alpha"));
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn failed_persist_publishes_nothing() {
    let mut harness = Harness::new(Metadata::Enabled);
    let mut mutation = TestSave::new("Alpha");
    mutation.fail = true;

    let error = harness
        .writer
        .commit(mutation, ProfileChangeReason::Saved)
        .await
        .unwrap_err();

    assert_eq!(error.kind(), AppErrorKind::Validation { field: "profile" });
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn disabled_metadata_saves_publishes_and_warns() {
    let mut harness = Harness::new(Metadata::Disabled);

    let commit = harness
        .writer
        .commit(TestSave::new("Alpha"), ProfileChangeReason::Saved)
        .await
        .unwrap();

    assert_eq!(commit.warnings, vec![CommitWarning::MetadataUnavailable]);
    assert_eq!(commit.new_revision_id, None);
    assert!(harness.profiles.profile_exists("Alpha"));
    assert_eq!(harness.next_event().await, Some(saved("Alpha")));
}

#[tokio::test]
async fn commit_invalidates_cached_health_snapshot() {
    let harness = Harness::new(Metadata::Enabled);
    let writer = harness.writer.clone();
    writer
        .commit(TestSave::new("Alpha"), ProfileChangeReason::Saved)
        .await
        .unwrap();
    let profile_id = harness
        .metadata
        .lookup_profile_id("Alpha")
        .unwrap()
        .unwrap();
    harness
        .metadata
        .upsert_health_snapshot(&profile_id, "healthy", 0, "2026-01-01T00:00:00Z")
        .unwrap();

    writer
        .commit(TestSave::new("Alpha"), ProfileChangeReason::Saved)
        .await
        .unwrap();

    assert!(harness
        .metadata
        .lookup_health_snapshot(&profile_id)
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn poisoned_write_lock_is_recovered() {
    let mut harness = Harness::new(Metadata::Enabled);
    let lock = harness.writer.write_lock.clone();
    let _ = std::thread::spawn(move || {
        let _guard = lock.lock().unwrap();
        panic!("poison the writer lock");
    })
    .join();
    assert!(harness.writer.write_lock.is_poisoned());

    harness
        .writer
        .commit(TestSave::new("Alpha"), ProfileChangeReason::Saved)
        .await
        .unwrap();

    assert_eq!(harness.next_event().await, Some(saved("Alpha")));
}

#[tokio::test]
async fn persisted_toml_is_the_store_serialization() {
    let harness = Harness::new(Metadata::Disabled);
    let mutation = TestSave::new("Alpha");
    let expected = toml::to_string_pretty(&mutation.profile.storage_profile()).unwrap();

    harness
        .writer
        .commit(mutation, ProfileChangeReason::Saved)
        .await
        .unwrap();

    let on_disk = fs::read_to_string(harness.profiles.base_path.join("Alpha.toml")).unwrap();
    assert_eq!(on_disk, expected);
}
