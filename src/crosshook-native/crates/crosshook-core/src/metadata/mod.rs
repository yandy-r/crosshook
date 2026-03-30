mod cache_store;
mod collections;
mod community_index;
mod config_history_store;
mod db;
mod health_store;
mod launch_history;
mod launcher_sync;
mod migrations;
mod models;
mod preset_store;
pub mod profile_sync;
mod version_store;

pub use health_store::HealthSnapshotRow;
pub use models::{
    BundledOptimizationPresetRow, CacheEntryStatus, CollectionRow, CommunityProfileRow,
    CommunityTapRow, ConfigRevisionRow, ConfigRevisionSource, DriftState, FailureTrendRow,
    LaunchOutcome, MetadataStoreError, ProfileLaunchPresetOrigin, SyncReport, SyncSource,
    VersionCorrelationStatus, VersionSnapshotRow, MAX_CACHE_PAYLOAD_BYTES,
    MAX_CONFIG_REVISIONS_PER_PROFILE, MAX_DIAGNOSTIC_JSON_BYTES, MAX_HISTORY_LIST_LIMIT,
    MAX_SNAPSHOT_TOML_BYTES, MAX_VERSION_SNAPSHOTS_PER_PROFILE,
};
pub use version_store::{compute_correlation_status, hash_trainer_file};

use crate::community::taps::CommunityTapSyncResult;
use crate::launch::diagnostics::models::DiagnosticReport;
use crate::profile::{GameProfile, ProfileStore};
use chrono::Utc;
use directories::BaseDirs;
use rusqlite::{params_from_iter, Connection, OptionalExtension};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct MetadataStore {
    conn: Option<Arc<Mutex<Connection>>>,
    available: bool,
}

impl MetadataStore {
    fn in_clause_placeholders(count: usize) -> String {
        std::iter::repeat_n("?", count)
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn try_new() -> Result<Self, String> {
        let path = BaseDirs::new()
            .ok_or("home directory not found — CrossHook requires a user home directory")?
            .data_local_dir()
            .join("crosshook/metadata.db");
        Self::open(&path).map_err(|error| error.to_string())
    }

    pub fn with_path(path: &Path) -> Result<Self, MetadataStoreError> {
        Self::open(path)
    }

    pub fn open_in_memory() -> Result<Self, MetadataStoreError> {
        Self::open_with_connection(db::open_in_memory()?)
    }

    pub fn disabled() -> Self {
        Self {
            conn: None,
            available: false,
        }
    }

    pub fn is_available(&self) -> bool {
        self.available && self.conn.is_some()
    }

    fn open(path: &Path) -> Result<Self, MetadataStoreError> {
        Self::open_with_connection(db::open_at_path(path)?)
    }

    fn open_with_connection(conn: Connection) -> Result<Self, MetadataStoreError> {
        migrations::run_migrations(&conn)?;
        Ok(Self {
            conn: Some(Arc::new(Mutex::new(conn))),
            available: true,
        })
    }

    fn with_conn<F, T>(&self, action: &'static str, f: F) -> Result<T, MetadataStoreError>
    where
        F: FnOnce(&Connection) -> Result<T, MetadataStoreError>,
        T: Default,
    {
        if !self.available {
            return Ok(T::default());
        }

        let Some(conn) = &self.conn else {
            return Ok(T::default());
        };

        let guard = conn.lock().map_err(|_| {
            MetadataStoreError::Corrupt(format!("metadata store mutex poisoned while {action}"))
        })?;
        f(&guard)
    }

    fn with_conn_mut<F, T>(&self, action: &'static str, f: F) -> Result<T, MetadataStoreError>
    where
        F: FnOnce(&mut Connection) -> Result<T, MetadataStoreError>,
        T: Default,
    {
        if !self.available {
            return Ok(T::default());
        }

        let Some(conn) = &self.conn else {
            return Ok(T::default());
        };

        let mut guard = conn.lock().map_err(|_| {
            MetadataStoreError::Corrupt(format!("metadata store mutex poisoned while {action}"))
        })?;
        f(&mut guard)
    }

    pub fn observe_profile_write(
        &self,
        name: &str,
        profile: &GameProfile,
        path: &Path,
        source: SyncSource,
        source_profile_id: Option<&str>,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("observe a profile write", |conn| {
            profile_sync::observe_profile_write(
                conn,
                name,
                profile,
                path,
                source,
                source_profile_id,
            )
        })
    }

    pub fn lookup_profile_id(&self, name: &str) -> Result<Option<String>, MetadataStoreError> {
        self.with_conn("look up a profile id", |conn| {
            profile_sync::lookup_profile_id(conn, name)
        })
    }

    pub fn query_profile_ids_for_names(
        &self,
        profile_names: &[String],
    ) -> Result<Vec<(String, String)>, MetadataStoreError> {
        if profile_names.is_empty() {
            return Ok(Vec::new());
        }

        self.with_conn("query profile ids for names", |conn| {
            let placeholders = Self::in_clause_placeholders(profile_names.len());
            let sql = format!(
                "SELECT current_filename, profile_id \
                 FROM profiles \
                 WHERE deleted_at IS NULL AND current_filename IN ({placeholders})"
            );
            let mut stmt = conn
                .prepare(&sql)
                .map_err(|source| MetadataStoreError::Database {
                    action: "prepare query_profile_ids_for_names statement",
                    source,
                })?;
            let rows = stmt
                .query_map(params_from_iter(profile_names.iter()), |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|source| MetadataStoreError::Database {
                    action: "execute query_profile_ids_for_names",
                    source,
                })?;

            let mut result = Vec::new();
            for row in rows {
                result.push(row.map_err(|source| MetadataStoreError::Database {
                    action: "read a query_profile_ids_for_names row",
                    source,
                })?);
            }
            Ok(result)
        })
    }

    pub fn observe_profile_rename(
        &self,
        old_name: &str,
        new_name: &str,
        old_path: &Path,
        new_path: &Path,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("observe a profile rename", |conn| {
            profile_sync::observe_profile_rename(conn, old_name, new_name, old_path, new_path)
        })
    }

    pub fn observe_profile_delete(&self, name: &str) -> Result<(), MetadataStoreError> {
        self.with_conn("observe a profile delete", |conn| {
            profile_sync::observe_profile_delete(conn, name)
        })
    }

    pub fn sync_profiles_from_store(
        &self,
        store: &ProfileStore,
    ) -> Result<SyncReport, MetadataStoreError> {
        self.with_conn("sync profiles from store", |conn| {
            profile_sync::sync_profiles_from_store(conn, store)
        })
    }

    pub fn observe_launcher_exported(
        &self,
        profile_name: Option<&str>,
        slug: &str,
        display_name: &str,
        script_path: &str,
        desktop_entry_path: &str,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("observe a launcher export", |conn| {
            launcher_sync::observe_launcher_exported(
                conn,
                profile_name,
                slug,
                display_name,
                script_path,
                desktop_entry_path,
            )
        })
    }

    pub fn observe_launcher_deleted(&self, launcher_slug: &str) -> Result<(), MetadataStoreError> {
        self.with_conn("observe a launcher deletion", |conn| {
            launcher_sync::observe_launcher_deleted(conn, launcher_slug)
        })
    }

    pub fn observe_launcher_renamed(
        &self,
        old_slug: &str,
        new_slug: &str,
        new_display_name: &str,
        new_script_path: &str,
        new_desktop_entry_path: &str,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn_mut("observe a launcher rename", |conn| {
            launcher_sync::observe_launcher_renamed(
                conn,
                old_slug,
                new_slug,
                new_display_name,
                new_script_path,
                new_desktop_entry_path,
            )
        })
    }

    pub fn record_launch_started(
        &self,
        profile_name: Option<&str>,
        method: &str,
        log_path: Option<&str>,
    ) -> Result<String, MetadataStoreError> {
        self.with_conn("record a launch start", |conn| {
            launch_history::record_launch_started(conn, profile_name, method, log_path)
        })
    }

    pub fn record_launch_finished(
        &self,
        operation_id: &str,
        exit_code: Option<i32>,
        signal: Option<i32>,
        report: &DiagnosticReport,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("record a launch finish", |conn| {
            launch_history::record_launch_finished(conn, operation_id, exit_code, signal, report)
        })
    }

    pub fn sweep_abandoned_operations(&self) -> Result<usize, MetadataStoreError> {
        self.with_conn("sweep abandoned operations", |conn| {
            launch_history::sweep_abandoned_operations(conn)
        })
    }

    // -------------------------------------------------------------------------
    // Phase 3: Community index
    // -------------------------------------------------------------------------

    pub fn index_community_tap_result(
        &self,
        result: &CommunityTapSyncResult,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn_mut("index a community tap", |conn| {
            community_index::index_community_tap_result(conn, result)
        })
    }

    pub fn list_community_tap_profiles(
        &self,
        tap_url: Option<&str>,
    ) -> Result<Vec<CommunityProfileRow>, MetadataStoreError> {
        self.with_conn("list community tap profiles", |conn| {
            community_index::list_community_tap_profiles(conn, tap_url)
        })
    }

    // -------------------------------------------------------------------------
    // Phase 3: Collections
    // -------------------------------------------------------------------------

    pub fn list_collections(&self) -> Result<Vec<CollectionRow>, MetadataStoreError> {
        self.with_conn("list collections", |conn| {
            collections::list_collections(conn)
        })
    }

    pub fn create_collection(&self, name: &str) -> Result<String, MetadataStoreError> {
        self.with_conn("create a collection", |conn| {
            collections::create_collection(conn, name)
        })
    }

    pub fn delete_collection(&self, collection_id: &str) -> Result<(), MetadataStoreError> {
        self.with_conn("delete a collection", |conn| {
            collections::delete_collection(conn, collection_id)
        })
    }

    pub fn add_profile_to_collection(
        &self,
        collection_id: &str,
        profile_name: &str,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("add a profile to a collection", |conn| {
            collections::add_profile_to_collection(conn, collection_id, profile_name)
        })
    }

    pub fn remove_profile_from_collection(
        &self,
        collection_id: &str,
        profile_name: &str,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("remove a profile from a collection", |conn| {
            collections::remove_profile_from_collection(conn, collection_id, profile_name)
        })
    }

    pub fn list_profiles_in_collection(
        &self,
        collection_id: &str,
    ) -> Result<Vec<String>, MetadataStoreError> {
        self.with_conn("list profiles in a collection", |conn| {
            collections::list_profiles_in_collection(conn, collection_id)
        })
    }

    // -------------------------------------------------------------------------
    // Phase 3: Favorites
    // -------------------------------------------------------------------------

    pub fn set_profile_favorite(
        &self,
        profile_name: &str,
        favorite: bool,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("set a profile favorite", |conn| {
            collections::set_profile_favorite(conn, profile_name, favorite)
        })
    }

    pub fn list_favorite_profiles(&self) -> Result<Vec<String>, MetadataStoreError> {
        self.with_conn("list favorite profiles", |conn| {
            collections::list_favorite_profiles(conn)
        })
    }

    // -------------------------------------------------------------------------
    // Phase 3: Cache
    // -------------------------------------------------------------------------

    pub fn get_cache_entry(&self, cache_key: &str) -> Result<Option<String>, MetadataStoreError> {
        self.with_conn("get a cache entry", |conn| {
            cache_store::get_cache_entry(conn, cache_key)
        })
    }

    pub fn put_cache_entry(
        &self,
        source_url: &str,
        cache_key: &str,
        payload: &str,
        expires_at: Option<&str>,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("put a cache entry", |conn| {
            cache_store::put_cache_entry(conn, source_url, cache_key, payload, expires_at)
        })
    }

    pub fn evict_expired_cache_entries(&self) -> Result<usize, MetadataStoreError> {
        self.with_conn("evict expired cache entries", |conn| {
            cache_store::evict_expired_cache_entries(conn)
        })
    }

    // -------------------------------------------------------------------------
    // Phase 3: Usage insights (inline SQL over launch_operations)
    // -------------------------------------------------------------------------

    pub fn query_most_launched(
        &self,
        limit: usize,
    ) -> Result<Vec<(String, i64)>, MetadataStoreError> {
        self.with_conn("query most launched profiles", |conn| {
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
            let placeholders = Self::in_clause_placeholders(profile_names.len());
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
        })
    }

    pub fn query_last_success_per_profile(
        &self,
    ) -> Result<Vec<(String, String)>, MetadataStoreError> {
        self.with_conn("query last success per profile", |conn| {
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
        })
    }

    /// Returns `(failures, successes)` for a single profile over the last `days`.
    /// Missing rows are represented as zero counts.
    pub fn query_failure_trend_for_profile(
        &self,
        profile_name: &str,
        days: u32,
    ) -> Result<(i64, i64), MetadataStoreError> {
        self.with_conn("query failure trend for profile", |conn| {
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
        })
    }

    /// Returns the latest succeeded launch timestamp for a single profile.
    pub fn query_last_success_for_profile(
        &self,
        profile_name: &str,
    ) -> Result<Option<String>, MetadataStoreError> {
        self.with_conn("query last success for profile", |conn| {
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
        })
    }

    /// Returns the total launches recorded for a single profile.
    pub fn query_total_launches_for_profile(
        &self,
        profile_name: &str,
    ) -> Result<i64, MetadataStoreError> {
        self.with_conn("query total launches for profile", |conn| {
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
        })
    }

    /// Returns the most recent launcher drift state for a given profile_id,
    /// or `None` if no launcher rows exist for that profile.
    pub fn query_launcher_drift_for_profile(
        &self,
        profile_id: &str,
    ) -> Result<Option<DriftState>, MetadataStoreError> {
        self.with_conn("query launcher drift for profile", |conn| {
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
        })
    }

    /// Returns the `source` field from the profiles table for a given profile name,
    /// or `None` if the profile is not found in the metadata store.
    pub fn query_profile_source(&self, name: &str) -> Result<Option<String>, MetadataStoreError> {
        self.with_conn("query profile source", |conn| {
            conn.query_row(
                "SELECT source FROM profiles WHERE current_filename = ?1 AND deleted_at IS NULL",
                rusqlite::params![name],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map(|opt| opt.flatten())
            .map_err(|source| MetadataStoreError::Database {
                action: "query profile source",
                source,
            })
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
            let placeholders = Self::in_clause_placeholders(profile_names.len());
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
            let placeholders = Self::in_clause_placeholders(profile_ids.len());
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
        })
    }

    pub fn query_failure_trends(
        &self,
        days: u32,
    ) -> Result<Vec<FailureTrendRow>, MetadataStoreError> {
        self.with_conn("query failure trends", |conn| {
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
        })
    }

    // -------------------------------------------------------------------------
    // Phase D: Health snapshot persistence
    // -------------------------------------------------------------------------

    pub fn upsert_health_snapshot(
        &self,
        profile_id: &str,
        status: &str,
        issue_count: usize,
        checked_at: &str,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("upsert a health snapshot", |conn| {
            health_store::upsert_health_snapshot(conn, profile_id, status, issue_count, checked_at)
        })
    }

    pub fn load_health_snapshots(&self) -> Result<Vec<HealthSnapshotRow>, MetadataStoreError> {
        self.with_conn("load health snapshots", |conn| {
            health_store::load_health_snapshots(conn)
        })
    }

    pub fn lookup_health_snapshot(
        &self,
        profile_id: &str,
    ) -> Result<Option<HealthSnapshotRow>, MetadataStoreError> {
        self.with_conn("look up a health snapshot", |conn| {
            health_store::lookup_health_snapshot(conn, profile_id)
        })
    }

    // -------------------------------------------------------------------------
    // Version snapshots
    // -------------------------------------------------------------------------

    pub fn upsert_version_snapshot(
        &self,
        profile_id: &str,
        steam_app_id: &str,
        steam_build_id: Option<&str>,
        trainer_version: Option<&str>,
        trainer_file_hash: Option<&str>,
        human_game_ver: Option<&str>,
        status: &str,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn_mut("upsert a version snapshot", |conn| {
            version_store::upsert_version_snapshot(
                conn,
                profile_id,
                steam_app_id,
                steam_build_id,
                trainer_version,
                trainer_file_hash,
                human_game_ver,
                status,
            )
        })
    }

    pub fn lookup_latest_version_snapshot(
        &self,
        profile_id: &str,
    ) -> Result<Option<VersionSnapshotRow>, MetadataStoreError> {
        self.with_conn("look up the latest version snapshot", |conn| {
            version_store::lookup_latest_version_snapshot(conn, profile_id)
        })
    }

    pub fn load_version_snapshots_for_profiles(
        &self,
    ) -> Result<Vec<VersionSnapshotRow>, MetadataStoreError> {
        self.with_conn("load version snapshots for profiles", |conn| {
            version_store::load_version_snapshots_for_profiles(conn)
        })
    }

    pub fn acknowledge_version_change(
        &self,
        profile_id: &str,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn_mut("acknowledge a version change", |conn| {
            version_store::acknowledge_version_change(conn, profile_id)
        })
    }

    // -------------------------------------------------------------------------
    // Config revision history
    // -------------------------------------------------------------------------

    /// Insert a config revision for the profile, skipping if the hash matches the
    /// latest recorded revision. Returns the new id when inserted, `None` on dedup.
    pub fn insert_config_revision(
        &self,
        profile_id: &str,
        profile_name_at_write: &str,
        source: ConfigRevisionSource,
        content_hash: &str,
        snapshot_toml: &str,
        source_revision_id: Option<i64>,
    ) -> Result<Option<i64>, MetadataStoreError> {
        self.with_conn_mut("insert a config revision", |conn| {
            config_history_store::insert_config_revision(
                conn,
                profile_id,
                profile_name_at_write,
                source,
                content_hash,
                snapshot_toml,
                source_revision_id,
            )
        })
    }

    /// List config revisions for a profile ordered newest first.
    /// `limit` defaults to `MAX_CONFIG_REVISIONS_PER_PROFILE` when `None`.
    pub fn list_config_revisions(
        &self,
        profile_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<ConfigRevisionRow>, MetadataStoreError> {
        self.with_conn("list config revisions", |conn| {
            config_history_store::list_config_revisions(conn, profile_id, limit)
        })
    }

    /// Get a single config revision by id, scoped to `profile_id`.
    /// Returns `None` if not found or the revision belongs to a different profile.
    pub fn get_config_revision(
        &self,
        profile_id: &str,
        revision_id: i64,
    ) -> Result<Option<ConfigRevisionRow>, MetadataStoreError> {
        self.with_conn("get a config revision", |conn| {
            config_history_store::get_config_revision(conn, profile_id, revision_id)
        })
    }

    /// Mark a revision as known-good for the profile, clearing the marker on all
    /// other revisions for that profile (single-active-marker semantics).
    pub fn set_known_good_revision(
        &self,
        profile_id: &str,
        revision_id: i64,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn_mut("set a known-good config revision", |conn| {
            config_history_store::set_known_good_revision(conn, profile_id, revision_id)
        })
    }

    /// Clear the known-good marker from all revisions for the given profile.
    pub fn clear_known_good_revision(&self, profile_id: &str) -> Result<(), MetadataStoreError> {
        self.with_conn("clear known-good config revision markers", |conn| {
            config_history_store::clear_known_good_revision(conn, profile_id)
        })
    }

    // -------------------------------------------------------------------------
    // Bundled / user launch optimization presets (metadata)
    // -------------------------------------------------------------------------

    pub fn list_bundled_optimization_presets(
        &self,
    ) -> Result<Vec<BundledOptimizationPresetRow>, MetadataStoreError> {
        self.with_conn("list bundled optimization presets", |conn| {
            preset_store::list_bundled_optimization_presets(conn)
        })
    }

    pub fn get_bundled_optimization_preset(
        &self,
        preset_id: &str,
    ) -> Result<Option<BundledOptimizationPresetRow>, MetadataStoreError> {
        self.with_conn("get bundled optimization preset", |conn| {
            preset_store::get_bundled_optimization_preset(conn, preset_id)
        })
    }

    pub fn upsert_profile_launch_preset_metadata(
        &self,
        profile_id: &str,
        preset_name: &str,
        origin: ProfileLaunchPresetOrigin,
        source_bundled_preset_id: Option<&str>,
    ) -> Result<(), MetadataStoreError> {
        let now = Utc::now().to_rfc3339();
        self.with_conn("upsert profile launch preset metadata", |conn| {
            preset_store::upsert_profile_launch_preset_metadata(
                conn,
                profile_id,
                preset_name,
                origin,
                source_bundled_preset_id,
                &now,
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::community::index::{CommunityProfileIndex, CommunityProfileIndexEntry};
    use crate::community::taps::{
        CommunityTapSubscription, CommunityTapSyncResult, CommunityTapSyncStatus,
        CommunityTapWorkspace,
    };
    use crate::community::{
        CommunityProfileManifest, CommunityProfileMetadata, CompatibilityRating,
    };
    use crate::launch::diagnostics::models::{
        ActionableSuggestion, DiagnosticReport, ExitCodeInfo, FailureMode,
    };
    use crate::launch::request::ValidationSeverity;
    use crate::profile::{
        GameProfile, GameSection, InjectionSection, LaunchSection, LauncherSection,
        LocalOverrideSection, ProfileStore, RuntimeSection, SteamSection, TrainerLoadingMode,
        TrainerSection,
    };
    use rusqlite::params;
    use std::fs;
    use std::os::unix::fs::{symlink, PermissionsExt};
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn sample_profile() -> GameProfile {
        GameProfile {
            game: GameSection {
                name: "Elden Ring".to_string(),
                executable_path: "/games/elden-ring/eldenring.exe".to_string(),
            },
            trainer: TrainerSection {
                path: "/trainers/elden-ring.exe".to_string(),
                kind: "fling".to_string(),
                loading_mode: TrainerLoadingMode::SourceDirectory,
            },
            injection: InjectionSection {
                dll_paths: vec!["/dlls/a.dll".to_string(), "/dlls/b.dll".to_string()],
                inject_on_launch: vec![true, false],
            },
            steam: SteamSection {
                enabled: true,
                app_id: "1245620".to_string(),
                compatdata_path: "/steam/compatdata/1245620".to_string(),
                proton_path: "/steam/proton/proton".to_string(),
                launcher: LauncherSection {
                    icon_path: "/icons/elden-ring.png".to_string(),
                    display_name: "Elden Ring".to_string(),
                },
            },
            runtime: RuntimeSection {
                prefix_path: String::new(),
                proton_path: String::new(),
                working_directory: String::new(),
            },
            launch: LaunchSection {
                method: "steam_applaunch".to_string(),
                ..Default::default()
            },
            local_override: LocalOverrideSection::default(),
        }
    }

    fn connection(store: &MetadataStore) -> std::sync::MutexGuard<'_, Connection> {
        store
            .conn
            .as_ref()
            .expect("metadata store should expose a connection in tests")
            .lock()
            .expect("metadata store mutex should not be poisoned")
    }

    #[test]
    fn test_observe_profile_write_creates_row() {
        let store = MetadataStore::open_in_memory().unwrap();
        let profile = sample_profile();
        let path = std::path::Path::new("/profiles/elden-ring.toml");

        store
            .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
            .unwrap();

        let conn = connection(&store);
        let (profile_id, current_filename, game_name, launch_method): (
            String,
            String,
            Option<String>,
            Option<String>,
        ) = conn
            .query_row(
                "SELECT profile_id, current_filename, game_name, launch_method FROM profiles WHERE current_filename = ?1",
                params!["elden-ring"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();

        assert!(!profile_id.trim().is_empty());
        assert_eq!(current_filename, "elden-ring");
        assert_eq!(game_name.as_deref(), Some("Elden Ring"));
        assert_eq!(launch_method.as_deref(), Some("steam_applaunch"));
    }

    #[test]
    fn test_observe_profile_write_idempotent() {
        let store = MetadataStore::open_in_memory().unwrap();
        let profile = sample_profile();
        let path = std::path::Path::new("/profiles/elden-ring.toml");

        store
            .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
            .unwrap();
        store
            .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
            .unwrap();

        let conn = connection(&store);
        let row_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM profiles WHERE current_filename = ?1",
                params!["elden-ring"],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(row_count, 1);
    }

    #[test]
    fn test_observe_profile_rename_creates_history() {
        let store = MetadataStore::open_in_memory().unwrap();
        let profile = sample_profile();
        let old_path = std::path::Path::new("/profiles/old-name.toml");
        let new_path = std::path::Path::new("/profiles/new-name.toml");

        store
            .observe_profile_write("old-name", &profile, old_path, SyncSource::AppWrite, None)
            .unwrap();
        store
            .observe_profile_rename("old-name", "new-name", old_path, new_path)
            .unwrap();

        let conn = connection(&store);
        let renamed_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM profiles WHERE current_filename = ?1",
                params!["new-name"],
                |row| row.get(0),
            )
            .unwrap();
        let history: (String, String, String, String) = conn
            .query_row(
                "SELECT old_name, new_name, old_path, new_path FROM profile_name_history",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();

        assert_eq!(renamed_count, 1);
        assert_eq!(history.0, "old-name");
        assert_eq!(history.1, "new-name");
        assert_eq!(history.2, old_path.to_string_lossy());
        assert_eq!(history.3, new_path.to_string_lossy());
    }

    #[test]
    fn test_observe_profile_delete_tombstones() {
        let store = MetadataStore::open_in_memory().unwrap();
        let profile = sample_profile();
        let path = std::path::Path::new("/profiles/elden-ring.toml");

        store
            .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
            .unwrap();
        store.observe_profile_delete("elden-ring").unwrap();

        let conn = connection(&store);
        let (row_count, deleted_at): (i64, Option<String>) = conn
            .query_row(
                "SELECT COUNT(*), MAX(deleted_at) FROM profiles WHERE current_filename = ?1",
                params!["elden-ring"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(row_count, 1);
        assert!(deleted_at.is_some());
    }

    #[test]
    fn test_sync_profiles_from_store() {
        let temp_dir = tempdir().unwrap();
        let store = MetadataStore::open_in_memory().unwrap();
        let profile_store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
        let profile = sample_profile();

        profile_store.save("alpha", &profile).unwrap();
        profile_store.save("beta", &profile).unwrap();
        profile_store.save("gamma", &profile).unwrap();

        let report = store.sync_profiles_from_store(&profile_store).unwrap();

        let conn = connection(&store);
        let row_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM profiles", [], |row| row.get(0))
            .unwrap();

        assert_eq!(report.profiles_seen, 3);
        assert_eq!(report.created, 3);
        assert_eq!(report.updated, 0);
        assert_eq!(report.deleted, 0);
        assert!(report.errors.is_empty());
        assert_eq!(row_count, 3);
    }

    #[test]
    fn test_unavailable_store_noop() {
        let temp_dir = tempdir().unwrap();
        let store = MetadataStore::disabled();
        let profile = sample_profile();
        let profile_store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));

        assert!(store
            .observe_profile_write(
                "elden-ring",
                &profile,
                std::path::Path::new("/profiles/elden-ring.toml"),
                SyncSource::AppWrite,
                None,
            )
            .is_ok());
        assert!(store
            .observe_profile_rename(
                "elden-ring",
                "elden-ring-renamed",
                std::path::Path::new("/profiles/elden-ring.toml"),
                std::path::Path::new("/profiles/elden-ring-renamed.toml"),
            )
            .is_ok());
        assert!(store.observe_profile_delete("elden-ring").is_ok());

        let report = store.sync_profiles_from_store(&profile_store).unwrap();
        assert_eq!(report.profiles_seen, 0);
        assert_eq!(report.created, 0);
        assert_eq!(report.updated, 0);
        assert_eq!(report.deleted, 0);
        assert!(report.errors.is_empty());
    }

    #[test]
    fn test_file_permissions() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("metadata.db");

        let _store = MetadataStore::with_path(&db_path).unwrap();

        let mode = fs::metadata(&db_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn test_symlink_rejected() {
        let temp_dir = tempdir().unwrap();
        let target_path = temp_dir.path().join("real-metadata.db");
        let symlink_path = temp_dir.path().join("metadata.db");

        fs::write(&target_path, b"").unwrap();
        symlink(&target_path, &symlink_path).unwrap();

        let error = match MetadataStore::with_path(&symlink_path) {
            Ok(_) => panic!("expected metadata symlink path to be rejected"),
            Err(error) => error,
        };
        assert!(matches!(error, MetadataStoreError::SymlinkDetected(path) if path == symlink_path));
    }

    fn clean_exit_report() -> DiagnosticReport {
        DiagnosticReport {
            severity: ValidationSeverity::Info,
            summary: "Clean exit".to_string(),
            exit_info: ExitCodeInfo {
                code: Some(0),
                signal: None,
                signal_name: None,
                core_dumped: false,
                failure_mode: FailureMode::CleanExit,
                description: "Process exited cleanly".to_string(),
                severity: ValidationSeverity::Info,
            },
            pattern_matches: vec![],
            suggestions: vec![],
            launch_method: "native".to_string(),
            log_tail_path: None,
            analyzed_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

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
            "small report ({} bytes) must be under 4KB for this test",
            small_json_len
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
                title: format!("Suggestion title number {} with extra padding to push over 4KB boundary", i),
                description: format!(
                    "Suggestion description number {} with a lot of extra text to ensure that the serialized JSON grows large enough to exceed the 4096-byte limit imposed by MAX_DIAGNOSTIC_JSON_BYTES",
                    i
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
        };

        let large_json_len = serde_json::to_string(&large_report).unwrap().len();
        assert!(
            large_json_len > MAX_DIAGNOSTIC_JSON_BYTES,
            "large report ({} bytes) must exceed 4KB for this test",
            large_json_len
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

    // -------------------------------------------------------------------------
    // Phase 3 test helpers
    // -------------------------------------------------------------------------

    fn sample_tap_workspace(url: &str) -> CommunityTapWorkspace {
        CommunityTapWorkspace {
            subscription: CommunityTapSubscription {
                url: url.to_string(),
                branch: None,
                pinned_commit: None,
            },
            local_path: PathBuf::from("/tmp/test-tap"),
        }
    }

    fn sample_index_entry(
        tap_url: &str,
        relative_path: &str,
        game_name: &str,
    ) -> CommunityProfileIndexEntry {
        CommunityProfileIndexEntry {
            tap_url: tap_url.to_string(),
            tap_branch: None,
            tap_path: PathBuf::from("/tmp/test-tap"),
            manifest_path: PathBuf::from(format!("/tmp/test-tap/{relative_path}")),
            relative_path: PathBuf::from(relative_path),
            manifest: CommunityProfileManifest::new(
                CommunityProfileMetadata {
                    game_name: game_name.to_string(),
                    game_version: "1.0".to_string(),
                    trainer_name: "TestTrainer".to_string(),
                    trainer_version: "1".to_string(),
                    proton_version: "9".to_string(),
                    platform_tags: vec!["linux".to_string()],
                    compatibility_rating: CompatibilityRating::Working,
                    author: "TestAuthor".to_string(),
                    description: "Test profile".to_string(),
                },
                GameProfile::default(),
            ),
        }
    }

    fn sample_sync_result(
        tap_url: &str,
        head_commit: &str,
        entries: Vec<CommunityProfileIndexEntry>,
    ) -> CommunityTapSyncResult {
        CommunityTapSyncResult {
            workspace: sample_tap_workspace(tap_url),
            status: CommunityTapSyncStatus::Updated,
            head_commit: head_commit.to_string(),
            index: CommunityProfileIndex {
                entries,
                diagnostics: vec![],
            },
        }
    }

    // -------------------------------------------------------------------------
    // Phase 3: Community index tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_index_tap_result_inserts_tap_and_profile_rows() {
        let store = MetadataStore::open_in_memory().unwrap();
        let tap_url = "https://example.invalid/tap.git";
        let result = sample_sync_result(
            tap_url,
            "abc123",
            vec![
                sample_index_entry(tap_url, "profiles/game-a/community-profile.json", "Game A"),
                sample_index_entry(tap_url, "profiles/game-b/community-profile.json", "Game B"),
            ],
        );

        store.index_community_tap_result(&result).unwrap();

        let conn = connection(&store);

        let (tap_count, last_head_commit, profile_count): (i64, String, i64) = conn
            .query_row(
                "SELECT COUNT(*), last_head_commit, profile_count FROM community_taps WHERE tap_url = ?1",
                params![tap_url],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(tap_count, 1);
        assert_eq!(last_head_commit, "abc123");
        assert_eq!(profile_count, 2);

        let community_profile_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM community_profiles cp \
                 JOIN community_taps ct ON cp.tap_id = ct.tap_id \
                 WHERE ct.tap_url = ?1",
                params![tap_url],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(community_profile_count, 2);
    }

    #[test]
    fn test_index_tap_result_skips_on_unchanged_head() {
        let store = MetadataStore::open_in_memory().unwrap();
        let tap_url = "https://example.invalid/tap.git";
        let result = sample_sync_result(
            tap_url,
            "abc123",
            vec![sample_index_entry(
                tap_url,
                "profiles/game-a/community-profile.json",
                "Game A",
            )],
        );

        store.index_community_tap_result(&result).unwrap();

        let updated_at_first: String = {
            let conn = connection(&store);
            conn.query_row(
                "SELECT updated_at FROM community_taps WHERE tap_url = ?1",
                params![tap_url],
                |row| row.get(0),
            )
            .unwrap()
        };

        // Index again with same head_commit — should be a no-op watermark skip.
        store.index_community_tap_result(&result).unwrap();

        let (updated_at_second, profile_count): (String, i64) = {
            let conn = connection(&store);
            conn.query_row(
                "SELECT updated_at, profile_count FROM community_taps WHERE tap_url = ?1",
                params![tap_url],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap()
        };

        assert_eq!(
            updated_at_first, updated_at_second,
            "updated_at must not change on watermark skip"
        );
        assert_eq!(profile_count, 1);
    }

    #[test]
    fn test_index_tap_result_replaces_stale_profiles() {
        let store = MetadataStore::open_in_memory().unwrap();
        let tap_url = "https://example.invalid/tap.git";

        // First index: 3 profiles.
        let result_v1 = sample_sync_result(
            tap_url,
            "commit-v1",
            vec![
                sample_index_entry(tap_url, "profiles/game-a/community-profile.json", "Game A"),
                sample_index_entry(tap_url, "profiles/game-b/community-profile.json", "Game B"),
                sample_index_entry(tap_url, "profiles/game-c/community-profile.json", "Game C"),
            ],
        );
        store.index_community_tap_result(&result_v1).unwrap();

        // Second index: only 1 profile, different HEAD commit.
        let result_v2 = sample_sync_result(
            tap_url,
            "commit-v2",
            vec![sample_index_entry(
                tap_url,
                "profiles/game-a/community-profile.json",
                "Game A",
            )],
        );
        store.index_community_tap_result(&result_v2).unwrap();

        let conn = connection(&store);
        let profile_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM community_profiles cp \
                 JOIN community_taps ct ON cp.tap_id = ct.tap_id \
                 WHERE ct.tap_url = ?1",
                params![tap_url],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            profile_count, 1,
            "stale profiles should have been removed on re-index"
        );
    }

    #[test]
    fn test_community_profiles_fk_cascades_on_tap_delete() {
        let store = MetadataStore::open_in_memory().unwrap();
        let tap_url = "https://example.invalid/tap.git";
        let result = sample_sync_result(
            tap_url,
            "commit-v1",
            vec![sample_index_entry(
                tap_url,
                "profiles/game-a/community-profile.json",
                "Game A",
            )],
        );
        store.index_community_tap_result(&result).unwrap();

        let conn = connection(&store);
        let tap_id: String = conn
            .query_row(
                "SELECT tap_id FROM community_taps WHERE tap_url = ?1",
                params![tap_url],
                |row| row.get(0),
            )
            .unwrap();

        conn.execute(
            "DELETE FROM community_taps WHERE tap_id = ?1",
            params![&tap_id],
        )
        .unwrap();

        let orphan_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM community_profiles WHERE tap_id = ?1",
                params![&tap_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            orphan_count, 0,
            "deleting a tap should cascade delete community profiles"
        );
    }

    #[test]
    fn test_index_tap_result_disabled_store_noop() {
        let store = MetadataStore::disabled();
        let tap_url = "https://example.invalid/tap.git";
        let result = sample_sync_result(tap_url, "abc123", vec![]);

        let outcome = store.index_community_tap_result(&result);
        assert!(outcome.is_ok());
    }

    // -------------------------------------------------------------------------
    // Phase 3: Cache store tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_put_get_cache_entry_round_trip() {
        let store = MetadataStore::open_in_memory().unwrap();

        store
            .put_cache_entry(
                "https://example.invalid/source",
                "my-cache-key",
                r#"{"data":"hello"}"#,
                None,
            )
            .unwrap();

        let result = store.get_cache_entry("my-cache-key").unwrap();

        assert_eq!(result.as_deref(), Some(r#"{"data":"hello"}"#));
    }

    #[test]
    fn test_put_cache_entry_idempotent() {
        let store = MetadataStore::open_in_memory().unwrap();

        store
            .put_cache_entry(
                "https://example.invalid/source",
                "dedup-key",
                "payload-v1",
                None,
            )
            .unwrap();
        store
            .put_cache_entry(
                "https://example.invalid/source",
                "dedup-key",
                "payload-v2",
                None,
            )
            .unwrap();

        let conn = connection(&store);
        let row_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM external_cache_entries WHERE cache_key = ?1",
                params!["dedup-key"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(row_count, 1, "UPSERT should not create duplicate rows");
    }

    #[test]
    fn test_cache_payload_oversized_stored_as_null() {
        let store = MetadataStore::open_in_memory().unwrap();

        // Build a payload larger than MAX_CACHE_PAYLOAD_BYTES (524_288 bytes / 512 KiB).
        let oversized_payload = "x".repeat(MAX_CACHE_PAYLOAD_BYTES + 1);
        let original_size = oversized_payload.len();

        store
            .put_cache_entry(
                "https://example.invalid/source",
                "oversized-key",
                &oversized_payload,
                None,
            )
            .unwrap();

        let conn = connection(&store);
        let (payload_json, payload_size): (Option<String>, i64) = conn
            .query_row(
                "SELECT payload_json, payload_size FROM external_cache_entries WHERE cache_key = ?1",
                params!["oversized-key"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert!(
            payload_json.is_none(),
            "oversized payload should be stored as NULL"
        );
        assert_eq!(
            payload_size, original_size as i64,
            "payload_size should record the original size"
        );
    }

    #[test]
    fn test_evict_expired_entries() {
        let store = MetadataStore::open_in_memory().unwrap();

        // Insert a non-expired entry (expires far in the future).
        store
            .put_cache_entry(
                "https://example.invalid/source",
                "live-key",
                "live-payload",
                Some("2099-01-01T00:00:00Z"),
            )
            .unwrap();

        // Insert an expired entry directly via raw SQL (already past expiry).
        {
            let conn = connection(&store);
            conn.execute(
                "INSERT INTO external_cache_entries \
                 (cache_id, source_url, cache_key, payload_json, payload_size, fetched_at, expires_at, created_at, updated_at) \
                 VALUES ('expired-id', 'https://example.invalid/source', 'expired-key', 'expired', 7, \
                 '2020-01-01T00:00:00Z', '2020-01-02T00:00:00Z', '2020-01-01T00:00:00Z', '2020-01-01T00:00:00Z')",
                [],
            )
            .unwrap();
        }

        let evicted = store.evict_expired_cache_entries().unwrap();
        assert_eq!(evicted, 1);

        let conn = connection(&store);
        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM external_cache_entries", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(remaining, 1, "only the non-expired entry should remain");
    }

    #[test]
    fn test_cache_entry_disabled_store_noop() {
        let store = MetadataStore::disabled();

        let result = store.get_cache_entry("any-key").unwrap();
        assert!(result.is_none());
    }

    // -------------------------------------------------------------------------
    // Phase 3: Collections tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_create_collection_returns_id() {
        let store = MetadataStore::open_in_memory().unwrap();

        let collection_id = store.create_collection("My Favorites").unwrap();
        assert!(!collection_id.trim().is_empty());

        let conn = connection(&store);
        let row_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM collections WHERE name = ?1",
                params!["My Favorites"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(row_count, 1);
    }

    #[test]
    fn test_add_profile_to_collection() {
        let store = MetadataStore::open_in_memory().unwrap();
        let profile = sample_profile();
        let path = std::path::Path::new("/profiles/elden-ring.toml");

        store
            .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
            .unwrap();

        let collection_id = store.create_collection("Test Collection").unwrap();
        store
            .add_profile_to_collection(&collection_id, "elden-ring")
            .unwrap();

        let conn = connection(&store);
        let row_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM collection_profiles WHERE collection_id = ?1",
                params![collection_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(row_count, 1);
    }

    #[test]
    fn test_collection_delete_cascades() {
        let store = MetadataStore::open_in_memory().unwrap();
        let profile = sample_profile();
        let path = std::path::Path::new("/profiles/elden-ring.toml");

        store
            .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
            .unwrap();

        let collection_id = store.create_collection("To Delete").unwrap();
        store
            .add_profile_to_collection(&collection_id, "elden-ring")
            .unwrap();

        store.delete_collection(&collection_id).unwrap();

        let conn = connection(&store);
        let member_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM collection_profiles WHERE collection_id = ?1",
                params![collection_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            member_count, 0,
            "collection_profiles rows should cascade-delete with the collection"
        );
    }

    #[test]
    fn test_set_profile_favorite_toggles() {
        let store = MetadataStore::open_in_memory().unwrap();
        let profile = sample_profile();
        let path = std::path::Path::new("/profiles/elden-ring.toml");

        store
            .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
            .unwrap();

        store.set_profile_favorite("elden-ring", true).unwrap();

        let conn = connection(&store);
        let is_favorite: i64 = conn
            .query_row(
                "SELECT is_favorite FROM profiles WHERE current_filename = ?1",
                params!["elden-ring"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(is_favorite, 1);

        drop(conn);
        store.set_profile_favorite("elden-ring", false).unwrap();

        let conn = connection(&store);
        let is_favorite: i64 = conn
            .query_row(
                "SELECT is_favorite FROM profiles WHERE current_filename = ?1",
                params!["elden-ring"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(is_favorite, 0);
    }

    #[test]
    fn test_list_favorite_profiles_excludes_deleted() {
        let store = MetadataStore::open_in_memory().unwrap();
        let profile = sample_profile();

        store
            .observe_profile_write(
                "keep-me",
                &profile,
                std::path::Path::new("/profiles/keep-me.toml"),
                SyncSource::AppWrite,
                None,
            )
            .unwrap();
        store
            .observe_profile_write(
                "delete-me",
                &profile,
                std::path::Path::new("/profiles/delete-me.toml"),
                SyncSource::AppWrite,
                None,
            )
            .unwrap();

        store.set_profile_favorite("keep-me", true).unwrap();
        store.set_profile_favorite("delete-me", true).unwrap();
        store.observe_profile_delete("delete-me").unwrap();

        let favorites = store.list_favorite_profiles().unwrap();
        assert_eq!(favorites, vec!["keep-me".to_string()]);
    }

    // -------------------------------------------------------------------------
    // Phase 3: Usage insights tests
    // -------------------------------------------------------------------------

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
    fn test_migration_9_to_10_seeds_bundled_gpu_presets() {
        let store = MetadataStore::open_in_memory().unwrap();
        let rows = store.list_bundled_optimization_presets().unwrap();
        assert_eq!(rows.len(), 4);
        let ids: Vec<_> = rows.iter().map(|r| r.preset_id.as_str()).collect();
        assert!(ids.contains(&"nvidia_performance"));
        assert!(ids.contains(&"nvidia_quality"));
        assert!(ids.contains(&"amd_performance"));
        assert!(ids.contains(&"amd_quality"));
    }

    #[test]
    fn test_migration_8_to_9_version_snapshots_table_exists() {
        let store = MetadataStore::open_in_memory().unwrap();
        let profile = sample_profile();
        let path = std::path::Path::new("/profiles/test-game.toml");

        // Seed a profile row so the FK constraint is satisfied.
        store
            .observe_profile_write("test-game", &profile, path, SyncSource::AppWrite, None)
            .unwrap();

        let conn = connection(&store);

        // Retrieve the profile_id for the seeded row.
        let profile_id: String = conn
            .query_row(
                "SELECT profile_id FROM profiles WHERE current_filename = ?1",
                params!["test-game"],
                |row| row.get(0),
            )
            .unwrap();

        // INSERT roundtrip: verify the table and its columns exist.
        conn.execute(
            "INSERT INTO version_snapshots
                (profile_id, steam_app_id, steam_build_id, trainer_version,
                 trainer_file_hash, human_game_ver, status, checked_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                profile_id,
                "1245620",
                "12345678",
                "v1.0.0",
                "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
                "1.0",
                "untracked",
                "2026-01-01T00:00:00+00:00",
            ],
        )
        .unwrap();

        let (row_profile_id, steam_app_id, steam_build_id, status): (
            String,
            String,
            Option<String>,
            String,
        ) = conn
            .query_row(
                "SELECT profile_id, steam_app_id, steam_build_id, status
                 FROM version_snapshots
                 WHERE profile_id = ?1",
                params![profile_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();

        assert_eq!(row_profile_id, profile_id);
        assert_eq!(steam_app_id, "1245620");
        assert_eq!(steam_build_id.as_deref(), Some("12345678"));
        assert_eq!(status, "untracked");
    }

    // -------------------------------------------------------------------------
    // Version store tests
    // -------------------------------------------------------------------------

    fn insert_test_profile_row(conn: &Connection, profile_id: &str) {
        conn.execute(
            "INSERT INTO profiles (profile_id, current_filename, current_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                profile_id,
                format!("{profile_id}_file"),
                format!("/path/{profile_id}.toml"),
                "2024-01-01T00:00:00+00:00",
                "2024-01-01T00:00:00+00:00",
            ],
        )
        .unwrap();
    }

    #[test]
    fn test_version_snapshot_upsert_and_lookup_lifecycle() {
        let store = MetadataStore::open_in_memory().unwrap();
        {
            let conn = connection(&store);
            insert_test_profile_row(&conn, "lifecycle-profile");
        }

        store
            .upsert_version_snapshot(
                "lifecycle-profile",
                "99999",
                Some("build-abc"),
                Some("v1.2.3"),
                Some("deadbeef01234567deadbeef01234567deadbeef01234567deadbeef01234567"),
                Some("1.2.3"),
                "matched",
            )
            .unwrap();

        let snapshot = store
            .lookup_latest_version_snapshot("lifecycle-profile")
            .unwrap()
            .expect("snapshot should be present after upsert");

        assert_eq!(snapshot.profile_id, "lifecycle-profile");
        assert_eq!(snapshot.steam_app_id, "99999");
        assert_eq!(snapshot.steam_build_id.as_deref(), Some("build-abc"));
        assert_eq!(snapshot.trainer_version.as_deref(), Some("v1.2.3"));
        assert_eq!(snapshot.status, "matched");
        assert!(!snapshot.checked_at.is_empty());
    }

    #[test]
    fn test_version_snapshot_lookup_returns_latest() {
        let store = MetadataStore::open_in_memory().unwrap();
        {
            let conn = connection(&store);
            insert_test_profile_row(&conn, "latest-profile");
        }

        // Insert two snapshots with distinct checked_at values via raw SQL
        // so we can control ordering.
        {
            let conn = connection(&store);
            conn.execute(
                "INSERT INTO version_snapshots
                 (profile_id, steam_app_id, steam_build_id, status, checked_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    "latest-profile",
                    "11111",
                    "build-old",
                    "untracked",
                    "2024-01-01T00:00:00+00:00",
                ],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO version_snapshots
                 (profile_id, steam_app_id, steam_build_id, status, checked_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    "latest-profile",
                    "11111",
                    "build-new",
                    "matched",
                    "2024-06-01T00:00:00+00:00",
                ],
            )
            .unwrap();
        }

        let snapshot = store
            .lookup_latest_version_snapshot("latest-profile")
            .unwrap()
            .expect("snapshot should be present");

        assert_eq!(snapshot.steam_build_id.as_deref(), Some("build-new"));
        assert_eq!(snapshot.status, "matched");
    }

    #[test]
    fn test_version_snapshot_pruning_at_max_limit() {
        let store = MetadataStore::open_in_memory().unwrap();
        {
            let conn = connection(&store);
            insert_test_profile_row(&conn, "prune-profile");
        }

        // Insert MAX+1 rows — the prune step must keep exactly MAX.
        for i in 0..=MAX_VERSION_SNAPSHOTS_PER_PROFILE {
            store
                .upsert_version_snapshot(
                    "prune-profile",
                    "55555",
                    Some(&format!("build-{i:04}")),
                    None,
                    None,
                    None,
                    "untracked",
                )
                .unwrap();
        }

        let conn = connection(&store);
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM version_snapshots WHERE profile_id = 'prune-profile'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(
            count,
            MAX_VERSION_SNAPSHOTS_PER_PROFILE as i64,
            "row count must be exactly MAX after pruning"
        );
    }

    #[test]
    fn test_acknowledge_version_change_sets_matched() {
        let store = MetadataStore::open_in_memory().unwrap();
        {
            let conn = connection(&store);
            insert_test_profile_row(&conn, "ack-profile");
        }

        store
            .upsert_version_snapshot("ack-profile", "77777", None, None, None, None, "game_updated")
            .unwrap();

        // Confirm initial status is game_updated.
        let before = store
            .lookup_latest_version_snapshot("ack-profile")
            .unwrap()
            .unwrap();
        assert_eq!(before.status, "game_updated");

        store.acknowledge_version_change("ack-profile").unwrap();

        let after = store
            .lookup_latest_version_snapshot("ack-profile")
            .unwrap()
            .unwrap();
        assert_eq!(after.status, "matched");
    }

    #[test]
    fn test_load_version_snapshots_for_profiles_returns_latest_per_profile() {
        let store = MetadataStore::open_in_memory().unwrap();
        {
            let conn = connection(&store);
            insert_test_profile_row(&conn, "bulk-profile-a");
            insert_test_profile_row(&conn, "bulk-profile-b");
        }

        // Profile A: two snapshots — the second (game_updated) should win.
        {
            let conn = connection(&store);
            conn.execute(
                "INSERT INTO version_snapshots (profile_id, steam_app_id, status, checked_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    "bulk-profile-a",
                    "10001",
                    "untracked",
                    "2024-01-01T00:00:00+00:00",
                ],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO version_snapshots (profile_id, steam_app_id, status, checked_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    "bulk-profile-a",
                    "10001",
                    "game_updated",
                    "2024-06-01T00:00:00+00:00",
                ],
            )
            .unwrap();
        }

        // Profile B: one snapshot.
        {
            let conn = connection(&store);
            conn.execute(
                "INSERT INTO version_snapshots (profile_id, steam_app_id, status, checked_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    "bulk-profile-b",
                    "20002",
                    "matched",
                    "2024-03-01T00:00:00+00:00",
                ],
            )
            .unwrap();
        }

        let snapshots = store.load_version_snapshots_for_profiles().unwrap();

        assert_eq!(snapshots.len(), 2, "should return one row per profile");

        let snap_a = snapshots
            .iter()
            .find(|s| s.profile_id == "bulk-profile-a")
            .expect("profile-a snapshot must be present");
        let snap_b = snapshots
            .iter()
            .find(|s| s.profile_id == "bulk-profile-b")
            .expect("profile-b snapshot must be present");

        // MAX(id) picks the last-inserted row for profile-a, which is game_updated.
        assert_eq!(snap_a.status, "game_updated");
        assert_eq!(snap_b.status, "matched");
    }

    #[test]
    fn test_compute_correlation_status_update_in_progress() {
        // state_flags Some(non-4) → UpdateInProgress regardless of other inputs.
        assert!(matches!(
            compute_correlation_status("build1", Some("build1"), None, None, Some(0)),
            VersionCorrelationStatus::UpdateInProgress
        ));
        assert!(matches!(
            compute_correlation_status("build1", Some("build1"), None, None, Some(6)),
            VersionCorrelationStatus::UpdateInProgress
        ));
        // state_flags None (manifest not found) → falls through to comparison, not UpdateInProgress.
        assert!(matches!(
            compute_correlation_status("build1", Some("build1"), None, None, None),
            VersionCorrelationStatus::Matched
        ));
    }

    #[test]
    fn test_compute_correlation_status_untracked() {
        // No snapshot → Untracked (when state_flags is stable).
        assert!(matches!(
            compute_correlation_status("build1", None, None, None, Some(4)),
            VersionCorrelationStatus::Untracked
        ));
    }

    #[test]
    fn test_compute_correlation_status_matched() {
        assert!(matches!(
            compute_correlation_status(
                "build1",
                Some("build1"),
                Some("hash-a"),
                Some("hash-a"),
                Some(4)
            ),
            VersionCorrelationStatus::Matched
        ));
        // Both trainer hashes None → also matched.
        assert!(matches!(
            compute_correlation_status("build1", Some("build1"), None, None, Some(4)),
            VersionCorrelationStatus::Matched
        ));
    }

    #[test]
    fn test_compute_correlation_status_game_updated() {
        assert!(matches!(
            compute_correlation_status(
                "build-new",
                Some("build-old"),
                Some("hash-a"),
                Some("hash-a"),
                Some(4)
            ),
            VersionCorrelationStatus::GameUpdated
        ));
    }

    #[test]
    fn test_compute_correlation_status_trainer_changed() {
        assert!(matches!(
            compute_correlation_status(
                "build1",
                Some("build1"),
                Some("hash-new"),
                Some("hash-old"),
                Some(4)
            ),
            VersionCorrelationStatus::TrainerChanged
        ));
    }

    #[test]
    fn test_compute_correlation_status_both_changed() {
        assert!(matches!(
            compute_correlation_status(
                "build-new",
                Some("build-old"),
                Some("hash-new"),
                Some("hash-old"),
                Some(4)
            ),
            VersionCorrelationStatus::BothChanged
        ));
    }

    #[test]
    fn test_version_store_disabled_store_noop() {
        let store = MetadataStore::disabled();

        assert!(store
            .upsert_version_snapshot(
                "any-profile", "12345", None, None, None, None, "untracked"
            )
            .is_ok());
        let snapshot = store.lookup_latest_version_snapshot("any-profile").unwrap();
        assert!(snapshot.is_none());
        let snapshots = store.load_version_snapshots_for_profiles().unwrap();
        assert!(snapshots.is_empty());
        assert!(store.acknowledge_version_change("any-profile").is_ok());
    }
}
