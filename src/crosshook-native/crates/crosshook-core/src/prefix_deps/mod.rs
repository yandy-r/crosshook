pub mod detection;
pub mod lock;
pub mod models;
pub mod runner;
pub mod validation;

pub use models::*;

#[cfg(test)]
mod integration_tests {
    use crate::metadata::{MetadataStore, MetadataStoreError};
    use crate::prefix_deps::validation::validate_protontricks_verbs;
    use chrono::Utc;
    use rusqlite::params;

    /// Create an in-memory MetadataStore (migrations run internally) and insert a test profile.
    fn setup_store() -> MetadataStore {
        let store = MetadataStore::open_in_memory().unwrap();
        store
            .with_sqlite_conn("insert test profile", |conn| {
                conn.execute(
                    "INSERT INTO profiles (profile_id, current_filename, current_path, game_name, created_at, updated_at)
                     VALUES ('integ-prof', 'integ.toml', '/tmp/integ.toml', 'Integration Test', datetime('now'), datetime('now'))",
                    [],
                )
                .map(|_| ())
                .map_err(|source| MetadataStoreError::Database {
                    action: "insert integration test profile",
                    source,
                })
            })
            .unwrap();
        store
    }

    #[test]
    fn full_check_and_store_cycle() {
        let store = setup_store();

        // Simulate a check result: some packages installed, one missing
        let known_installed = vec!["vcrun2019".to_string(), "dotnet48".to_string()];
        let required = vec![
            "vcrun2019".to_string(),
            "dotnet48".to_string(),
            "d3dx9".to_string(),
        ];

        // Upsert states based on simulated check
        for verb in &required {
            let state = if known_installed.contains(verb) {
                "installed"
            } else {
                "missing"
            };
            store
                .upsert_prefix_dep_state("integ-prof", verb, "/tmp/pfx", state, None)
                .unwrap();
        }

        // Query back and verify
        let rows = store.load_prefix_dep_states("integ-prof").unwrap();
        assert_eq!(rows.len(), 3);

        // d3dx9 should be first (alphabetical order)
        assert_eq!(rows[0].package_name, "d3dx9");
        assert_eq!(rows[0].state, "missing");

        assert_eq!(rows[1].package_name, "dotnet48");
        assert_eq!(rows[1].state, "installed");

        assert_eq!(rows[2].package_name, "vcrun2019");
        assert_eq!(rows[2].state, "installed");

        // Simulate installing d3dx9 -- upsert changes state
        store
            .upsert_prefix_dep_state("integ-prof", "d3dx9", "/tmp/pfx", "installed", None)
            .unwrap();

        let updated = store
            .load_prefix_dep_state("integ-prof", "d3dx9", "/tmp/pfx")
            .unwrap()
            .unwrap();
        assert_eq!(updated.state, "installed");
        assert!(updated.installed_at.is_some());
    }

    #[test]
    fn stale_cache_cleared_after_ttl() {
        let store = setup_store();

        // Insert with old checked_at timestamp (48 hours ago)
        let old_time = (Utc::now() - chrono::Duration::hours(48)).to_rfc3339();
        store
            .with_sqlite_conn("insert stale dep state", |conn| {
                conn.execute(
                    "INSERT INTO prefix_dependency_state
                     (profile_id, package_name, prefix_path, state, checked_at, created_at, updated_at)
                     VALUES ('integ-prof', 'vcrun2019', '/tmp/pfx', 'installed', ?1, ?1, ?1)",
                    params![old_time],
                )
                .map(|_| ())
                .map_err(|source| MetadataStoreError::Database {
                    action: "insert stale dep state",
                    source,
                })
            })
            .unwrap();

        // Insert a fresh one
        store
            .upsert_prefix_dep_state("integ-prof", "dotnet48", "/tmp/pfx", "installed", None)
            .unwrap();

        // Clear stale (24h TTL)
        let deleted = store.clear_stale_prefix_dep_states(24).unwrap();
        assert_eq!(deleted, 1, "should delete only the stale entry");

        let remaining = store.load_prefix_dep_states("integ-prof").unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].package_name, "dotnet48");
    }

    #[test]
    fn validation_blocks_bad_verbs_before_store() {
        let store = setup_store();

        // Attempt to validate bad verbs
        let bad_verbs = vec!["-q".to_string()];
        let result = validate_protontricks_verbs(&bad_verbs);
        assert!(
            result.is_err(),
            "validation should reject flag-injection verbs"
        );

        // Verify nothing was stored (since validation failed before any DB operation)
        let rows = store.load_prefix_dep_states("integ-prof").unwrap();
        assert!(
            rows.is_empty(),
            "no rows should be stored when validation fails"
        );
    }
}
