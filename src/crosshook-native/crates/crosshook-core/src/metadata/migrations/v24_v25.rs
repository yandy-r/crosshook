use super::MetadataStoreError;
use rusqlite::Connection;

/// Adds `community_profiles.trainer_loading_mode` for the discovery catalog
/// loading-mode facet (Forgejo #27) and resets tap watermarks so the next
/// `community_sync` re-indexes every tap and backfills the column offline.
pub(super) fn migrate_24_to_25(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        ALTER TABLE community_profiles ADD COLUMN trainer_loading_mode TEXT;
        UPDATE community_taps SET last_head_commit = NULL;
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 24 to 25",
        source,
    })?;

    Ok(())
}
