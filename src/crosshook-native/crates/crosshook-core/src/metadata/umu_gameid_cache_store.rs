use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

use crate::metadata::MetadataStoreError;

pub(crate) const MAX_UMU_GAMEID_LOOKUP_KEY_LEN: usize = 128;

/// A single row from the `umu_gameid_lookup_cache` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UmuGameIdCacheRow {
    pub store: String,
    pub codename: String,
    pub umu_id: Option<String>,
    pub status: String,
    pub payload_json: Option<String>,
    pub fetched_at: String,
    pub expires_at: Option<String>,
    pub last_error: Option<String>,
    pub updated_at: String,
}

impl UmuGameIdCacheRow {
    pub fn found(
        store: impl AsRef<str>,
        codename: impl AsRef<str>,
        umu_id: impl Into<String>,
        payload_json: Option<String>,
        fetched_at: impl Into<String>,
        expires_at: Option<String>,
    ) -> Result<Self, MetadataStoreError> {
        let umu_id = umu_id.into().trim().to_string();
        Self::new(
            store,
            codename,
            Some(umu_id),
            "found",
            payload_json,
            fetched_at,
            expires_at,
            None,
        )
    }

    pub fn missing(
        store: impl AsRef<str>,
        codename: impl AsRef<str>,
        payload_json: Option<String>,
        fetched_at: impl Into<String>,
        expires_at: Option<String>,
    ) -> Result<Self, MetadataStoreError> {
        Self::new(
            store,
            codename,
            None,
            "missing",
            payload_json,
            fetched_at,
            expires_at,
            None,
        )
    }

    pub fn error(
        store: impl AsRef<str>,
        codename: impl AsRef<str>,
        last_error: impl Into<String>,
        fetched_at: impl Into<String>,
        expires_at: Option<String>,
    ) -> Result<Self, MetadataStoreError> {
        Self::new(
            store,
            codename,
            None,
            "error",
            None,
            fetched_at,
            expires_at,
            Some(last_error.into()),
        )
    }

    fn new(
        store: impl AsRef<str>,
        codename: impl AsRef<str>,
        umu_id: Option<String>,
        status: impl Into<String>,
        payload_json: Option<String>,
        fetched_at: impl Into<String>,
        expires_at: Option<String>,
        last_error: Option<String>,
    ) -> Result<Self, MetadataStoreError> {
        let store = normalize_store(store.as_ref())?;
        let codename = normalize_codename(codename.as_ref())?;
        let status = status.into();
        validate_status(&status)?;
        if status == "found" {
            let Some(umu_id) = umu_id.as_deref() else {
                return Err(MetadataStoreError::Validation(
                    "umu GAMEID cache found row requires umu_id".to_string(),
                ));
            };
            validate_cache_umu_id(umu_id)?;
        }
        let updated_at = Utc::now().to_rfc3339();

        Ok(Self {
            store,
            codename,
            umu_id,
            status,
            payload_json,
            fetched_at: fetched_at.into(),
            expires_at,
            last_error,
            updated_at,
        })
    }
}

pub(crate) fn put_umu_gameid_cache_entry(
    conn: &Connection,
    row: &UmuGameIdCacheRow,
) -> Result<(), MetadataStoreError> {
    let store = normalize_store(&row.store)?;
    let codename = normalize_codename(&row.codename)?;
    validate_status(&row.status)?;
    let updated_at = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO umu_gameid_lookup_cache (
            store,
            codename,
            umu_id,
            status,
            payload_json,
            fetched_at,
            expires_at,
            last_error,
            updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ON CONFLICT(store, codename) DO UPDATE SET
            umu_id = excluded.umu_id,
            status = excluded.status,
            payload_json = excluded.payload_json,
            fetched_at = excluded.fetched_at,
            expires_at = excluded.expires_at,
            last_error = excluded.last_error,
            updated_at = excluded.updated_at",
        params![
            store,
            codename,
            row.umu_id,
            row.status,
            row.payload_json,
            row.fetched_at,
            row.expires_at,
            row.last_error,
            updated_at,
        ],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "upsert umu GAMEID cache entry",
        source,
    })?;

    Ok(())
}

pub(crate) fn get_umu_gameid_cache_entry(
    conn: &Connection,
    store: &str,
    codename: &str,
) -> Result<Option<UmuGameIdCacheRow>, MetadataStoreError> {
    let store = normalize_store(store)?;
    let codename = normalize_codename(codename)?;
    let now = Utc::now().to_rfc3339();

    query_umu_gameid_cache_row(
        conn,
        "SELECT store, codename, umu_id, status, payload_json, fetched_at,
                expires_at, last_error, updated_at
         FROM umu_gameid_lookup_cache
         WHERE store = ?1
           AND codename = ?2
           AND status IN ('found', 'missing')
           AND (expires_at IS NULL OR expires_at > ?3)",
        params![store, codename, now],
        "query fresh umu GAMEID cache entry",
    )
}

pub(crate) fn get_stale_umu_gameid_cache_entry(
    conn: &Connection,
    store: &str,
    codename: &str,
) -> Result<Option<UmuGameIdCacheRow>, MetadataStoreError> {
    let store = normalize_store(store)?;
    let codename = normalize_codename(codename)?;
    let now = Utc::now().to_rfc3339();

    query_umu_gameid_cache_row(
        conn,
        "SELECT store, codename, umu_id, status, payload_json, fetched_at,
                expires_at, last_error, updated_at
         FROM umu_gameid_lookup_cache
         WHERE store = ?1
           AND codename = ?2
           AND expires_at IS NOT NULL
           AND expires_at <= ?3
           AND status = 'found'
           AND umu_id IS NOT NULL",
        params![store, codename, now],
        "query stale umu GAMEID cache entry",
    )
}

pub(crate) fn clear_umu_gameid_cache(conn: &Connection) -> Result<usize, MetadataStoreError> {
    conn.execute("DELETE FROM umu_gameid_lookup_cache", [])
        .map_err(|source| MetadataStoreError::Database {
            action: "clear umu GAMEID cache",
            source,
        })
}

fn query_umu_gameid_cache_row<P>(
    conn: &Connection,
    sql: &str,
    params: P,
    action: &'static str,
) -> Result<Option<UmuGameIdCacheRow>, MetadataStoreError>
where
    P: rusqlite::Params,
{
    conn.query_row(sql, params, |row| {
        Ok(UmuGameIdCacheRow {
            store: row.get(0)?,
            codename: row.get(1)?,
            umu_id: row.get(2)?,
            status: row.get(3)?,
            payload_json: row.get(4)?,
            fetched_at: row.get(5)?,
            expires_at: row.get(6)?,
            last_error: row.get(7)?,
            updated_at: row.get(8)?,
        })
    })
    .optional()
    .map_err(|source| MetadataStoreError::Database { action, source })
}

pub(crate) fn normalize_umu_gameid_store(store: &str) -> Result<String, MetadataStoreError> {
    let normalized = store.trim().to_ascii_lowercase();
    validate_umu_gameid_lookup_part(&normalized, "store")?;
    Ok(normalized)
}

pub(crate) fn normalize_umu_gameid_codename(codename: &str) -> Result<String, MetadataStoreError> {
    let normalized = codename.trim().to_string();
    validate_umu_gameid_lookup_part(&normalized, "codename")?;
    Ok(normalized)
}

fn validate_umu_gameid_lookup_part(
    value: &str,
    label: &'static str,
) -> Result<(), MetadataStoreError> {
    if value.is_empty() {
        return Err(MetadataStoreError::Validation(format!(
            "umu GAMEID cache {label} cannot be empty"
        )));
    }
    if value.len() > MAX_UMU_GAMEID_LOOKUP_KEY_LEN {
        return Err(MetadataStoreError::Validation(format!(
            "umu GAMEID cache {label} exceeds {MAX_UMU_GAMEID_LOOKUP_KEY_LEN} bytes"
        )));
    }
    if value.chars().any(char::is_control) {
        return Err(MetadataStoreError::Validation(format!(
            "umu GAMEID cache {label} contains control characters"
        )));
    }
    Ok(())
}

fn validate_cache_umu_id(umu_id: &str) -> Result<(), MetadataStoreError> {
    let trimmed = umu_id.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_UMU_GAMEID_LOOKUP_KEY_LEN {
        return Err(MetadataStoreError::Validation(
            "umu GAMEID cache umu_id length is invalid".to_string(),
        ));
    }
    if !trimmed
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
    {
        return Err(MetadataStoreError::Validation(
            "umu GAMEID cache umu_id contains unsupported characters".to_string(),
        ));
    }
    Ok(())
}

fn normalize_store(store: &str) -> Result<String, MetadataStoreError> {
    normalize_umu_gameid_store(store)
}

fn normalize_codename(codename: &str) -> Result<String, MetadataStoreError> {
    normalize_umu_gameid_codename(codename)
}

fn validate_status(status: &str) -> Result<(), MetadataStoreError> {
    match status {
        "found" | "missing" | "error" => Ok(()),
        other => Err(MetadataStoreError::Validation(format!(
            "unsupported umu GAMEID cache status '{other}'"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::db;
    use crate::metadata::migrations::run_migrations;

    fn open_v24() -> Connection {
        let conn = db::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn umu_gameid_cache_round_trip_normalizes_key() {
        let conn = open_v24();
        let row = UmuGameIdCacheRow::found(
            " GOG ",
            " cyberpunk_2077 ",
            "UMU-12345",
            Some(r#"{"umu_id":"UMU-12345"}"#.to_string()),
            "2026-06-07T00:00:00Z",
            Some("2099-01-01T00:00:00Z".to_string()),
        )
        .unwrap();

        put_umu_gameid_cache_entry(&conn, &row).unwrap();

        let fetched = get_umu_gameid_cache_entry(&conn, "gog", "cyberpunk_2077")
            .unwrap()
            .unwrap();
        assert_eq!(fetched.store, "gog");
        assert_eq!(fetched.codename, "cyberpunk_2077");
        assert_eq!(fetched.umu_id.as_deref(), Some("UMU-12345"));
        assert_eq!(fetched.status, "found");
    }

    #[test]
    fn found_row_rejects_invalid_umu_id() {
        let row = UmuGameIdCacheRow::found(
            "gog",
            "game",
            "bad/id",
            None,
            "2026-06-07T00:00:00Z",
            Some("2099-01-01T00:00:00Z".to_string()),
        );

        assert!(
            matches!(row, Err(MetadataStoreError::Validation(_))),
            "expected invalid umu_id to be rejected, got {row:?}"
        );
    }

    #[test]
    fn umu_gameid_cache_distinguishes_fresh_and_stale_hit() {
        let conn = open_v24();
        let row = UmuGameIdCacheRow::found(
            "gog",
            "old_game",
            "UMU-OLD",
            None,
            "2026-06-01T00:00:00Z",
            Some("2026-06-02T00:00:00Z".to_string()),
        )
        .unwrap();

        put_umu_gameid_cache_entry(&conn, &row).unwrap();

        assert!(
            get_umu_gameid_cache_entry(&conn, "gog", "old_game")
                .unwrap()
                .is_none(),
            "expired rows should not appear as fresh cache entries"
        );
        let stale = get_stale_umu_gameid_cache_entry(&conn, "gog", "old_game")
            .unwrap()
            .unwrap();
        assert_eq!(stale.umu_id.as_deref(), Some("UMU-OLD"));
    }

    #[test]
    fn umu_gameid_cache_upsert_replaces_status() {
        let conn = open_v24();
        let missing = UmuGameIdCacheRow::missing(
            "gog",
            "unknown",
            Some("[]".to_string()),
            "2026-06-01T00:00:00Z",
            Some("2099-01-01T00:00:00Z".to_string()),
        )
        .unwrap();
        let found = UmuGameIdCacheRow::found(
            "gog",
            "unknown",
            "UMU-999",
            Some(r#"{"umu_id":"UMU-999"}"#.to_string()),
            "2026-06-02T00:00:00Z",
            Some("2099-01-01T00:00:00Z".to_string()),
        )
        .unwrap();

        put_umu_gameid_cache_entry(&conn, &missing).unwrap();
        put_umu_gameid_cache_entry(&conn, &found).unwrap();

        let fetched = get_umu_gameid_cache_entry(&conn, "gog", "unknown")
            .unwrap()
            .unwrap();
        assert_eq!(fetched.status, "found");
        assert_eq!(fetched.umu_id.as_deref(), Some("UMU-999"));

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM umu_gameid_lookup_cache WHERE store = 'gog' AND codename = 'unknown'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn umu_gameid_cache_clear_deletes_only_cache_rows() {
        let conn = open_v24();
        let row = UmuGameIdCacheRow::error(
            "gog",
            "broken",
            "timeout",
            "2026-06-01T00:00:00Z",
            Some("2099-01-01T00:00:00Z".to_string()),
        )
        .unwrap();
        put_umu_gameid_cache_entry(&conn, &row).unwrap();

        let deleted = clear_umu_gameid_cache(&conn).unwrap();
        assert_eq!(deleted, 1);
        assert!(get_umu_gameid_cache_entry(&conn, "gog", "broken")
            .unwrap()
            .is_none());
    }

    #[test]
    fn umu_gameid_cache_fresh_lookup_excludes_error_rows() {
        let conn = open_v24();
        let row = UmuGameIdCacheRow::error(
            "gog",
            "temporary-outage",
            "timeout",
            "2026-06-01T00:00:00Z",
            Some("2099-01-01T00:00:00Z".to_string()),
        )
        .unwrap();

        put_umu_gameid_cache_entry(&conn, &row).unwrap();

        assert!(
            get_umu_gameid_cache_entry(&conn, "gog", "temporary-outage")
                .unwrap()
                .is_none(),
            "transient lookup errors must not suppress fresh retries"
        );
        assert!(
            get_stale_umu_gameid_cache_entry(&conn, "gog", "temporary-outage")
                .unwrap()
                .is_none(),
            "error rows are diagnostics only and must not be stale fallbacks"
        );
    }

    #[test]
    fn umu_gameid_cache_rejects_oversized_and_control_keys() {
        let oversized = "a".repeat(MAX_UMU_GAMEID_LOOKUP_KEY_LEN + 1);
        assert!(
            UmuGameIdCacheRow::missing(oversized, "game", None, "2026-06-01T00:00:00Z", None,)
                .is_err()
        );
        assert!(
            UmuGameIdCacheRow::missing("gog", "bad\nname", None, "2026-06-01T00:00:00Z", None,)
                .is_err()
        );
    }
}
