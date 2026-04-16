use std::collections::HashSet;

use chrono::{Duration, Utc};

use super::MetadataStore;
use crate::metadata::MetadataStoreError;

impl MetadataStore {
    /// Record a dismissed readiness nag with TTL in days (system-global).
    pub fn dismiss_readiness_nag(
        &self,
        tool_id: &str,
        ttl_days: u32,
    ) -> Result<(), MetadataStoreError> {
        let now = Utc::now();
        let dismissed_at = now.to_rfc3339();
        let expires_at = (now + Duration::days(i64::from(ttl_days))).to_rfc3339();

        self.with_conn_mut("dismiss readiness nag", |conn| {
            conn.execute(
                "INSERT INTO readiness_nag_dismissals (tool_id, dismissed_at, expires_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(tool_id) DO UPDATE SET
                    dismissed_at = excluded.dismissed_at,
                    expires_at = excluded.expires_at",
                rusqlite::params![tool_id, dismissed_at, expires_at],
            )
            .map_err(|source| MetadataStoreError::Database {
                action: "dismiss readiness nag",
                source,
            })?;
            Ok(())
        })
    }

    /// Active dismissed tool IDs, evicting expired rows first.
    pub fn get_dismissed_readiness_nags(&self) -> Result<HashSet<String>, MetadataStoreError> {
        let now = Utc::now().to_rfc3339();

        self.with_conn_mut("get dismissed readiness nags", |conn| {
            conn.execute(
                "DELETE FROM readiness_nag_dismissals WHERE expires_at < ?1",
                rusqlite::params![now],
            )
            .map_err(|source| MetadataStoreError::Database {
                action: "evict expired readiness nag dismissals",
                source,
            })?;

            let mut stmt = conn
                .prepare("SELECT tool_id FROM readiness_nag_dismissals")
                .map_err(|source| MetadataStoreError::Database {
                    action: "prepare get dismissed readiness nags",
                    source,
                })?;

            let keys = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|source| MetadataStoreError::Database {
                    action: "query dismissed readiness nags",
                    source,
                })?
                .collect::<Result<HashSet<String>, _>>()
                .map_err(|source| MetadataStoreError::Database {
                    action: "decode readiness nag dismissal row",
                    source,
                })?;

            Ok(keys)
        })
    }

    /// Evict all expired readiness nag dismissals. Returns rows deleted.
    pub fn evict_expired_readiness_dismissals(&self) -> Result<usize, MetadataStoreError> {
        let now = Utc::now().to_rfc3339();

        self.with_conn_mut("evict all expired readiness nag dismissals", |conn| {
            let count = conn
                .execute(
                    "DELETE FROM readiness_nag_dismissals WHERE expires_at < ?1",
                    rusqlite::params![now],
                )
                .map_err(|source| MetadataStoreError::Database {
                    action: "evict all expired readiness nag dismissals",
                    source,
                })?;

            Ok(count)
        })
    }
}
