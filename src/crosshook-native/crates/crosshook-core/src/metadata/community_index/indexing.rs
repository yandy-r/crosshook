//! Community tap profile indexing.

use super::helpers::*;
use super::{db, MetadataStoreError};
use crate::community::taps::CommunityTapSyncResult;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

/// Index a community tap sync result and its trainer sources in one call.
///
/// Wraps [`index_community_tap_result`] + trainer sources indexing from the
/// `trainer_sources` module, looking up the `tap_id` between the two steps via
/// a row read on `community_taps`.
pub fn index_community_tap_result_with_trainers(
    conn: &mut Connection,
    result: &CommunityTapSyncResult,
) -> Result<(), MetadataStoreError> {
    index_community_tap_result(conn, result)?;

    if result.index.trainer_sources.is_empty() {
        return Ok(());
    }

    let tap_url = &result.workspace.subscription.url;
    let tap_branch = result
        .workspace
        .subscription
        .branch
        .as_deref()
        .unwrap_or("");

    let tap_id = conn
        .query_row(
            "SELECT tap_id FROM community_taps WHERE tap_url = ?1 AND tap_branch = ?2",
            params![tap_url, tap_branch],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|source| MetadataStoreError::Database {
            action: "look up tap_id for trainer sources indexing",
            source,
        })?;

    if let Some(tap_id) = tap_id {
        super::trainer_sources::index_trainer_sources(
            conn,
            &tap_id,
            &result.index.trainer_sources,
        )?;
    }

    Ok(())
}

/// Index the sync result for a single community tap into the metadata store.
///
/// If the tap's HEAD commit is unchanged since the last index, this is a no-op
/// (watermark skip). Otherwise, the tap's `community_profiles` rows are replaced
/// via transactional DELETE+INSERT to eliminate stale ghost entries.
pub fn index_community_tap_result(
    conn: &mut Connection,
    result: &CommunityTapSyncResult,
) -> Result<(), MetadataStoreError> {
    let tap_url = &result.workspace.subscription.url;
    let tap_branch = result
        .workspace
        .subscription
        .branch
        .as_deref()
        .unwrap_or("");

    // Watermark skip: if HEAD is unchanged, nothing to do.
    let stored_head = get_tap_head_commit(conn, tap_url, tap_branch)?;
    if stored_head.as_deref() == Some(&result.head_commit) {
        return Ok(());
    }

    let now = Utc::now().to_rfc3339();
    let local_path = result.workspace.local_path.to_string_lossy();
    let profile_count = result.index.entries.len() as i64;

    // Transactional UPSERT+DELETE+INSERT so watermark does not advance on partial failures.
    let tx = Transaction::new(conn, TransactionBehavior::Immediate).map_err(|source| {
        MetadataStoreError::Database {
            action: "start a community profiles re-index transaction",
            source,
        }
    })?;

    tx.execute(
        "INSERT INTO community_taps (
            tap_id, tap_url, tap_branch, local_path,
            last_head_commit, profile_count, last_indexed_at,
            created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ON CONFLICT(tap_url, tap_branch) DO UPDATE SET
            local_path = excluded.local_path,
            last_head_commit = excluded.last_head_commit,
            profile_count = excluded.profile_count,
            last_indexed_at = excluded.last_indexed_at,
            updated_at = excluded.updated_at",
        params![
            db::new_id(),
            tap_url,
            tap_branch,
            local_path.as_ref(),
            result.head_commit,
            profile_count,
            now,
            now,
            now,
        ],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "upsert a community_taps row",
        source,
    })?;

    // Retrieve the tap_id for this (tap_url, tap_branch).
    let tap_id: String = tx
        .query_row(
            "SELECT tap_id FROM community_taps WHERE tap_url = ?1 AND tap_branch = ?2",
            params![tap_url, tap_branch],
            |row| row.get(0),
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "look up community_taps tap_id after upsert",
            source,
        })?;

    tx.execute(
        "DELETE FROM community_profiles WHERE tap_id = ?1",
        params![tap_id],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "delete stale community_profiles rows for tap",
        source,
    })?;

    for entry in &result.index.entries {
        let platform_tags = match check_a6_bounds(entry) {
            Ok(joined_tags) => joined_tags,
            Err(reason) => {
                tracing::warn!(
                    relative_path = %entry.relative_path.display(),
                    reason = %reason,
                    "skipping community profile entry due to A6 field length violation"
                );
                continue;
            }
        };
        let relative_path = entry.relative_path.to_string_lossy();
        let manifest_path = entry.manifest_path.to_string_lossy();
        let compatibility_rating = compatibility_rating_str(entry);

        tx.execute(
            "INSERT INTO community_profiles (
                tap_id, relative_path, manifest_path,
                game_name, game_version, trainer_name, trainer_version,
                proton_version, compatibility_rating, author, description,
                platform_tags, schema_version, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                tap_id,
                relative_path.as_ref(),
                manifest_path.as_ref(),
                nullable_text(&entry.manifest.metadata.game_name),
                nullable_text(&entry.manifest.metadata.game_version),
                nullable_text(&entry.manifest.metadata.trainer_name),
                nullable_text(&entry.manifest.metadata.trainer_version),
                nullable_text(&entry.manifest.metadata.proton_version),
                compatibility_rating,
                nullable_text(&entry.manifest.metadata.author),
                nullable_text(&entry.manifest.metadata.description),
                nullable_text(&platform_tags),
                entry.manifest.schema_version as i64,
                now,
            ],
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "insert a community_profiles row",
            source,
        })?;
    }

    // Update profile_count to the actual inserted count.
    tx.execute(
        "UPDATE community_taps
         SET profile_count = (SELECT COUNT(*) FROM community_profiles WHERE tap_id = ?1)
         WHERE tap_id = ?1",
        params![tap_id],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "update community_taps profile_count after re-index",
        source,
    })?;

    tx.commit().map_err(|source| MetadataStoreError::Database {
        action: "commit the community profiles re-index transaction",
        source,
    })?;
    Ok(())
}
