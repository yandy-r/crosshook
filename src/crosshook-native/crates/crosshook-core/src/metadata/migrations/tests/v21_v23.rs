use crate::metadata::db;

use super::super::run_migrations;
use super::super::v21_v23;

#[test]
fn migration_20_to_21_creates_readiness_tables() {
    let conn = db::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();

    let version: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert!(
        version >= 21,
        "schema version should be at least 21 after migration 20→21, got {version}"
    );

    for table in [
        "host_readiness_catalog",
        "readiness_nag_dismissals",
        "host_readiness_snapshots",
    ] {
        let exists: bool = conn
            .prepare(&format!(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='{table}'"
            ))
            .unwrap()
            .exists([])
            .unwrap();
        assert!(exists, "table {table} should exist");
    }

    let idx_exists: bool = conn
        .prepare(
            "SELECT 1 FROM sqlite_master WHERE type='index' AND name='idx_readiness_nag_dismissals_tool_id'",
        )
        .unwrap()
        .exists([])
        .unwrap();
    assert!(
        idx_exists,
        "unique index idx_readiness_nag_dismissals_tool_id should exist"
    );
}

#[test]
fn migration_21_to_22_creates_proton_release_catalog_table() {
    let conn = db::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();

    let version: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert!(
        version >= 22,
        "schema version should be at least 22 after migration 21→22, got {version}"
    );

    // Table exists.
    let table_exists: bool = conn
        .prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name='proton_release_catalog'")
        .unwrap()
        .exists([])
        .unwrap();
    assert!(table_exists, "proton_release_catalog table should exist");

    // Index exists.
    let idx_exists: bool = conn
        .prepare(
            "SELECT 1 FROM sqlite_master WHERE type='index' AND name='idx_proton_catalog_provider_fetched'",
        )
        .unwrap()
        .exists([])
        .unwrap();
    assert!(
        idx_exists,
        "index idx_proton_catalog_provider_fetched should exist"
    );
}

#[test]
fn migration_21_to_22_evicts_legacy_protonup_cache_entries() {
    // Run up to version 21 so external_cache_entries exists.
    let conn = db::open_in_memory().unwrap();
    // Run all migrations up to 21 manually (stop before 22) by running the full set;
    // at schema v21 the external_cache_entries table already exists (added in v3→v4).
    run_migrations(&conn).unwrap();

    // Reset to v21 and re-insert the legacy row to simulate a pre-migration state.
    // Because run_migrations is idempotent (CREATE TABLE IF NOT EXISTS), we simulate the
    // v21 steady state by inserting a legacy cache row, dropping down to v21, re-running
    // only migrate_21_to_22, and asserting it's gone.
    //
    // Simpler approach: insert a legacy row now (post-full-migration, table still exists),
    // verify it is deleted by checking current count.
    conn.execute(
        "INSERT INTO external_cache_entries
         (cache_id, source_url, cache_key, payload_json, payload_size, fetched_at, expires_at, created_at, updated_at)
         VALUES ('test-id', 'https://example.com', 'protonup:catalog:ge-proton', '{}', 2,
                 datetime('now'), datetime('now', '+1 hour'), datetime('now'), datetime('now'))",
        [],
    )
    .unwrap();

    // Confirm the row is visible before the migration evicts it.
    let before: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM external_cache_entries WHERE cache_key LIKE 'protonup:catalog:%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(before, 1, "legacy row should be present before eviction");

    // Run the migration directly (simulating what run_migrations does at v21→v22).
    v21_v23::migrate_21_to_22(&conn).unwrap();

    // Assert the legacy row is gone.
    let after: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM external_cache_entries WHERE cache_key LIKE 'protonup:catalog:%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        after, 0,
        "legacy protonup:catalog:* entries should be evicted by the migration"
    );
}

#[test]
fn migration_22_to_23_evicts_stale_proton_release_catalog_rows() {
    // Run migrations so proton_release_catalog exists at v22.
    let conn = db::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();

    // Seed a row as if written by v22 code (no published_at in payload_json).
    conn.execute(
        "INSERT INTO proton_release_catalog
         (provider_id, version_tag, payload_json, fetched_at)
         VALUES ('ge-proton', 'GE-Proton9-21',
                 '{\"provider\":\"ge-proton\",\"version\":\"GE-Proton9-21\"}',
                 datetime('now'))",
        [],
    )
    .unwrap();

    let before: i64 = conn
        .query_row("SELECT COUNT(*) FROM proton_release_catalog", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(before, 1, "seeded row must exist before eviction");

    v21_v23::migrate_22_to_23(&conn).unwrap();

    let after: i64 = conn
        .query_row("SELECT COUNT(*) FROM proton_release_catalog", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(
        after, 0,
        "v22→v23 migration must evict proton_release_catalog rows so published_at repopulates"
    );
}
