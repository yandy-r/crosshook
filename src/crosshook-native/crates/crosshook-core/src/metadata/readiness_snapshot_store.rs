use rusqlite::{Connection, OptionalExtension};

use crate::metadata::MetadataStoreError;
use crate::onboarding::HostToolCheckResult;

/// Cached last host readiness snapshot row (single logical row `id = 1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostReadinessSnapshotRow {
    pub detected_distro_family: String,
    pub tool_results_json: String,
    pub all_passed: bool,
    pub critical_failures: i64,
    pub warnings: i64,
    pub checked_at: String,
}

pub fn upsert_host_readiness_snapshot_impl(
    conn: &mut Connection,
    tool_checks: &[HostToolCheckResult],
    detected_distro_family: &str,
    all_passed: bool,
    critical_failures: usize,
    warnings: usize,
) -> Result<(), MetadataStoreError> {
    let tool_json = serde_json::to_string(tool_checks).map_err(|e| {
        MetadataStoreError::Validation(format!("serialize host readiness tool_checks: {e}"))
    })?;
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT OR REPLACE INTO host_readiness_snapshots (
            id, detected_distro_family, tool_results_json,
            all_passed, critical_failures, warnings, checked_at
        ) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            detected_distro_family,
            tool_json,
            all_passed as i64,
            critical_failures as i64,
            warnings as i64,
            now,
        ],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "upsert host readiness snapshot",
        source,
    })?;

    Ok(())
}

pub fn get_host_readiness_snapshot_impl(
    conn: &Connection,
) -> Result<Option<HostReadinessSnapshotRow>, MetadataStoreError> {
    let row = conn
        .query_row(
            "SELECT detected_distro_family, tool_results_json, all_passed, critical_failures, warnings, checked_at
             FROM host_readiness_snapshots WHERE id = 1",
            [],
            |row| {
                Ok(HostReadinessSnapshotRow {
                    detected_distro_family: row.get(0)?,
                    tool_results_json: row.get(1)?,
                    all_passed: row.get::<_, i64>(2)? != 0,
                    critical_failures: row.get(3)?,
                    warnings: row.get(4)?,
                    checked_at: row.get(5)?,
                })
            },
        )
        .optional()
        .map_err(|source| MetadataStoreError::Database {
            action: "query host readiness snapshot",
            source,
        })?;
    Ok(row)
}
