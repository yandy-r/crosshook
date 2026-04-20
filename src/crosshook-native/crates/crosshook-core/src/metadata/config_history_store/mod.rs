use super::models::{
    ConfigRevisionRow, ConfigRevisionSource, MAX_CONFIG_REVISIONS_PER_PROFILE,
    MAX_SNAPSHOT_TOML_BYTES,
};
use super::MetadataStoreError;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

/// Insert a new config revision for the given profile, skipping the insert when
/// the content hash matches the latest recorded revision (dedup). Prunes older
/// rows beyond the retention limit in the same transaction. Returns the new row's
/// id when a row is inserted, or `None` when skipped due to dedup.
pub fn insert_config_revision(
    conn: &Connection,
    profile_id: &str,
    profile_name_at_write: &str,
    source: ConfigRevisionSource,
    content_hash: &str,
    snapshot_toml: &str,
    source_revision_id: Option<i64>,
) -> Result<Option<i64>, MetadataStoreError> {
    if snapshot_toml.len() > MAX_SNAPSHOT_TOML_BYTES {
        return Err(MetadataStoreError::Validation(format!(
            "snapshot_toml for profile '{profile_id}' exceeds the {MAX_SNAPSHOT_TOML_BYTES}-byte limit ({} bytes)",
            snapshot_toml.len()
        )));
    }

    let tx =
        Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(|source| {
            MetadataStoreError::Database {
                action: "start a config revision insert transaction",
                source,
            }
        })?;

    // Dedup: skip if the latest revision for this profile already has the same hash.
    let latest_hash: Option<String> = tx
        .query_row(
            "SELECT content_hash FROM config_revisions
             WHERE profile_id = ?1
             ORDER BY id DESC
             LIMIT 1",
            params![profile_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|source| MetadataStoreError::Database {
            action: "check latest config revision hash for dedup",
            source,
        })?;

    if latest_hash.as_deref() == Some(content_hash) {
        tx.commit().map_err(|source| MetadataStoreError::Database {
            action: "commit the config revision dedup check transaction",
            source,
        })?;
        return Ok(None);
    }

    // Validate that source_revision_id (if provided) belongs to the same profile.
    if let Some(src_rev_id) = source_revision_id {
        let owner: Option<String> = tx
            .query_row(
                "SELECT profile_id FROM config_revisions WHERE id = ?1",
                params![src_rev_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|source| MetadataStoreError::Database {
                action: "validate source_revision_id ownership",
                source,
            })?;
        match owner.as_deref() {
            Some(owner_pid) if owner_pid == profile_id => {}
            _ => {
                return Err(MetadataStoreError::Validation(format!(
                    "source_revision_id {src_rev_id} does not belong to profile {profile_id}"
                )));
            }
        }
    }

    let now = Utc::now().to_rfc3339();
    tx.execute(
        "INSERT INTO config_revisions
             (profile_id, profile_name_at_write, source, content_hash, snapshot_toml,
              source_revision_id, is_last_known_working, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
        params![
            profile_id,
            profile_name_at_write,
            source.as_str(),
            content_hash,
            snapshot_toml,
            source_revision_id,
            now,
        ],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "insert a config revision row",
        source,
    })?;

    let new_id = tx.last_insert_rowid();

    tx.execute(
        "DELETE FROM config_revisions
         WHERE profile_id = ?1
           AND id NOT IN (
               SELECT id FROM config_revisions
               WHERE profile_id = ?1
               ORDER BY id DESC
               LIMIT ?2
           )
           AND id NOT IN (
               SELECT source_revision_id FROM config_revisions
               WHERE profile_id = ?1
                 AND source_revision_id IS NOT NULL
           )",
        params![profile_id, MAX_CONFIG_REVISIONS_PER_PROFILE as i64],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "prune old config revision rows",
        source,
    })?;

    tx.commit().map_err(|source| MetadataStoreError::Database {
        action: "commit the config revision insert transaction",
        source,
    })?;

    Ok(Some(new_id))
}

/// List config revisions for a profile ordered by id DESC (newest first).
/// `limit` defaults to `MAX_CONFIG_REVISIONS_PER_PROFILE` when `None`.
pub fn list_config_revisions(
    conn: &Connection,
    profile_id: &str,
    limit: Option<usize>,
) -> Result<Vec<ConfigRevisionRow>, MetadataStoreError> {
    let row_limit = limit.unwrap_or(MAX_CONFIG_REVISIONS_PER_PROFILE) as i64;

    let mut stmt = conn
        .prepare(
            "SELECT id, profile_id, profile_name_at_write, source, content_hash, snapshot_toml,
                    source_revision_id, is_last_known_working, created_at
             FROM config_revisions
             WHERE profile_id = ?1
             ORDER BY id DESC
             LIMIT ?2",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare list config revisions query",
            source,
        })?;

    let rows = stmt
        .query_map(params![profile_id, row_limit], |row| {
            Ok(ConfigRevisionRow {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                profile_name_at_write: row.get(2)?,
                source: row.get(3)?,
                content_hash: row.get(4)?,
                snapshot_toml: row.get(5)?,
                source_revision_id: row.get(6)?,
                is_last_known_working: row.get::<_, i64>(7)? != 0,
                created_at: row.get(8)?,
            })
        })
        .map_err(|source| MetadataStoreError::Database {
            action: "query config revisions for profile",
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| MetadataStoreError::Database {
            action: "collect config revision rows",
            source,
        })?;

    Ok(rows)
}

/// Get a single config revision by id, scoped to `profile_id` to enforce ownership.
/// Returns `None` if the revision does not exist or belongs to a different profile.
pub fn get_config_revision(
    conn: &Connection,
    profile_id: &str,
    revision_id: i64,
) -> Result<Option<ConfigRevisionRow>, MetadataStoreError> {
    conn.query_row(
        "SELECT id, profile_id, profile_name_at_write, source, content_hash, snapshot_toml,
                source_revision_id, is_last_known_working, created_at
         FROM config_revisions
         WHERE id = ?1 AND profile_id = ?2",
        params![revision_id, profile_id],
        |row| {
            Ok(ConfigRevisionRow {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                profile_name_at_write: row.get(2)?,
                source: row.get(3)?,
                content_hash: row.get(4)?,
                snapshot_toml: row.get(5)?,
                source_revision_id: row.get(6)?,
                is_last_known_working: row.get::<_, i64>(7)? != 0,
                created_at: row.get(8)?,
            })
        },
    )
    .optional()
    .map_err(|source| MetadataStoreError::Database {
        action: "get a config revision by id",
        source,
    })
}

/// Mark the given revision as known-good for its profile. Clears the known-good
/// marker on all other revisions for the same profile in the same transaction to
/// enforce single-active-marker semantics. Returns an error if the revision is not
/// found for the given profile.
pub fn set_known_good_revision(
    conn: &Connection,
    profile_id: &str,
    revision_id: i64,
) -> Result<(), MetadataStoreError> {
    let tx =
        Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(|source| {
            MetadataStoreError::Database {
                action: "start a set-known-good transaction",
                source,
            }
        })?;

    tx.execute(
        "UPDATE config_revisions
         SET is_last_known_working = 0
         WHERE profile_id = ?1",
        params![profile_id],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "clear known-good markers for profile",
        source,
    })?;

    let updated = tx
        .execute(
            "UPDATE config_revisions
             SET is_last_known_working = 1
             WHERE id = ?1 AND profile_id = ?2",
            params![revision_id, profile_id],
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "set known-good marker on config revision",
            source,
        })?;

    if updated == 0 {
        return Err(MetadataStoreError::Corrupt(format!(
            "config revision {revision_id} not found for profile '{profile_id}' when setting known-good"
        )));
    }

    tx.commit().map_err(|source| MetadataStoreError::Database {
        action: "commit the set-known-good transaction",
        source,
    })?;

    Ok(())
}

/// Clear the known-good marker from all revisions for the given profile.
pub fn clear_known_good_revision(
    conn: &Connection,
    profile_id: &str,
) -> Result<(), MetadataStoreError> {
    conn.execute(
        "UPDATE config_revisions
         SET is_last_known_working = 0
         WHERE profile_id = ?1",
        params![profile_id],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "clear known-good markers for profile",
        source,
    })?;

    Ok(())
}

#[cfg(test)]
mod tests;
