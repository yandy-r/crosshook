use super::{db, MetadataStoreError};
use crate::community::index::CommunityProfileIndexEntry;
use crate::community::taps::CommunityTapSyncResult;
use crate::discovery::models::TrainerSourcesManifest;
use crate::metadata::models::CommunityProfileRow;
use crate::profile::community_schema::CompatibilityRating;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

/// A6 string length bounds (advisory security finding).
const MAX_GAME_NAME_BYTES: usize = 512;
const MAX_DESCRIPTION_BYTES: usize = 4_096;
const MAX_PLATFORM_TAGS_BYTES: usize = 2_048;
const MAX_TRAINER_NAME_BYTES: usize = 512;
const MAX_AUTHOR_BYTES: usize = 512;
const MAX_VERSION_BYTES: usize = 256;

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

/// A6 string length bounds for trainer source entries.
const MAX_SOURCE_URL_BYTES: usize = 2_048;
const MAX_SOURCE_NAME_BYTES: usize = 512;
const MAX_NOTES_BYTES: usize = 4_096;

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

/// Look up the stored HEAD commit for a tap, if any.
fn get_tap_head_commit(
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
fn check_a6_bounds(entry: &CommunityProfileIndexEntry) -> Result<String, String> {
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

fn nullable_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn compatibility_rating_str(entry: &CommunityProfileIndexEntry) -> Option<String> {
    let rating = match &entry.manifest.metadata.compatibility_rating {
        CompatibilityRating::Unknown => "unknown",
        CompatibilityRating::Broken => "broken",
        CompatibilityRating::Partial => "partial",
        CompatibilityRating::Working => "working",
        CompatibilityRating::Platinum => "platinum",
    };
    Some(rating.to_string())
}

fn map_community_profile_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CommunityProfileRow> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::community::index::CommunityProfileIndexEntry;
    use crate::community::{
        CommunityProfileManifest, CommunityProfileMetadata, CompatibilityRating,
    };
    use crate::discovery::models::{TrainerSourceEntry, TrainerSourcesManifest};
    use crate::metadata::migrations;
    use crate::profile::GameProfile;
    use std::path::PathBuf;

    /// Insert a minimal `community_taps` row and return its `tap_id`.
    fn insert_test_tap(conn: &Connection) -> String {
        let tap_id = db::new_id();
        conn.execute(
            "INSERT INTO community_taps (
                tap_id, tap_url, tap_branch, local_path,
                last_head_commit, profile_count, last_indexed_at,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                tap_id,
                "https://example.invalid/tap.git",
                "main",
                "/tmp/tap",
                "abc1234",
                0i64,
                "2024-01-01T00:00:00Z",
                "2024-01-01T00:00:00Z",
                "2024-01-01T00:00:00Z",
            ],
        )
        .unwrap();
        tap_id
    }

    fn make_manifest(game_name: &str, source_url: &str) -> TrainerSourcesManifest {
        TrainerSourcesManifest {
            schema_version: 1,
            game_name: game_name.to_string(),
            steam_app_id: None,
            sources: vec![TrainerSourceEntry {
                source_name: "Test Source".to_string(),
                source_url: source_url.to_string(),
                trainer_version: None,
                game_version: None,
                notes: None,
                sha256: None,
            }],
        }
    }

    fn make_trainer_source_entry(
        name: &str,
        url: &str,
        notes: Option<String>,
    ) -> TrainerSourceEntry {
        TrainerSourceEntry {
            source_name: name.to_string(),
            source_url: url.to_string(),
            trainer_version: None,
            game_version: None,
            notes,
            sha256: None,
        }
    }

    fn make_manifest_with_entry(
        game_name: &str,
        entry: TrainerSourceEntry,
    ) -> TrainerSourcesManifest {
        TrainerSourcesManifest {
            schema_version: 1,
            game_name: game_name.to_string(),
            steam_app_id: None,
            sources: vec![entry],
        }
    }

    #[test]
    fn index_trainer_sources_inserts_entries() {
        let conn = db::open_in_memory().unwrap();
        migrations::run_migrations(&conn).unwrap();
        let tap_id = insert_test_tap(&conn);

        let manifest = make_manifest("Elden Ring", "https://example.com/trainer.exe");
        let sources = vec![("sources/elden-ring".to_string(), manifest)];

        let mut conn = conn;
        let inserted = index_trainer_sources(&mut conn, &tap_id, &sources).unwrap();
        assert_eq!(inserted, 1);

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM trainer_sources WHERE tap_id = ?1",
                params![tap_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn index_trainer_sources_rejects_http_url() {
        let conn = db::open_in_memory().unwrap();
        migrations::run_migrations(&conn).unwrap();
        let tap_id = insert_test_tap(&conn);

        let manifest = make_manifest("Elden Ring", "http://example.com/trainer.exe");
        let sources = vec![("sources/elden-ring".to_string(), manifest)];

        let mut conn = conn;
        let inserted = index_trainer_sources(&mut conn, &tap_id, &sources).unwrap();
        assert_eq!(inserted, 0);

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM trainer_sources WHERE tap_id = ?1",
                params![tap_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    fn make_entry(
        game_version: String,
        trainer_version: String,
        proton_version: String,
    ) -> CommunityProfileIndexEntry {
        CommunityProfileIndexEntry {
            tap_url: "https://example.invalid".to_string(),
            tap_branch: None,
            tap_path: PathBuf::from("/tmp"),
            manifest_path: PathBuf::from("/tmp/community-profile.json"),
            relative_path: PathBuf::from("community-profile.json"),
            manifest: CommunityProfileManifest::new(
                CommunityProfileMetadata {
                    game_name: "Test Game".to_string(),
                    game_version,
                    trainer_name: String::new(),
                    trainer_version,
                    proton_version,
                    platform_tags: vec![],
                    compatibility_rating: CompatibilityRating::Unknown,
                    author: String::new(),
                    description: String::new(),
                    trainer_sha256: None,
                },
                GameProfile::default(),
            ),
        }
    }

    #[test]
    fn rejects_oversized_game_version() {
        let entry = make_entry("a".repeat(257), String::new(), String::new());
        let err = check_a6_bounds(&entry).unwrap_err();
        assert!(
            err.contains("game_version"),
            "expected game_version in error: {err}"
        );
    }

    #[test]
    fn rejects_oversized_trainer_version() {
        let entry = make_entry(String::new(), "a".repeat(257), String::new());
        let err = check_a6_bounds(&entry).unwrap_err();
        assert!(
            err.contains("trainer_version"),
            "expected trainer_version in error: {err}"
        );
    }

    #[test]
    fn rejects_oversized_proton_version() {
        let entry = make_entry(String::new(), String::new(), "a".repeat(257));
        let err = check_a6_bounds(&entry).unwrap_err();
        assert!(
            err.contains("proton_version"),
            "expected proton_version in error: {err}"
        );
    }

    #[test]
    fn accepts_exactly_256_byte_version_strings() {
        let entry = make_entry("a".repeat(256), "a".repeat(256), "a".repeat(256));
        assert!(check_a6_bounds(&entry).is_ok());
    }

    #[test]
    fn index_trainer_sources_rejects_javascript_url() {
        let conn = db::open_in_memory().unwrap();
        migrations::run_migrations(&conn).unwrap();
        let tap_id = insert_test_tap(&conn);

        let entry = make_trainer_source_entry("Malicious Source", "javascript:alert(1)", None);
        let manifest = make_manifest_with_entry("Some Game", entry);
        let sources = vec![("sources/some-game".to_string(), manifest)];

        let mut conn = conn;
        let inserted = index_trainer_sources(&mut conn, &tap_id, &sources).unwrap();
        assert_eq!(inserted, 0, "javascript: URL must be rejected");

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM trainer_sources WHERE tap_id = ?1",
                params![tap_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn index_trainer_sources_enforces_a6_bounds_on_source_url() {
        let conn = db::open_in_memory().unwrap();
        migrations::run_migrations(&conn).unwrap();
        let tap_id = insert_test_tap(&conn);

        // 2049 bytes: starts with "https://" (8 bytes) + 2041 'a' bytes = 2049 total.
        let oversized_url = format!("https://{}", "a".repeat(MAX_SOURCE_URL_BYTES - 7));
        assert!(oversized_url.len() > MAX_SOURCE_URL_BYTES);

        let entry = make_trainer_source_entry("Long URL Source", &oversized_url, None);
        let manifest = make_manifest_with_entry("Some Game", entry);
        let sources = vec![("sources/some-game".to_string(), manifest)];

        let mut conn = conn;
        let inserted = index_trainer_sources(&mut conn, &tap_id, &sources).unwrap();
        assert_eq!(inserted, 0, "oversized source_url must be rejected");
    }

    #[test]
    fn index_trainer_sources_enforces_a6_bounds_on_game_name() {
        let conn = db::open_in_memory().unwrap();
        migrations::run_migrations(&conn).unwrap();
        let tap_id = insert_test_tap(&conn);

        let oversized_game_name = "a".repeat(MAX_GAME_NAME_BYTES + 1);
        let entry =
            make_trainer_source_entry("Valid Source", "https://example.com/trainer.exe", None);
        let manifest = make_manifest_with_entry(&oversized_game_name, entry);
        let sources = vec![("sources/some-game".to_string(), manifest)];

        let mut conn = conn;
        let inserted = index_trainer_sources(&mut conn, &tap_id, &sources).unwrap();
        assert_eq!(inserted, 0, "oversized game_name must be rejected");
    }

    #[test]
    fn index_trainer_sources_enforces_a6_bounds_on_source_name() {
        let conn = db::open_in_memory().unwrap();
        migrations::run_migrations(&conn).unwrap();
        let tap_id = insert_test_tap(&conn);

        let oversized_name = "a".repeat(MAX_SOURCE_NAME_BYTES + 1);
        let entry =
            make_trainer_source_entry(&oversized_name, "https://example.com/trainer.exe", None);
        let manifest = make_manifest_with_entry("Some Game", entry);
        let sources = vec![("sources/some-game".to_string(), manifest)];

        let mut conn = conn;
        let inserted = index_trainer_sources(&mut conn, &tap_id, &sources).unwrap();
        assert_eq!(inserted, 0, "oversized source_name must be rejected");
    }

    #[test]
    fn index_trainer_sources_enforces_a6_bounds_on_notes() {
        let conn = db::open_in_memory().unwrap();
        migrations::run_migrations(&conn).unwrap();
        let tap_id = insert_test_tap(&conn);

        let oversized_notes = "a".repeat(MAX_NOTES_BYTES + 1);
        let entry = make_trainer_source_entry(
            "Valid Source",
            "https://example.com/trainer.exe",
            Some(oversized_notes),
        );
        let manifest = make_manifest_with_entry("Some Game", entry);
        let sources = vec![("sources/some-game".to_string(), manifest)];

        let mut conn = conn;
        let inserted = index_trainer_sources(&mut conn, &tap_id, &sources).unwrap();
        assert_eq!(inserted, 0, "oversized notes must be rejected");
    }

    #[test]
    fn index_trainer_sources_deletes_and_reinserts_on_reindex() {
        let conn = db::open_in_memory().unwrap();
        migrations::run_migrations(&conn).unwrap();
        let tap_id = insert_test_tap(&conn);

        // First index: Elden Ring.
        let first_manifest = make_manifest("Elden Ring", "https://example.com/elden.exe");
        let first_sources = vec![("sources/elden-ring".to_string(), first_manifest)];

        let mut conn = conn;
        let inserted = index_trainer_sources(&mut conn, &tap_id, &first_sources).unwrap();
        assert_eq!(inserted, 1);

        // Verify first entry exists.
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM trainer_sources WHERE tap_id = ?1 AND game_name = 'Elden Ring'",
                params![tap_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        // Second index with different data: Cyberpunk 2077.
        let second_manifest = make_manifest("Cyberpunk 2077", "https://example.com/cyberpunk.exe");
        let second_sources = vec![("sources/cyberpunk".to_string(), second_manifest)];
        let inserted = index_trainer_sources(&mut conn, &tap_id, &second_sources).unwrap();
        assert_eq!(inserted, 1);

        // Old Elden Ring entry must be gone.
        let old_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM trainer_sources WHERE tap_id = ?1 AND game_name = 'Elden Ring'",
                params![tap_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            old_count, 0,
            "stale Elden Ring entry should have been deleted"
        );

        // New Cyberpunk entry must be present.
        let new_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM trainer_sources WHERE tap_id = ?1 AND game_name = 'Cyberpunk 2077'",
                params![tap_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(new_count, 1, "new Cyberpunk 2077 entry should be present");
    }
}
