use super::{db, MetadataStoreError};
use super::models::CollectionRow;
use super::profile_sync::lookup_profile_id;
use chrono::Utc;
use rusqlite::{params, Connection};

pub fn list_collections(conn: &Connection) -> Result<Vec<CollectionRow>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT c.collection_id, c.name, c.description, c.created_at, c.updated_at, \
             (SELECT COUNT(*) FROM collection_profiles cp WHERE cp.collection_id = c.collection_id) as profile_count \
             FROM collections c ORDER BY c.name",
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
    let profile_id = lookup_profile_id(conn, profile_name)?;
    let Some(profile_id) = profile_id else {
        tracing::warn!(
            profile_name,
            collection_id,
            "profile not found in metadata index when adding to collection — skipping"
        );
        return Ok(());
    };

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
