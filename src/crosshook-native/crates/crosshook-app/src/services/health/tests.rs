use super::*;

#[tokio::test]
async fn batch_and_cached_snapshots_work_without_metadata() {
    let root = tempfile::tempdir().unwrap();
    let profiles = ProfileStore::with_base_path(root.path().join("profiles"));
    let service = HealthService::new(
        profiles,
        MetadataStore::disabled(),
        crate::jobs::JobRegistry::default(),
    );
    let summary = service.batch_summary().await.unwrap();
    assert_eq!(summary.total_count, 0);
    assert!(summary.profiles.is_empty());
    assert!(service.cached_health_snapshots().await.unwrap().is_empty());
    assert!(service
        .cached_offline_readiness_snapshots()
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn missing_profile_error_stays_sanitized() {
    let root = tempfile::tempdir().unwrap();
    let service = HealthService::new(
        ProfileStore::with_base_path(root.path().join("profiles")),
        MetadataStore::disabled(),
        crate::jobs::JobRegistry::default(),
    );
    let error = service.profile_health("missing".into()).await.unwrap_err();
    assert!(!error.to_string().contains("/home/"));
}

#[test]
fn health_wire_shape_preserves_flattening_and_omission() {
    let report = EnrichedProfileHealthReport {
        core: crosshook_core::profile::health::ProfileHealthReport {
            name: "sample".into(),
            status: crosshook_core::profile::health::HealthStatus::Healthy,
            launch_method: "proton_run".into(),
            issues: vec![],
            checked_at: "now".into(),
        },
        metadata: None,
        offline_readiness: None,
    };
    let value = serde_json::to_value(report).unwrap();
    assert_eq!(value["name"], "sample");
    assert_eq!(value["status"], "healthy");
    assert!(value["metadata"].is_null());
    assert!(value.get("core").is_none());
    assert!(value.get("offline_readiness").is_none());
}

/// Seeds `launch_operations` with rows spread across the 30-day window:
/// one failure 45 days ago (outside), one failure 1 day ago (inside) and one
/// success 40 days ago (outside, but still the last success on record).
fn seed_windowed_launches(metadata: &MetadataStore, profile: &str) -> String {
    metadata
        .with_sqlite_conn("seed windowed launches", |conn| {
            let insert = |id: &str, status: &str, started: &str, finished: &str| {
                conn.execute(
                    "INSERT INTO launch_operations \
                     (operation_id, profile_name, launch_method, status, started_at, finished_at) \
                     VALUES (?1, ?2, 'native', ?3, datetime('now', ?4), datetime('now', ?5))",
                    rusqlite::params![id, profile, status, started, finished],
                )
                .expect("seed launch operation");
            };
            insert("old-failure", "failed", "-45 days", "-45 days");
            insert("recent-failure", "failed", "-1 days", "-1 days");
            insert("old-success", "succeeded", "-40 days", "-40 days");
            Ok(())
        })
        .expect("seed launches");
    metadata
        .with_sqlite_conn("read seeded last success", |conn| {
            Ok(conn
                .query_row(
                    "SELECT finished_at FROM launch_operations WHERE operation_id = 'old-success'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .expect("read seeded success"))
        })
        .expect("read seeded last success")
}

fn windowed_service(root: &std::path::Path) -> (HealthService, String) {
    let profiles = ProfileStore::with_base_path(root.join("profiles"));
    profiles
        .save("windowed", &crosshook_core::profile::GameProfile::default())
        .unwrap();
    let metadata = MetadataStore::open_in_memory().unwrap();
    metadata.sync_profiles_from_store(&profiles).unwrap();
    let old_success = seed_windowed_launches(&metadata, "windowed");
    let service = HealthService::new(profiles, metadata, crate::jobs::JobRegistry::default());
    (service, old_success)
}

/// AC1: the batch runs on the blocking pool as a supervised job, so the
/// caller's executor keeps running while the pool is saturated, and the job
/// is reaped once the summary is returned.
#[test]
fn batch_summary_yields_while_blocking_pool_is_gated_and_reaps_job() {
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::Duration;

    let root = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    let jobs = crate::jobs::JobRegistry::new(Default::default()).unwrap();
    let service = HealthService::new(
        ProfileStore::with_base_path(root.path().join("profiles")),
        MetadataStore::disabled(),
        jobs.clone(),
    );
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let gate = runtime.spawn_blocking(move || {
        entered_tx.send(()).unwrap();
        release_rx.recv().unwrap();
    });
    entered_rx.recv().unwrap();
    let release = Arc::new(Mutex::new(Some(release_tx)));
    let unblock = release.clone();
    let (done_tx, done_rx) = mpsc::channel();
    // The watchdog only releases the gate if the executor stops making
    // progress; no sleeps or runtime timers are needed.
    let watchdog = std::thread::spawn(move || {
        if done_rx.recv_timeout(Duration::from_secs(5)).is_err() {
            if let Some(sender) = unblock.lock().unwrap().take() {
                let _ = sender.send(());
            }
        }
    });
    runtime.block_on(async {
        let batch = tokio::spawn(async move { service.batch_summary().await });
        tokio::task::yield_now().await;
        assert!(
            !batch.is_finished(),
            "batch must wait for the blocking pool"
        );
        assert_eq!(jobs.job_counts(), (1, 0), "batch runs as a supervised job");
        let sender = release.lock().unwrap().take().expect("watchdog fired");
        sender.send(()).unwrap();
        done_tx.send(()).unwrap();
        gate.await.unwrap();
        let summary = batch.await.unwrap().unwrap();
        assert_eq!(summary.total_count, 0);
        assert_eq!(jobs.job_counts(), (0, 0), "finished job is reaped");
    });
    watchdog.join().unwrap();
}

#[tokio::test]
async fn batch_summary_counts_only_in_window_failures_and_keeps_last_success() {
    let root = tempfile::tempdir().unwrap();
    let (service, old_success) = windowed_service(root.path());

    let summary = service.batch_summary().await.unwrap();

    let metadata = summary
        .profiles
        .iter()
        .find(|report| report.core.name == "windowed")
        .and_then(|report| report.metadata.as_ref())
        .expect("metadata enrichment for the seeded profile");
    assert_eq!(metadata.failure_count_30d, 1, "45-day-old failure excluded");
    assert_eq!(metadata.last_success.as_deref(), Some(old_success.as_str()));
    assert_eq!(metadata.total_launches, 3);
}

#[tokio::test]
async fn profile_health_counts_only_in_window_failures_and_keeps_last_success() {
    let root = tempfile::tempdir().unwrap();
    let (service, old_success) = windowed_service(root.path());

    let report = service.profile_health("windowed".into()).await.unwrap();

    let metadata = report.metadata.expect("metadata enrichment");
    assert_eq!(metadata.failure_count_30d, 1, "45-day-old failure excluded");
    assert_eq!(metadata.last_success.as_deref(), Some(old_success.as_str()));
    assert_eq!(metadata.total_launches, 3);
}

/// The batch is a supervised job: a registry cancelled before the call (app
/// shutdown) resolves to `Cancelled` instead of running validation.
#[tokio::test]
async fn batch_summary_is_cancelled_when_the_registry_is_shut_down() {
    let root = tempfile::tempdir().unwrap();
    let token = tokio_util::sync::CancellationToken::new();
    let jobs = crate::jobs::JobRegistry::for_app(tokio::runtime::Handle::current(), &token);
    token.cancel();
    let service = HealthService::new(
        ProfileStore::with_base_path(root.path().join("profiles")),
        MetadataStore::disabled(),
        jobs,
    );

    let error = service.batch_summary().await.unwrap_err();

    assert_eq!(error.kind(), AppErrorKind::Cancelled);
}
