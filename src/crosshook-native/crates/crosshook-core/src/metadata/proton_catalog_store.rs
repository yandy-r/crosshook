use rusqlite::Connection;

use crate::metadata::MetadataStoreError;

/// A single row from the `proton_release_catalog` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtonCatalogRow {
    pub provider_id: String,
    pub version_tag: String,
    pub payload_json: String,
    pub release_url: Option<String>,
    pub download_url: Option<String>,
    pub checksum_url: Option<String>,
    pub checksum_kind: Option<String>,
    pub asset_size: Option<i64>,
    /// ISO-8601 timestamp.
    pub fetched_at: String,
    pub expires_at: Option<String>,
}

/// Batch-upsert rows into `proton_release_catalog`.
///
/// Uses `INSERT OR REPLACE` so repeated fetches for the same `(provider_id, version_tag)` key
/// update the cached payload in place.
pub fn put_proton_catalog_impl(
    conn: &mut Connection,
    rows: &[ProtonCatalogRow],
) -> Result<(), MetadataStoreError> {
    let tx = conn
        .transaction()
        .map_err(|source| MetadataStoreError::Database {
            action: "begin proton catalog upsert transaction",
            source,
        })?;

    for row in rows {
        tx.execute(
            "INSERT OR REPLACE INTO proton_release_catalog (
                provider_id, version_tag, payload_json, release_url, download_url,
                checksum_url, checksum_kind, asset_size, fetched_at, expires_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                row.provider_id,
                row.version_tag,
                row.payload_json,
                row.release_url,
                row.download_url,
                row.checksum_url,
                row.checksum_kind,
                row.asset_size,
                row.fetched_at,
                row.expires_at,
            ],
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "upsert proton catalog row",
            source,
        })?;
    }

    tx.commit().map_err(|source| MetadataStoreError::Database {
        action: "commit proton catalog upsert transaction",
        source,
    })?;

    Ok(())
}

/// Return all cached rows for `provider_id`, ordered by `fetched_at` descending.
pub fn get_proton_catalog_impl(
    conn: &Connection,
    provider_id: &str,
) -> Result<Vec<ProtonCatalogRow>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT provider_id, version_tag, payload_json, release_url, download_url,
                    checksum_url, checksum_kind, asset_size, fetched_at, expires_at
             FROM proton_release_catalog
             WHERE provider_id = ?1
             ORDER BY fetched_at DESC",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare get proton catalog query",
            source,
        })?;

    let rows = stmt
        .query_map([provider_id], |row| {
            Ok(ProtonCatalogRow {
                provider_id: row.get(0)?,
                version_tag: row.get(1)?,
                payload_json: row.get(2)?,
                release_url: row.get(3)?,
                download_url: row.get(4)?,
                checksum_url: row.get(5)?,
                checksum_kind: row.get(6)?,
                asset_size: row.get(7)?,
                fetched_at: row.get(8)?,
                expires_at: row.get(9)?,
            })
        })
        .map_err(|source| MetadataStoreError::Database {
            action: "query proton catalog rows",
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| MetadataStoreError::Database {
            action: "decode proton catalog row",
            source,
        })?;

    Ok(rows)
}

/// Delete all cached rows for `provider_id` (used to evict stale entries before a refresh).
pub fn clear_proton_catalog_impl(
    conn: &mut Connection,
    provider_id: &str,
) -> Result<(), MetadataStoreError> {
    conn.execute(
        "DELETE FROM proton_release_catalog WHERE provider_id = ?1",
        [provider_id],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "clear proton catalog for provider",
        source,
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::db;
    use crate::metadata::migrations::run_migrations;

    fn open_v22() -> Connection {
        let conn = db::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn proton_catalog_round_trip() {
        let mut conn = open_v22();

        let rows = vec![
            ProtonCatalogRow {
                provider_id: "ge-proton".to_string(),
                version_tag: "GE-Proton9-1".to_string(),
                payload_json: r#"{"tag":"GE-Proton9-1"}"#.to_string(),
                release_url: Some("https://example.com/release/9-1".to_string()),
                download_url: Some("https://example.com/GE-Proton9-1.tar.gz".to_string()),
                checksum_url: Some("https://example.com/GE-Proton9-1.sha512sum".to_string()),
                checksum_kind: Some("sha512".to_string()),
                asset_size: Some(512_000_000),
                fetched_at: "2026-04-17T00:00:00Z".to_string(),
                expires_at: Some("2026-04-18T00:00:00Z".to_string()),
            },
            ProtonCatalogRow {
                provider_id: "ge-proton".to_string(),
                version_tag: "GE-Proton9-2".to_string(),
                payload_json: r#"{"tag":"GE-Proton9-2"}"#.to_string(),
                release_url: None,
                download_url: None,
                checksum_url: None,
                checksum_kind: None,
                asset_size: None,
                fetched_at: "2026-04-17T01:00:00Z".to_string(),
                expires_at: None,
            },
        ];

        put_proton_catalog_impl(&mut conn, &rows).unwrap();

        let fetched = get_proton_catalog_impl(&conn, "ge-proton").unwrap();
        assert_eq!(fetched.len(), 2);

        // Ordered by fetched_at DESC — GE-Proton9-2 comes first.
        assert_eq!(fetched[0].version_tag, "GE-Proton9-2");
        assert_eq!(fetched[1].version_tag, "GE-Proton9-1");
        assert_eq!(fetched[1].asset_size, Some(512_000_000));
        assert_eq!(fetched[0].download_url, None);

        // Unknown provider returns empty.
        let empty = get_proton_catalog_impl(&conn, "unknown-provider").unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn proton_catalog_clear_evicts_provider() {
        let mut conn = open_v22();

        let row = ProtonCatalogRow {
            provider_id: "proton-tkg".to_string(),
            version_tag: "proton-tkg-1".to_string(),
            payload_json: "{}".to_string(),
            release_url: None,
            download_url: None,
            checksum_url: None,
            checksum_kind: None,
            asset_size: None,
            fetched_at: "2026-04-17T00:00:00Z".to_string(),
            expires_at: None,
        };
        put_proton_catalog_impl(&mut conn, &[row]).unwrap();

        let before = get_proton_catalog_impl(&conn, "proton-tkg").unwrap();
        assert_eq!(before.len(), 1);

        clear_proton_catalog_impl(&mut conn, "proton-tkg").unwrap();

        let after = get_proton_catalog_impl(&conn, "proton-tkg").unwrap();
        assert!(
            after.is_empty(),
            "clear should evict all rows for the provider"
        );
    }

    #[test]
    fn proton_catalog_upsert_replaces_existing() {
        let mut conn = open_v22();

        let original = ProtonCatalogRow {
            provider_id: "ge-proton".to_string(),
            version_tag: "GE-Proton9-1".to_string(),
            payload_json: r#"{"original":true}"#.to_string(),
            release_url: None,
            download_url: None,
            checksum_url: None,
            checksum_kind: None,
            asset_size: None,
            fetched_at: "2026-04-17T00:00:00Z".to_string(),
            expires_at: None,
        };
        put_proton_catalog_impl(&mut conn, &[original]).unwrap();

        let updated = ProtonCatalogRow {
            provider_id: "ge-proton".to_string(),
            version_tag: "GE-Proton9-1".to_string(),
            payload_json: r#"{"updated":true}"#.to_string(),
            release_url: Some("https://example.com".to_string()),
            download_url: None,
            checksum_url: None,
            checksum_kind: None,
            asset_size: Some(1),
            fetched_at: "2026-04-17T02:00:00Z".to_string(),
            expires_at: None,
        };
        put_proton_catalog_impl(&mut conn, &[updated]).unwrap();

        let rows = get_proton_catalog_impl(&conn, "ge-proton").unwrap();
        assert_eq!(rows.len(), 1, "upsert must not duplicate the row");
        assert_eq!(rows[0].payload_json, r#"{"updated":true}"#);
        assert_eq!(rows[0].asset_size, Some(1));
    }
}
