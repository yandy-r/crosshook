use super::models::CollectionRow;
use super::profile_sync::lookup_profile_id;
use super::{db, MetadataStoreError};
use crate::profile::CollectionDefaultsSection;
use chrono::Utc;
use rusqlite::{params, Connection};

pub fn list_collections(conn: &Connection) -> Result<Vec<CollectionRow>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT c.collection_id, c.name, c.description, c.created_at, c.updated_at, \
             (SELECT COUNT(*) FROM collection_profiles cp WHERE cp.collection_id = c.collection_id) as profile_count \
             FROM collections c ORDER BY c.sort_order ASC, c.name ASC",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare list_collections query",
            source,
        })?;

    let rows = stmt
        .query_map([], |row| {
            Ok(CollectionRow {
                collection_id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                profile_count: row.get(5)?,
            })
        })
        .map_err(|source| MetadataStoreError::Database {
            action: "query collections",
            source,
        })?;

    let mut collections = Vec::new();
    for row in rows {
        collections.push(row.map_err(|source| MetadataStoreError::Database {
            action: "read a collection row",
            source,
        })?);
    }

    Ok(collections)
}

pub fn create_collection(conn: &Connection, name: &str) -> Result<String, MetadataStoreError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(MetadataStoreError::Validation(
            "collection name must not be empty".to_string(),
        ));
    }

    let collection_id = db::new_id();
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO collections (collection_id, name, description, created_at, updated_at) \
         VALUES (?1, ?2, NULL, ?3, ?4)",
        params![collection_id, trimmed, now, now],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "insert a new collection",
        source,
    })?;

    Ok(collection_id)
}

pub fn delete_collection(conn: &Connection, collection_id: &str) -> Result<(), MetadataStoreError> {
    conn.execute(
        "DELETE FROM collections WHERE collection_id = ?1",
        params![collection_id],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "delete a collection",
        source,
    })?;

    Ok(())
}

pub fn add_profile_to_collection(
    conn: &Connection,
    collection_id: &str,
    profile_name: &str,
) -> Result<(), MetadataStoreError> {
    let trimmed = profile_name.trim();
    let profile_id = lookup_profile_id(conn, trimmed)?.ok_or_else(|| {
        MetadataStoreError::Validation(format!(
            "profile not found when adding to collection: {trimmed}"
        ))
    })?;

    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR IGNORE INTO collection_profiles (collection_id, profile_id, added_at) \
         VALUES (?1, ?2, ?3)",
        params![collection_id, profile_id, now],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "add a profile to a collection",
        source,
    })?;

    Ok(())
}

pub fn remove_profile_from_collection(
    conn: &Connection,
    collection_id: &str,
    profile_name: &str,
) -> Result<(), MetadataStoreError> {
    let profile_id = lookup_profile_id(conn, profile_name)?;
    let Some(profile_id) = profile_id else {
        return Ok(());
    };

    conn.execute(
        "DELETE FROM collection_profiles WHERE collection_id = ?1 AND profile_id = ?2",
        params![collection_id, profile_id],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "remove a profile from a collection",
        source,
    })?;

    Ok(())
}

pub fn list_profiles_in_collection(
    conn: &Connection,
    collection_id: &str,
) -> Result<Vec<String>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT p.current_filename \
             FROM collection_profiles cp \
             JOIN profiles p ON cp.profile_id = p.profile_id \
             WHERE cp.collection_id = ?1 AND p.deleted_at IS NULL \
             ORDER BY p.current_filename",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare list_profiles_in_collection query",
            source,
        })?;

    let names = stmt
        .query_map(params![collection_id], |row| row.get::<_, String>(0))
        .map_err(|source| MetadataStoreError::Database {
            action: "query profiles in collection",
            source,
        })?;

    let mut result = Vec::new();
    for name in names {
        result.push(name.map_err(|source| MetadataStoreError::Database {
            action: "read a profile name from collection query",
            source,
        })?);
    }

    Ok(result)
}

pub fn set_profile_favorite(
    conn: &Connection,
    profile_name: &str,
    favorite: bool,
) -> Result<(), MetadataStoreError> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE profiles SET is_favorite = ?1, updated_at = ?2 \
         WHERE current_filename = ?3 AND deleted_at IS NULL",
        params![favorite as i32, now, profile_name],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "update profile favorite flag",
        source,
    })?;

    Ok(())
}

pub fn list_favorite_profiles(conn: &Connection) -> Result<Vec<String>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT current_filename FROM profiles \
             WHERE is_favorite = 1 AND deleted_at IS NULL \
             ORDER BY current_filename",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare list_favorite_profiles query",
            source,
        })?;

    let names = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|source| MetadataStoreError::Database {
            action: "query favorite profiles",
            source,
        })?;

    let mut result = Vec::new();
    for name in names {
        result.push(name.map_err(|source| MetadataStoreError::Database {
            action: "read a favorite profile name",
            source,
        })?);
    }

    Ok(result)
}

pub fn rename_collection(
    conn: &Connection,
    collection_id: &str,
    new_name: &str,
) -> Result<(), MetadataStoreError> {
    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        return Err(MetadataStoreError::Validation(
            "collection name must not be empty".to_string(),
        ));
    }

    let now = Utc::now().to_rfc3339();
    let affected = conn
        .execute(
            "UPDATE collections SET name = ?1, updated_at = ?2 WHERE collection_id = ?3",
            params![trimmed, now, collection_id],
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "rename a collection",
            source,
        })?;

    if affected == 0 {
        return Err(MetadataStoreError::Validation(format!(
            "collection not found: {collection_id}"
        )));
    }

    Ok(())
}

pub fn update_collection_description(
    conn: &Connection,
    collection_id: &str,
    description: Option<&str>,
) -> Result<(), MetadataStoreError> {
    let normalized: Option<String> = description
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let now = Utc::now().to_rfc3339();
    let affected = conn
        .execute(
            "UPDATE collections SET description = ?1, updated_at = ?2 WHERE collection_id = ?3",
            params![normalized, now, collection_id],
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "update a collection description",
            source,
        })?;

    if affected == 0 {
        return Err(MetadataStoreError::Validation(format!(
            "collection not found: {collection_id}"
        )));
    }

    Ok(())
}

pub fn collections_for_profile(
    conn: &Connection,
    profile_name: &str,
) -> Result<Vec<CollectionRow>, MetadataStoreError> {
    let trimmed = profile_name.trim();
    let profile_id = match lookup_profile_id(conn, trimmed)? {
        Some(id) => id,
        None => return Ok(Vec::new()),
    };

    let mut stmt = conn
        .prepare(
            "SELECT c.collection_id, c.name, c.description, c.created_at, c.updated_at, \
             (SELECT COUNT(*) FROM collection_profiles cp2 WHERE cp2.collection_id = c.collection_id) as profile_count \
             FROM collections c \
             INNER JOIN collection_profiles cp ON cp.collection_id = c.collection_id \
             WHERE cp.profile_id = ?1 \
             ORDER BY c.sort_order ASC, c.name ASC",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare collections_for_profile query",
            source,
        })?;

    let rows = stmt
        .query_map(params![profile_id], |row| {
            Ok(CollectionRow {
                collection_id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                profile_count: row.get(5)?,
            })
        })
        .map_err(|source| MetadataStoreError::Database {
            action: "query collections for profile",
            source,
        })?;

    let mut collections = Vec::new();
    for row in rows {
        collections.push(row.map_err(|source| MetadataStoreError::Database {
            action: "read a collection row in collections_for_profile",
            source,
        })?);
    }

    Ok(collections)
}

/// Read the per-collection launch defaults stored as inline JSON in
/// `collections.defaults_json`. Returns `Ok(None)` when the collection has no defaults
/// (column is `NULL` or empty), `Ok(Some(_))` on a successful parse, or:
///
/// - `Err(Database)` when the collection row does not exist (caller surfaces
///   "collection not found" via the `QueryReturnedNoRows` source).
/// - `Err(Corrupt)` when the column contains invalid JSON. The raw bytes remain
///   on disk so the user can clear them by saving fresh defaults.
pub fn get_collection_defaults(
    conn: &Connection,
    collection_id: &str,
) -> Result<Option<CollectionDefaultsSection>, MetadataStoreError> {
    let json: Option<String> = conn
        .query_row(
            "SELECT defaults_json FROM collections WHERE collection_id = ?1",
            params![collection_id],
            |row| row.get(0),
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "read collection defaults",
            source,
        })?;

    let Some(json) = json else {
        return Ok(None);
    };
    if json.trim().is_empty() {
        return Ok(None);
    }

    let parsed: CollectionDefaultsSection = serde_json::from_str(&json).map_err(|e| {
        MetadataStoreError::Corrupt(format!(
            "corrupt collection defaults JSON for {collection_id}: {e}"
        ))
    })?;
    Ok(Some(parsed))
}

/// Write per-collection launch defaults into `collections.defaults_json`.
///
/// Passing `None` or an effectively-empty defaults struct (all fields cleared)
/// normalizes to a `NULL` column write so empty state never round-trips as `{}`.
/// `updated_at` is refreshed to give sidebar/UI cache layers an invalidation signal.
pub fn set_collection_defaults(
    conn: &Connection,
    collection_id: &str,
    defaults: Option<&CollectionDefaultsSection>,
) -> Result<(), MetadataStoreError> {
    let json: Option<String> = match defaults {
        Some(d) if !d.is_empty() => Some(serde_json::to_string(d).map_err(|e| {
            MetadataStoreError::Corrupt(format!(
                "failed to serialize collection defaults for {collection_id}: {e}"
            ))
        })?),
        _ => None,
    };

    let now = Utc::now().to_rfc3339();
    let affected = conn
        .execute(
            "UPDATE collections SET defaults_json = ?1, updated_at = ?2 WHERE collection_id = ?3",
            params![json, now, collection_id],
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "write collection defaults",
            source,
        })?;

    if affected == 0 {
        return Err(MetadataStoreError::Validation(format!(
            "collection not found: {collection_id}"
        )));
    }

    Ok(())
}
