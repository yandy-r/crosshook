use chrono::Utc;
use rusqlite::{params, OptionalExtension};

use crate::metadata::{MetadataStore, MetadataStoreError};

use super::{CachedRssRow, CACHE_NAMESPACE};

/// Builds a per-source, per-query cache key.
/// Format: `trainer:source:v1:{source_id}:{normalized_query}`
pub(super) fn cache_key_for_source_query(source_id: &str, game_name: &str) -> String {
    let normalized = game_name.trim().to_lowercase().replace(' ', "_");
    format!("{CACHE_NAMESPACE}:{source_id}:{normalized}")
}

pub(super) fn load_cached_rss_row(
    metadata_store: &MetadataStore,
    key: &str,
    allow_expired: bool,
) -> Option<CachedRssRow> {
    if !metadata_store.is_available() {
        return None;
    }

    let now = Utc::now().to_rfc3339();
    let action = if allow_expired {
        "load a cached external search row"
    } else {
        "load a valid cached external search row"
    };

    metadata_store
        .with_sqlite_conn(action, |conn| {
            let sql = if allow_expired {
                "SELECT payload_json, fetched_at, expires_at \
                 FROM external_cache_entries WHERE cache_key = ?1 LIMIT 1"
            } else {
                "SELECT payload_json, fetched_at, expires_at \
                 FROM external_cache_entries \
                 WHERE cache_key = ?1 AND (expires_at IS NULL OR expires_at > ?2) LIMIT 1"
            };

            let row_params = if allow_expired {
                params![key]
            } else {
                params![key, now]
            };

            conn.query_row(sql, row_params, |row| {
                Ok(CachedRssRow {
                    payload_json: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    fetched_at: row.get::<_, String>(1)?,
                    _expires_at: row.get::<_, Option<String>>(2)?,
                })
            })
            .optional()
            .map_err(|source| MetadataStoreError::Database {
                action: "query an external search cache row",
                source,
            })
        })
        .ok()
        .flatten()
}
