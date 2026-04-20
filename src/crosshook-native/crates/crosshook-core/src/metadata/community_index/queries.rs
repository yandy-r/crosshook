//! Query operations for community profile data.

use super::helpers::map_community_profile_row;
use super::MetadataStoreError;
use crate::metadata::models::CommunityProfileRow;
use rusqlite::{params, Connection};

/// List community profile rows, optionally filtered by tap URL.
///
/// Returns all `community_profiles` rows joined with their parent `community_taps`
/// row so that `tap_url` is populated on each result.
pub fn list_community_tap_profiles(
    conn: &Connection,
    tap_url: Option<&str>,
) -> Result<Vec<CommunityProfileRow>, MetadataStoreError> {
    let rows = match tap_url {
        Some(url) => {
            let mut stmt = conn
                .prepare(
                    "SELECT cp.id, cp.tap_id, ct.tap_url, cp.relative_path, cp.manifest_path,
                            cp.game_name, cp.game_version, cp.trainer_name, cp.trainer_version,
                            cp.proton_version, cp.compatibility_rating, cp.author, cp.description,
                            cp.platform_tags, cp.schema_version, cp.created_at
                     FROM community_profiles cp
                     JOIN community_taps ct ON cp.tap_id = ct.tap_id
                     WHERE ct.tap_url = ?1",
                )
                .map_err(|source| MetadataStoreError::Database {
                    action: "prepare list community profiles by tap_url query",
                    source,
                })?;
            let collected = stmt
                .query_map(params![url], map_community_profile_row)
                .map_err(|source| MetadataStoreError::Database {
                    action: "execute list community profiles by tap_url query",
                    source,
                })?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|source| MetadataStoreError::Database {
                    action: "read community profile rows by tap_url",
                    source,
                })?;
            collected
        }
        None => {
            let mut stmt = conn
                .prepare(
                    "SELECT cp.id, cp.tap_id, ct.tap_url, cp.relative_path, cp.manifest_path,
                            cp.game_name, cp.game_version, cp.trainer_name, cp.trainer_version,
                            cp.proton_version, cp.compatibility_rating, cp.author, cp.description,
                            cp.platform_tags, cp.schema_version, cp.created_at
                     FROM community_profiles cp
                     JOIN community_taps ct ON cp.tap_id = ct.tap_id",
                )
                .map_err(|source| MetadataStoreError::Database {
                    action: "prepare list all community profiles query",
                    source,
                })?;
            let collected = stmt
                .query_map([], map_community_profile_row)
                .map_err(|source| MetadataStoreError::Database {
                    action: "execute list all community profiles query",
                    source,
                })?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|source| MetadataStoreError::Database {
                    action: "read all community profile rows",
                    source,
                })?;
            collected
        }
    };

    Ok(rows)
}
