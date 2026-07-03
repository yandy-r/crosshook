//! Row-level CRUD for the `profile_mods` table (per-profile mod coexistence
//! registry). Input validation happens here — fail fast at the boundary.

use rusqlite::{params, Connection, ErrorCode, OptionalExtension};
use std::str::FromStr;

use super::models::MetadataStoreError;
use crate::mods::{ModCategory, ModProvenance, ProfileModInput, ProfileModRecord};

pub(super) const MAX_MOD_NAME_CHARS: usize = 200;
pub(super) const MAX_MOD_PATHS: usize = 32;
pub(super) const MAX_MOD_PATH_CHARS: usize = 1024;
pub(super) const MAX_MOD_PATHS_JSON_BYTES: usize = 8 * 1024;
pub(super) const MAX_MOD_SOURCE_URL_CHARS: usize = 2048;

const MOD_COLUMNS: &str = "mod_id, profile_id, name, category, paths_json, enabled, provenance, \
     source_url, created_at, updated_at";

/// Validates and normalizes a create/update payload: name trimmed, empty path
/// rows dropped, `source_url` trimmed with empty → `None`.
pub(super) fn validate_mod_input(
    input: &ProfileModInput,
) -> Result<ProfileModInput, MetadataStoreError> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(MetadataStoreError::Validation(
            "mod name is required".to_string(),
        ));
    }
    if name.chars().count() > MAX_MOD_NAME_CHARS {
        return Err(MetadataStoreError::Validation(format!(
            "mod name exceeds {MAX_MOD_NAME_CHARS} characters"
        )));
    }

    let paths: Vec<String> = input
        .paths
        .iter()
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
        .collect();
    if paths.len() > MAX_MOD_PATHS {
        return Err(MetadataStoreError::Validation(format!(
            "a mod may register at most {MAX_MOD_PATHS} paths"
        )));
    }
    for path in &paths {
        if path.contains('\0') {
            return Err(MetadataStoreError::Validation(
                "mod path may not contain NUL".to_string(),
            ));
        }
        if path.chars().count() > MAX_MOD_PATH_CHARS {
            return Err(MetadataStoreError::Validation(format!(
                "mod path exceeds {MAX_MOD_PATH_CHARS} characters"
            )));
        }
    }
    let paths_json = serialize_paths(&paths)?;
    if paths_json.len() > MAX_MOD_PATHS_JSON_BYTES {
        return Err(MetadataStoreError::Validation(
            "mod paths exceed the 8 KiB storage budget".to_string(),
        ));
    }

    let source_url = input
        .source_url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .map(str::to_string);
    if let Some(url) = &source_url {
        let lower = url.to_ascii_lowercase();
        if !lower.starts_with("http://") && !lower.starts_with("https://") {
            return Err(MetadataStoreError::Validation(
                "source_url must be an http(s) URL".to_string(),
            ));
        }
        if url.chars().count() > MAX_MOD_SOURCE_URL_CHARS {
            return Err(MetadataStoreError::Validation(format!(
                "source_url exceeds {MAX_MOD_SOURCE_URL_CHARS} characters"
            )));
        }
    }

    Ok(ProfileModInput {
        name,
        category: input.category,
        paths,
        enabled: input.enabled,
        source_url,
        provenance: input.provenance,
    })
}

pub(super) fn list_profile_mods(
    conn: &Connection,
    profile_id: &str,
) -> Result<Vec<ProfileModRecord>, MetadataStoreError> {
    query_mods(
        conn,
        profile_id,
        &format!("SELECT {MOD_COLUMNS} FROM profile_mods WHERE profile_id = ?1 ORDER BY name COLLATE NOCASE"),
        "list profile mods",
    )
}

pub(super) fn list_enabled_profile_mods(
    conn: &Connection,
    profile_id: &str,
) -> Result<Vec<ProfileModRecord>, MetadataStoreError> {
    query_mods(
        conn,
        profile_id,
        &format!("SELECT {MOD_COLUMNS} FROM profile_mods WHERE profile_id = ?1 AND enabled = 1 ORDER BY name COLLATE NOCASE"),
        "list enabled profile mods",
    )
}

pub(super) fn insert_profile_mod(
    conn: &Connection,
    profile_id: &str,
    input: &ProfileModInput,
    now: &str,
) -> Result<ProfileModRecord, MetadataStoreError> {
    let mod_id = super::db::new_id();
    let paths_json = serialize_paths(&input.paths)?;
    conn.execute(
        "INSERT INTO profile_mods \
         (mod_id, profile_id, name, category, paths_json, enabled, provenance, source_url, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
        params![
            mod_id,
            profile_id,
            input.name,
            input.category.as_str(),
            paths_json,
            input.enabled,
            input.provenance.as_str(),
            input.source_url,
            now,
        ],
    )
    .map_err(|source| map_write_error(source, &input.name, "insert profile_mods row"))?;

    Ok(ProfileModRecord {
        mod_id,
        profile_id: profile_id.to_string(),
        name: input.name.clone(),
        category: input.category,
        paths: input.paths.clone(),
        enabled: input.enabled,
        provenance: input.provenance,
        source_url: input.source_url.clone(),
        created_at: now.to_string(),
        updated_at: now.to_string(),
    })
}

pub(super) fn update_profile_mod(
    conn: &Connection,
    profile_id: &str,
    mod_id: &str,
    input: &ProfileModInput,
    now: &str,
) -> Result<ProfileModRecord, MetadataStoreError> {
    let paths_json = serialize_paths(&input.paths)?;
    let updated = conn
        .execute(
            "UPDATE profile_mods SET \
             name = ?1, category = ?2, paths_json = ?3, enabled = ?4, provenance = ?5, \
             source_url = ?6, updated_at = ?7 \
             WHERE profile_id = ?8 AND mod_id = ?9",
            params![
                input.name,
                input.category.as_str(),
                paths_json,
                input.enabled,
                input.provenance.as_str(),
                input.source_url,
                now,
                profile_id,
                mod_id,
            ],
        )
        .map_err(|source| map_write_error(source, &input.name, "update profile_mods row"))?;

    if updated == 0 {
        return Err(MetadataStoreError::Validation("mod not found".to_string()));
    }

    get_profile_mod(conn, profile_id, mod_id)?
        .ok_or_else(|| MetadataStoreError::Validation("mod not found".to_string()))
}

pub(super) fn delete_profile_mod(
    conn: &Connection,
    profile_id: &str,
    mod_id: &str,
) -> Result<bool, MetadataStoreError> {
    let deleted = conn
        .execute(
            "DELETE FROM profile_mods WHERE profile_id = ?1 AND mod_id = ?2",
            params![profile_id, mod_id],
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "delete profile_mods row",
            source,
        })?;
    Ok(deleted > 0)
}

fn get_profile_mod(
    conn: &Connection,
    profile_id: &str,
    mod_id: &str,
) -> Result<Option<ProfileModRecord>, MetadataStoreError> {
    let raw = conn
        .query_row(
            &format!(
                "SELECT {MOD_COLUMNS} FROM profile_mods WHERE profile_id = ?1 AND mod_id = ?2"
            ),
            params![profile_id, mod_id],
            raw_from_row,
        )
        .optional()
        .map_err(|source| MetadataStoreError::Database {
            action: "get profile_mods row",
            source,
        })?;
    raw.map(record_from_raw).transpose()
}

fn query_mods(
    conn: &Connection,
    profile_id: &str,
    sql: &str,
    action: &'static str,
) -> Result<Vec<ProfileModRecord>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(sql)
        .map_err(|source| MetadataStoreError::Database { action, source })?;
    let rows = stmt
        .query_map(params![profile_id], raw_from_row)
        .map_err(|source| MetadataStoreError::Database { action, source })?;

    let mut records = Vec::new();
    for raw in rows {
        let raw = raw.map_err(|source| MetadataStoreError::Database { action, source })?;
        records.push(record_from_raw(raw)?);
    }
    Ok(records)
}

struct RawModRow {
    mod_id: String,
    profile_id: String,
    name: String,
    category: String,
    paths_json: String,
    enabled: bool,
    provenance: String,
    source_url: Option<String>,
    created_at: String,
    updated_at: String,
}

fn raw_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawModRow> {
    Ok(RawModRow {
        mod_id: row.get(0)?,
        profile_id: row.get(1)?,
        name: row.get(2)?,
        category: row.get(3)?,
        paths_json: row.get(4)?,
        enabled: row.get(5)?,
        provenance: row.get(6)?,
        source_url: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn record_from_raw(raw: RawModRow) -> Result<ProfileModRecord, MetadataStoreError> {
    let paths: Vec<String> = serde_json::from_str(&raw.paths_json).map_err(|error| {
        MetadataStoreError::Corrupt(format!(
            "profile_mods.paths_json unreadable for mod {}: {error}",
            raw.mod_id
        ))
    })?;
    let category = ModCategory::from_str(&raw.category).map_err(|error| {
        MetadataStoreError::Corrupt(format!(
            "profile_mods.category unreadable for mod {}: {error}",
            raw.mod_id
        ))
    })?;
    let provenance = ModProvenance::from_str(&raw.provenance).map_err(|error| {
        MetadataStoreError::Corrupt(format!(
            "profile_mods.provenance unreadable for mod {}: {error}",
            raw.mod_id
        ))
    })?;

    Ok(ProfileModRecord {
        mod_id: raw.mod_id,
        profile_id: raw.profile_id,
        name: raw.name,
        category,
        paths,
        enabled: raw.enabled,
        provenance,
        source_url: raw.source_url,
        created_at: raw.created_at,
        updated_at: raw.updated_at,
    })
}

fn serialize_paths(paths: &[String]) -> Result<String, MetadataStoreError> {
    serde_json::to_string(paths).map_err(|error| {
        MetadataStoreError::Corrupt(format!("profile_mods paths failed to serialize: {error}"))
    })
}

fn map_write_error(
    source: rusqlite::Error,
    mod_name: &str,
    action: &'static str,
) -> MetadataStoreError {
    if let rusqlite::Error::SqliteFailure(err, _) = &source {
        if err.code == ErrorCode::ConstraintViolation {
            return MetadataStoreError::Validation(format!(
                "a mod named “{mod_name}” is already registered for this profile"
            ));
        }
    }
    MetadataStoreError::Database { action, source }
}
