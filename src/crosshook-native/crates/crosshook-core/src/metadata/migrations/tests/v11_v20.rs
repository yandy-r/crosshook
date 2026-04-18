use crate::metadata::db;

use super::super::run_migrations;

#[test]
fn migration_11_to_12_creates_optimization_catalog_table() {
    let conn = db::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();
    // Verify the table exists by querying it
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM optimization_catalog", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn migration_12_to_13_creates_offline_tables() {
    let conn = db::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();
    for table in [
        "trainer_hash_cache",
        "offline_readiness_snapshots",
        "community_tap_offline_state",
    ] {
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "missing table {table}");
    }
}

#[test]
fn migration_13_to_14_creates_game_image_cache_table() {
    let conn = db::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();

    // Verify the table exists
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'game_image_cache'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(n, 1, "missing table game_image_cache");

    // Verify the unique index exists
    let idx: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_game_image_cache_app_type_source'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(idx, 1, "missing index idx_game_image_cache_app_type_source");

    let expires_idx: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_game_image_cache_expires'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(expires_idx, 1, "missing index idx_game_image_cache_expires");
}

#[test]
fn migration_14_to_15_creates_prefix_dependency_state_table() {
    let conn = db::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();

    // Verify schema version (latest after all migrations)
    let version: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert!(
        version >= 15,
        "schema version should be at least 15, got {version}"
    );

    // Verify table exists
    let table_exists: bool = conn
        .prepare(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='prefix_dependency_state'",
        )
        .unwrap()
        .exists([])
        .unwrap();
    assert!(table_exists, "prefix_dependency_state table should exist");

    // Verify unique index exists
    let idx_exists: bool = conn
        .prepare("SELECT 1 FROM sqlite_master WHERE type='index' AND name='idx_prefix_dep_state_profile_package_prefix'")
        .unwrap()
        .exists([])
        .unwrap();
    assert!(idx_exists, "unique index should exist");

    // Verify FK cascade: insert profile, insert dep state, delete profile, verify cascade
    conn.execute(
        "INSERT INTO profiles (profile_id, current_filename, current_path, game_name, created_at, updated_at)
         VALUES ('test-prof', 'test.toml', '/tmp/test.toml', 'Test', datetime('now'), datetime('now'))",
        [],
    ).unwrap();

    conn.execute(
        "INSERT INTO prefix_dependency_state (profile_id, package_name, prefix_path, state, created_at, updated_at)
         VALUES ('test-prof', 'vcrun2019', '/tmp/pfx', 'installed', datetime('now'), datetime('now'))",
        [],
    ).unwrap();

    // Delete profile
    conn.execute("DELETE FROM profiles WHERE profile_id = 'test-prof'", [])
        .unwrap();

    // Verify cascade deleted the dep state
    let dep_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM prefix_dependency_state WHERE profile_id = 'test-prof'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        dep_count, 0,
        "dep state should be cascade-deleted with profile"
    );
}

#[test]
fn migration_15_to_16_creates_prefix_storage_tables() {
    let conn = db::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();

    let version: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert!(
        version >= 16,
        "schema version should be at least 16, got {version}"
    );

    for table in ["prefix_storage_snapshots", "prefix_storage_cleanup_audit"] {
        let exists: bool = conn
            .prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1")
            .unwrap()
            .exists([table])
            .unwrap();
        assert!(exists, "missing table {table}");
    }

    for idx in [
        "idx_prefix_storage_snapshots_prefix_path_scanned_at",
        "idx_prefix_storage_snapshots_scanned_at",
        "idx_prefix_storage_cleanup_audit_created_at",
        "idx_prefix_storage_cleanup_audit_prefix_path",
    ] {
        let exists: bool = conn
            .prepare("SELECT 1 FROM sqlite_master WHERE type='index' AND name=?1")
            .unwrap()
            .exists([idx])
            .unwrap();
        assert!(exists, "missing index {idx}");
    }
}

#[test]
fn migration_16_to_17_creates_suggestion_dismissals_table() {
    let conn = db::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();

    let version: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert!(
        version >= 17,
        "schema version should be at least 17, got {version}"
    );

    let table_exists: bool = conn
        .prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name='suggestion_dismissals'")
        .unwrap()
        .exists([])
        .unwrap();
    assert!(table_exists, "suggestion_dismissals table should exist");

    let idx_exists: bool = conn
        .prepare("SELECT 1 FROM sqlite_master WHERE type='index' AND name='idx_suggestion_dismissals_unique'")
        .unwrap()
        .exists([])
        .unwrap();
    assert!(
        idx_exists,
        "unique index idx_suggestion_dismissals_unique should exist"
    );
}

#[test]
fn migration_17_to_18_creates_trainer_sources_table() {
    let conn = db::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();

    let version: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert!(
        version >= 18,
        "schema version should be at least 18, got {version}"
    );

    let table_exists: bool = conn
        .prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name='trainer_sources'")
        .unwrap()
        .exists([])
        .unwrap();
    assert!(table_exists, "trainer_sources table should exist");

    let game_idx_exists: bool = conn
        .prepare(
            "SELECT 1 FROM sqlite_master WHERE type='index' AND name='idx_trainer_sources_game'",
        )
        .unwrap()
        .exists([])
        .unwrap();
    assert!(
        game_idx_exists,
        "index idx_trainer_sources_game should exist"
    );

    let app_id_idx_exists: bool = conn
        .prepare(
            "SELECT 1 FROM sqlite_master WHERE type='index' AND name='idx_trainer_sources_app_id'",
        )
        .unwrap()
        .exists([])
        .unwrap();
    assert!(
        app_id_idx_exists,
        "index idx_trainer_sources_app_id should exist"
    );
}

#[test]
fn migration_18_to_19_adds_sort_order_and_cascade() {
    let conn = db::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();

    let version: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert!(
        version >= 19,
        "schema version should be at least 19, got {version}"
    );

    // 1. sort_order column exists with NOT NULL DEFAULT 0.
    let mut stmt = conn.prepare("PRAGMA table_info(collections)").unwrap();
    let columns: Vec<(String, String, i64, Option<String>)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,         // name
                row.get::<_, String>(2)?,         // type
                row.get::<_, i64>(3)?,            // notnull
                row.get::<_, Option<String>>(4)?, // dflt_value
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let sort_order = columns
        .iter()
        .find(|(name, _, _, _)| name == "sort_order")
        .expect("sort_order column should exist");
    assert_eq!(sort_order.1, "INTEGER");
    assert_eq!(sort_order.2, 1, "sort_order should be NOT NULL");
    assert_eq!(
        sort_order.3.as_deref(),
        Some("0"),
        "sort_order should default to 0"
    );

    // 2. collection_profiles.profile_id FK has ON DELETE CASCADE.
    // Insert a profile, add to a collection, delete the profile, verify the
    // membership row cascades away.
    conn.execute(
        "INSERT INTO profiles (profile_id, current_filename, current_path, game_name, created_at, updated_at)
         VALUES ('pf-1', 'game.toml', '/tmp/game.toml', 'Game', datetime('now'), datetime('now'))",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO collections (collection_id, name, created_at, updated_at)
         VALUES ('col-1', 'Test', datetime('now'), datetime('now'))",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO collection_profiles (collection_id, profile_id, added_at)
         VALUES ('col-1', 'pf-1', datetime('now'))",
        [],
    )
    .unwrap();

    conn.execute("DELETE FROM profiles WHERE profile_id = 'pf-1'", [])
        .unwrap();

    let orphan_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM collection_profiles WHERE profile_id = 'pf-1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        orphan_count, 0,
        "collection_profiles rows must cascade when the profile is deleted"
    );

    // 3. Collection→collection_profiles cascade still works (regression check).
    let member_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM collection_profiles WHERE collection_id = ?1",
            ["col-1"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(member_count, 0);
}

#[test]
fn migration_19_to_20_adds_defaults_json_column() {
    let conn = db::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();

    let version: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert!(
        version >= 20,
        "schema version should be at least 20 after migration 19→20, got {version}"
    );

    // Verify defaults_json column exists, is TEXT, and is nullable.
    let mut stmt = conn.prepare("PRAGMA table_info(collections)").unwrap();
    let columns: Vec<(String, String, i64)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?, // name
                row.get::<_, String>(2)?, // type
                row.get::<_, i64>(3)?,    // notnull
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let defaults_json = columns
        .iter()
        .find(|(name, _, _)| name == "defaults_json")
        .expect("defaults_json column should exist");
    assert_eq!(defaults_json.1, "TEXT");
    assert_eq!(defaults_json.2, 0, "defaults_json should be nullable");

    // Round-trip a JSON payload.
    conn.execute(
        "INSERT INTO collections (collection_id, name, created_at, updated_at, defaults_json)
         VALUES ('col-1', 'Test', datetime('now'), datetime('now'), ?1)",
        [r#"{"method":"proton_run"}"#],
    )
    .unwrap();
    let payload: Option<String> = conn
        .query_row(
            "SELECT defaults_json FROM collections WHERE collection_id = 'col-1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(payload.as_deref(), Some(r#"{"method":"proton_run"}"#));

    // NULL round-trip — column defaults to NULL when not provided.
    conn.execute(
        "INSERT INTO collections (collection_id, name, created_at, updated_at)
         VALUES ('col-2', 'Test2', datetime('now'), datetime('now'))",
        [],
    )
    .unwrap();
    let empty: Option<String> = conn
        .query_row(
            "SELECT defaults_json FROM collections WHERE collection_id = 'col-2'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(empty, None, "defaults_json should default to NULL");
}
