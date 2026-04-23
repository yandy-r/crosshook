#![cfg(test)]

use super::test_support::{clean_exit_report, sample_profile};
use super::{LaunchHistoryEntry, MetadataStore, SyncSource};
use crate::launch::diagnostics::models::{DiagnosticReport, ExitCodeInfo, FailureMode};
use crate::launch::request::ValidationSeverity;

fn assert_launch_history_newest_first(entries: &[LaunchHistoryEntry]) {
    for w in entries.windows(2) {
        assert!(
            w[0].started_at >= w[1].started_at,
            "expected non-increasing started_at (newest first), got {:?} then {:?}",
            w[0].started_at,
            w[1].started_at
        );
    }
}

#[test]
fn test_query_most_launched() {
    let store = MetadataStore::open_in_memory().unwrap();
    let report = clean_exit_report();

    // Profile A: 3 launches
    for _ in 0..3 {
        let op_id = store
            .record_launch_started(Some("profile-a"), "native", None)
            .unwrap();
        store
            .record_launch_finished(&op_id, Some(0), None, &report)
            .unwrap();
    }

    // Profile B: 1 launch
    let op_id = store
        .record_launch_started(Some("profile-b"), "native", None)
        .unwrap();
    store
        .record_launch_finished(&op_id, Some(0), None, &report)
        .unwrap();

    // Profile C: 2 launches
    for _ in 0..2 {
        let op_id = store
            .record_launch_started(Some("profile-c"), "native", None)
            .unwrap();
        store
            .record_launch_finished(&op_id, Some(0), None, &report)
            .unwrap();
    }

    let most_launched = store.query_most_launched(10).unwrap();

    assert_eq!(most_launched.len(), 3);
    assert_eq!(most_launched[0].0, "profile-a");
    assert_eq!(most_launched[0].1, 3);
    assert_eq!(most_launched[1].0, "profile-c");
    assert_eq!(most_launched[1].1, 2);
    assert_eq!(most_launched[2].0, "profile-b");
    assert_eq!(most_launched[2].1, 1);
}

#[test]
fn test_query_failure_trends() {
    let store = MetadataStore::open_in_memory().unwrap();

    let clean_report = clean_exit_report();

    // Profile with failures: 1 success + 2 failures
    let op_ok = store
        .record_launch_started(Some("flaky-profile"), "native", None)
        .unwrap();
    store
        .record_launch_finished(&op_ok, Some(0), None, &clean_report)
        .unwrap();

    let failure_report = DiagnosticReport {
        severity: ValidationSeverity::Warning,
        summary: "Non-zero exit".to_string(),
        exit_info: ExitCodeInfo {
            code: Some(1),
            signal: None,
            signal_name: None,
            core_dumped: false,
            failure_mode: FailureMode::NonZeroExit,
            description: "Process exited with code 1".to_string(),
            severity: ValidationSeverity::Warning,
        },
        pattern_matches: vec![],
        suggestions: vec![],
        launch_method: "native".to_string(),
        log_tail_path: None,
        analyzed_at: "2026-01-01T00:00:00Z".to_string(),
        teardown_reason: None,
    };

    for _ in 0..2 {
        let op_fail = store
            .record_launch_started(Some("flaky-profile"), "native", None)
            .unwrap();
        store
            .record_launch_finished(&op_fail, Some(1), None, &failure_report)
            .unwrap();
    }

    // Profile with no failures: 2 successes only
    for _ in 0..2 {
        let op_id = store
            .record_launch_started(Some("clean-profile"), "native", None)
            .unwrap();
        store
            .record_launch_finished(&op_id, Some(0), None, &clean_report)
            .unwrap();
    }

    let trends = store.query_failure_trends(30).unwrap();

    assert_eq!(trends.len(), 1, "only profiles with failures should appear");
    assert_eq!(trends[0].profile_name, "flaky-profile");
    assert_eq!(trends[0].successes, 1);
    assert_eq!(trends[0].failures, 2);
}

#[test]
fn test_single_profile_usage_queries() {
    let store = MetadataStore::open_in_memory().unwrap();
    let clean_report = clean_exit_report();

    let ok = store
        .record_launch_started(Some("target-profile"), "native", None)
        .unwrap();
    store
        .record_launch_finished(&ok, Some(0), None, &clean_report)
        .unwrap();

    let failure_report = DiagnosticReport {
        severity: ValidationSeverity::Warning,
        summary: "Non-zero exit".to_string(),
        exit_info: ExitCodeInfo {
            code: Some(1),
            signal: None,
            signal_name: None,
            core_dumped: false,
            failure_mode: FailureMode::NonZeroExit,
            description: "Process exited with code 1".to_string(),
            severity: ValidationSeverity::Warning,
        },
        pattern_matches: vec![],
        suggestions: vec![],
        launch_method: "native".to_string(),
        log_tail_path: None,
        analyzed_at: "2026-01-01T00:00:00Z".to_string(),
        teardown_reason: None,
    };

    let failed = store
        .record_launch_started(Some("target-profile"), "native", None)
        .unwrap();
    store
        .record_launch_finished(&failed, Some(1), None, &failure_report)
        .unwrap();

    let (failures, successes) = store
        .query_failure_trend_for_profile("target-profile", 30)
        .unwrap();
    assert_eq!(failures, 1);
    assert_eq!(successes, 1);

    let last_success = store
        .query_last_success_for_profile("target-profile")
        .unwrap();
    assert!(last_success.is_some());

    let total_launches = store
        .query_total_launches_for_profile("target-profile")
        .unwrap();
    assert_eq!(total_launches, 2);
}

#[test]
fn test_query_launch_history_for_profile() {
    let store = MetadataStore::open_in_memory().unwrap();
    let report = clean_exit_report();

    for _ in 0..3 {
        let op = store
            .record_launch_started(Some("history-alpha"), "proton_run", None)
            .unwrap();
        store
            .record_launch_finished(&op, Some(0), None, &report)
            .unwrap();
    }

    let _other = store
        .record_launch_started(Some("history-beta"), "native", None)
        .unwrap();
    let other = store
        .record_launch_started(Some("history-beta"), "native", None)
        .unwrap();
    store
        .record_launch_finished(&other, Some(0), None, &report)
        .unwrap();

    let in_flight = store
        .record_launch_started(Some("history-alpha"), "native", None)
        .unwrap();

    let alpha = store
        .query_launch_history_for_profile("history-alpha", 20)
        .unwrap();
    assert_eq!(alpha.len(), 4, "3 finished + 1 in progress");
    assert_launch_history_newest_first(&alpha);
    assert!(alpha
        .iter()
        .any(|e| e.operation_id == in_flight && e.status == "started"));

    let alpha_limited = store
        .query_launch_history_for_profile("history-alpha", 2)
        .unwrap();
    assert_eq!(alpha_limited.len(), 2);
    assert_launch_history_newest_first(&alpha_limited);

    let beta = store
        .query_launch_history_for_profile("history-beta", 10)
        .unwrap();
    assert_eq!(beta.len(), 2);
    assert_launch_history_newest_first(&beta);
    assert!(
        !alpha.iter().any(|e| e.operation_id == other),
        "alpha history must not include beta launches"
    );

    assert!(store
        .query_launch_history_for_profile("", 10)
        .unwrap()
        .is_empty());
}

#[test]
fn test_query_launch_history_survives_profile_rename() {
    let store = MetadataStore::open_in_memory().unwrap();
    let profile = sample_profile();
    let old_path = std::path::Path::new("/profiles/rename-launch-old.toml");
    let new_path = std::path::Path::new("/profiles/rename-launch-new.toml");

    store
        .observe_profile_write(
            "rename-launch-old",
            &profile,
            old_path,
            SyncSource::AppWrite,
            None,
        )
        .unwrap();

    let report = clean_exit_report();
    let op = store
        .record_launch_started(Some("rename-launch-old"), "native", None)
        .unwrap();
    store
        .record_launch_finished(&op, Some(0), None, &report)
        .unwrap();

    store
        .observe_profile_rename("rename-launch-old", "rename-launch-new", old_path, new_path)
        .unwrap();

    let history = store
        .query_launch_history_for_profile("rename-launch-new", 10)
        .unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].operation_id, op);
    assert_eq!(history[0].status, "succeeded");
}
