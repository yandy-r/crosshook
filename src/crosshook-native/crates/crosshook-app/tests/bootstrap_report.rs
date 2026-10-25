//! Integration tests for the public bootstrap contract: `App::bootstrap`
//! against explicit tempdir roots, a fake-Flatpak Phase-A token built through
//! the `test-seams` host seam, the live startup snapshot, job lifecycle via
//! `StartupHandle::wait_finished` (no sleeps), and `AppEvent` publication.
//!
//! Every test uses:
//! - `StoreRoots::Explicit` — no XDG fallback, no real user directories.
//! - `LoggingMode::Skip` — the tracing subscriber is once-per-process.
//! - `UmuRefresh::Skip` — no network in tests.
//! - `RuntimeChoice::Handle(Handle::current())` on the current-thread test
//!   runtime: jobs are spawned but not polled until the first `.await`, so
//!   subscribing before awaiting makes every `StartupJobFinished` event
//!   deterministic to observe.

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::Path;

use crosshook_app::bootstrap::{
    pre_runtime_with_host, BootstrapError, BootstrapOptions, BootstrapStep, BootstrappedApp,
    FlatpakMigrationReport, FlatpakMode, JobSnapshotKind, JobState, LoggingMode, PreRuntimeHost,
    PreRuntimeOptions, StepOutcome, StoreKind, StoreRoots, UmuRefresh,
};
use crosshook_app::{App, AppEvent, StartupJobKind};
use crosshook_core::metadata::MetadataStatus;
use crosshook_core::profile::{GameProfile, ProfileStore};
use crosshook_core::settings::{AppSettingsData, SettingsStore};
use tempfile::tempdir;

/// In-memory Phase-A host: single-threaded, Flatpak-isolated, no real
/// environment mutation. The migration outcome mirrors one imported config,
/// one imported subtree and one skipped subtree so the report assertions are
/// exact.
struct FakeFlatpakHost {
    env: HashMap<OsString, OsString>,
}

impl PreRuntimeHost for FakeFlatpakHost {
    fn thread_count(&self) -> Option<u32> {
        Some(1)
    }

    fn is_flatpak(&self) -> bool {
        true
    }

    fn host_xdg_opt_in(&self) -> bool {
        false
    }

    fn override_xdg(&mut self) {}

    fn run_flatpak_migration(&mut self) -> Result<FlatpakMigrationReport, String> {
        Ok(FlatpakMigrationReport {
            imported_config: true,
            imported_subtrees: vec!["crosshook/community".to_owned()],
            skipped_subtrees: vec!["crosshook/prefixes".to_owned()],
            error: None,
        })
    }

    fn migrate_legacy_app_id(&mut self) {}

    fn env_var(&self, key: &OsStr) -> Option<OsString> {
        self.env.get(key).cloned()
    }

    fn set_env_var(&mut self, key: &OsStr, value: &OsStr) {
        self.env.insert(key.to_owned(), value.to_owned());
    }

    fn remove_env_var(&mut self, key: &OsStr) {
        self.env.remove(key);
    }
}

/// A fake-Flatpak Phase-A token (isolated mode), built without touching real
/// environment variables.
fn fake_flatpak_pre_runtime() -> crosshook_app::bootstrap::PreRuntime {
    let mut host = FakeFlatpakHost {
        env: HashMap::new(),
    };
    pre_runtime_with_host(PreRuntimeOptions::new(), &mut host).expect("fake-host Phase A succeeds")
}

fn explicit_roots(dir: &Path) -> StoreRoots {
    StoreRoots::Explicit {
        config_dir: dir.join("config"),
        data_dir: dir.join("data"),
    }
}

fn test_options(dir: &Path) -> BootstrapOptions {
    BootstrapOptions::new()
        .with_runtime_handle(tokio::runtime::Handle::current())
        .with_store_roots(explicit_roots(dir))
        .with_logging(LoggingMode::Skip)
        .with_umu_refresh(UmuRefresh::Skip {
            reason: "integration test",
        })
}

fn write_settings(dir: &Path, settings: &AppSettingsData) {
    let store = SettingsStore::with_base_path(dir.join("config"));
    fs::create_dir_all(dir.join("config")).unwrap();
    store.save(settings).unwrap();
}

fn save_profile(dir: &Path, name: &str) {
    let store = ProfileStore::with_base_path(dir.join("config").join("profiles"));
    store.save(name, &GameProfile::default()).unwrap();
}

const ALL_JOB_KINDS: [StartupJobKind; 5] = [
    StartupJobKind::HealthBatch,
    StartupJobKind::OfflineReadiness,
    StartupJobKind::VersionScan,
    StartupJobKind::UmuRefresh,
    StartupJobKind::BackgroundPortal,
];

/// The returned report carries the Phase-A flatpak outcome verbatim, fresh
/// stores are not degraded, and the step log records both phases in the
/// canonical order.
#[tokio::test]
async fn bootstrap_returns_initial_report_with_flatpak_outcome_and_full_step_log() {
    let dir = tempdir().unwrap();
    let (app, initial) = App::bootstrap(fake_flatpak_pre_runtime(), test_options(dir.path()))
        .expect("bootstrap succeeds");

    // Flatpak outcome moves from the Phase-A token into the report untouched.
    assert_eq!(initial.flatpak.mode, FlatpakMode::Isolated);
    let migration = initial.flatpak.migration.as_ref().expect("migration ran");
    assert!(migration.imported_config);
    assert_eq!(migration.imported_subtrees, ["crosshook/community"]);
    assert_eq!(migration.skipped_subtrees, ["crosshook/prefixes"]);
    assert!(migration.error.is_none());

    // Fresh config dir: no auto-load, onboarding offered, nothing degraded.
    assert_eq!(initial.auto_load_profile, None);
    assert!(initial.onboarding.show);
    assert!(!initial.onboarding.has_profiles);
    assert!(!initial.degraded.settings_unreadable);
    assert!(!initial.degraded.logging_unavailable);
    // The umu skip is stored before any poll (it finishes inline during
    // bootstrap); the scheduled jobs are still pending because nothing has
    // been awaited on this current-thread runtime yet.
    assert!(matches!(initial.jobs.umu_refresh, JobState::Skipped { .. }));
    assert!(matches!(initial.jobs.health, JobState::Pending));
    assert!(matches!(initial.jobs.offline_readiness, JobState::Pending));
    assert!(matches!(initial.jobs.version_scan, JobState::Pending));
    assert!(matches!(initial.jobs.background_portal, JobState::Pending));

    // App accessors and the startup handle agree with the returned report.
    let startup = app.startup();
    let snapshot = startup.snapshot();
    assert_eq!(snapshot.auto_load_profile, initial.auto_load_profile);
    assert_eq!(snapshot.onboarding.show, initial.onboarding.show);
    assert_eq!(snapshot.flatpak.mode, FlatpakMode::Isolated);

    // Full step log: Phase A steps then Phase B steps, canonical order.
    let steps: Vec<BootstrapStep> = app
        .bootstrap_steps()
        .records()
        .iter()
        .map(|record| record.step)
        .collect();
    assert_eq!(
        steps,
        vec![
            BootstrapStep::FlatpakGate,
            BootstrapStep::FlatpakMigration,
            BootstrapStep::LegacyAppIdMigration,
            BootstrapStep::EnvPolicy,
            BootstrapStep::Runtime,
            BootstrapStep::SettingsStore,
            BootstrapStep::Logging,
            BootstrapStep::DevelopmentScripts,
            BootstrapStep::ProfileStore,
            BootstrapStep::RecentFilesStore,
            BootstrapStep::CommunityTapStore,
            BootstrapStep::MetadataStore,
            BootstrapStep::Catalogs,
            BootstrapStep::MetadataReconciliation,
            BootstrapStep::NagSync,
            BootstrapStep::StartupJobs,
        ]
    );
}

/// `auto_load_profile` and `onboarding` are live: recomputed per snapshot from
/// the settings store, so completing onboarding (a settings save) flips the
/// next snapshot without re-bootstrapping.
#[tokio::test]
async fn live_fields_recompute_after_settings_save() {
    let dir = tempdir().unwrap();
    save_profile(dir.path(), "elden-ring");
    write_settings(
        dir.path(),
        &AppSettingsData {
            auto_load_last_profile: true,
            last_used_profile: "elden-ring".to_owned(),
            onboarding_completed: false,
            ..Default::default()
        },
    );
    let (app, initial) = App::bootstrap(fake_flatpak_pre_runtime(), test_options(dir.path()))
        .expect("bootstrap succeeds");

    // Pre-save: onboarding shows, auto-load resolves to the existing profile.
    assert!(initial.onboarding.show);
    assert!(initial.onboarding.has_profiles);
    assert_eq!(initial.auto_load_profile.as_deref(), Some("elden-ring"));

    // Complete onboarding: only settings changed, no re-bootstrap.
    let mut completed = AppSettingsData::default();
    completed.auto_load_last_profile = true;
    completed.last_used_profile = "elden-ring".to_owned();
    completed.onboarding_completed = true;
    app.stores().settings.save(&completed).unwrap();

    let after = app.startup().snapshot();
    assert!(!after.onboarding.show, "onboarding is live per snapshot");
    assert!(after.onboarding.has_profiles);
    assert_eq!(after.auto_load_profile.as_deref(), Some("elden-ring"));
}

/// Jobs resolve to terminal states via `wait_finished` (no sleeps), every job
/// publishes exactly one `StartupJobFinished` event, the snapshot read off
/// each event already contains the terminal state (state-then-publish), and a
/// snapshot taken after all completions still carries the results (the
/// late-subscriber guarantee).
#[tokio::test]
async fn jobs_finish_with_events_and_late_snapshot_keeps_results() {
    let dir = tempdir().unwrap();
    save_profile(dir.path(), "game");

    let (app, initial) = App::bootstrap(fake_flatpak_pre_runtime(), test_options(dir.path()))
        .expect("bootstrap succeeds");
    // Subscribe before any await: the current-thread runtime has not polled
    // the spawned jobs yet, so all five completion events are observable.
    let mut receiver = app.subscribe();
    let startup = app.startup();
    assert!(matches!(initial.jobs.umu_refresh, JobState::Skipped { .. }));

    for kind in ALL_JOB_KINDS {
        let finished = startup.wait_finished(kind).await;
        assert_ne!(
            finished,
            JobSnapshotKind::Pending,
            "{kind:?} must reach a terminal state"
        );
        assert_ne!(finished, JobSnapshotKind::Running);
    }

    // Late-subscriber guarantee: the snapshot after all completions still
    // carries each job's result — a webview mounting late never misses one.
    let late = startup.snapshot();
    let offline = match &late.jobs.offline_readiness {
        JobState::Complete { data } => data.total_profiles,
        other => panic!("offline readiness should be complete, got {other:?}"),
    };
    assert_eq!(offline, 1, "one profile in the store");
    assert!(matches!(
        &late.jobs.version_scan,
        JobState::Complete { data } if data.scanned == 0
    ));
    match &late.jobs.umu_refresh {
        JobState::Skipped { reason } => assert_eq!(reason, "integration test"),
        other => panic!("umu refresh should be skipped, got {other:?}"),
    }
    assert!(matches!(
        &late.jobs.background_portal,
        JobState::Complete { .. }
    ));

    // One completion event per scheduled job, and state-then-publish holds:
    // at the moment the event is observed, the snapshot already shows the
    // terminal state for that job. The umu skip finished inline during
    // `App::bootstrap` (before any subscriber could exist), so exactly the
    // four scheduled jobs publish here.
    let mut seen: Vec<StartupJobKind> = Vec::new();
    for _ in 0..4 {
        let event = receiver
            .recv()
            .await
            .expect("a StartupJobFinished event per scheduled job");
        match event {
            AppEvent::StartupJobFinished { job } => {
                assert!(
                    !matches!(job, StartupJobKind::UmuRefresh),
                    "umu skip event fired during bootstrap, before subscribers"
                );
                assert!(
                    !seen.contains(&job),
                    "duplicate completion event for {job:?}"
                );
                seen.push(job);
                let snapshot = startup.snapshot();
                assert!(
                    snapshot_has_terminal_state(&snapshot, job),
                    "state-then-publish violated for {job:?}: snapshot still shows a non-terminal state"
                );
            }
            AppEvent::Resync { .. } => panic!("no lag expected on this bus"),
            _ => {}
        }
    }
    assert_eq!(seen.len(), 4);
}

/// Whether the live snapshot already shows a terminal state for `job`.
fn snapshot_has_terminal_state(
    report: &crosshook_app::bootstrap::StartupReport,
    job: StartupJobKind,
) -> bool {
    let terminal = match job {
        StartupJobKind::HealthBatch => job_kind_of(&report.jobs.health),
        StartupJobKind::OfflineReadiness => job_kind_of(&report.jobs.offline_readiness),
        StartupJobKind::VersionScan => job_kind_of(&report.jobs.version_scan),
        StartupJobKind::UmuRefresh => job_kind_of(&report.jobs.umu_refresh),
        StartupJobKind::BackgroundPortal => job_kind_of(&report.jobs.background_portal),
    };
    !matches!(
        terminal,
        JobSnapshotKind::Pending | JobSnapshotKind::Running
    )
}

/// Terminal-kind projection mirroring `JobSnapshotKind` for any job payload.
fn job_kind_of<T>(state: &JobState<T>) -> JobSnapshotKind {
    match state {
        JobState::Pending => JobSnapshotKind::Pending,
        JobState::Running => JobSnapshotKind::Running,
        JobState::Complete { .. } => JobSnapshotKind::Complete,
        JobState::Failed { .. } => JobSnapshotKind::Failed,
        JobState::Skipped { .. } => JobSnapshotKind::Skipped,
    }
}

/// A metadata DB path that cannot be a database degrades the metadata store
/// (never fatal), and the report says so in `degraded` — on the returned
/// report and on later snapshots.
#[tokio::test]
async fn metadata_db_directory_degrades_instead_of_failing() {
    let dir = tempdir().unwrap();
    let data = dir.path().join("data");
    fs::create_dir_all(data.join("metadata.db")).unwrap();

    let (app, report) = App::bootstrap(fake_flatpak_pre_runtime(), test_options(dir.path()))
        .expect("bootstrap must not fail");

    assert!(matches!(
        report.degraded.metadata,
        MetadataStatus::Disabled { .. }
    ));
    assert!(
        report.degraded.metadata_error.is_some(),
        "degraded reason must be carried"
    );
    assert!(matches!(
        app.startup().snapshot().degraded.metadata,
        MetadataStatus::Disabled { .. }
    ));
}

/// An unreadable settings.toml is degraded, not fatal: defaults load, the
/// report flags `settings_unreadable`, and the step log records the
/// degradation.
#[tokio::test]
async fn unreadable_settings_degrades_to_defaults() {
    let dir = tempdir().unwrap();
    let config = dir.path().join("config");
    fs::create_dir_all(&config).unwrap();
    // A directory where settings.toml must be a file: loading fails.
    fs::create_dir_all(config.join("settings.toml")).unwrap();

    let (app, report) = App::bootstrap(fake_flatpak_pre_runtime(), test_options(dir.path()))
        .expect("bootstrap must not fail");

    assert!(report.degraded.settings_unreadable);
    // Defaults: onboarding shown, no auto-load.
    assert_eq!(report.auto_load_profile, None);
    assert!(report.onboarding.show);
    assert!(matches!(
        app.bootstrap_steps()
            .records()
            .iter()
            .find(|record| record.step == BootstrapStep::SettingsStore)
            .map(|record| record.outcome),
        Some(StepOutcome::Degraded(_))
    ));
}

/// A config root that is a file is a typed fatal error — bootstrap returns
/// `Err` instead of exiting the process.
#[tokio::test]
async fn config_dir_file_is_typed_store_init_error() {
    let dir = tempdir().unwrap();
    let file_root = dir.path().join("not-a-dir");
    fs::write(&file_root, b"file").unwrap();

    let options = BootstrapOptions::new()
        .with_runtime_handle(tokio::runtime::Handle::current())
        .with_store_roots(StoreRoots::Explicit {
            config_dir: file_root,
            data_dir: dir.path().join("data"),
        })
        .with_logging(LoggingMode::Skip)
        .with_umu_refresh(UmuRefresh::Skip {
            reason: "integration test",
        });

    let error = App::bootstrap(fake_flatpak_pre_runtime(), options)
        .err()
        .expect("file-as-config-root must fail");

    match &error {
        BootstrapError::StoreInit { store, detail } => {
            assert_eq!(*store, StoreKind::Settings);
            assert!(!detail.is_empty());
        }
        other => panic!("expected StoreInit, got {other:?}"),
    }
    assert!(!error.user_message().is_empty());
}

/// The umu skip reason from `BootstrapOptions` is carried into the report.
#[tokio::test]
async fn umu_skip_reason_lands_in_report() {
    let dir = tempdir().unwrap();
    let (app, report) = App::bootstrap(fake_flatpak_pre_runtime(), test_options(dir.path()))
        .expect("bootstrap succeeds");
    let _ = app.subscribe();
    match &report.jobs.umu_refresh {
        JobState::Skipped { reason } => assert_eq!(reason, "integration test"),
        other => panic!("umu refresh should be skipped, got {other:?}"),
    }
}

/// `wait_finished` after completion returns immediately (late waiters never
/// hang on the retained watch state).
#[tokio::test]
async fn wait_finished_after_completion_returns_immediately() {
    let dir = tempdir().unwrap();
    let (app, _) = App::bootstrap(fake_flatpak_pre_runtime(), test_options(dir.path()))
        .expect("bootstrap succeeds");
    let startup = app.startup();
    // Drive the jobs to completion first.
    for kind in ALL_JOB_KINDS {
        let _ = startup.wait_finished(kind).await;
    }
    // A second wait is immediate and agrees with the terminal kind.
    let again = startup.wait_finished(StartupJobKind::VersionScan).await;
    assert_eq!(again, JobSnapshotKind::Complete);
}

/// A bootstrapped owner exposes its stores and startup state without any
/// panicking accessor: clones share them and `Deref` reaches the runtime owner.
#[tokio::test]
async fn bootstrapped_app_clones_share_state_and_deref_to_app() {
    let dir = tempdir().unwrap();
    let (app, _) = App::bootstrap(fake_flatpak_pre_runtime(), test_options(dir.path()))
        .expect("bootstrap succeeds");
    let clone: BootstrappedApp = app.clone();
    assert!(std::ptr::eq(app.stores(), clone.stores()));
    let _events = clone.subscribe();
    assert_eq!(clone.startup_report().flatpak.mode, FlatpakMode::Isolated);
}
