use super::MetadataStoreError;
use rusqlite::Connection;

/// Adds a crash-safe journal for restoring prefix Windows compatibility after
/// Winetricks/Protontricks dependency installation.
pub(super) fn migrate_26_to_27(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        CREATE TABLE prefix_version_restore_journal (
            prefix_path TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
            binary_path TEXT NOT NULL,
            tool_type TEXT NOT NULL CHECK (tool_type IN ('winetricks','protontricks')),
            steam_app_id TEXT,
            restore_verb TEXT NOT NULL,
            state TEXT NOT NULL CHECK (state IN ('pending','failed')),
            last_error TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX idx_prefix_version_restore_profile
            ON prefix_version_restore_journal(profile_id);
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 26 to 27",
        source,
    })?;

    Ok(())
}
