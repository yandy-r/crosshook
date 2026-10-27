use std::fs;

use crate::jobs::{JobEventKind, JobId, JobRegistry, JobRegistryConfig, Outcome};

use crate::log_tail::LogBatch;
use crate::services::storage::StorageService;
use crate::AppErrorKind;

use super::helpers::{finish, Harness};

#[tokio::test]
async fn scan_persists_snapshot_fields_for_every_prefix() {
    let harness = Harness::new(true);
    let orphan = harness.orphan();
    let staged = harness.staged(true);

    let result = harness.service.scan().await.unwrap();
    assert_eq!(result.prefixes.len(), 2);
    assert!(!result.inventory_incomplete);
    assert_eq!(result.orphan_targets, vec![orphan]);
    assert_eq!(result.stale_staged_targets, vec![staged]);
    let rows = harness
        .metadata
        .list_latest_prefix_storage_snapshots(50)
        .unwrap();
    assert_eq!(rows.len(), result.prefixes.len());
    for entry in &result.prefixes {
        let row = rows
            .iter()
            .find(|row| row.resolved_prefix_path == entry.resolved_prefix_path)
            .unwrap();
        assert_eq!(
            row.id,
            format!("{}-{}", entry.resolved_prefix_path, result.scanned_at)
        );
        assert_eq!(row.total_bytes, entry.total_bytes as i64);
        assert_eq!(
            row.staged_trainers_bytes,
            entry.staged_trainers_bytes as i64
        );
        assert_eq!(row.is_orphan, entry.is_orphan);
        assert_eq!(
            row.stale_staged_count,
            entry.stale_staged_trainers.len() as i64
        );
        assert_eq!(row.scanned_at, result.scanned_at);
        assert_eq!(
            serde_json::from_str::<Vec<String>>(&row.referenced_profiles_json).unwrap(),
            entry.referenced_by_profiles
        );
    }
}

#[tokio::test]
async fn concurrent_start_conflicts_and_job_result_is_taken_once_across_clones() {
    let harness = Harness::new(true);
    let handle = harness.service.start_scan().unwrap();
    let id = handle.id().clone();
    // This current-thread runtime has not yielded to the job yet.
    let error = harness.service.start_scan().err().unwrap();
    assert_eq!(error.kind(), AppErrorKind::Conflict);
    assert!(harness.service.scan_result(&id).unwrap().is_none());
    assert_eq!(finish(handle).await, Outcome::Succeeded);
    assert!(harness.service.clone().scan_result(&id).unwrap().is_some());
    assert!(harness.service.scan_result(&id).unwrap().is_none());
    assert_eq!(
        finish(harness.service.start_scan().unwrap()).await,
        Outcome::Succeeded
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropped_scan_waiter_removes_its_stored_result_slot() {
    let harness = Harness::new(true);
    let service = harness.service.clone();
    let scan = tokio::spawn(async move { service.scan().await });
    while harness.service.scan_results.lock().unwrap().is_empty() {
        tokio::task::yield_now().await;
    }

    scan.abort();
    assert!(scan.await.unwrap_err().is_cancelled());
    assert!(harness.service.scan_results.lock().unwrap().is_empty());
}

#[tokio::test]
async fn start_scan_prunes_finished_results_evicted_from_the_registry() {
    let harness = Harness::new(true);
    let jobs = JobRegistry::new(JobRegistryConfig {
        event_capacity: 8,
        retention: 0,
    })
    .unwrap();
    let service = StorageService::new(harness.profiles, harness.metadata, jobs);

    let first = service.start_scan().unwrap();
    let first_id = first.id().clone();
    assert_eq!(finish(first).await, Outcome::Succeeded);
    assert_eq!(service.scan_results.lock().unwrap().len(), 1);
    assert_eq!(
        service.scan_result(&first_id).unwrap_err().kind(),
        AppErrorKind::NotFound
    );

    assert_eq!(
        finish(service.start_scan().unwrap()).await,
        Outcome::Succeeded
    );
    assert_eq!(service.scan_results.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn scan_job_publishes_prefix_progress_and_summary_log() {
    let harness = Harness::new(true);
    harness.orphan();
    let mut events = harness.service.start_scan().unwrap().into_events();
    let mut progress = Vec::new();
    let mut logs = Vec::new();
    while let Some(event) = events.recv().await {
        match event.kind {
            JobEventKind::Progress(value) => progress.push(value),
            JobEventKind::Log(LogBatch::Lines(lines)) => logs.extend(lines),
            JobEventKind::Log(LogBatch::Dropped { .. }) => panic!("unexpected dropped scan log"),
            JobEventKind::Log(LogBatch::Truncated { .. }) => {
                panic!("unexpected truncated scan log")
            }
            JobEventKind::Finished(outcome) => assert_eq!(outcome, Outcome::Succeeded),
        }
    }
    assert_eq!(progress.len(), 2);
    for (index, value) in progress.iter().enumerate() {
        assert_eq!(value.current, index as u64 + 1);
        assert_eq!(value.total, Some(2));
        assert!(value.message.is_some());
    }
    assert_eq!(logs, vec!["scanned 2 prefixes"]);
}

#[tokio::test]
async fn cancelled_scan_has_no_result() {
    let harness = Harness::new(true);
    let handle = harness.service.start_scan().unwrap();
    let id = handle.id().clone();
    handle.cancel();
    assert_eq!(finish(handle).await, Outcome::Cancelled);
    assert!(harness.service.scan_result(&id).unwrap().is_none());
    assert!(harness
        .service
        .history()
        .await
        .unwrap()
        .snapshots
        .is_empty());
}

#[tokio::test]
async fn unknown_job_returns_not_found() {
    let harness = Harness::new(true);
    let error = harness
        .service
        .scan_result(&JobId::from_string("unknown"))
        .unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::NotFound);
}

#[tokio::test]
async fn disabled_metadata_does_not_prevent_scan() {
    let harness = Harness::new(false);
    harness.orphan();
    let result = harness.service.scan().await.unwrap();
    assert_eq!(result.prefixes.len(), 2);
    assert_eq!(result.orphan_targets.len(), 1);
    assert!(harness
        .service
        .history()
        .await
        .unwrap()
        .snapshots
        .is_empty());
}

#[tokio::test]
async fn snapshot_insert_error_is_fail_soft() {
    let harness = Harness::new(true);
    super::helpers::execute_sql(&harness.metadata, "DROP TABLE prefix_storage_snapshots");
    let orphan = harness.orphan();
    fs::write(
        harness.profiles.base_path.join("Broken.toml"),
        "not valid TOML = [",
    )
    .unwrap();
    let result = harness.service.scan().await.unwrap();
    assert!(result.inventory_incomplete);
    assert!(result.orphan_targets.is_empty());
    assert!(result
        .prefixes
        .iter()
        .any(
            |entry| entry.resolved_prefix_path == orphan.resolved_prefix_path && !entry.is_orphan
        ));
}

#[tokio::test]
async fn failed_scan_surfaces_error_and_leaves_no_result() {
    let harness = Harness::new(true);
    fs::remove_dir_all(&harness.profiles.base_path).unwrap();
    fs::write(&harness.profiles.base_path, b"not a directory").unwrap();
    let handle = harness.service.start_scan().unwrap();
    let id = handle.id().clone();
    let Outcome::Failed(error) = finish(handle).await else {
        panic!("scan should fail");
    };
    assert_eq!(error.kind(), AppErrorKind::Internal);
    assert!(error
        .message()
        .starts_with("failed to list profiles for prefix scan:"));
    assert!(harness.service.scan_result(&id).unwrap().is_none());
    assert_eq!(
        harness.service.scan().await.unwrap_err().kind(),
        AppErrorKind::Internal
    );
}
