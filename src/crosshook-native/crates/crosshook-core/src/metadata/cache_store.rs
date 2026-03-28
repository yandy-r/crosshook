use super::{db, MetadataStoreError};
use super::models::MAX_CACHE_PAYLOAD_BYTES;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

pub fn get_cache_entry(
    conn: &Connection,
    cache_key: &str,
) -> Result<Option<String>, MetadataStoreError> {
    let now = Utc::now().to_rfc3339();

    // Returns Option<Option<String>> — the outer Option is "row found", inner is nullable column.
    let row: Option<Option<String>> = conn
        .query_row(
            "SELECT payload_json FROM external_cache_entries \
             WHERE cache_key = ?1 AND (expires_at IS NULL OR expires_at > ?2)",
            params![cache_key, now],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|source| MetadataStoreError::Database {
            action: "query an external cache entry",
            source,
        })?;

    Ok(row.flatten())
}

pub fn put_cache_entry(
    conn: &Connection,
    source_url: &str,
    cache_key: &str,
    payload: &str,
    expires_at: Option<&str>,
) -> Result<(), MetadataStoreError> {
    let payload_size = payload.len();
    let payload_json: Option<&str> = if payload_size > MAX_CACHE_PAYLOAD_BYTES {
        tracing::warn!(
            cache_key = %cache_key,
            payload_size = %payload_size,
            max_bytes = %MAX_CACHE_PAYLOAD_BYTES,
            "cache payload exceeds size limit — storing NULL payload_json"
        );
        None
    } else {
        Some(payload)
    };

    let now = Utc::now().to_rfc3339();
    let cache_id = db::new_id();

    conn.execute(
        "INSERT INTO external_cache_entries (
            cache_id,
            source_url,
            cache_key,
            payload_json,
            payload_size,
            fetched_at,
            expires_at,
            created_at,
            updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ON CONFLICT(cache_key) DO UPDATE SET
            source_url = excluded.source_url,
            payload_json = excluded.payload_json,
            payload_size = excluded.payload_size,
            fetched_at = excluded.fetched_at,
            expires_at = excluded.expires_at,
            updated_at = excluded.updated_at",
        params![
            cache_id,
            source_url,
            cache_key,
            payload_json,
            payload_size as i64,
            now,
            expires_at,
            now,
            now,
        ],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "upsert an external cache entry",
        source,
    })?;

    Ok(())
}

pub fn evict_expired_cache_entries(conn: &Connection) -> Result<usize, MetadataStoreError> {
    let now = Utc::now().to_rfc3339();

    let rows_deleted = conn
        .execute(
            "DELETE FROM external_cache_entries \
             WHERE expires_at IS NOT NULL AND expires_at < ?1",
            params![now],
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "evict expired external cache entries",
            source,
        })?;

    Ok(rows_deleted)
}
