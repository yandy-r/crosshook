//! Service-level startup recovery (D8) and explicit repair: pending rows are
//! restored under the prefix key and reported in the status; `failed` rows,
//! live prefixes and an unavailable store are left alone.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use crosshook_core::prefix_deps::PrefixVersionRestoreState;

use super::super::journal::RecoveryResult;
use super::super::seams::MetadataJournalStore;
use super::super::{LastRecovery, RecoveryOutcome};
use super::fakes::{FakeInUse, FakeProbe};
use super::helpers::{pending_row, read_calls, Harness, ToolScript};
use super::install::raw_prefix;
use crate::jobs::JobRegistry;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_restores_pending_row_and_reports_it() {
    let h = Harness::new();
    let tool = h.tool(&ToolScript::default());
    h.seed_row(&pending_row(&h.prefix, &tool));
    let jobs = JobRegistry::default();
    let service = h.service(jobs.clone(), &tool);

    let results = service.recover_on_startup().await;

    assert_eq!(results, vec![(h.prefix.clone(), RecoveryResult::Restored)]);
    assert_eq!(read_calls(&h.calls), vec!["-q win10"]);
    assert!(h.row().is_none());
    let status = service.repair_status(raw_prefix(&h)).await.unwrap();
    assert!(!status.required);
    assert!(!status.in_progress);
    assert_eq!(
        status.last_recovery,
        Some(LastRecovery {
            outcome: RecoveryOutcome::Restored,
            message: None,
        })
    );
    assert_eq!(jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_failure_is_reported_as_failed_recovery() {
    let h = Harness::new();
    let tool = h.tool(&ToolScript {
        restore_exit: 1,
        ..ToolScript::default()
    });
    h.seed_row(&pending_row(&h.prefix, &tool));
    let service = h.service(JobRegistry::default(), &tool);

    let results = service.recover_on_startup().await;

    assert_eq!(results.len(), 1);
    assert!(matches!(results[0].1, RecoveryResult::RestoreFailed { .. }));
    let status = service.repair_status(raw_prefix(&h)).await.unwrap();
    assert!(status.required);
    assert_eq!(status.state, Some(PrefixVersionRestoreState::Failed));
    let recovery = status.last_recovery.expect("the failure is reported");
    assert_eq!(recovery.outcome, RecoveryOutcome::Failed);
    let message = recovery.message.expect("with its error");
    assert!(!message.contains("/home/"), "sanitized: {message}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unavailable_store_recovers_nothing() {
    let h = Harness::new();
    let tool = h.tool(&ToolScript::default());
    h.seed_row(&pending_row(&h.prefix, &tool));
    let store = h.recording_store();
    store.available.store(false, Ordering::SeqCst);
    let restorer = h.restorer();
    let jobs = JobRegistry::default();
    let service = h.service_with(
        jobs.clone(),
        store.clone(),
        restorer.clone(),
        Arc::new(FakeInUse::new(false, &h.prefix)),
        Arc::new(FakeProbe::winetricks(&tool)),
    );

    let results = service.recover_on_startup().await;

    assert!(results.is_empty());
    assert!(store.ops().is_empty(), "the journal is not even listed");
    assert_eq!(restorer.count(), 0);
    assert!(read_calls(&h.calls).is_empty());
    assert_eq!(h.row().map(|row| row.state).as_deref(), Some("pending"));
    assert_eq!(jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_row_waits_for_explicit_repair() {
    let h = Harness::new();
    let tool = h.tool(&ToolScript::default());
    h.seed_row(&pending_row(&h.prefix, &tool));
    h.fail_row("restore timed out");
    let restorer = h.restorer();
    let service = h.service_with(
        JobRegistry::default(),
        Arc::new(MetadataJournalStore(h.metadata.clone())),
        restorer.clone(),
        Arc::new(FakeInUse::new(false, &h.prefix)),
        Arc::new(FakeProbe::winetricks(&tool)),
    );

    let results = service.recover_on_startup().await;

    assert!(results.is_empty());
    assert_eq!(restorer.count(), 0);
    assert!(read_calls(&h.calls).is_empty());
    let row = h.row().expect("the failed row is untouched");
    assert_eq!(row.state, "failed");
    assert_eq!(row.last_error.as_deref(), Some("restore timed out"));

    // The explicit repair does retry it.
    let status = service.repair(raw_prefix(&h)).await.unwrap();
    assert!(!status.required);
    assert_eq!(read_calls(&h.calls), vec!["-q win10"]);
    assert!(h.row().is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_skips_prefix_in_use_and_leaves_it_pending() {
    let h = Harness::new();
    let tool = h.tool(&ToolScript::default());
    h.seed_row(&pending_row(&h.prefix, &tool));
    let in_use = Arc::new(FakeInUse::new(true, &h.prefix));
    let service = h.service_with(
        JobRegistry::default(),
        Arc::new(MetadataJournalStore(h.metadata.clone())),
        h.restorer(),
        in_use.clone(),
        Arc::new(FakeProbe::winetricks(&tool)),
    );

    let results = service.recover_on_startup().await;

    assert_eq!(
        results,
        vec![(h.prefix.clone(), RecoveryResult::SkippedInUse)]
    );
    assert_eq!(in_use.calls.load(Ordering::SeqCst), 1);
    assert!(read_calls(&h.calls).is_empty());
    assert_eq!(h.row().map(|row| row.state).as_deref(), Some("pending"));
    let status = service.repair_status(raw_prefix(&h)).await.unwrap();
    assert_eq!(status.last_recovery, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repair_of_clean_prefix_is_idempotent() {
    let h = Harness::new();
    let tool = h.tool(&ToolScript::default());
    let jobs = JobRegistry::default();
    let service = h.service(jobs.clone(), &tool);

    let status = service.repair(raw_prefix(&h)).await.unwrap();

    assert!(!status.required);
    assert_eq!(status.state, None);
    assert!(read_calls(&h.calls).is_empty());
    assert_eq!(jobs.job_counts(), (0, 0));
}
