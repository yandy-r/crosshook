use std::path::{Path, PathBuf};
use std::process::Command;

use crosshook_core::community::CommunityTapSyncStatus;

use super::helpers::{tap, write_manifest, Harness};
use crate::{
    jobs::{JobEventKind, Outcome},
    AppErrorKind,
};

fn git(directory: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args([
            "-c",
            "user.name=CrossHook",
            "-c",
            "user.email=crosshook@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(super) struct BareFixture {
    pub(super) remote: PathBuf,
    seed: PathBuf,
}
impl BareFixture {
    pub(super) fn new(root: &Path) -> Self {
        let remote = root.join("remote.git");
        let seed = root.join("seed");
        git(
            root,
            &[
                "init",
                "--bare",
                "--initial-branch=main",
                remote.to_str().unwrap(),
            ],
        );
        git(
            root,
            &["init", "--initial-branch=main", seed.to_str().unwrap()],
        );
        git(
            &seed,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        let fixture = Self { remote, seed };
        fixture.update("First Game");
        fixture
    }
    fn update(&self, name: &str) {
        write_manifest(
            &self.seed.join("profiles/game/community-profile.json"),
            name,
        );
        git(&self.seed, &["add", "."]);
        git(&self.seed, &["commit", "-m", name]);
        git(&self.seed, &["push", "origin", "main"]);
    }
}

#[tokio::test]
async fn community_bare_sync_indexes_updates_backfills_and_preserves_cached_timestamp() {
    let harness = Harness::new();
    let fixture = BareFixture::new(harness.root.path());
    let subscription = tap(fixture.remote.to_str().unwrap());
    harness.service.add_tap(subscription.clone()).await.unwrap();
    let first = harness.service.sync().await.unwrap().remove(0);
    assert_eq!(first.status, CommunityTapSyncStatus::Cloned);
    assert_eq!(first.index.entries.len(), 1);
    assert!(first.last_sync_at.is_some());
    assert_eq!(harness.service.jobs().job_counts(), (0, 0));
    let rows = harness.service.list_indexed_profiles().await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].game_name.as_deref(), Some("First Game"));
    let tap_id = harness
        .metadata
        .lookup_community_tap_id(&subscription.url, "")
        .unwrap()
        .unwrap();
    let offline = harness
        .metadata
        .lookup_community_tap_offline_state_row(&tap_id)
        .unwrap()
        .unwrap();
    assert_eq!(offline.last_sync_at, first.last_sync_at);

    harness.metadata.with_sqlite_conn("reset test watermark", |conn| {
        conn.execute_batch("UPDATE community_taps SET last_head_commit = NULL; UPDATE community_profiles SET trainer_loading_mode = NULL;").unwrap();
        Ok(())
    }).unwrap();
    let same_head = harness.service.sync().await.unwrap().remove(0);
    assert_eq!(same_head.head_commit, first.head_commit);
    harness
        .metadata
        .with_sqlite_conn("read loading mode", |conn| {
            let mode: Option<String> = conn
                .query_row(
                    "SELECT trainer_loading_mode FROM community_profiles",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(mode.is_some());
            Ok(())
        })
        .unwrap();

    fixture.update("Updated Game");
    let updated = harness.service.sync().await.unwrap().remove(0);
    assert_eq!(updated.status, CommunityTapSyncStatus::Updated);
    assert_ne!(updated.head_commit, first.head_commit);
    assert_eq!(
        harness.service.list_indexed_profiles().await.unwrap()[0]
            .game_name
            .as_deref(),
        Some("Updated Game")
    );
    std::fs::rename(&fixture.remote, harness.root.path().join("unavailable.git")).unwrap();
    let cached = harness.service.sync().await.unwrap().remove(0);
    assert!(cached.from_cache);
    assert_eq!(cached.status, CommunityTapSyncStatus::CachedFallback);
    assert_eq!(cached.last_sync_at, updated.last_sync_at);
    assert_eq!(harness.service.jobs().job_counts(), (0, 0));
}

#[tokio::test]
async fn community_result_is_take_once_and_unknown_after_reap() {
    let harness = Harness::new();
    let mut job = harness.service.start_sync().unwrap();
    let id = job.id().clone();
    while let Some(event) = job.recv().await {
        if let JobEventKind::Finished(outcome) = event.kind {
            assert_eq!(outcome, Outcome::Succeeded);
            break;
        }
    }
    assert_eq!(harness.service.sync_result(&id).unwrap(), Some(vec![]));
    assert_eq!(harness.service.sync_result(&id).unwrap(), None);
    harness.service.jobs().reap(&id).unwrap();
    assert_eq!(
        harness.service.sync_result(&id).unwrap_err().kind(),
        AppErrorKind::NotFound
    );
}

#[tokio::test]
async fn community_sync_remains_usable_with_disabled_metadata() {
    let harness = Harness::with_metadata(crosshook_core::metadata::MetadataStore::disabled());
    let fixture = BareFixture::new(harness.root.path());
    harness
        .service
        .add_tap(tap(fixture.remote.to_str().unwrap()))
        .await
        .unwrap();
    let result = harness.service.sync().await.unwrap().remove(0);
    assert_eq!(result.index.entries.len(), 1);
    assert!(result.last_sync_at.is_none());
    assert_eq!(harness.service.jobs().job_counts(), (0, 0));
}

#[tokio::test]
async fn community_hard_error_aborts_before_any_metadata_finalize() {
    let harness = Harness::new();
    let fixture = BareFixture::new(harness.root.path());
    harness
        .service
        .add_tap(tap(fixture.remote.to_str().unwrap()))
        .await
        .unwrap();
    harness
        .service
        .add_tap(tap(harness
            .root
            .path()
            .join("missing.git")
            .to_str()
            .unwrap()))
        .await
        .unwrap();
    assert!(harness.service.sync().await.is_err());
    assert!(harness
        .metadata
        .list_community_tap_profiles(None)
        .unwrap()
        .is_empty());
    assert_eq!(harness.service.jobs().job_counts(), (0, 0));
}

#[tokio::test]
async fn community_sync_result_survives_outcome_eviction_and_another_sync() {
    let mut harness = Harness::new();
    harness.service.jobs = crate::jobs::JobRegistry::new(crate::jobs::JobRegistryConfig {
        retention: 0,
        ..crate::jobs::JobRegistryConfig::default()
    })
    .unwrap();
    let mut first = harness.service.start_sync().unwrap();
    let first_id = first.id().clone();
    assert!(matches!(
        first.recv().await.unwrap().kind,
        JobEventKind::Finished(Outcome::Succeeded)
    ));
    assert_eq!(
        harness
            .service
            .jobs()
            .outcome(&first_id)
            .unwrap_err()
            .kind(),
        AppErrorKind::NotFound
    );

    // Starting a later job must not prune a result merely because the job
    // registry has evicted its outcome.
    assert_eq!(harness.service.sync().await.unwrap(), vec![]);
    assert_eq!(
        harness.service.sync_result(&first_id).unwrap(),
        Some(vec![])
    );
    assert_eq!(
        harness.service.sync_result(&first_id).unwrap_err().kind(),
        AppErrorKind::NotFound
    );
    let unknown = crate::jobs::JobId::from_string("unknown");
    assert_eq!(
        harness.service.sync_result(&unknown).unwrap_err().kind(),
        AppErrorKind::NotFound
    );
}
