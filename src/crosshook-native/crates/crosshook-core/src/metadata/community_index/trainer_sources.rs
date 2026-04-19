//! Trainer source manifest indexing for community taps.

use super::constants::*;
use super::MetadataStoreError;
use crate::discovery::models::TrainerSourcesManifest;
use rusqlite::{params, Connection, Transaction, TransactionBehavior};

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
    let tx = Transaction::new(conn, TransactionBehavior::Immediate).map_err(|source| {
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
        if manifest.game_name.len() > MAX_GAME_NAME_BYTES {
            tracing::warn!(
                game_name_len = manifest.game_name.len(),
                max = MAX_GAME_NAME_BYTES,
                relative_path = %relative_path,
                "skipping trainer source manifest: game_name exceeds {} bytes", MAX_GAME_NAME_BYTES
            );
            continue;
        }

        for entry in &manifest.sources {
            if !entry.source_url.starts_with("https://") {
                tracing::warn!(
                    source_url = %entry.source_url,
                    game_name = %manifest.game_name,
                    relative_path = %relative_path,
                    "skipping trainer source entry with non-HTTPS source_url"
                );
                continue;
            }

            if entry.source_url.len() > MAX_SOURCE_URL_BYTES {
                tracing::warn!(
                    source_url_len = entry.source_url.len(),
                    max = MAX_SOURCE_URL_BYTES,
                    game_name = %manifest.game_name,
                    relative_path = %relative_path,
                    "skipping trainer source entry: source_url exceeds {} bytes", MAX_SOURCE_URL_BYTES
                );
                continue;
            }

            if entry.source_name.len() > MAX_SOURCE_NAME_BYTES {
                tracing::warn!(
                    source_name_len = entry.source_name.len(),
                    max = MAX_SOURCE_NAME_BYTES,
                    game_name = %manifest.game_name,
                    relative_path = %relative_path,
                    "skipping trainer source entry: source_name exceeds {} bytes", MAX_SOURCE_NAME_BYTES
                );
                continue;
            }

            if let Some(notes) = &entry.notes {
                if notes.len() > MAX_NOTES_BYTES {
                    tracing::warn!(
                        notes_len = notes.len(),
                        max = MAX_NOTES_BYTES,
                        game_name = %manifest.game_name,
                        relative_path = %relative_path,
                        "skipping trainer source entry: notes exceeds {} bytes", MAX_NOTES_BYTES
                    );
                    continue;
                }
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
