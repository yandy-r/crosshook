//! Bundled launch optimization presets (catalog) and per-profile preset origin metadata.

use super::models::{BundledOptimizationPresetRow, MetadataStoreError, ProfileLaunchPresetOrigin};
use rusqlite::{params, Connection, OptionalExtension};

pub fn list_bundled_optimization_presets(
    conn: &Connection,
) -> Result<Vec<BundledOptimizationPresetRow>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT preset_id, display_name, vendor, mode, option_ids_json, catalog_version \
             FROM bundled_optimization_presets \
             ORDER BY vendor DESC, mode ASC, preset_id ASC",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare list_bundled_optimization_presets",
            source,
        })?;

    let rows = stmt
        .query_map([], |row| {
            Ok(BundledOptimizationPresetRow {
                preset_id: row.get(0)?,
                display_name: row.get(1)?,
                vendor: row.get(2)?,
                mode: row.get(3)?,
                option_ids_json: row.get(4)?,
                catalog_version: row.get(5)?,
            })
        })
        .map_err(|source| MetadataStoreError::Database {
            action: "execute list_bundled_optimization_presets",
            source,
        })?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|source| MetadataStoreError::Database {
            action: "read bundled_optimization_presets row",
            source,
        })?);
    }
    Ok(out)
}

pub fn get_bundled_optimization_preset(
    conn: &Connection,
    preset_id: &str,
) -> Result<Option<BundledOptimizationPresetRow>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT preset_id, display_name, vendor, mode, option_ids_json, catalog_version \
             FROM bundled_optimization_presets WHERE preset_id = ?1",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare get_bundled_optimization_preset",
            source,
        })?;

    let row = stmt
        .query_row(params![preset_id], |row| {
            Ok(BundledOptimizationPresetRow {
                preset_id: row.get(0)?,
                display_name: row.get(1)?,
                vendor: row.get(2)?,
                mode: row.get(3)?,
                option_ids_json: row.get(4)?,
                catalog_version: row.get(5)?,
            })
        })
        .optional()
        .map_err(|source| MetadataStoreError::Database {
            action: "query get_bundled_optimization_preset",
            source,
        })?;

    Ok(row)
}

/// Upserts metadata for a named launch preset on a profile (bundled or user-created).
pub fn upsert_profile_launch_preset_metadata(
    conn: &Connection,
    profile_id: &str,
    preset_name: &str,
    origin: ProfileLaunchPresetOrigin,
    source_bundled_preset_id: Option<&str>,
    now_rfc3339: &str,
) -> Result<(), MetadataStoreError> {
    conn.execute(
        "INSERT INTO profile_launch_preset_metadata \
         (profile_id, preset_name, origin, source_bundled_preset_id, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?5) \
         ON CONFLICT(profile_id, preset_name) DO UPDATE SET \
         origin = excluded.origin, \
         source_bundled_preset_id = excluded.source_bundled_preset_id, \
         updated_at = excluded.updated_at",
        params![
            profile_id,
            preset_name,
            origin.as_str(),
            source_bundled_preset_id,
            now_rfc3339,
        ],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "upsert profile_launch_preset_metadata",
        source,
    })?;

    Ok(())
}
