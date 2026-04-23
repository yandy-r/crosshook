//! Usage-insights queries over `launch_operations` and the `launchers` / `profiles`
//! tables. Free functions so they stay testable without a `MetadataStore`; thin
//! delegating methods on `MetadataStore` at the bottom preserve the public API.

use rusqlite::{params_from_iter, Connection, OptionalExtension};

use super::profile_sync::lookup_profile_id;
use super::util::in_clause_placeholders;
use super::{FailureTrendRow, MetadataStore, MetadataStoreError};
use crate::metadata::models::{DriftState, LaunchHistoryEntry};

pub(super) fn query_most_launched(
    conn: &Connection,
    limit: usize,
) -> Result<Vec<(String, i64)>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT profile_name, COUNT(*) as launch_count \
             FROM launch_operations \
             WHERE status IN ('succeeded', 'failed') AND profile_name IS NOT NULL \
             GROUP BY profile_name \
             ORDER BY launch_count DESC \
             LIMIT ?1",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare query_most_launched statement",
            source,
        })?;

    let rows = stmt
        .query_map(rusqlite::params![limit as i64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|source| MetadataStoreError::Database {
            action: "execute query_most_launched",
            source,
        })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|source| MetadataStoreError::Database {
            action: "read a query_most_launched row",
            source,
        })?);
    }
    Ok(result)
}

pub(super) fn query_total_launches_for_profiles(
    conn: &Connection,
    profile_names: &[String],
) -> Result<Vec<(String, i64)>, MetadataStoreError> {
    let placeholders = in_clause_placeholders(profile_names.len());
    let sql = format!(
        "SELECT profile_name, COUNT(*) as launch_count \
         FROM launch_operations \
         WHERE status IN ('succeeded', 'failed') \
           AND profile_name IS NOT NULL \
           AND profile_name IN ({placeholders}) \
         GROUP BY profile_name"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare query_total_launches_for_profiles statement",
            source,
        })?;
    let rows = stmt
        .query_map(params_from_iter(profile_names.iter()), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|source| MetadataStoreError::Database {
            action: "execute query_total_launches_for_profiles",
            source,
        })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|source| MetadataStoreError::Database {
            action: "read a query_total_launches_for_profiles row",
            source,
        })?);
    }
    Ok(result)
}

pub(super) fn query_last_success_per_profile(
    conn: &Connection,
) -> Result<Vec<(String, String)>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT profile_name, MAX(finished_at) as last_success \
             FROM launch_operations \
             WHERE status = 'succeeded' AND profile_name IS NOT NULL \
             GROUP BY profile_name",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare query_last_success_per_profile statement",
            source,
        })?;

    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|source| MetadataStoreError::Database {
            action: "execute query_last_success_per_profile",
            source,
        })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|source| MetadataStoreError::Database {
            action: "read a query_last_success_per_profile row",
            source,
        })?);
    }
    Ok(result)
}

pub(super) fn query_failure_trend_for_profile(
    conn: &Connection,
    profile_name: &str,
    days: u32,
) -> Result<(i64, i64), MetadataStoreError> {
    let interval = format!("-{days} days");
    conn.query_row(
        "SELECT COUNT(*) FILTER (WHERE status = 'failed') as failures, \
                COUNT(*) FILTER (WHERE status = 'succeeded') as successes \
         FROM launch_operations \
         WHERE started_at >= datetime('now', ?1) AND profile_name = ?2",
        rusqlite::params![&interval, profile_name],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "query failure trend for profile",
        source,
    })
}

pub(super) fn query_last_success_for_profile(
    conn: &Connection,
    profile_name: &str,
) -> Result<Option<String>, MetadataStoreError> {
    conn.query_row(
        "SELECT MAX(finished_at) \
         FROM launch_operations \
         WHERE status = 'succeeded' AND profile_name = ?1",
        rusqlite::params![profile_name],
        |row| row.get::<_, Option<String>>(0),
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "query last success for profile",
        source,
    })
}

pub(super) fn query_total_launches_for_profile(
    conn: &Connection,
    profile_name: &str,
) -> Result<i64, MetadataStoreError> {
    conn.query_row(
        "SELECT COUNT(*) \
         FROM launch_operations \
         WHERE status IN ('succeeded', 'failed') AND profile_name = ?1",
        rusqlite::params![profile_name],
        |row| row.get::<_, i64>(0),
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "query total launches for profile",
        source,
    })
}

pub(super) fn query_launcher_drift_for_profile(
    conn: &Connection,
    profile_id: &str,
) -> Result<Option<DriftState>, MetadataStoreError> {
    let result = conn
        .query_row(
            "SELECT drift_state FROM launchers \
             WHERE profile_id = ?1 \
             ORDER BY updated_at DESC LIMIT 1",
            rusqlite::params![profile_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|source| MetadataStoreError::Database {
            action: "query launcher drift for profile",
            source,
        })?;

    let drift = result.map(|s| match s.as_str() {
        "aligned" => DriftState::Aligned,
        "missing" => DriftState::Missing,
        "moved" => DriftState::Moved,
        "stale" => DriftState::Stale,
        _ => DriftState::Unknown,
    });
    Ok(drift)
}

pub(super) fn query_profile_source(
    conn: &Connection,
    name: &str,
) -> Result<Option<String>, MetadataStoreError> {
    conn.query_row(
        "SELECT source FROM profiles WHERE current_filename = ?1 AND deleted_at IS NULL",
        rusqlite::params![name],
        |row| row.get::<_, Option<String>>(0),
    )
    .optional()
    .map(std::option::Option::flatten)
    .map_err(|source| MetadataStoreError::Database {
        action: "query profile source",
        source,
    })
}

pub(super) fn query_profile_sources_for_names(
    conn: &Connection,
    profile_names: &[String],
) -> Result<Vec<(String, Option<String>)>, MetadataStoreError> {
    let placeholders = in_clause_placeholders(profile_names.len());
    let sql = format!(
        "SELECT current_filename, source \
         FROM profiles \
         WHERE deleted_at IS NULL AND current_filename IN ({placeholders})"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare query_profile_sources_for_names statement",
            source,
        })?;
    let rows = stmt
        .query_map(params_from_iter(profile_names.iter()), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .map_err(|source| MetadataStoreError::Database {
            action: "execute query_profile_sources_for_names",
            source,
        })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|source| MetadataStoreError::Database {
            action: "read a query_profile_sources_for_names row",
            source,
        })?);
    }
    Ok(result)
}

pub(super) fn query_launcher_drift_for_profile_ids(
    conn: &Connection,
    profile_ids: &[String],
) -> Result<Vec<(String, DriftState)>, MetadataStoreError> {
    let placeholders = in_clause_placeholders(profile_ids.len());
    let sql = format!(
        "SELECT l.profile_id, l.drift_state \
         FROM launchers l \
         INNER JOIN ( \
           SELECT profile_id, MAX(updated_at) AS max_updated_at \
           FROM launchers \
           WHERE profile_id IN ({placeholders}) \
           GROUP BY profile_id \
         ) latest \
           ON latest.profile_id = l.profile_id AND latest.max_updated_at = l.updated_at"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare query_launcher_drift_for_profile_ids statement",
            source,
        })?;
    let rows = stmt
        .query_map(params_from_iter(profile_ids.iter()), |row| {
            let profile_id = row.get::<_, String>(0)?;
            let drift_state = row.get::<_, String>(1)?;
            let drift = match drift_state.as_str() {
                "aligned" => DriftState::Aligned,
                "missing" => DriftState::Missing,
                "moved" => DriftState::Moved,
                "stale" => DriftState::Stale,
                _ => DriftState::Unknown,
            };
            Ok((profile_id, drift))
        })
        .map_err(|source| MetadataStoreError::Database {
            action: "execute query_launcher_drift_for_profile_ids",
            source,
        })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|source| MetadataStoreError::Database {
            action: "read a query_launcher_drift_for_profile_ids row",
            source,
        })?);
    }
    Ok(result)
}

pub(super) fn query_failure_trends(
    conn: &Connection,
    days: u32,
) -> Result<Vec<FailureTrendRow>, MetadataStoreError> {
    let interval = format!("-{days} days");
    let mut stmt = conn
        .prepare(
            "SELECT profile_name, \
                    COUNT(*) FILTER (WHERE status = 'succeeded') as successes, \
                    COUNT(*) FILTER (WHERE status = 'failed') as failures, \
                    GROUP_CONCAT(DISTINCT failure_mode) as failure_modes \
             FROM launch_operations \
             WHERE started_at >= datetime('now', ?1) AND profile_name IS NOT NULL \
             GROUP BY profile_name \
             HAVING failures > 0 \
             ORDER BY failures DESC",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare query_failure_trends statement",
            source,
        })?;

    let rows = stmt
        .query_map(rusqlite::params![&interval], |row| {
            Ok(FailureTrendRow {
                profile_name: row.get(0)?,
                successes: row.get(1)?,
                failures: row.get(2)?,
                failure_modes: row.get(3)?,
            })
        })
        .map_err(|source| MetadataStoreError::Database {
            action: "execute query_failure_trends",
            source,
        })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|source| MetadataStoreError::Database {
            action: "read a query_failure_trends row",
            source,
        })?);
    }
    Ok(result)
}

pub(super) fn query_launch_history_for_profile(
    conn: &Connection,
    profile_name: &str,
    limit: usize,
) -> Result<Vec<LaunchHistoryEntry>, MetadataStoreError> {
    if profile_name.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }

    let cap = i64::try_from(limit).unwrap_or(i64::MAX);
    let profile_id = lookup_profile_id(conn, profile_name)?;

    let mut result = Vec::new();

    if let Some(pid) = profile_id {
        let mut stmt = conn
            .prepare(
                "SELECT operation_id, launch_method, status, started_at, finished_at, \
                        exit_code, signal, severity, failure_mode \
                 FROM launch_operations \
                 WHERE profile_id = ?1 \
                    OR (profile_id IS NULL AND profile_name = ?2) \
                 ORDER BY started_at DESC \
                 LIMIT ?3",
            )
            .map_err(|source| MetadataStoreError::Database {
                action: "prepare query_launch_history_for_profile statement",
                source,
            })?;

        let rows = stmt
            .query_map(rusqlite::params![pid, profile_name, cap], |row| {
                Ok(LaunchHistoryEntry {
                    operation_id: row.get(0)?,
                    launch_method: row.get(1)?,
                    status: row.get(2)?,
                    started_at: row.get(3)?,
                    finished_at: row.get(4)?,
                    exit_code: row.get(5)?,
                    signal: row.get(6)?,
                    severity: row.get(7)?,
                    failure_mode: row.get(8)?,
                })
            })
            .map_err(|source| MetadataStoreError::Database {
                action: "execute query_launch_history_for_profile",
                source,
            })?;

        for row in rows {
            result.push(row.map_err(|source| MetadataStoreError::Database {
                action: "read a query_launch_history_for_profile row",
                source,
            })?);
        }
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT operation_id, launch_method, status, started_at, finished_at, \
                        exit_code, signal, severity, failure_mode \
                 FROM launch_operations \
                 WHERE profile_name = ?1 \
                 ORDER BY started_at DESC \
                 LIMIT ?2",
            )
            .map_err(|source| MetadataStoreError::Database {
                action: "prepare query_launch_history_for_profile statement",
                source,
            })?;

        let rows = stmt
            .query_map(rusqlite::params![profile_name, cap], |row| {
                Ok(LaunchHistoryEntry {
                    operation_id: row.get(0)?,
                    launch_method: row.get(1)?,
                    status: row.get(2)?,
                    started_at: row.get(3)?,
                    finished_at: row.get(4)?,
                    exit_code: row.get(5)?,
                    signal: row.get(6)?,
                    severity: row.get(7)?,
                    failure_mode: row.get(8)?,
                })
            })
            .map_err(|source| MetadataStoreError::Database {
                action: "execute query_launch_history_for_profile",
                source,
            })?;

        for row in rows {
            result.push(row.map_err(|source| MetadataStoreError::Database {
                action: "read a query_launch_history_for_profile row",
                source,
            })?);
        }
    }

    Ok(result)
}

impl MetadataStore {
    pub fn query_most_launched(
        &self,
        limit: usize,
    ) -> Result<Vec<(String, i64)>, MetadataStoreError> {
        self.with_conn("query most launched profiles", |conn| {
            query_most_launched(conn, limit)
        })
    }

    pub fn query_total_launches_for_profiles(
        &self,
        profile_names: &[String],
    ) -> Result<Vec<(String, i64)>, MetadataStoreError> {
        if profile_names.is_empty() {
            return Ok(Vec::new());
        }
        self.with_conn("query total launches for profiles", |conn| {
            query_total_launches_for_profiles(conn, profile_names)
        })
    }

    pub fn query_last_success_per_profile(
        &self,
    ) -> Result<Vec<(String, String)>, MetadataStoreError> {
        self.with_conn(
            "query last success per profile",
            query_last_success_per_profile,
        )
    }

    /// Returns `(failures, successes)` for a single profile over the last `days`.
    /// Missing rows are represented as zero counts.
    pub fn query_failure_trend_for_profile(
        &self,
        profile_name: &str,
        days: u32,
    ) -> Result<(i64, i64), MetadataStoreError> {
        self.with_conn("query failure trend for profile", |conn| {
            query_failure_trend_for_profile(conn, profile_name, days)
        })
    }

    /// Returns the latest succeeded launch timestamp for a single profile.
    pub fn query_last_success_for_profile(
        &self,
        profile_name: &str,
    ) -> Result<Option<String>, MetadataStoreError> {
        self.with_conn("query last success for profile", |conn| {
            query_last_success_for_profile(conn, profile_name)
        })
    }

    /// Returns the total launches recorded for a single profile.
    pub fn query_total_launches_for_profile(
        &self,
        profile_name: &str,
    ) -> Result<i64, MetadataStoreError> {
        self.with_conn("query total launches for profile", |conn| {
            query_total_launches_for_profile(conn, profile_name)
        })
    }

    /// Returns the most recent launcher drift state for a given profile_id,
    /// or `None` if no launcher rows exist for that profile.
    pub fn query_launcher_drift_for_profile(
        &self,
        profile_id: &str,
    ) -> Result<Option<DriftState>, MetadataStoreError> {
        self.with_conn("query launcher drift for profile", |conn| {
            query_launcher_drift_for_profile(conn, profile_id)
        })
    }

    /// Returns the `source` field from the profiles table for a given profile name,
    /// or `None` if the profile is not found in the metadata store.
    pub fn query_profile_source(&self, name: &str) -> Result<Option<String>, MetadataStoreError> {
        self.with_conn("query profile source", |conn| {
            query_profile_source(conn, name)
        })
    }

    pub fn query_profile_sources_for_names(
        &self,
        profile_names: &[String],
    ) -> Result<Vec<(String, Option<String>)>, MetadataStoreError> {
        if profile_names.is_empty() {
            return Ok(Vec::new());
        }
        self.with_conn("query profile sources for names", |conn| {
            query_profile_sources_for_names(conn, profile_names)
        })
    }

    pub fn query_launcher_drift_for_profile_ids(
        &self,
        profile_ids: &[String],
    ) -> Result<Vec<(String, DriftState)>, MetadataStoreError> {
        if profile_ids.is_empty() {
            return Ok(Vec::new());
        }
        self.with_conn("query launcher drift for profile ids", |conn| {
            query_launcher_drift_for_profile_ids(conn, profile_ids)
        })
    }

    pub fn query_failure_trends(
        &self,
        days: u32,
    ) -> Result<Vec<FailureTrendRow>, MetadataStoreError> {
        self.with_conn("query failure trends", |conn| {
            query_failure_trends(conn, days)
        })
    }

    /// Recent launch rows for a profile, newest first (by `started_at`). Omits `diagnostic_json`.
    pub fn query_launch_history_for_profile(
        &self,
        profile_name: &str,
        limit: usize,
    ) -> Result<Vec<LaunchHistoryEntry>, MetadataStoreError> {
        if profile_name.is_empty() {
            return Ok(Vec::new());
        }
        self.with_conn("query launch history for profile", |conn| {
            query_launch_history_for_profile(conn, profile_name, limit)
        })
    }
}
