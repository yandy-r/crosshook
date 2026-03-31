use rusqlite::Connection;

use super::MetadataStoreError;
use crate::launch::catalog::OptimizationEntry;

/// Persists the full optimization catalog to the database.
///
/// Uses a transactional DELETE + INSERT to replace all rows atomically.
pub fn persist_optimization_catalog(
    conn: &mut Connection,
    entries: &[OptimizationEntry],
    catalog_version: u32,
) -> Result<(), MetadataStoreError> {
    let tx = conn.transaction().map_err(|source| MetadataStoreError::Database {
        action: "begin optimization catalog transaction",
        source,
    })?;

    tx.execute("DELETE FROM optimization_catalog", [])
        .map_err(|source| MetadataStoreError::Database {
            action: "clear optimization catalog",
            source,
        })?;

    let now = chrono::Utc::now().to_rfc3339();

    for (index, entry) in entries.iter().enumerate() {
        let env_json = serde_json::to_string(&entry.env).unwrap_or_else(|_| "[]".to_string());
        let wrappers_json =
            serde_json::to_string(&entry.wrappers).unwrap_or_else(|_| "[]".to_string());
        let conflicts_with_json =
            serde_json::to_string(&entry.conflicts_with).unwrap_or_else(|_| "[]".to_string());
        let applicable_methods_json =
            serde_json::to_string(&entry.applicable_methods).unwrap_or_else(|_| "[]".to_string());

        let source = if entry.community { "community" } else { "default" };

        tx.execute(
            "INSERT INTO optimization_catalog (
                sort_order, id, applies_to_method, env_json, wrappers_json,
                conflicts_with_json, required_binary, label, description,
                help_text, category, target_gpu_vendor, advanced, community,
                applicable_methods_json, source, catalog_version, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            rusqlite::params![
                index as i64,
                entry.id,
                entry.applies_to_method,
                env_json,
                wrappers_json,
                conflicts_with_json,
                entry.required_binary,
                entry.label,
                entry.description,
                entry.help_text,
                entry.category,
                entry.target_gpu_vendor,
                entry.advanced as i64,
                entry.community as i64,
                applicable_methods_json,
                source,
                catalog_version as i64,
                now,
            ],
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "insert optimization catalog entry",
            source,
        })?;
    }

    tx.commit().map_err(|source| MetadataStoreError::Database {
        action: "commit optimization catalog transaction",
        source,
    })?;

    Ok(())
}
