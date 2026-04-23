#![cfg(test)]

use super::test_support::clean_exit_report;
use super::MetadataStore;
use crate::launch::diagnostics::models::{DiagnosticReport, ExitCodeInfo, FailureMode};
use crate::launch::request::ValidationSeverity;

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
    assert!(alpha
        .iter()
        .any(|e| e.operation_id == in_flight && e.status == "started"));
    assert_eq!(
        store
            .query_launch_history_for_profile("history-alpha", 2)
            .unwrap()
            .len(),
        2
    );

    let beta = store
        .query_launch_history_for_profile("history-beta", 10)
        .unwrap();
    assert_eq!(beta.len(), 2);
    assert!(
        !alpha.iter().any(|e| e.operation_id == other),
        "alpha history must not include beta launches"
    );

    assert!(store
        .query_launch_history_for_profile("", 10)
        .unwrap()
        .is_empty());
}
