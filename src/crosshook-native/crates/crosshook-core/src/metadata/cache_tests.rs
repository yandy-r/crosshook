#![cfg(test)]

use rusqlite::params;

use super::test_support::connection;
use super::{MetadataStore, MAX_CACHE_PAYLOAD_BYTES};

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
