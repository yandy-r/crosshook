//! Catalog fetch glue: SQL fetch-all for the metadata path and conversions
//! from the filesystem tap index for the degraded path. Both feed the same
//! pure [`build_catalog_page`] pass.

use super::helpers::{check_a6_bounds, compatibility_rating_str, nullable_text};
use super::trainer_sources::{trainer_manifest_rejected, trainer_source_rejection};
use super::MetadataStoreError;
use crate::community::index::CommunityProfileIndex;
use crate::community::{CommunityTapStore, CommunityTapSubscription};
use crate::discovery::catalog::{
    build_catalog_page, CatalogEntry, CatalogPage, CatalogQuery, CatalogRowInput, CatalogSource,
    CatalogSourceInput,
};
use crate::discovery::models::TrainerSourcesManifest;
use rusqlite::Connection;

/// Run the aggregated cross-tap catalog query against the metadata DB.
pub fn query_community_catalog(
    conn: &Connection,
    query: &CatalogQuery,
) -> Result<CatalogPage, MetadataStoreError> {
    let rows = fetch_catalog_rows(conn)?;
    let sources = fetch_source_groups(conn)?;
    Ok(build_catalog_page(rows, sources, query, false))
}

fn fetch_catalog_rows(conn: &Connection) -> Result<Vec<CatalogRowInput>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT cp.id, cp.tap_id, ct.tap_url, ct.local_path, cp.relative_path, cp.manifest_path,
                    cp.game_name, cp.game_version, cp.trainer_name, cp.trainer_version,
                    cp.proton_version, cp.compatibility_rating, cp.author, cp.description,
                    cp.platform_tags, cp.trainer_loading_mode, cp.schema_version
             FROM community_profiles cp
             JOIN community_taps ct ON cp.tap_id = ct.tap_id
             ORDER BY cp.game_name COLLATE NOCASE, cp.manifest_path",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare fetch community catalog rows query",
            source,
        })?;

    let rows = stmt
        .query_map([], |row| {
            Ok(CatalogRowInput {
                tap_id: row.get(1)?,
                entry: CatalogEntry {
                    id: Some(row.get(0)?),
                    tap_url: row.get(2)?,
                    tap_local_path: row.get(3)?,
                    relative_path: row.get(4)?,
                    manifest_path: row.get(5)?,
                    game_name: row.get(6)?,
                    game_version: row.get(7)?,
                    trainer_name: row.get(8)?,
                    trainer_version: row.get(9)?,
                    proton_version: row.get(10)?,
                    compatibility_rating: row.get(11)?,
                    author: row.get(12)?,
                    description: row.get(13)?,
                    platform_tags: row.get(14)?,
                    trainer_loading_mode: row.get(15)?,
                    schema_version: row.get(16)?,
                    sources: Vec::new(),
                },
            })
        })
        .map_err(|source| MetadataStoreError::Database {
            action: "execute fetch community catalog rows query",
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| MetadataStoreError::Database {
            action: "read community catalog rows",
            source,
        })?;

    Ok(rows)
}

fn fetch_source_groups(conn: &Connection) -> Result<Vec<CatalogSourceInput>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT ts.tap_id, ts.game_name, ts.source_name, ts.source_url, ts.sha256,
                    ts.trainer_version, ts.game_version, ts.notes, ct.tap_url, ct.local_path
             FROM trainer_sources ts
             JOIN community_taps ct ON ts.tap_id = ct.tap_id
             ORDER BY ts.tap_id, ts.game_name COLLATE NOCASE, ts.source_name COLLATE NOCASE",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare fetch trainer source groups query",
            source,
        })?;

    let sources = stmt
        .query_map([], |row| {
            Ok(CatalogSourceInput {
                tap_id: row.get(0)?,
                tap_url: row.get(8)?,
                tap_local_path: row.get(9)?,
                game_name: row.get(1)?,
                source: CatalogSource {
                    source_name: row.get(2)?,
                    source_url: row.get(3)?,
                    sha256: row.get(4)?,
                    trainer_version: row.get(5)?,
                    game_version: row.get(6)?,
                    notes: row.get(7)?,
                },
            })
        })
        .map_err(|source| MetadataStoreError::Database {
            action: "execute fetch trainer source groups query",
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| MetadataStoreError::Database {
            action: "read trainer source group rows",
            source,
        })?;

    Ok(sources)
}

/// Degraded-mode conversion: filesystem tap index entries → catalog row
/// inputs, applying the same A6 bounds filter the indexer applies.
pub fn rows_from_index(index: &CommunityProfileIndex) -> Vec<CatalogRowInput> {
    index
        .entries
        .iter()
        .filter_map(|entry| {
            let joined_tags = match check_a6_bounds(entry) {
                Ok(joined_tags) => joined_tags,
                Err(reason) => {
                    tracing::warn!(
                        relative_path = %entry.relative_path.display(),
                        reason = %reason,
                        "skipping community profile entry due to A6 field length violation"
                    );
                    return None;
                }
            };
            let metadata = &entry.manifest.metadata;
            Some(CatalogRowInput {
                tap_id: entry.tap_url.clone(),
                entry: CatalogEntry {
                    id: None,
                    tap_url: entry.tap_url.clone(),
                    tap_local_path: entry.tap_path.to_string_lossy().into_owned(),
                    relative_path: entry.relative_path.to_string_lossy().into_owned(),
                    manifest_path: entry.manifest_path.to_string_lossy().into_owned(),
                    game_name: nullable_text(&metadata.game_name),
                    game_version: nullable_text(&metadata.game_version),
                    trainer_name: nullable_text(&metadata.trainer_name),
                    trainer_version: nullable_text(&metadata.trainer_version),
                    proton_version: nullable_text(&metadata.proton_version),
                    compatibility_rating: compatibility_rating_str(entry),
                    author: nullable_text(&metadata.author),
                    description: nullable_text(&metadata.description),
                    platform_tags: nullable_text(&joined_tags),
                    trainer_loading_mode: Some(
                        entry
                            .manifest
                            .profile
                            .trainer
                            .loading_mode
                            .as_str()
                            .to_string(),
                    ),
                    schema_version: entry.manifest.schema_version as i64,
                    sources: Vec::new(),
                },
            })
        })
        .collect()
}

/// Degraded-mode conversion: parsed trainer-source manifests for one tap →
/// catalog source inputs, applying the same HTTPS-only + byte-cap filters the
/// indexer applies. `tap_url` doubles as the degraded-mode join key.
pub fn sources_from_index(
    tap_url: &str,
    tap_local_path: &str,
    sources: &[(String, TrainerSourcesManifest)],
) -> Vec<CatalogSourceInput> {
    let mut inputs = Vec::new();
    for (_, manifest) in sources {
        if trainer_manifest_rejected(manifest) {
            continue;
        }
        for entry in &manifest.sources {
            if trainer_source_rejection(entry).is_some() {
                continue;
            }
            inputs.push(CatalogSourceInput {
                tap_id: tap_url.to_string(),
                tap_url: tap_url.to_string(),
                tap_local_path: tap_local_path.to_string(),
                game_name: manifest.game_name.clone(),
                source: CatalogSource {
                    source_name: entry.source_name.clone(),
                    source_url: entry.source_url.clone(),
                    sha256: entry.sha256.clone(),
                    trainer_version: entry.trainer_version.clone(),
                    game_version: entry.game_version.clone(),
                    notes: entry.notes.clone(),
                },
            });
        }
    }
    inputs
}

/// Degraded-path orchestration: build the aggregated catalog straight from
/// the filesystem tap index when the metadata DB is unavailable. Per-tap
/// failures are logged and skipped so one bad tap yields a partial catalog
/// instead of blanking it.
pub fn degraded_catalog_from_taps(
    tap_store: &CommunityTapStore,
    taps: &[CommunityTapSubscription],
    query: &CatalogQuery,
) -> CatalogPage {
    let mut rows = Vec::new();
    let mut sources = Vec::new();
    for tap in taps {
        let workspace = match tap_store.resolve_workspace(tap) {
            Ok(workspace) => workspace,
            Err(error) => {
                tracing::warn!(
                    tap_url = %tap.url,
                    %error,
                    "skipping tap in degraded catalog: workspace resolution failed"
                );
                continue;
            }
        };
        let index = match tap_store.index_workspaces(std::slice::from_ref(&workspace)) {
            Ok(index) => index,
            Err(error) => {
                tracing::warn!(
                    tap_url = %tap.url,
                    %error,
                    "skipping tap in degraded catalog: workspace indexing failed"
                );
                continue;
            }
        };
        rows.extend(rows_from_index(&index));
        sources.extend(sources_from_index(
            &workspace.subscription.url,
            &workspace.local_path.to_string_lossy(),
            &index.trainer_sources,
        ));
    }
    build_catalog_page(rows, sources, query, true)
}

#[cfg(test)]
mod tests {
    use super::super::constants::MAX_SOURCE_NAME_BYTES;
    use super::*;
    use crate::community::index::CommunityProfileIndexEntry;
    use crate::community::{
        CommunityProfileManifest, CommunityProfileMetadata, CompatibilityRating,
    };
    use crate::discovery::models::TrainerSourceEntry;
    use crate::metadata::MetadataStore;
    use crate::profile::{GameProfile, TrainerLoadingMode};
    use rusqlite::params;
    use std::path::PathBuf;

    const TAP_URL: &str = "https://example.com/tap.git";
    const TAP_LOCAL_PATH: &str = "/tmp/tap";

    fn insert_test_tap(conn: &Connection, tap_id: &str, tap_url: &str, local_path: &str) {
        conn.execute(
            "INSERT INTO community_taps (tap_id, tap_url, tap_branch, local_path, created_at, updated_at)
             VALUES (?1, ?2, '', ?3, datetime('now'), datetime('now'))",
            params![tap_id, tap_url, local_path],
        )
        .unwrap();
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_test_profile(
        conn: &Connection,
        tap_id: &str,
        relative_path: &str,
        game_name: Option<&str>,
        trainer_name: Option<&str>,
        rating: Option<&str>,
        loading_mode: Option<&str>,
        author: Option<&str>,
    ) {
        conn.execute(
            "INSERT INTO community_profiles (
                tap_id, relative_path, manifest_path, game_name, trainer_name,
                compatibility_rating, author, trainer_loading_mode, schema_version, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, datetime('now'))",
            params![
                tap_id,
                relative_path,
                format!("{TAP_LOCAL_PATH}/{relative_path}"),
                game_name,
                trainer_name,
                rating,
                author,
                loading_mode,
            ],
        )
        .unwrap();
    }

    fn insert_test_source(
        conn: &Connection,
        tap_id: &str,
        game_name: &str,
        source_name: &str,
        source_url: &str,
    ) {
        conn.execute(
            "INSERT INTO trainer_sources (tap_id, game_name, source_name, source_url, relative_path, created_at)
             VALUES (?1, ?2, ?3, ?4, 'sources/test', datetime('now'))",
            params![tap_id, game_name, source_name, source_url],
        )
        .unwrap();
    }

    fn make_index_entry(
        game_name: &str,
        relative_path: &str,
        rating: CompatibilityRating,
        loading_mode: TrainerLoadingMode,
        author: &str,
    ) -> CommunityProfileIndexEntry {
        let mut profile = GameProfile::default();
        profile.trainer.loading_mode = loading_mode;
        CommunityProfileIndexEntry {
            tap_url: TAP_URL.to_string(),
            tap_branch: None,
            tap_path: PathBuf::from(TAP_LOCAL_PATH),
            manifest_path: PathBuf::from(format!("{TAP_LOCAL_PATH}/{relative_path}")),
            relative_path: PathBuf::from(relative_path),
            manifest: CommunityProfileManifest::new(
                CommunityProfileMetadata {
                    game_name: game_name.to_string(),
                    game_version: String::new(),
                    trainer_name: String::new(),
                    trainer_version: String::new(),
                    proton_version: String::new(),
                    platform_tags: vec![],
                    compatibility_rating: rating,
                    author: author.to_string(),
                    description: String::new(),
                    trainer_sha256: None,
                },
                profile,
            ),
        }
    }

    fn make_source_manifest(
        game_name: &str,
        entries: Vec<TrainerSourceEntry>,
    ) -> TrainerSourcesManifest {
        TrainerSourcesManifest {
            schema_version: 1,
            game_name: game_name.to_string(),
            steam_app_id: None,
            sources: entries,
        }
    }

    fn make_source_entry(source_name: &str, source_url: &str) -> TrainerSourceEntry {
        TrainerSourceEntry {
            source_name: source_name.to_string(),
            source_url: source_url.to_string(),
            trainer_version: None,
            game_version: None,
            notes: None,
            sha256: None,
        }
    }

    #[test]
    fn fetch_catalog_rows_joins_tap_url_and_local_path() {
        let store = MetadataStore::open_in_memory().unwrap();
        store
            .with_sqlite_conn("test catalog join", |conn| {
                insert_test_tap(conn, "tap-001", TAP_URL, TAP_LOCAL_PATH);
                insert_test_profile(
                    conn,
                    "tap-001",
                    "elden/community-profile.json",
                    Some("Elden Ring"),
                    None,
                    Some("working"),
                    Some("source_directory"),
                    None,
                );

                let rows = fetch_catalog_rows(conn)?;
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].tap_id, "tap-001");
                assert_eq!(rows[0].entry.tap_url, TAP_URL);
                assert_eq!(rows[0].entry.tap_local_path, TAP_LOCAL_PATH);
                assert!(rows[0].entry.id.is_some());
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn fetch_catalog_rows_reads_null_and_populated_trainer_loading_mode() {
        let store = MetadataStore::open_in_memory().unwrap();
        store
            .with_sqlite_conn("test loading mode read", |conn| {
                insert_test_tap(conn, "tap-001", TAP_URL, TAP_LOCAL_PATH);
                insert_test_profile(
                    conn,
                    "tap-001",
                    "a/community-profile.json",
                    Some("Alpha"),
                    None,
                    None,
                    Some("copy_to_prefix"),
                    None,
                );
                insert_test_profile(
                    conn,
                    "tap-001",
                    "b/community-profile.json",
                    Some("Beta"),
                    None,
                    None,
                    None,
                    None,
                );

                let rows = fetch_catalog_rows(conn)?;
                assert_eq!(rows.len(), 2);
                let alpha = rows
                    .iter()
                    .find(|row| row.entry.game_name.as_deref() == Some("Alpha"))
                    .unwrap();
                let beta = rows
                    .iter()
                    .find(|row| row.entry.game_name.as_deref() == Some("Beta"))
                    .unwrap();
                assert_eq!(
                    alpha.entry.trainer_loading_mode.as_deref(),
                    Some("copy_to_prefix")
                );
                assert_eq!(beta.entry.trainer_loading_mode, None);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn fetch_source_groups_returns_all_rows_in_stable_order() {
        let store = MetadataStore::open_in_memory().unwrap();
        store
            .with_sqlite_conn("test source groups order", |conn| {
                insert_test_tap(conn, "tap-001", TAP_URL, TAP_LOCAL_PATH);
                insert_test_source(
                    conn,
                    "tap-001",
                    "Zeta",
                    "B Source",
                    "https://example.com/z.exe",
                );
                insert_test_source(
                    conn,
                    "tap-001",
                    "Alpha",
                    "Z Source",
                    "https://example.com/a2.exe",
                );
                insert_test_source(
                    conn,
                    "tap-001",
                    "Alpha",
                    "A Source",
                    "https://example.com/a1.exe",
                );

                let sources = fetch_source_groups(conn)?;
                let names: Vec<&str> = sources
                    .iter()
                    .map(|input| input.source.source_name.as_str())
                    .collect();
                assert_eq!(
                    names,
                    ["A Source", "Z Source", "B Source"],
                    "ordered by tap, game name, source name"
                );
                Ok(())
            })
            .unwrap();
    }

    fn seeded_index() -> CommunityProfileIndex {
        CommunityProfileIndex {
            entries: vec![
                make_index_entry(
                    "Elden Ring",
                    "elden/community-profile.json",
                    CompatibilityRating::Working,
                    TrainerLoadingMode::SourceDirectory,
                    "FromFan",
                ),
                make_index_entry(
                    "Sekiro",
                    "sekiro/community-profile.json",
                    CompatibilityRating::Platinum,
                    TrainerLoadingMode::CopyToPrefix,
                    "",
                ),
            ],
            diagnostics: Vec::new(),
            trainer_sources: vec![
                (
                    "sources/elden".to_string(),
                    make_source_manifest(
                        "Elden Ring",
                        vec![make_source_entry("FLiNG", "https://example.com/elden.exe")],
                    ),
                ),
                (
                    "sources/orphan".to_string(),
                    make_source_manifest(
                        "Orphan Game",
                        vec![make_source_entry("WeMod", "https://example.com/orphan.exe")],
                    ),
                ),
            ],
        }
    }

    /// Insert the same logical corpus [`seeded_index`] describes into the DB tables.
    fn seed_db(conn: &Connection) {
        insert_test_tap(conn, "tap-001", TAP_URL, TAP_LOCAL_PATH);
        insert_test_profile(
            conn,
            "tap-001",
            "elden/community-profile.json",
            Some("Elden Ring"),
            None,
            Some("working"),
            Some("source_directory"),
            Some("FromFan"),
        );
        insert_test_profile(
            conn,
            "tap-001",
            "sekiro/community-profile.json",
            Some("Sekiro"),
            None,
            Some("platinum"),
            Some("copy_to_prefix"),
            None,
        );
        insert_test_source(
            conn,
            "tap-001",
            "Elden Ring",
            "FLiNG",
            "https://example.com/elden.exe",
        );
        insert_test_source(
            conn,
            "tap-001",
            "Orphan Game",
            "WeMod",
            "https://example.com/orphan.exe",
        );
    }

    fn assert_pages_match(db_page: &CatalogPage, degraded_page: &CatalogPage) {
        assert!(!db_page.degraded);
        assert!(degraded_page.degraded);
        assert_eq!(db_page.total_count, degraded_page.total_count);
        assert_eq!(db_page.tap_count, degraded_page.tap_count);
        assert_eq!(db_page.facets, degraded_page.facets);
        assert_eq!(db_page.entries.len(), degraded_page.entries.len());
        for (db_entry, degraded_entry) in db_page.entries.iter().zip(&degraded_page.entries) {
            if db_entry.manifest_path.is_empty() {
                assert!(db_entry.id.is_none(), "source-only entries carry no rowid");
            } else {
                assert!(db_entry.id.is_some());
            }
            assert!(degraded_entry.id.is_none());
            let mut normalized = db_entry.clone();
            normalized.id = None;
            assert_eq!(&normalized, degraded_entry);
        }
    }

    #[test]
    fn db_and_degraded_paths_yield_identical_pages() {
        let store = MetadataStore::open_in_memory().unwrap();
        let index = seeded_index();

        let queries = vec![
            CatalogQuery::default(),
            CatalogQuery {
                query: Some("elden".to_string()),
                ..CatalogQuery::default()
            },
            CatalogQuery {
                query: Some("orphan".to_string()),
                ..CatalogQuery::default()
            },
            CatalogQuery {
                game_titles: vec!["Sekiro".to_string()],
                ..CatalogQuery::default()
            },
            CatalogQuery {
                loading_modes: vec!["copy_to_prefix".to_string()],
                ..CatalogQuery::default()
            },
            CatalogQuery {
                compatibility_bands: vec!["working".to_string()],
                ..CatalogQuery::default()
            },
            CatalogQuery {
                tap_urls: vec![TAP_URL.to_string()],
                ..CatalogQuery::default()
            },
        ];

        store
            .with_sqlite_conn("test db/degraded parity", |conn| {
                seed_db(conn);
                for query in &queries {
                    let db_page = query_community_catalog(conn, query)?;
                    let degraded_page = build_catalog_page(
                        rows_from_index(&index),
                        sources_from_index(TAP_URL, TAP_LOCAL_PATH, &index.trainer_sources),
                        query,
                        true,
                    );
                    assert_pages_match(&db_page, &degraded_page);
                }
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn source_only_entries_appear_identically_in_both_paths() {
        let store = MetadataStore::open_in_memory().unwrap();
        let index = seeded_index();

        store
            .with_sqlite_conn("test source-only parity", |conn| {
                seed_db(conn);
                let db_page = query_community_catalog(conn, &CatalogQuery::default())?;
                let degraded_page = build_catalog_page(
                    rows_from_index(&index),
                    sources_from_index(TAP_URL, TAP_LOCAL_PATH, &index.trainer_sources),
                    &CatalogQuery::default(),
                    true,
                );

                for page in [&db_page, &degraded_page] {
                    let orphan = page
                        .entries
                        .iter()
                        .find(|entry| entry.game_name.as_deref() == Some("Orphan Game"))
                        .expect("source-only game must surface in the catalog");
                    assert!(orphan.manifest_path.is_empty());
                    assert_eq!(orphan.tap_url, TAP_URL);
                    assert_eq!(orphan.tap_local_path, TAP_LOCAL_PATH);
                    assert_eq!(orphan.sources.len(), 1);
                    assert_eq!(orphan.sources[0].source_name, "WeMod");
                }
                assert_pages_match(&db_page, &degraded_page);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn degraded_sources_apply_https_and_byte_cap_filters() {
        let sources = vec![(
            "sources/mixed".to_string(),
            make_source_manifest(
                "Elden Ring",
                vec![
                    make_source_entry("Valid", "https://example.com/ok.exe"),
                    make_source_entry("Insecure", "http://example.com/bad.exe"),
                    make_source_entry(
                        &"a".repeat(MAX_SOURCE_NAME_BYTES + 1),
                        "https://example.com/long-name.exe",
                    ),
                ],
            ),
        )];

        let inputs = sources_from_index(TAP_URL, TAP_LOCAL_PATH, &sources);
        assert_eq!(
            inputs.len(),
            1,
            "http and oversized-name sources are dropped"
        );
        assert_eq!(inputs[0].source.source_name, "Valid");
        assert_eq!(inputs[0].tap_id, TAP_URL);
        assert_eq!(inputs[0].tap_url, TAP_URL);
        assert_eq!(inputs[0].tap_local_path, TAP_LOCAL_PATH);
        assert_eq!(inputs[0].game_name, "Elden Ring");
    }

    #[test]
    fn degraded_catalog_skips_failing_taps_and_keeps_good_ones() {
        let temp = tempfile::tempdir().unwrap();
        let store = crate::community::CommunityTapStore::with_base_path(temp.path().join("taps"));

        let good = crate::community::CommunityTapSubscription {
            url: "https://example.com/good-tap.git".to_string(),
            branch: None,
            pinned_commit: None,
        };
        let bad = crate::community::CommunityTapSubscription {
            url: "git://forbidden.example/tap.git".to_string(),
            branch: None,
            pinned_commit: None,
        };

        let workspace = store.resolve_workspace(&good).unwrap();
        let profile_dir = workspace.local_path.join("profiles/good-game");
        std::fs::create_dir_all(&profile_dir).unwrap();
        let manifest = CommunityProfileManifest::new(
            CommunityProfileMetadata {
                game_name: "Good Game".to_string(),
                game_version: String::new(),
                trainer_name: String::new(),
                trainer_version: String::new(),
                proton_version: String::new(),
                platform_tags: vec![],
                compatibility_rating: CompatibilityRating::Working,
                author: String::new(),
                description: String::new(),
                trainer_sha256: None,
            },
            GameProfile::default(),
        );
        std::fs::write(
            profile_dir.join("community-profile.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();

        let page = degraded_catalog_from_taps(&store, &[bad, good], &CatalogQuery::default());

        assert!(page.degraded);
        assert_eq!(
            page.total_count, 1,
            "the bad tap is skipped, the good tap still serves entries"
        );
        assert_eq!(page.entries[0].game_name.as_deref(), Some("Good Game"));
    }
}
