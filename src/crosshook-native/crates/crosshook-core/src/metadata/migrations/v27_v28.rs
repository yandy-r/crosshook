use super::MetadataStoreError;
use rusqlite::Connection;

/// Adds nullable `config_revisions.hmac_signature` for config-history tamper
/// evidence (ADR-0005). Existing rows keep `NULL` and verify hash-only.
pub(super) fn migrate_27_to_28(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch("ALTER TABLE config_revisions ADD COLUMN hmac_signature TEXT;")
        .map_err(|source| MetadataStoreError::Database {
            action: "run metadata migration 27 to 28",
            source,
        })?;

    Ok(())
}
