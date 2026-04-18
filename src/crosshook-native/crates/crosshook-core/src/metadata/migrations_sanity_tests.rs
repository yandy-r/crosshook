#![cfg(test)]

use rusqlite::params;

use super::test_support::{connection, sample_profile};
use super::{MetadataStore, SyncSource};

#[test]
fn test_migration_9_to_10_seeds_bundled_gpu_presets() {
    let store = MetadataStore::open_in_memory().unwrap();
    let rows = store.list_bundled_optimization_presets().unwrap();
    assert_eq!(rows.len(), 4);
    let ids: Vec<_> = rows.iter().map(|r| r.preset_id.as_str()).collect();
    assert!(ids.contains(&"nvidia_performance"));
    assert!(ids.contains(&"nvidia_quality"));
    assert!(ids.contains(&"amd_performance"));
    assert!(ids.contains(&"amd_quality"));
}

#[test]
fn test_migration_8_to_9_version_snapshots_table_exists() {
    let store = MetadataStore::open_in_memory().unwrap();
    let profile = sample_profile();
    let path = std::path::Path::new("/profiles/test-game.toml");

    // Seed a profile row so the FK constraint is satisfied.
    store
        .observe_profile_write("test-game", &profile, path, SyncSource::AppWrite, None)
        .unwrap();

    let conn = connection(&store);

    // Retrieve the profile_id for the seeded row.
    let profile_id: String = conn
        .query_row(
            "SELECT profile_id FROM profiles WHERE current_filename = ?1",
            params!["test-game"],
            |row| row.get(0),
        )
        .unwrap();

    // INSERT roundtrip: verify the table and its columns exist.
    conn.execute(
        "INSERT INTO version_snapshots
            (profile_id, steam_app_id, steam_build_id, trainer_version,
             trainer_file_hash, human_game_ver, status, checked_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            profile_id,
            "1245620",
            "12345678",
            "v1.0.0",
            "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
            "1.0",
            "untracked",
            "2026-01-01T00:00:00+00:00",
        ],
    )
    .unwrap();

    let (row_profile_id, steam_app_id, steam_build_id, status): (
        String,
        String,
        Option<String>,
        String,
    ) = conn
        .query_row(
            "SELECT profile_id, steam_app_id, steam_build_id, status
             FROM version_snapshots
             WHERE profile_id = ?1",
            params![profile_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();

    assert_eq!(row_profile_id, profile_id);
    assert_eq!(steam_app_id, "1245620");
    assert_eq!(steam_build_id.as_deref(), Some("12345678"));
    assert_eq!(status, "untracked");
}
