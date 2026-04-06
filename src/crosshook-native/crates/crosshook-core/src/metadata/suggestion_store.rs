use std::collections::HashSet;

use chrono::{Duration, Utc};

use super::MetadataStore;
use crate::metadata::MetadataStoreError;

impl MetadataStore {
    /// Upsert a suggestion dismissal with a TTL in days.
    pub fn dismiss_suggestion(
        &self,
        profile_id: &str,
        app_id: &str,
        suggestion_key: &str,
        ttl_days: u32,
    ) -> Result<(), MetadataStoreError> {
        let now = Utc::now();
        let dismissed_at = now.to_rfc3339();
        let expires_at = (now + Duration::days(i64::from(ttl_days))).to_rfc3339();

        self.with_conn_mut("dismiss suggestion", |conn| {
            conn.execute(
                "INSERT INTO suggestion_dismissals (profile_id, app_id, suggestion_key, dismissed_at, expires_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(profile_id, app_id, suggestion_key)
                 DO UPDATE SET dismissed_at = excluded.dismissed_at, expires_at = excluded.expires_at",
                rusqlite::params![profile_id, app_id, suggestion_key, dismissed_at, expires_at],
            )
            .map_err(|source| MetadataStoreError::Database {
                action: "dismiss suggestion",
                source,
            })?;

            Ok(())
        })
    }

    /// Return the set of dismissed suggestion keys for a profile+app, evicting expired rows first.
    pub fn get_dismissed_keys(
        &self,
        profile_id: &str,
        app_id: &str,
    ) -> Result<HashSet<String>, MetadataStoreError> {
        let now = Utc::now().to_rfc3339();

        self.with_conn_mut("get dismissed keys", |conn| {
            // Evict expired rows for this profile+app
            conn.execute(
                "DELETE FROM suggestion_dismissals WHERE profile_id = ?1 AND app_id = ?2 AND expires_at < ?3",
                rusqlite::params![profile_id, app_id, now],
            )
            .map_err(|source| MetadataStoreError::Database {
                action: "evict expired suggestion dismissals",
                source,
            })?;

            let mut stmt = conn
                .prepare("SELECT suggestion_key FROM suggestion_dismissals WHERE profile_id = ?1 AND app_id = ?2")
                .map_err(|source| MetadataStoreError::Database {
                    action: "prepare get dismissed keys query",
                    source,
                })?;

            let keys = stmt
                .query_map(rusqlite::params![profile_id, app_id], |row| {
                    row.get::<_, String>(0)
                })
                .map_err(|source| MetadataStoreError::Database {
                    action: "query dismissed keys",
                    source,
                })?
                .collect::<Result<HashSet<String>, _>>()
                .map_err(|source| MetadataStoreError::Database {
                    action: "decode dismissed key row",
                    source,
                })?;

            Ok(keys)
        })
    }

    /// Evict all expired dismissals across all profiles. Returns the number of rows deleted.
    pub fn evict_expired_dismissals(&self) -> Result<usize, MetadataStoreError> {
        let now = Utc::now().to_rfc3339();

        self.with_conn_mut("evict all expired suggestion dismissals", |conn| {
            let count = conn
                .execute(
                    "DELETE FROM suggestion_dismissals WHERE expires_at < ?1",
                    rusqlite::params![now],
                )
                .map_err(|source| MetadataStoreError::Database {
                    action: "evict all expired suggestion dismissals",
                    source,
                })?;

            Ok(count)
        })
    }
}
