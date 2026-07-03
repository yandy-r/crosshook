use super::MetadataStoreError;
use rusqlite::Connection;

/// Adds `profile_mods` — per-profile mod coexistence registry (Forgejo #28).
/// Machine-local operational metadata; never exported with community profiles.
pub(super) fn migrate_25_to_26(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        CREATE TABLE profile_mods (
            mod_id      TEXT PRIMARY KEY,
            profile_id  TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
            name        TEXT NOT NULL COLLATE NOCASE,
            category    TEXT NOT NULL CHECK (category IN
                            ('overlay_injection','script_extender','file_replacement','other')),
            paths_json  TEXT NOT NULL DEFAULT '[]',
            enabled     INTEGER NOT NULL DEFAULT 1,
            provenance  TEXT NOT NULL CHECK (provenance IN ('manual','detected')),
            source_url  TEXT,
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL,
            UNIQUE (profile_id, name)
        );
        CREATE INDEX idx_profile_mods_profile_id ON profile_mods(profile_id);
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 25 to 26",
        source,
    })?;

    Ok(())
}
