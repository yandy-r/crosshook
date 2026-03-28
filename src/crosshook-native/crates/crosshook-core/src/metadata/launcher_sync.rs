use super::{db, models::DriftState, profile_sync::lookup_profile_id, MetadataStoreError};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

pub fn observe_launcher_exported(
    conn: &Connection,
    profile_name: Option<&str>,
    slug: &str,
    display_name: &str,
    script_path: &str,
    desktop_entry_path: &str,
) -> Result<(), MetadataStoreError> {
    let profile_id: Option<String> = match profile_name {
        Some(name) => lookup_profile_id(conn, name)?,
        None => None,
    };

    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO launchers (
            launcher_id, profile_id, launcher_slug, display_name,
            script_path, desktop_entry_path, drift_state, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ON CONFLICT(launcher_slug) DO UPDATE SET
            profile_id = COALESCE(excluded.profile_id, launchers.profile_id),
            display_name = excluded.display_name,
            script_path = excluded.script_path,
            desktop_entry_path = excluded.desktop_entry_path,
            drift_state = excluded.drift_state,
            updated_at = excluded.updated_at",
        params![
            db::new_id(),
            profile_id,
            slug,
            display_name,
            script_path,
            desktop_entry_path,
            DriftState::Aligned.as_str(),
            now,
            now,
        ],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "upsert a launcher metadata row",
        source,
    })?;

    Ok(())
}

pub fn observe_launcher_deleted(
    conn: &Connection,
    launcher_slug: &str,
) -> Result<(), MetadataStoreError> {
    conn.execute(
        "UPDATE launchers SET drift_state = 'missing', updated_at = ?1 WHERE launcher_slug = ?2",
        params![Utc::now().to_rfc3339(), launcher_slug],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "tombstone a launcher metadata row as missing",
        source,
    })?;

    Ok(())
}

pub fn observe_launcher_renamed(
    conn: &mut Connection,
    old_slug: &str,
    new_slug: &str,
    new_display_name: &str,
    new_script_path: &str,
    new_desktop_entry_path: &str,
) -> Result<(), MetadataStoreError> {
    let tx = Transaction::new(conn, TransactionBehavior::Immediate).map_err(|source| {
        MetadataStoreError::Database {
            action: "start a launcher rename transaction",
            source,
        }
    })?;

    let now = Utc::now().to_rfc3339();

    tx.execute(
        "UPDATE launchers SET drift_state = 'missing', updated_at = ?1 WHERE launcher_slug = ?2",
        params![now, old_slug],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "tombstone the old launcher metadata row",
        source,
    })?;

    let profile_id: Option<String> = tx
        .query_row(
            "SELECT profile_id FROM launchers WHERE launcher_slug = ?1 LIMIT 1",
            params![old_slug],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|source| MetadataStoreError::Database {
            action: "load existing launcher profile linkage",
            source,
        })?
        .flatten();

    tx.execute(
        "INSERT INTO launchers (
            launcher_id, profile_id, launcher_slug, display_name,
            script_path, desktop_entry_path, drift_state, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ON CONFLICT(launcher_slug) DO UPDATE SET
            profile_id = COALESCE(excluded.profile_id, launchers.profile_id),
            display_name = excluded.display_name,
            script_path = excluded.script_path,
            desktop_entry_path = excluded.desktop_entry_path,
            drift_state = excluded.drift_state,
            updated_at = excluded.updated_at",
        params![
            db::new_id(),
            profile_id,
            new_slug,
            new_display_name,
            new_script_path,
            new_desktop_entry_path,
            DriftState::Aligned.as_str(),
            now,
            now,
        ],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "upsert a launcher metadata row",
        source,
    })?;

    tx.commit().map_err(|source| MetadataStoreError::Database {
        action: "commit the launcher rename transaction",
        source,
    })?;

    Ok(())
}
