//! Trainer source manifest indexing for community taps.

use super::constants::*;
use super::MetadataStoreError;
use crate::discovery::models::{TrainerSourceEntry, TrainerSourcesManifest};
use rusqlite::{params, Connection};

/// Returns `true` when a trainer sources manifest must be skipped entirely
/// (A6 byte-cap violation on `game_name`).
pub(super) fn trainer_manifest_rejected(manifest: &TrainerSourcesManifest) -> bool {
    manifest.game_name.len() > MAX_GAME_NAME_BYTES
}

/// Returns `Some(reason)` when a trainer source entry must be skipped
/// (non-HTTPS URL or A6 byte-cap violation); `None` when acceptable.
pub(super) fn trainer_source_rejection(entry: &TrainerSourceEntry) -> Option<&'static str> {
    if !entry.source_url.starts_with("https://") {
        return Some("non-HTTPS source_url");
    }
    if entry.source_url.len() > MAX_SOURCE_URL_BYTES {
        return Some("source_url exceeds byte cap");
    }
    if entry.source_name.len() > MAX_SOURCE_NAME_BYTES {
        return Some("source_name exceeds byte cap");
    }
    if entry
        .notes
        .as_ref()
        .is_some_and(|notes| notes.len() > MAX_NOTES_BYTES)
    {
        return Some("notes exceed byte cap");
    }
    None
}

/// Index trainer source manifests for a single tap into the `trainer_sources` table.
///
/// Performs a transactional DELETE+INSERT: all existing rows for the given `tap_id` are
/// removed and replaced with the entries from `sources`. Entries that fail A6 field-length
/// validation or have a non-HTTPS `source_url` are logged with `tracing::warn!` and skipped.
///
/// Returns the number of rows inserted.
pub fn index_trainer_sources(
    conn: &mut Connection,
    tap_id: &str,
    sources: &[(String, TrainerSourcesManifest)],
) -> Result<usize, MetadataStoreError> {
    let tx = crate::metadata::util::write_savepoint(conn).map_err(|source| {
        MetadataStoreError::Database {
            action: "start a trainer sources re-index transaction",
            source,
        }
    })?;

    tx.execute(
        "DELETE FROM trainer_sources WHERE tap_id = ?1",
        params![tap_id],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "delete stale trainer_sources rows for tap",
        source,
    })?;

    let mut inserted: usize = 0;

    for (relative_path, manifest) in sources {
        if trainer_manifest_rejected(manifest) {
            tracing::warn!(
                game_name_len = manifest.game_name.len(),
                max = MAX_GAME_NAME_BYTES,
                relative_path = %relative_path,
                "skipping trainer source manifest: game_name exceeds {} bytes", MAX_GAME_NAME_BYTES
            );
            continue;
        }

        for entry in &manifest.sources {
            if let Some(reason) = trainer_source_rejection(entry) {
                tracing::warn!(
                    source_url = %entry.source_url,
                    game_name = %manifest.game_name,
                    relative_path = %relative_path,
                    reason = %reason,
                    "skipping trainer source entry"
                );
                continue;
            }

            tx.execute(
                "INSERT INTO trainer_sources (
                    tap_id, game_name, steam_app_id, source_name, source_url,
                    trainer_version, game_version, notes, sha256, relative_path, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'))",
                params![
                    tap_id,
                    manifest.game_name,
                    manifest.steam_app_id,
                    entry.source_name,
                    entry.source_url,
                    entry.trainer_version,
                    entry.game_version,
                    entry.notes,
                    entry.sha256,
                    relative_path,
                ],
            )
            .map_err(|source| MetadataStoreError::Database {
                action: "insert a trainer_sources row",
                source,
            })?;

            inserted += 1;
        }
    }

    tx.commit().map_err(|source| MetadataStoreError::Database {
        action: "commit the trainer sources re-index transaction",
        source,
    })?;

    Ok(inserted)
}
