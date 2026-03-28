use super::{db, MetadataStoreError};
use super::models::{LaunchOutcome, MAX_DIAGNOSTIC_JSON_BYTES};
use super::profile_sync::lookup_profile_id;
use crate::launch::diagnostics::models::{DiagnosticReport, FailureMode};
use chrono::Utc;
use rusqlite::{params, Connection};

pub fn record_launch_started(
    conn: &Connection,
    profile_name: Option<&str>,
    method: &str,
    log_path: Option<&str>,
) -> Result<String, MetadataStoreError> {
    let operation_id = db::new_id();
    let now = Utc::now().to_rfc3339();

    let profile_id = match profile_name {
        Some(name) => lookup_profile_id(conn, name)?,
        None => None,
    };

    conn.execute(
        "INSERT INTO launch_operations (
            operation_id,
            profile_id,
            profile_name,
            launch_method,
            status,
            exit_code,
            signal,
            log_path,
            diagnostic_json,
            severity,
            failure_mode,
            started_at,
            finished_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, ?6, NULL, NULL, NULL, ?7, NULL)",
        params![
            operation_id,
            profile_id,
            profile_name,
            method,
            LaunchOutcome::Started.as_str(),
            log_path,
            now,
        ],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "insert a launch operation row",
        source,
    })?;

    Ok(operation_id)
}

pub fn record_launch_finished(
    conn: &Connection,
    operation_id: &str,
    exit_code: Option<i32>,
    signal: Option<i32>,
    report: &DiagnosticReport,
) -> Result<(), MetadataStoreError> {
    let now = Utc::now().to_rfc3339();

    // Serialize the full report; nullify if over 4KB (malformed partial JSON is worse than NULL)
    let json = serde_json::to_string(report).ok();
    let json = json.filter(|s| s.len() <= MAX_DIAGNOSTIC_JSON_BYTES);

    // Determine outcome from failure_mode
    let outcome = match report.exit_info.failure_mode {
        FailureMode::CleanExit => LaunchOutcome::Succeeded,
        _ => LaunchOutcome::Failed,
    };

    // Extract promoted columns regardless of JSON truncation
    let severity = serde_json::to_value(&report.severity)
        .ok()
        .and_then(|v| v.as_str().map(|s| s.to_owned()));

    let failure_mode = serde_json::to_value(&report.exit_info.failure_mode)
        .ok()
        .and_then(|v| v.as_str().map(|s| s.to_owned()));

    let rows_affected = conn
        .execute(
            "UPDATE launch_operations
             SET status = ?1,
                 exit_code = ?2,
                 signal = ?3,
                 diagnostic_json = ?4,
                 severity = ?5,
                 failure_mode = ?6,
                 finished_at = ?7
             WHERE operation_id = ?8",
            params![
                outcome.as_str(),
                exit_code,
                signal,
                json,
                severity,
                failure_mode,
                now,
                operation_id,
            ],
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "update the finished launch operation",
            source,
        })?;

    if rows_affected == 0 {
        tracing::warn!(
            operation_id = %operation_id,
            "record_launch_finished found no matching operation row — skipping update"
        );
    }

    Ok(())
}

pub fn sweep_abandoned_operations(conn: &Connection) -> Result<usize, MetadataStoreError> {
    let now = Utc::now().to_rfc3339();

    let rows_affected = conn
        .execute(
            "UPDATE launch_operations
             SET status = 'abandoned', finished_at = ?1
             WHERE status = 'started' AND finished_at IS NULL",
            params![now],
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "sweep abandoned launch operations",
            source,
        })?;

    Ok(rows_affected)
}
