//! Helper functions for community profile indexing.

use super::constants::*;
use crate::community::index::CommunityProfileIndexEntry;
use crate::metadata::models::CommunityProfileRow;
use crate::profile::community_schema::CompatibilityRating;
use rusqlite::{params, Connection, OptionalExtension};

use super::MetadataStoreError;

/// Look up the stored HEAD commit for a tap, if any.
pub(super) fn get_tap_head_commit(
    conn: &Connection,
    tap_url: &str,
    tap_branch: &str,
) -> Result<Option<String>, MetadataStoreError> {
    conn.query_row(
        "SELECT last_head_commit FROM community_taps WHERE tap_url = ?1 AND tap_branch = ?2",
        params![tap_url, tap_branch],
        |row| row.get(0),
    )
    .optional()
    .map(std::option::Option::flatten)
    .map_err(|source| MetadataStoreError::Database {
        action: "look up tap HEAD commit watermark",
        source,
    })
}

/// Validate A6 string length bounds for a community profile entry.
///
/// Returns `Ok(joined_platform_tags)` if all bounds are satisfied, or
/// `Err(reason)` describing which field exceeded its limit.
pub(super) fn check_a6_bounds(entry: &CommunityProfileIndexEntry) -> Result<String, String> {
    let meta = &entry.manifest.metadata;

    if meta.game_name.len() > MAX_GAME_NAME_BYTES {
        return Err(format!(
            "game_name exceeds {} bytes ({} bytes)",
            MAX_GAME_NAME_BYTES,
            meta.game_name.len()
        ));
    }

    if meta.description.len() > MAX_DESCRIPTION_BYTES {
        return Err(format!(
            "description exceeds {} bytes ({} bytes)",
            MAX_DESCRIPTION_BYTES,
            meta.description.len()
        ));
    }

    let joined_tags = meta.platform_tags.join(" ");
    if joined_tags.len() > MAX_PLATFORM_TAGS_BYTES {
        return Err(format!(
            "platform_tags exceeds {} bytes ({} bytes joined)",
            MAX_PLATFORM_TAGS_BYTES,
            joined_tags.len()
        ));
    }

    if meta.trainer_name.len() > MAX_TRAINER_NAME_BYTES {
        return Err(format!(
            "trainer_name exceeds {} bytes ({} bytes)",
            MAX_TRAINER_NAME_BYTES,
            meta.trainer_name.len()
        ));
    }

    if meta.author.len() > MAX_AUTHOR_BYTES {
        return Err(format!(
            "author exceeds {} bytes ({} bytes)",
            MAX_AUTHOR_BYTES,
            meta.author.len()
        ));
    }

    if meta.game_version.len() > MAX_VERSION_BYTES {
        return Err(format!(
            "game_version exceeds {} bytes ({} bytes)",
            MAX_VERSION_BYTES,
            meta.game_version.len()
        ));
    }

    if meta.trainer_version.len() > MAX_VERSION_BYTES {
        return Err(format!(
            "trainer_version exceeds {} bytes ({} bytes)",
            MAX_VERSION_BYTES,
            meta.trainer_version.len()
        ));
    }

    if meta.proton_version.len() > MAX_VERSION_BYTES {
        return Err(format!(
            "proton_version exceeds {} bytes ({} bytes)",
            MAX_VERSION_BYTES,
            meta.proton_version.len()
        ));
    }

    Ok(joined_tags)
}

/// Convert a string to `Some(trimmed)` or `None` if empty after trimming.
pub(super) fn nullable_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Map a `CompatibilityRating` enum to its database string representation.
pub(super) fn compatibility_rating_str(entry: &CommunityProfileIndexEntry) -> Option<String> {
    let rating = match &entry.manifest.metadata.compatibility_rating {
        CompatibilityRating::Unknown => "unknown",
        CompatibilityRating::Broken => "broken",
        CompatibilityRating::Partial => "partial",
        CompatibilityRating::Working => "working",
        CompatibilityRating::Platinum => "platinum",
    };
    Some(rating.to_string())
}

/// Map a SQLite row to a `CommunityProfileRow`.
pub(super) fn map_community_profile_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<CommunityProfileRow> {
    Ok(CommunityProfileRow {
        id: row.get(0)?,
        tap_id: row.get(1)?,
        tap_url: row.get(2)?,
        relative_path: row.get(3)?,
        manifest_path: row.get(4)?,
        game_name: row.get(5)?,
        game_version: row.get(6)?,
        trainer_name: row.get(7)?,
        trainer_version: row.get(8)?,
        proton_version: row.get(9)?,
        compatibility_rating: row.get(10)?,
        author: row.get(11)?,
        description: row.get(12)?,
        platform_tags: row.get(13)?,
        schema_version: row.get(14)?,
        created_at: row.get(15)?,
    })
}
