//! Tests for community profile index operations.

use super::constants::*;
use super::db;
use super::helpers::check_a6_bounds;
use super::trainer_sources::index_trainer_sources;
use crate::community::index::CommunityProfileIndexEntry;
use crate::community::{CommunityProfileManifest, CommunityProfileMetadata, CompatibilityRating};
use crate::discovery::models::{TrainerSourceEntry, TrainerSourcesManifest};
use crate::metadata::migrations;
use crate::profile::GameProfile;
use rusqlite::{params, Connection};
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

fn make_trainer_source_entry(name: &str, url: &str, notes: Option<String>) -> TrainerSourceEntry {
    TrainerSourceEntry {
        source_name: name.to_string(),
        source_url: url.to_string(),
        trainer_version: None,
        game_version: None,
        notes,
        sha256: None,
    }
}

fn make_manifest_with_entry(game_name: &str, entry: TrainerSourceEntry) -> TrainerSourcesManifest {
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
    let entry = make_trainer_source_entry("Valid Source", "https://example.com/trainer.exe", None);
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
    let entry = make_trainer_source_entry(&oversized_name, "https://example.com/trainer.exe", None);
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
