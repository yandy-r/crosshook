use rusqlite::Connection;

use crate::metadata::MetadataStoreError;

/// Drops two indexes that no query uses any more: `trainer_sources.steam_app_id` (the column is
/// no longer written) and the `game_image_cache` expiry index (its only reader, the expired-image
/// evictor, was removed). Columns are left in place: they are inert and dropping them would
/// rebuild tables for no runtime benefit.
pub(super) fn migrate_28_to_29(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "DROP INDEX IF EXISTS idx_trainer_sources_app_id;
         DROP INDEX IF EXISTS idx_game_image_cache_expires;",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 28 to 29",
        source,
    })?;

    Ok(())
}
