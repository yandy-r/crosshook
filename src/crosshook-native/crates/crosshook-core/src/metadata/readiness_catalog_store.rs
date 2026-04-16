use rusqlite::Connection;

use super::MetadataStoreError;
use crate::onboarding::HostToolEntry;

/// Persists the merged host readiness catalog to SQLite (full snapshot).
pub fn persist_readiness_catalog(
    conn: &mut Connection,
    entries: &[HostToolEntry],
    catalog_version: u32,
) -> Result<(), MetadataStoreError> {
    let tx = conn
        .transaction()
        .map_err(|source| MetadataStoreError::Database {
            action: "begin host readiness catalog transaction",
            source,
        })?;

    tx.execute("DELETE FROM host_readiness_catalog", [])
        .map_err(|source| MetadataStoreError::Database {
            action: "clear host readiness catalog",
            source,
        })?;

    let now = chrono::Utc::now().to_rfc3339();

    for entry in entries {
        let install_json =
            serde_json::to_string(&entry.install_commands).unwrap_or_else(|_| "[]".to_string());

        tx.execute(
            "INSERT INTO host_readiness_catalog (
                tool_id, binary_name, display_name, description, docs_url,
                required, category, install_commands_json, source, catalog_version, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                entry.tool_id,
                entry.binary_name,
                entry.display_name,
                entry.description,
                entry.docs_url,
                entry.required as i64,
                entry.category,
                install_json,
                "default",
                catalog_version as i64,
                now,
            ],
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "insert host readiness catalog entry",
            source,
        })?;
    }

    tx.commit().map_err(|source| MetadataStoreError::Database {
        action: "commit host readiness catalog transaction",
        source,
    })?;

    Ok(())
}
