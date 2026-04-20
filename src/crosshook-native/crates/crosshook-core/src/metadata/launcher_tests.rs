#![cfg(test)]

use rusqlite::params;

use super::test_support::{clean_exit_report, connection};
use super::{MetadataStore, MAX_DIAGNOSTIC_JSON_BYTES};
use crate::launch::diagnostics::models::{
    ActionableSuggestion, DiagnosticReport, ExitCodeInfo, FailureMode,
};
use crate::launch::request::ValidationSeverity;

#[test]
fn test_observe_launcher_exported_creates_row() {
    let store = MetadataStore::open_in_memory().unwrap();

    store
        .observe_launcher_exported(
            None,
            "test-slug",
            "Test Name",
            "/path/script.sh",
            "/path/desktop.desktop",
        )
        .unwrap();

    let conn = connection(&store);
    let (launcher_id, slug, drift_state): (String, String, String) = conn
        .query_row(
            "SELECT launcher_id, launcher_slug, drift_state FROM launchers WHERE launcher_slug = ?1",
            params!["test-slug"],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();

    assert!(!launcher_id.trim().is_empty());
    assert_eq!(slug, "test-slug");
    assert_eq!(drift_state, "aligned");
}

#[test]
fn test_observe_launcher_exported_idempotent() {
    let store = MetadataStore::open_in_memory().unwrap();

    store
        .observe_launcher_exported(
            None,
            "test-slug",
            "Test Name",
            "/path/script.sh",
            "/path/desktop.desktop",
        )
        .unwrap();
    store
        .observe_launcher_exported(
            None,
            "test-slug",
            "Test Name Updated",
            "/path/script.sh",
            "/path/desktop.desktop",
        )
        .unwrap();

    let conn = connection(&store);
    let row_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM launchers WHERE launcher_slug = ?1",
            params!["test-slug"],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(row_count, 1);
}

#[test]
fn test_observe_launcher_deleted_tombstones() {
    let store = MetadataStore::open_in_memory().unwrap();

    store
        .observe_launcher_exported(
            None,
            "test-slug",
            "Test Name",
            "/path/script.sh",
            "/path/desktop.desktop",
        )
        .unwrap();
    store.observe_launcher_deleted("test-slug").unwrap();

    let conn = connection(&store);
    let (row_count, drift_state): (i64, String) = conn
        .query_row(
            "SELECT COUNT(*), drift_state FROM launchers WHERE launcher_slug = ?1",
            params!["test-slug"],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();

    assert_eq!(row_count, 1);
    assert_eq!(drift_state, "missing");
}

#[test]
fn test_record_launch_started_returns_operation_id() {
    let store = MetadataStore::open_in_memory().unwrap();

    let operation_id = store
        .record_launch_started(Some("test-profile"), "native", None)
        .unwrap();

    assert!(!operation_id.trim().is_empty());

    let conn = connection(&store);
    let (status, started_at): (String, String) = conn
        .query_row(
            "SELECT status, started_at FROM launch_operations WHERE operation_id = ?1",
            params![operation_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();

    assert_eq!(status, "started");
    assert!(!started_at.trim().is_empty());
}

#[test]
fn test_record_launch_finished_updates_row() {
    let store = MetadataStore::open_in_memory().unwrap();
    let report = clean_exit_report();

    let operation_id = store
        .record_launch_started(Some("test-profile"), "native", None)
        .unwrap();
    store
        .record_launch_finished(&operation_id, Some(0), None, &report)
        .unwrap();

    let conn = connection(&store);
    let (status, exit_code, diagnostic_json, severity, failure_mode, finished_at): (
        String,
        Option<i32>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT status, exit_code, diagnostic_json, severity, failure_mode, finished_at
             FROM launch_operations WHERE operation_id = ?1",
            params![operation_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .unwrap();

    assert_eq!(status, "succeeded");
    assert_eq!(exit_code, Some(0));
    assert!(diagnostic_json.is_some());
    assert!(severity.is_some());
    assert!(failure_mode.is_some());
    assert!(finished_at.is_some());
}

#[test]
fn test_diagnostic_json_truncated_at_4kb() {
    let store = MetadataStore::open_in_memory().unwrap();

    // (a) Small report — diagnostic_json should be stored
    let small_report = clean_exit_report();
    let small_json_len = serde_json::to_string(&small_report).unwrap().len();
    assert!(
        small_json_len < MAX_DIAGNOSTIC_JSON_BYTES,
        "small report ({small_json_len} bytes) must be under 4KB for this test"
    );

    let op_id_small = store.record_launch_started(None, "native", None).unwrap();
    store
        .record_launch_finished(&op_id_small, Some(0), None, &small_report)
        .unwrap();

    let (diagnostic_json_small, severity_small, failure_mode_small): (
        Option<String>,
        Option<String>,
        Option<String>,
    ) = {
        let conn = connection(&store);
        conn.query_row(
            "SELECT diagnostic_json, severity, failure_mode FROM launch_operations WHERE operation_id = ?1",
            params![op_id_small],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap()
    };

    assert!(
        diagnostic_json_small.is_some(),
        "small report should have diagnostic_json stored"
    );
    assert!(severity_small.is_some());
    assert!(failure_mode_small.is_some());

    // (b) Large report — diagnostic_json should be NULL but severity/failure_mode still populated
    let large_suggestions: Vec<ActionableSuggestion> = (0..100)
        .map(|i| ActionableSuggestion {
            title: format!("Suggestion title number {i} with extra padding to push over 4KB boundary"),
            description: format!(
                "Suggestion description number {i} with a lot of extra text to ensure that the serialized JSON grows large enough to exceed the 4096-byte limit imposed by MAX_DIAGNOSTIC_JSON_BYTES"
            ),
            severity: ValidationSeverity::Warning,
        })
        .collect();

    let large_report = DiagnosticReport {
        severity: ValidationSeverity::Warning,
        summary: "Large report".to_string(),
        exit_info: ExitCodeInfo {
            code: Some(1),
            signal: None,
            signal_name: None,
            core_dumped: false,
            failure_mode: FailureMode::NonZeroExit,
            description: "Non-zero exit".to_string(),
            severity: ValidationSeverity::Warning,
        },
        pattern_matches: vec![],
        suggestions: large_suggestions,
        launch_method: "native".to_string(),
        log_tail_path: None,
        analyzed_at: "2026-01-01T00:00:00Z".to_string(),
        teardown_reason: None,
    };

    let large_json_len = serde_json::to_string(&large_report).unwrap().len();
    assert!(
        large_json_len > MAX_DIAGNOSTIC_JSON_BYTES,
        "large report ({large_json_len} bytes) must exceed 4KB for this test"
    );

    let op_id_large = store.record_launch_started(None, "native", None).unwrap();
    store
        .record_launch_finished(&op_id_large, Some(1), None, &large_report)
        .unwrap();

    let (diagnostic_json_large, severity_large, failure_mode_large): (
        Option<String>,
        Option<String>,
        Option<String>,
    ) = {
        let conn = connection(&store);
        conn.query_row(
            "SELECT diagnostic_json, severity, failure_mode FROM launch_operations WHERE operation_id = ?1",
            params![op_id_large],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap()
    };

    assert!(
        diagnostic_json_large.is_none(),
        "large report should have diagnostic_json nullified"
    );
    assert!(
        severity_large.is_some(),
        "severity should still be populated even when diagnostic_json is null"
    );
    assert!(
        failure_mode_large.is_some(),
        "failure_mode should still be populated even when diagnostic_json is null"
    );
}

#[test]
fn test_sweep_abandoned_marks_old_operations() {
    let store = MetadataStore::open_in_memory().unwrap();

    let operation_id = store.record_launch_started(None, "native", None).unwrap();

    let swept = store.sweep_abandoned_operations().unwrap();
    assert_eq!(swept, 1);

    let conn = connection(&store);
    let (status, finished_at): (String, Option<String>) = conn
        .query_row(
            "SELECT status, finished_at FROM launch_operations WHERE operation_id = ?1",
            params![operation_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();

    assert_eq!(status, "abandoned");
    assert!(finished_at.is_some());
}

#[test]
fn test_record_launch_finished_unknown_op_id_noop() {
    let store = MetadataStore::open_in_memory().unwrap();
    let report = clean_exit_report();

    let result = store.record_launch_finished("nonexistent-id", Some(0), None, &report);

    assert!(result.is_ok());
}

#[test]
fn test_observe_launcher_renamed_atomic() {
    let store = MetadataStore::open_in_memory().unwrap();

    store
        .observe_launcher_exported(
            None,
            "old-slug",
            "Old Name",
            "/path/old-script.sh",
            "/path/old.desktop",
        )
        .unwrap();

    store
        .observe_launcher_renamed(
            "old-slug",
            "new-slug",
            "New Name",
            "/path/new-script.sh",
            "/path/new.desktop",
        )
        .unwrap();

    let conn = connection(&store);

    let old_drift_state: String = conn
        .query_row(
            "SELECT drift_state FROM launchers WHERE launcher_slug = ?1",
            params!["old-slug"],
            |row| row.get(0),
        )
        .unwrap();

    let new_drift_state: String = conn
        .query_row(
            "SELECT drift_state FROM launchers WHERE launcher_slug = ?1",
            params!["new-slug"],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(old_drift_state, "missing");
    assert_eq!(new_drift_state, "aligned");
}

#[test]
fn test_phase2_disabled_store_noop() {
    let store = MetadataStore::disabled();
    let report = clean_exit_report();

    assert!(store
        .observe_launcher_exported(None, "slug", "Name", "/path/script.sh", "/path/app.desktop")
        .is_ok());
    assert!(store.observe_launcher_deleted("slug").is_ok());
    assert!(store
        .observe_launcher_renamed(
            "old",
            "new",
            "New Name",
            "/path/new.sh",
            "/path/new.desktop"
        )
        .is_ok());

    let operation_id = store.record_launch_started(None, "native", None).unwrap();
    assert!(operation_id.is_empty());

    assert!(store
        .record_launch_finished("any-id", Some(0), None, &report)
        .is_ok());

    let swept = store.sweep_abandoned_operations().unwrap();
    assert_eq!(swept, 0);
}
