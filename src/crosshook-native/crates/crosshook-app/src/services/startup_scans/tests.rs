use super::*;

use crate::bootstrap::state::StartupHandle;
use crate::bootstrap::{
    DegradedStores, FlatpakMode, FlatpakStartup, JobSnapshotKind, JobState, StartupJobs, Stores,
};
use crate::events::EventReceiver;
use crate::jobs::{JobEventKind, JobRegistry, JobSpec};
use crosshook_core::community::CommunityTapStore;
use crosshook_core::metadata::MetadataStatus;
use crosshook_core::profile::GameProfile;
use crosshook_core::settings::{RecentFilesStore, SettingsStore};
use tokio_util::sync::CancellationToken;

struct Fixture {
    _dir: tempfile::TempDir,
    stores: Stores,
    startup: StartupHandle,
    events: EventBus,
}

fn fixture(profile_names: &[&str], metadata: MetadataStore) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let stores = Stores {
        settings: SettingsStore::with_base_path(dir.path().join("config")),
        profiles: ProfileStore::with_base_path(dir.path().join("profiles")),
        recent_files: RecentFilesStore::with_path(dir.path().join("recent.toml")),
        community_taps: CommunityTapStore::with_base_path(dir.path().join("taps")),
        metadata,
    };
    for name in profile_names {
        stores.profiles.save(name, &GameProfile::default()).unwrap();
    }
    let startup = StartupHandle::new(
        stores.clone(),
        FlatpakStartup {
            mode: FlatpakMode::Native,
            migration: None,
        },
        DegradedStores {
            metadata: MetadataStatus::Ok,
            metadata_error: None,
            settings_unreadable: false,
            logging_unavailable: false,
        },
        StartupJobs {
            health: JobState::Pending,
            offline_readiness: JobState::Pending,
            version_scan: JobState::Pending,
            umu_refresh: JobState::Pending,
            background_portal: JobState::Pending,
        },
    );
    Fixture {
        _dir: dir,
        stores,
        startup,
        events: EventBus::default(),
    }
}

fn service(fixture: &Fixture, health_jobs: JobRegistry) -> StartupScansService {
    StartupScansService::new(
        HealthService::new(
            fixture.stores.profiles.clone(),
            fixture.stores.metadata.clone(),
            health_jobs,
        ),
        fixture.stores.profiles.clone(),
        fixture.stores.metadata.clone(),
        fixture.events.clone(),
    )
}

/// Runs the sequence as a real supervised job on `jobs` and returns its outcome.
async fn run_as_job(
    jobs: &JobRegistry,
    scans: StartupScansService,
    startup: StartupHandle,
) -> Outcome {
    let mut handle = jobs
        .start(JobSpec::new("startup-scans"), move |ctx| async move {
            scans.run(&ctx, &startup).await
        })
        .expect("job starts");
    loop {
        let event = handle.recv().await.expect("Finished precedes stream end");
        if let JobEventKind::Finished(outcome) = event.kind {
            return outcome;
        }
    }
}

async fn recv_n(receiver: &mut EventReceiver, n: usize) -> Vec<AppEvent> {
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(receiver.recv().await.expect("bus open"));
    }
    out
}

/// Success path: every step publishes `StartupJobFinished` first and its named
/// event second, in health → offline → version order, and the snapshot read
/// at each `StartupJobFinished` already holds the terminal state.
#[tokio::test]
async fn publishes_finished_then_named_event_per_step_in_order() {
    let fixture = fixture(&["alpha", "beta"], MetadataStore::disabled());
    let mut receiver = fixture.events.subscribe();
    let jobs = JobRegistry::default();

    let outcome = run_as_job(
        &jobs,
        service(&fixture, jobs.clone()),
        fixture.startup.clone(),
    )
    .await;
    assert_eq!(outcome, Outcome::Succeeded);

    let events = recv_n(&mut receiver, 6).await;
    let finished = |job| AppEvent::StartupJobFinished { job };
    assert_eq!(events[0], finished(StartupJobKind::HealthBatch));
    assert!(matches!(
        events[1],
        AppEvent::HealthScanCompleted { total: 2, .. }
    ));
    assert_eq!(events[2], finished(StartupJobKind::OfflineReadiness));
    assert_eq!(
        events[3],
        AppEvent::OfflineReadinessScanCompleted { total_profiles: 2 }
    );
    assert_eq!(events[4], finished(StartupJobKind::VersionScan));
    assert_eq!(
        events[5],
        AppEvent::VersionScanCompleted {
            scanned: 0,
            mismatches: 0
        }
    );

    // State-then-publish: the retained snapshot carries each result.
    let jobs_snapshot = fixture.startup.jobs_snapshot();
    match &jobs_snapshot.health {
        JobState::Complete { data } => {
            assert_eq!(data.total_count, 2);
            assert_eq!(
                events[1],
                AppEvent::HealthScanCompleted {
                    total: data.total_count,
                    healthy: data.healthy_count,
                    stale: data.stale_count,
                    broken: data.broken_count,
                }
            );
        }
        other => panic!("health should be complete, got {other:?}"),
    }
    assert!(matches!(
        jobs_snapshot.offline_readiness,
        JobState::Complete { data } if data.total_profiles == 2
    ));
    assert!(matches!(
        jobs_snapshot.version_scan,
        JobState::Complete { .. }
    ));
}

/// Named events must never precede the terminal state they describe.
#[tokio::test]
async fn named_events_are_published_after_state_is_stored() {
    let fixture = fixture(&["alpha"], MetadataStore::disabled());
    let mut receiver = fixture.events.subscribe();
    let startup = fixture.startup.clone();
    let observer = tokio::spawn(async move {
        // Stops after the version scan's named event.
        loop {
            match receiver.recv().await.expect("bus open") {
                AppEvent::HealthScanCompleted { .. } => {
                    assert_eq!(
                        startup.jobs_snapshot().health.kind(),
                        JobSnapshotKind::Complete
                    );
                }
                AppEvent::OfflineReadinessScanCompleted { .. } => {
                    assert_eq!(
                        startup.jobs_snapshot().offline_readiness.kind(),
                        JobSnapshotKind::Complete
                    );
                }
                AppEvent::VersionScanCompleted { .. } => {
                    assert_eq!(
                        startup.jobs_snapshot().version_scan.kind(),
                        JobSnapshotKind::Complete
                    );
                    return;
                }
                _ => {}
            }
        }
    });
    let jobs = JobRegistry::default();
    run_as_job(
        &jobs,
        service(&fixture, jobs.clone()),
        fixture.startup.clone(),
    )
    .await;
    observer.await.expect("observer assertions hold");
}

/// A job cancelled before it starts lands every step `skipped`, publishes only
/// `StartupJobFinished` events and no named completion event.
#[tokio::test]
async fn cancelled_job_skips_every_step_without_named_events() {
    let fixture = fixture(&["alpha"], MetadataStore::disabled());
    let mut receiver = fixture.events.subscribe();
    let token = CancellationToken::new();
    let jobs = JobRegistry::for_app(tokio::runtime::Handle::current(), &token);
    token.cancel();

    let outcome = run_as_job(
        &jobs,
        service(&fixture, JobRegistry::default()),
        fixture.startup.clone(),
    )
    .await;
    assert_eq!(outcome, Outcome::Cancelled);

    let events = recv_n(&mut receiver, 3).await;
    assert_eq!(
        events,
        vec![
            AppEvent::StartupJobFinished {
                job: StartupJobKind::HealthBatch
            },
            AppEvent::StartupJobFinished {
                job: StartupJobKind::OfflineReadiness
            },
            AppEvent::StartupJobFinished {
                job: StartupJobKind::VersionScan
            },
        ]
    );
    let snapshot = fixture.startup.jobs_snapshot();
    for kind in [
        snapshot.health.kind(),
        snapshot.offline_readiness.kind(),
        snapshot.version_scan.kind(),
    ] {
        assert_eq!(kind, JobSnapshotKind::Skipped);
    }
}

/// App shutdown cancelling the nested health batch job is not a failure: the
/// health step and every later step land `skipped`, no named scan event is
/// published, and the scan job resolves `Cancelled`.
#[tokio::test]
async fn cancelled_health_batch_skips_remaining_steps_instead_of_failing() {
    let fixture = fixture(&["alpha", "beta", "gamma"], MetadataStore::disabled());
    let mut receiver = fixture.events.subscribe();
    // The health service runs on a registry that is already shut down, so the
    // batch resolves `Cancelled` while the scan job itself runs normally.
    let token = CancellationToken::new();
    let dead_jobs = JobRegistry::for_app(tokio::runtime::Handle::current(), &token);
    token.cancel();
    let jobs = JobRegistry::default();

    let outcome = run_as_job(&jobs, service(&fixture, dead_jobs), fixture.startup.clone()).await;
    assert_eq!(outcome, Outcome::Cancelled);

    let events = recv_n(&mut receiver, 3).await;
    assert_eq!(
        events,
        vec![
            AppEvent::StartupJobFinished {
                job: StartupJobKind::HealthBatch
            },
            AppEvent::StartupJobFinished {
                job: StartupJobKind::OfflineReadiness
            },
            AppEvent::StartupJobFinished {
                job: StartupJobKind::VersionScan
            },
        ]
    );
    let snapshot = fixture.startup.jobs_snapshot();
    for kind in [
        snapshot.health.kind(),
        snapshot.offline_readiness.kind(),
        snapshot.version_scan.kind(),
    ] {
        assert_eq!(kind, JobSnapshotKind::Skipped);
    }
}

/// A step that fails to schedule lands `failed` in the snapshot before its
/// `StartupJobFinished` is published. `bootstrap::scans::schedule_scans`
/// applies this helper to each of the three steps; this test covers one.
#[tokio::test]
async fn start_failed_lands_failed_before_publishing() {
    let fixture = fixture(&[], MetadataStore::disabled());
    let mut receiver = fixture.events.subscribe();

    finish_start_failed(
        &fixture.startup,
        &fixture.events,
        StartupJobKind::HealthBatch,
        "job could not be scheduled".into(),
    );

    assert_eq!(
        receiver.recv().await,
        Some(AppEvent::StartupJobFinished {
            job: StartupJobKind::HealthBatch
        })
    );
    assert_eq!(
        fixture.startup.jobs_snapshot().health.kind(),
        JobSnapshotKind::Failed
    );
}
