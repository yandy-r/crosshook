mod cache_ops;
mod cache_store;
mod catalog_ops;
mod collections;
mod collections_ops;
mod community_index;
mod community_ops;
mod config_history_ops;
mod config_history_store;
mod db;
mod game_image_ops;
mod game_image_store;
mod health_ops;
mod health_store;
mod launch_history;
mod launch_queries;
mod launcher_ops;
mod launcher_sync;
mod migrations;
mod models;
mod offline_ops;
pub(crate) mod offline_store;
mod optimization_catalog_store;
mod prefix_deps_store;
mod prefix_ops;
mod prefix_storage_store;
mod preset_ops;
mod preset_store;
mod profile_ops;
pub mod profile_sync;
mod proton_catalog_store;
mod readiness_catalog_store;
mod readiness_dismissal_store;
mod readiness_snapshot_store;
mod store;
mod suggestion_store;
mod util;
mod version_ops;
mod version_store;

pub use game_image_store::GameImageCacheRow;
pub use health_store::HealthSnapshotRow;
pub use models::{
    BundledOptimizationPresetRow, CacheEntryStatus, CollectionRow, CommunityProfileRow,
    CommunityTapRow, ConfigRevisionRow, ConfigRevisionSource, DriftState, FailureTrendRow,
    LaunchOutcome, MetadataStoreError, PrefixDependencyStateRow, PrefixStorageCleanupAuditRow,
    PrefixStorageSnapshotRow, ProfileLaunchPresetOrigin, SyncReport, SyncSource,
    VersionCorrelationStatus, VersionSnapshotRow, MAX_CACHE_PAYLOAD_BYTES,
    MAX_CONFIG_REVISIONS_PER_PROFILE, MAX_DIAGNOSTIC_JSON_BYTES, MAX_HISTORY_LIST_LIMIT,
    MAX_SNAPSHOT_TOML_BYTES, MAX_VERSION_SNAPSHOTS_PER_PROFILE,
};
pub use offline_store::{CommunityTapOfflineRow, OfflineReadinessRow, TrainerHashCacheRow};
pub use profile_sync::sha256_hex;
pub use proton_catalog_store::ProtonCatalogRow;
pub use readiness_snapshot_store::HostReadinessSnapshotRow;
pub use store::MetadataStore;
pub use version_store::{compute_correlation_status, hash_trainer_file};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::community::index::{CommunityProfileIndex, CommunityProfileIndexEntry};
    use crate::community::taps::{
        CommunityTapSubscription, CommunityTapSyncResult, CommunityTapSyncStatus,
        CommunityTapWorkspace,
    };
    use crate::community::{
        CommunityProfileManifest, CommunityProfileMetadata, CompatibilityRating,
    };
    use crate::launch::diagnostics::models::{
        ActionableSuggestion, DiagnosticReport, ExitCodeInfo, FailureMode,
    };
    use crate::launch::request::ValidationSeverity;
    use crate::profile::{
        CollectionDefaultsSection, GameProfile, GameSection, InjectionSection, LaunchSection,
        LauncherSection, LocalOverrideSection, ProfileStore, RuntimeSection, SteamSection,
        TrainerLoadingMode, TrainerSection,
    };
    use rusqlite::{params, Connection};
    use std::fs;
    use std::os::unix::fs::{symlink, PermissionsExt};
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn sample_profile() -> GameProfile {
        GameProfile {
            game: GameSection {
                name: "Elden Ring".to_string(),
                executable_path: "/games/elden-ring/eldenring.exe".to_string(),
                custom_cover_art_path: String::new(),
                custom_portrait_art_path: String::new(),
                custom_background_art_path: String::new(),
            },
            trainer: TrainerSection {
                path: "/trainers/elden-ring.exe".to_string(),
                kind: "fling".to_string(),
                loading_mode: TrainerLoadingMode::SourceDirectory,
                trainer_type: "unknown".to_string(),
                required_protontricks: Vec::new(),
                community_trainer_sha256: String::new(),
            },
            injection: InjectionSection {
                dll_paths: vec!["/dlls/a.dll".to_string(), "/dlls/b.dll".to_string()],
                inject_on_launch: vec![true, false],
            },
            steam: SteamSection {
                enabled: true,
                app_id: "1245620".to_string(),
                compatdata_path: "/steam/compatdata/1245620".to_string(),
                proton_path: "/steam/proton/proton".to_string(),
                launcher: LauncherSection {
                    icon_path: "/icons/elden-ring.png".to_string(),
                    display_name: "Elden Ring".to_string(),
                },
            },
            runtime: RuntimeSection {
                prefix_path: String::new(),
                proton_path: String::new(),
                working_directory: String::new(),
                steam_app_id: String::new(),
                umu_game_id: String::new(),
                umu_preference: None,
            },
            launch: LaunchSection {
                method: "steam_applaunch".to_string(),
                ..Default::default()
            },
            local_override: LocalOverrideSection::default(),
        }
    }

    fn connection(store: &MetadataStore) -> std::sync::MutexGuard<'_, Connection> {
        store
            .conn
            .as_ref()
            .expect("metadata store should expose a connection in tests")
            .lock()
            .expect("metadata store mutex should not be poisoned")
    }

    #[test]
    fn test_observe_profile_write_creates_row() {
        let store = MetadataStore::open_in_memory().unwrap();
        let profile = sample_profile();
        let path = std::path::Path::new("/profiles/elden-ring.toml");

        store
            .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
            .unwrap();

        let conn = connection(&store);
        let (profile_id, current_filename, game_name, launch_method): (
            String,
            String,
            Option<String>,
            Option<String>,
        ) = conn
            .query_row(
                "SELECT profile_id, current_filename, game_name, launch_method FROM profiles WHERE current_filename = ?1",
                params!["elden-ring"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();

        assert!(!profile_id.trim().is_empty());
        assert_eq!(current_filename, "elden-ring");
        assert_eq!(game_name.as_deref(), Some("Elden Ring"));
        assert_eq!(launch_method.as_deref(), Some("steam_applaunch"));
    }

    #[test]
    fn test_observe_profile_write_idempotent() {
        let store = MetadataStore::open_in_memory().unwrap();
        let profile = sample_profile();
        let path = std::path::Path::new("/profiles/elden-ring.toml");

        store
            .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
            .unwrap();
        store
            .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
            .unwrap();

        let conn = connection(&store);
        let row_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM profiles WHERE current_filename = ?1",
                params!["elden-ring"],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(row_count, 1);
    }

    #[test]
    fn test_observe_profile_rename_creates_history() {
        let store = MetadataStore::open_in_memory().unwrap();
        let profile = sample_profile();
        let old_path = std::path::Path::new("/profiles/old-name.toml");
        let new_path = std::path::Path::new("/profiles/new-name.toml");

        store
            .observe_profile_write("old-name", &profile, old_path, SyncSource::AppWrite, None)
            .unwrap();
        store
            .observe_profile_rename("old-name", "new-name", old_path, new_path)
            .unwrap();

        let conn = connection(&store);
        let renamed_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM profiles WHERE current_filename = ?1",
                params!["new-name"],
                |row| row.get(0),
            )
            .unwrap();
        let history: (String, String, String, String) = conn
            .query_row(
                "SELECT old_name, new_name, old_path, new_path FROM profile_name_history",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();

        assert_eq!(renamed_count, 1);
        assert_eq!(history.0, "old-name");
        assert_eq!(history.1, "new-name");
        assert_eq!(history.2, old_path.to_string_lossy());
        assert_eq!(history.3, new_path.to_string_lossy());
    }

    #[test]
    fn test_observe_profile_delete_tombstones() {
        let store = MetadataStore::open_in_memory().unwrap();
        let profile = sample_profile();
        let path = std::path::Path::new("/profiles/elden-ring.toml");

        store
            .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
            .unwrap();
        store.observe_profile_delete("elden-ring").unwrap();

        let conn = connection(&store);
        let (row_count, deleted_at): (i64, Option<String>) = conn
            .query_row(
                "SELECT COUNT(*), MAX(deleted_at) FROM profiles WHERE current_filename = ?1",
                params!["elden-ring"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(row_count, 1);
        assert!(deleted_at.is_some());
    }

    #[test]
    fn test_sync_profiles_from_store() {
        let temp_dir = tempdir().unwrap();
        let store = MetadataStore::open_in_memory().unwrap();
        let profile_store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
        let profile = sample_profile();

        profile_store.save("alpha", &profile).unwrap();
        profile_store.save("beta", &profile).unwrap();
        profile_store.save("gamma", &profile).unwrap();

        let report = store.sync_profiles_from_store(&profile_store).unwrap();

        let conn = connection(&store);
        let row_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM profiles", [], |row| row.get(0))
            .unwrap();

        assert_eq!(report.profiles_seen, 3);
        assert_eq!(report.created, 3);
        assert_eq!(report.updated, 0);
        assert_eq!(report.deleted, 0);
        assert!(report.errors.is_empty());
        assert_eq!(row_count, 3);
    }

    #[test]
    fn test_unavailable_store_noop() {
        let temp_dir = tempdir().unwrap();
        let store = MetadataStore::disabled();
        let profile = sample_profile();
        let profile_store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));

        assert!(store
            .observe_profile_write(
                "elden-ring",
                &profile,
                std::path::Path::new("/profiles/elden-ring.toml"),
                SyncSource::AppWrite,
                None,
            )
            .is_ok());
        assert!(store
            .observe_profile_rename(
                "elden-ring",
                "elden-ring-renamed",
                std::path::Path::new("/profiles/elden-ring.toml"),
                std::path::Path::new("/profiles/elden-ring-renamed.toml"),
            )
            .is_ok());
        assert!(store.observe_profile_delete("elden-ring").is_ok());

        let report = store.sync_profiles_from_store(&profile_store).unwrap();
        assert_eq!(report.profiles_seen, 0);
        assert_eq!(report.created, 0);
        assert_eq!(report.updated, 0);
        assert_eq!(report.deleted, 0);
        assert!(report.errors.is_empty());
    }

    #[test]
    fn test_file_permissions() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("metadata.db");

        let _store = MetadataStore::with_path(&db_path).unwrap();

        let mode = fs::metadata(&db_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn test_symlink_rejected() {
        let temp_dir = tempdir().unwrap();
        let target_path = temp_dir.path().join("real-metadata.db");
        let symlink_path = temp_dir.path().join("metadata.db");

        fs::write(&target_path, b"").unwrap();
        symlink(&target_path, &symlink_path).unwrap();

        let error = match MetadataStore::with_path(&symlink_path) {
            Ok(_) => panic!("expected metadata symlink path to be rejected"),
            Err(error) => error,
        };
        assert!(matches!(error, MetadataStoreError::SymlinkDetected(path) if path == symlink_path));
    }

    fn clean_exit_report() -> DiagnosticReport {
        DiagnosticReport {
            severity: ValidationSeverity::Info,
            summary: "Clean exit".to_string(),
            exit_info: ExitCodeInfo {
                code: Some(0),
                signal: None,
                signal_name: None,
                core_dumped: false,
                failure_mode: FailureMode::CleanExit,
                description: "Process exited cleanly".to_string(),
                severity: ValidationSeverity::Info,
            },
            pattern_matches: vec![],
            suggestions: vec![],
            launch_method: "native".to_string(),
            log_tail_path: None,
            analyzed_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_observe_launcher_exported_creates_row() {
        let store = MetadataStore::open_in_memory().unwrap();

        store
            .observe_launcher_exported(
                None,
                "test-slug",
                "Test Name",
                "/path/script.sh",
                "/path/desktop.desktop",
            )
            .unwrap();

        let conn = connection(&store);
        let (launcher_id, slug, drift_state): (String, String, String) = conn
            .query_row(
                "SELECT launcher_id, launcher_slug, drift_state FROM launchers WHERE launcher_slug = ?1",
                params!["test-slug"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();

        assert!(!launcher_id.trim().is_empty());
        assert_eq!(slug, "test-slug");
        assert_eq!(drift_state, "aligned");
    }

    #[test]
    fn test_observe_launcher_exported_idempotent() {
        let store = MetadataStore::open_in_memory().unwrap();

        store
            .observe_launcher_exported(
                None,
                "test-slug",
                "Test Name",
                "/path/script.sh",
                "/path/desktop.desktop",
            )
            .unwrap();
        store
            .observe_launcher_exported(
                None,
                "test-slug",
                "Test Name Updated",
                "/path/script.sh",
                "/path/desktop.desktop",
            )
            .unwrap();

        let conn = connection(&store);
        let row_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM launchers WHERE launcher_slug = ?1",
                params!["test-slug"],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(row_count, 1);
    }

    #[test]
    fn test_observe_launcher_deleted_tombstones() {
        let store = MetadataStore::open_in_memory().unwrap();

        store
            .observe_launcher_exported(
                None,
                "test-slug",
                "Test Name",
                "/path/script.sh",
                "/path/desktop.desktop",
            )
            .unwrap();
        store.observe_launcher_deleted("test-slug").unwrap();

        let conn = connection(&store);
        let (row_count, drift_state): (i64, String) = conn
            .query_row(
                "SELECT COUNT(*), drift_state FROM launchers WHERE launcher_slug = ?1",
                params!["test-slug"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(row_count, 1);
        assert_eq!(drift_state, "missing");
    }

    #[test]
    fn test_record_launch_started_returns_operation_id() {
        let store = MetadataStore::open_in_memory().unwrap();

        let operation_id = store
            .record_launch_started(Some("test-profile"), "native", None)
            .unwrap();

        assert!(!operation_id.trim().is_empty());

        let conn = connection(&store);
        let (status, started_at): (String, String) = conn
            .query_row(
                "SELECT status, started_at FROM launch_operations WHERE operation_id = ?1",
                params![operation_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(status, "started");
        assert!(!started_at.trim().is_empty());
    }

    #[test]
    fn test_record_launch_finished_updates_row() {
        let store = MetadataStore::open_in_memory().unwrap();
        let report = clean_exit_report();

        let operation_id = store
            .record_launch_started(Some("test-profile"), "native", None)
            .unwrap();
        store
            .record_launch_finished(&operation_id, Some(0), None, &report)
            .unwrap();

        let conn = connection(&store);
        let (status, exit_code, diagnostic_json, severity, failure_mode, finished_at): (
            String,
            Option<i32>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ) = conn
            .query_row(
                "SELECT status, exit_code, diagnostic_json, severity, failure_mode, finished_at
                 FROM launch_operations WHERE operation_id = ?1",
                params![operation_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .unwrap();

        assert_eq!(status, "succeeded");
        assert_eq!(exit_code, Some(0));
        assert!(diagnostic_json.is_some());
        assert!(severity.is_some());
        assert!(failure_mode.is_some());
        assert!(finished_at.is_some());
    }

    #[test]
    fn test_diagnostic_json_truncated_at_4kb() {
        let store = MetadataStore::open_in_memory().unwrap();

        // (a) Small report — diagnostic_json should be stored
        let small_report = clean_exit_report();
        let small_json_len = serde_json::to_string(&small_report).unwrap().len();
        assert!(
            small_json_len < MAX_DIAGNOSTIC_JSON_BYTES,
            "small report ({small_json_len} bytes) must be under 4KB for this test"
        );

        let op_id_small = store.record_launch_started(None, "native", None).unwrap();
        store
            .record_launch_finished(&op_id_small, Some(0), None, &small_report)
            .unwrap();

        let (diagnostic_json_small, severity_small, failure_mode_small): (
            Option<String>,
            Option<String>,
            Option<String>,
        ) = {
            let conn = connection(&store);
            conn.query_row(
                "SELECT diagnostic_json, severity, failure_mode FROM launch_operations WHERE operation_id = ?1",
                params![op_id_small],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap()
        };

        assert!(
            diagnostic_json_small.is_some(),
            "small report should have diagnostic_json stored"
        );
        assert!(severity_small.is_some());
        assert!(failure_mode_small.is_some());

        // (b) Large report — diagnostic_json should be NULL but severity/failure_mode still populated
        let large_suggestions: Vec<ActionableSuggestion> = (0..100)
            .map(|i| ActionableSuggestion {
                title: format!("Suggestion title number {i} with extra padding to push over 4KB boundary"),
                description: format!(
                    "Suggestion description number {i} with a lot of extra text to ensure that the serialized JSON grows large enough to exceed the 4096-byte limit imposed by MAX_DIAGNOSTIC_JSON_BYTES"
                ),
                severity: ValidationSeverity::Warning,
            })
            .collect();

        let large_report = DiagnosticReport {
            severity: ValidationSeverity::Warning,
            summary: "Large report".to_string(),
            exit_info: ExitCodeInfo {
                code: Some(1),
                signal: None,
                signal_name: None,
                core_dumped: false,
                failure_mode: FailureMode::NonZeroExit,
                description: "Non-zero exit".to_string(),
                severity: ValidationSeverity::Warning,
            },
            pattern_matches: vec![],
            suggestions: large_suggestions,
            launch_method: "native".to_string(),
            log_tail_path: None,
            analyzed_at: "2026-01-01T00:00:00Z".to_string(),
        };

        let large_json_len = serde_json::to_string(&large_report).unwrap().len();
        assert!(
            large_json_len > MAX_DIAGNOSTIC_JSON_BYTES,
            "large report ({large_json_len} bytes) must exceed 4KB for this test"
        );

        let op_id_large = store.record_launch_started(None, "native", None).unwrap();
        store
            .record_launch_finished(&op_id_large, Some(1), None, &large_report)
            .unwrap();

        let (diagnostic_json_large, severity_large, failure_mode_large): (
            Option<String>,
            Option<String>,
            Option<String>,
        ) = {
            let conn = connection(&store);
            conn.query_row(
                "SELECT diagnostic_json, severity, failure_mode FROM launch_operations WHERE operation_id = ?1",
                params![op_id_large],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap()
        };

        assert!(
            diagnostic_json_large.is_none(),
            "large report should have diagnostic_json nullified"
        );
        assert!(
            severity_large.is_some(),
            "severity should still be populated even when diagnostic_json is null"
        );
        assert!(
            failure_mode_large.is_some(),
            "failure_mode should still be populated even when diagnostic_json is null"
        );
    }

    #[test]
    fn test_sweep_abandoned_marks_old_operations() {
        let store = MetadataStore::open_in_memory().unwrap();

        let operation_id = store.record_launch_started(None, "native", None).unwrap();

        let swept = store.sweep_abandoned_operations().unwrap();
        assert_eq!(swept, 1);

        let conn = connection(&store);
        let (status, finished_at): (String, Option<String>) = conn
            .query_row(
                "SELECT status, finished_at FROM launch_operations WHERE operation_id = ?1",
                params![operation_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(status, "abandoned");
        assert!(finished_at.is_some());
    }

    #[test]
    fn test_record_launch_finished_unknown_op_id_noop() {
        let store = MetadataStore::open_in_memory().unwrap();
        let report = clean_exit_report();

        let result = store.record_launch_finished("nonexistent-id", Some(0), None, &report);

        assert!(result.is_ok());
    }

    #[test]
    fn test_observe_launcher_renamed_atomic() {
        let store = MetadataStore::open_in_memory().unwrap();

        store
            .observe_launcher_exported(
                None,
                "old-slug",
                "Old Name",
                "/path/old-script.sh",
                "/path/old.desktop",
            )
            .unwrap();

        store
            .observe_launcher_renamed(
                "old-slug",
                "new-slug",
                "New Name",
                "/path/new-script.sh",
                "/path/new.desktop",
            )
            .unwrap();

        let conn = connection(&store);

        let old_drift_state: String = conn
            .query_row(
                "SELECT drift_state FROM launchers WHERE launcher_slug = ?1",
                params!["old-slug"],
                |row| row.get(0),
            )
            .unwrap();

        let new_drift_state: String = conn
            .query_row(
                "SELECT drift_state FROM launchers WHERE launcher_slug = ?1",
                params!["new-slug"],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(old_drift_state, "missing");
        assert_eq!(new_drift_state, "aligned");
    }

    #[test]
    fn test_phase2_disabled_store_noop() {
        let store = MetadataStore::disabled();
        let report = clean_exit_report();

        assert!(store
            .observe_launcher_exported(None, "slug", "Name", "/path/script.sh", "/path/app.desktop")
            .is_ok());
        assert!(store.observe_launcher_deleted("slug").is_ok());
        assert!(store
            .observe_launcher_renamed(
                "old",
                "new",
                "New Name",
                "/path/new.sh",
                "/path/new.desktop"
            )
            .is_ok());

        let operation_id = store.record_launch_started(None, "native", None).unwrap();
        assert!(operation_id.is_empty());

        assert!(store
            .record_launch_finished("any-id", Some(0), None, &report)
            .is_ok());

        let swept = store.sweep_abandoned_operations().unwrap();
        assert_eq!(swept, 0);
    }

    // -------------------------------------------------------------------------
    // Phase 3 test helpers
    // -------------------------------------------------------------------------

    fn sample_tap_workspace(url: &str) -> CommunityTapWorkspace {
        CommunityTapWorkspace {
            subscription: CommunityTapSubscription {
                url: url.to_string(),
                branch: None,
                pinned_commit: None,
            },
            local_path: PathBuf::from("/tmp/test-tap"),
        }
    }

    fn sample_index_entry(
        tap_url: &str,
        relative_path: &str,
        game_name: &str,
    ) -> CommunityProfileIndexEntry {
        CommunityProfileIndexEntry {
            tap_url: tap_url.to_string(),
            tap_branch: None,
            tap_path: PathBuf::from("/tmp/test-tap"),
            manifest_path: PathBuf::from(format!("/tmp/test-tap/{relative_path}")),
            relative_path: PathBuf::from(relative_path),
            manifest: CommunityProfileManifest::new(
                CommunityProfileMetadata {
                    game_name: game_name.to_string(),
                    game_version: "1.0".to_string(),
                    trainer_name: "TestTrainer".to_string(),
                    trainer_version: "1".to_string(),
                    proton_version: "9".to_string(),
                    platform_tags: vec!["linux".to_string()],
                    compatibility_rating: CompatibilityRating::Working,
                    author: "TestAuthor".to_string(),
                    description: "Test profile".to_string(),
                    trainer_sha256: None,
                },
                GameProfile::default(),
            ),
        }
    }

    fn sample_sync_result(
        tap_url: &str,
        head_commit: &str,
        entries: Vec<CommunityProfileIndexEntry>,
    ) -> CommunityTapSyncResult {
        CommunityTapSyncResult {
            workspace: sample_tap_workspace(tap_url),
            status: CommunityTapSyncStatus::Updated,
            head_commit: head_commit.to_string(),
            index: CommunityProfileIndex {
                entries,
                diagnostics: vec![],
                trainer_sources: vec![],
            },
            from_cache: false,
            last_sync_at: None,
        }
    }

    // -------------------------------------------------------------------------
    // Phase 3: Community index tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_index_tap_result_inserts_tap_and_profile_rows() {
        let store = MetadataStore::open_in_memory().unwrap();
        let tap_url = "https://example.invalid/tap.git";
        let result = sample_sync_result(
            tap_url,
            "abc123",
            vec![
                sample_index_entry(tap_url, "profiles/game-a/community-profile.json", "Game A"),
                sample_index_entry(tap_url, "profiles/game-b/community-profile.json", "Game B"),
            ],
        );

        store.index_community_tap_result(&result).unwrap();

        let conn = connection(&store);

        let (tap_count, last_head_commit, profile_count): (i64, String, i64) = conn
            .query_row(
                "SELECT COUNT(*), last_head_commit, profile_count FROM community_taps WHERE tap_url = ?1",
                params![tap_url],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(tap_count, 1);
        assert_eq!(last_head_commit, "abc123");
        assert_eq!(profile_count, 2);

        let community_profile_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM community_profiles cp \
                 JOIN community_taps ct ON cp.tap_id = ct.tap_id \
                 WHERE ct.tap_url = ?1",
                params![tap_url],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(community_profile_count, 2);
    }

    #[test]
    fn test_index_tap_result_skips_on_unchanged_head() {
        let store = MetadataStore::open_in_memory().unwrap();
        let tap_url = "https://example.invalid/tap.git";
        let result = sample_sync_result(
            tap_url,
            "abc123",
            vec![sample_index_entry(
                tap_url,
                "profiles/game-a/community-profile.json",
                "Game A",
            )],
        );

        store.index_community_tap_result(&result).unwrap();

        let updated_at_first: String = {
            let conn = connection(&store);
            conn.query_row(
                "SELECT updated_at FROM community_taps WHERE tap_url = ?1",
                params![tap_url],
                |row| row.get(0),
            )
            .unwrap()
        };

        // Index again with same head_commit — should be a no-op watermark skip.
        store.index_community_tap_result(&result).unwrap();

        let (updated_at_second, profile_count): (String, i64) = {
            let conn = connection(&store);
            conn.query_row(
                "SELECT updated_at, profile_count FROM community_taps WHERE tap_url = ?1",
                params![tap_url],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap()
        };

        assert_eq!(
            updated_at_first, updated_at_second,
            "updated_at must not change on watermark skip"
        );
        assert_eq!(profile_count, 1);
    }

    #[test]
    fn test_index_tap_result_replaces_stale_profiles() {
        let store = MetadataStore::open_in_memory().unwrap();
        let tap_url = "https://example.invalid/tap.git";

        // First index: 3 profiles.
        let result_v1 = sample_sync_result(
            tap_url,
            "commit-v1",
            vec![
                sample_index_entry(tap_url, "profiles/game-a/community-profile.json", "Game A"),
                sample_index_entry(tap_url, "profiles/game-b/community-profile.json", "Game B"),
                sample_index_entry(tap_url, "profiles/game-c/community-profile.json", "Game C"),
            ],
        );
        store.index_community_tap_result(&result_v1).unwrap();

        // Second index: only 1 profile, different HEAD commit.
        let result_v2 = sample_sync_result(
            tap_url,
            "commit-v2",
            vec![sample_index_entry(
                tap_url,
                "profiles/game-a/community-profile.json",
                "Game A",
            )],
        );
        store.index_community_tap_result(&result_v2).unwrap();

        let conn = connection(&store);
        let profile_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM community_profiles cp \
                 JOIN community_taps ct ON cp.tap_id = ct.tap_id \
                 WHERE ct.tap_url = ?1",
                params![tap_url],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            profile_count, 1,
            "stale profiles should have been removed on re-index"
        );
    }

    #[test]
    fn test_community_profiles_fk_cascades_on_tap_delete() {
        let store = MetadataStore::open_in_memory().unwrap();
        let tap_url = "https://example.invalid/tap.git";
        let result = sample_sync_result(
            tap_url,
            "commit-v1",
            vec![sample_index_entry(
                tap_url,
                "profiles/game-a/community-profile.json",
                "Game A",
            )],
        );
        store.index_community_tap_result(&result).unwrap();

        let conn = connection(&store);
        let tap_id: String = conn
            .query_row(
                "SELECT tap_id FROM community_taps WHERE tap_url = ?1",
                params![tap_url],
                |row| row.get(0),
            )
            .unwrap();

        conn.execute(
            "DELETE FROM community_taps WHERE tap_id = ?1",
            params![&tap_id],
        )
        .unwrap();

        let orphan_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM community_profiles WHERE tap_id = ?1",
                params![&tap_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            orphan_count, 0,
            "deleting a tap should cascade delete community profiles"
        );
    }

    #[test]
    fn test_index_tap_result_disabled_store_noop() {
        let store = MetadataStore::disabled();
        let tap_url = "https://example.invalid/tap.git";
        let result = sample_sync_result(tap_url, "abc123", vec![]);

        let outcome = store.index_community_tap_result(&result);
        assert!(outcome.is_ok());
    }

    // -------------------------------------------------------------------------
    // Phase 3: Cache store tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_put_get_cache_entry_round_trip() {
        let store = MetadataStore::open_in_memory().unwrap();

        store
            .put_cache_entry(
                "https://example.invalid/source",
                "my-cache-key",
                r#"{"data":"hello"}"#,
                None,
            )
            .unwrap();

        let result = store.get_cache_entry("my-cache-key").unwrap();

        assert_eq!(result.as_deref(), Some(r#"{"data":"hello"}"#));
    }

    #[test]
    fn test_put_cache_entry_idempotent() {
        let store = MetadataStore::open_in_memory().unwrap();

        store
            .put_cache_entry(
                "https://example.invalid/source",
                "dedup-key",
                "payload-v1",
                None,
            )
            .unwrap();
        store
            .put_cache_entry(
                "https://example.invalid/source",
                "dedup-key",
                "payload-v2",
                None,
            )
            .unwrap();

        let conn = connection(&store);
        let row_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM external_cache_entries WHERE cache_key = ?1",
                params!["dedup-key"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(row_count, 1, "UPSERT should not create duplicate rows");
    }

    #[test]
    fn test_cache_payload_oversized_stored_as_null() {
        let store = MetadataStore::open_in_memory().unwrap();

        // Build a payload larger than MAX_CACHE_PAYLOAD_BYTES (524_288 bytes / 512 KiB).
        let oversized_payload = "x".repeat(MAX_CACHE_PAYLOAD_BYTES + 1);
        let original_size = oversized_payload.len();

        store
            .put_cache_entry(
                "https://example.invalid/source",
                "oversized-key",
                &oversized_payload,
                None,
            )
            .unwrap();

        let conn = connection(&store);
        let (payload_json, payload_size): (Option<String>, i64) = conn
            .query_row(
                "SELECT payload_json, payload_size FROM external_cache_entries WHERE cache_key = ?1",
                params!["oversized-key"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert!(
            payload_json.is_none(),
            "oversized payload should be stored as NULL"
        );
        assert_eq!(
            payload_size, original_size as i64,
            "payload_size should record the original size"
        );
    }

    #[test]
    fn test_evict_expired_entries() {
        let store = MetadataStore::open_in_memory().unwrap();

        // Insert a non-expired entry (expires far in the future).
        store
            .put_cache_entry(
                "https://example.invalid/source",
                "live-key",
                "live-payload",
                Some("2099-01-01T00:00:00Z"),
            )
            .unwrap();

        // Insert an expired entry directly via raw SQL (already past expiry).
        {
            let conn = connection(&store);
            conn.execute(
                "INSERT INTO external_cache_entries \
                 (cache_id, source_url, cache_key, payload_json, payload_size, fetched_at, expires_at, created_at, updated_at) \
                 VALUES ('expired-id', 'https://example.invalid/source', 'expired-key', 'expired', 7, \
                 '2020-01-01T00:00:00Z', '2020-01-02T00:00:00Z', '2020-01-01T00:00:00Z', '2020-01-01T00:00:00Z')",
                [],
            )
            .unwrap();
        }

        let evicted = store.evict_expired_cache_entries().unwrap();
        assert_eq!(evicted, 1);

        let conn = connection(&store);
        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM external_cache_entries", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(remaining, 1, "only the non-expired entry should remain");
    }

    #[test]
    fn test_cache_entry_disabled_store_noop() {
        let store = MetadataStore::disabled();

        let result = store.get_cache_entry("any-key").unwrap();
        assert!(result.is_none());
    }

    // -------------------------------------------------------------------------
    // Phase 3: Collections tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_create_collection_returns_id() {
        let store = MetadataStore::open_in_memory().unwrap();

        let collection_id = store.create_collection("My Favorites").unwrap();
        assert!(!collection_id.trim().is_empty());

        let conn = connection(&store);
        let row_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM collections WHERE name = ?1",
                params!["My Favorites"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(row_count, 1);
    }

    #[test]
    fn test_add_profile_to_collection() {
        let store = MetadataStore::open_in_memory().unwrap();
        let profile = sample_profile();
        let path = std::path::Path::new("/profiles/elden-ring.toml");

        store
            .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
            .unwrap();

        let collection_id = store.create_collection("Test Collection").unwrap();
        store
            .add_profile_to_collection(&collection_id, "elden-ring")
            .unwrap();

        let conn = connection(&store);
        let row_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM collection_profiles WHERE collection_id = ?1",
                params![collection_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(row_count, 1);
    }

    #[test]
    fn test_add_profile_to_collection_missing_profile_errors() {
        let store = MetadataStore::open_in_memory().unwrap();
        let collection_id = store.create_collection("Ghosts").unwrap();

        let result = store.add_profile_to_collection(&collection_id, "does-not-exist");

        match result {
            Err(MetadataStoreError::Validation(msg)) => {
                assert!(
                    msg.contains("does-not-exist"),
                    "error message should include the missing profile name, got: {msg}"
                );
            }
            other => panic!("expected Validation error, got {other:?}"),
        }

        // Verify no row was inserted.
        let conn = connection(&store);
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM collection_profiles WHERE collection_id = ?1",
                params![collection_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_rename_collection_updates_name() {
        let store = MetadataStore::open_in_memory().unwrap();
        let id = store.create_collection("Old Name").unwrap();

        store.rename_collection(&id, "New Name").unwrap();

        let collections = store.list_collections().unwrap();
        assert_eq!(collections.len(), 1);
        assert_eq!(collections[0].name, "New Name");
    }

    #[test]
    fn test_rename_collection_unknown_id_errors() {
        let store = MetadataStore::open_in_memory().unwrap();
        let result = store.rename_collection("nope", "Whatever");
        assert!(matches!(result, Err(MetadataStoreError::Validation(_))));
    }

    #[test]
    fn test_rename_collection_duplicate_name_errors() {
        let store = MetadataStore::open_in_memory().unwrap();
        let _ = store.create_collection("A").unwrap();
        let id_b = store.create_collection("B").unwrap();

        // Duplicate name violates the UNIQUE constraint on collections.name.
        let result = store.rename_collection(&id_b, "A");
        assert!(
            matches!(result, Err(MetadataStoreError::Database { .. })),
            "duplicate name should bubble as a Database error (UNIQUE violation)"
        );
    }

    #[test]
    fn test_update_collection_description_set_and_clear() {
        let store = MetadataStore::open_in_memory().unwrap();
        let id = store.create_collection("Target").unwrap();

        store
            .update_collection_description(&id, Some("a helpful description"))
            .unwrap();
        let row = store
            .list_collections()
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(row.description.as_deref(), Some("a helpful description"));

        // Clearing with Some("   ") normalizes to None.
        store
            .update_collection_description(&id, Some("   "))
            .unwrap();
        let row = store
            .list_collections()
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(row.description, None);

        // Clearing with None also works.
        store
            .update_collection_description(&id, Some("again"))
            .unwrap();
        store.update_collection_description(&id, None).unwrap();
        let row = store
            .list_collections()
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(row.description, None);
    }

    #[test]
    fn test_collections_for_profile_returns_multi_membership() {
        let store = MetadataStore::open_in_memory().unwrap();
        let profile = sample_profile();
        let path = std::path::Path::new("/profiles/elden-ring.toml");
        store
            .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
            .unwrap();

        let id_a = store.create_collection("Action").unwrap();
        let id_b = store.create_collection("Backlog").unwrap();
        let _id_c = store.create_collection("Untouched").unwrap();

        store
            .add_profile_to_collection(&id_a, "elden-ring")
            .unwrap();
        store
            .add_profile_to_collection(&id_b, "elden-ring")
            .unwrap();

        let result = store.collections_for_profile("elden-ring").unwrap();
        assert_eq!(result.len(), 2);
        let names: Vec<&str> = result.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"Action"));
        assert!(names.contains(&"Backlog"));
        assert!(!names.contains(&"Untouched"));

        // Unknown profile name returns empty vec (not error).
        let empty = store.collections_for_profile("nobody").unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn test_profile_delete_cascades_collection_membership() {
        let store = MetadataStore::open_in_memory().unwrap();
        let profile = sample_profile();
        let path = std::path::Path::new("/profiles/vanishing.toml");
        store
            .observe_profile_write("vanishing", &profile, path, SyncSource::AppWrite, None)
            .unwrap();

        let collection_id = store.create_collection("Ephemeral").unwrap();
        store
            .add_profile_to_collection(&collection_id, "vanishing")
            .unwrap();

        // Hard-delete the profile row (bypassing the soft-delete code path, which
        // only sets deleted_at). We simulate a hard delete to verify the FK cascade.
        let conn = connection(&store);
        conn.execute(
            "DELETE FROM profiles WHERE current_filename = 'vanishing'",
            [],
        )
        .unwrap();
        drop(conn);

        // Membership row must be gone.
        let conn = connection(&store);
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM collection_profiles WHERE collection_id = ?1",
                params![collection_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 0,
            "collection_profiles row must cascade on profile delete"
        );
    }

    #[test]
    fn test_collection_delete_cascades() {
        let store = MetadataStore::open_in_memory().unwrap();
        let profile = sample_profile();
        let path = std::path::Path::new("/profiles/elden-ring.toml");

        store
            .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
            .unwrap();

        let collection_id = store.create_collection("To Delete").unwrap();
        store
            .add_profile_to_collection(&collection_id, "elden-ring")
            .unwrap();

        store.delete_collection(&collection_id).unwrap();

        let conn = connection(&store);
        let member_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM collection_profiles WHERE collection_id = ?1",
                params![collection_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            member_count, 0,
            "collection_profiles rows should cascade-delete with the collection"
        );
    }

    // --- Phase 3: per-collection launch defaults metadata ---

    #[test]
    fn test_collection_defaults_set_and_get_roundtrip() {
        let store = MetadataStore::open_in_memory().unwrap();
        let id = store.create_collection("Steam Deck").unwrap();

        // Initially, no defaults.
        let none = store.get_collection_defaults(&id).unwrap();
        assert!(none.is_none());

        let mut defaults = CollectionDefaultsSection::default();
        defaults.method = Some("proton_run".to_string());
        defaults
            .custom_env_vars
            .insert("DXVK_HUD".to_string(), "1".to_string());
        defaults.network_isolation = Some(false);

        store.set_collection_defaults(&id, Some(&defaults)).unwrap();

        let loaded = store
            .get_collection_defaults(&id)
            .unwrap()
            .expect("defaults should be set");
        assert_eq!(loaded.method.as_deref(), Some("proton_run"));
        assert_eq!(loaded.network_isolation, Some(false));
        assert_eq!(
            loaded.custom_env_vars.get("DXVK_HUD").cloned(),
            Some("1".to_string())
        );
    }

    #[test]
    fn test_collection_defaults_clear_writes_null() {
        let store = MetadataStore::open_in_memory().unwrap();
        let id = store.create_collection("Temp").unwrap();

        let mut defaults = CollectionDefaultsSection::default();
        defaults.method = Some("native".to_string());
        store.set_collection_defaults(&id, Some(&defaults)).unwrap();

        // Clearing via None writes NULL.
        store.set_collection_defaults(&id, None).unwrap();
        assert!(store.get_collection_defaults(&id).unwrap().is_none());

        // Clearing via empty-defaults struct ALSO writes NULL (is_empty() guard).
        store
            .set_collection_defaults(&id, Some(&CollectionDefaultsSection::default()))
            .unwrap();
        assert!(
            store.get_collection_defaults(&id).unwrap().is_none(),
            "empty defaults should normalize to NULL"
        );
    }

    #[test]
    fn test_collection_defaults_unknown_id_errors_on_set() {
        let store = MetadataStore::open_in_memory().unwrap();
        let mut defaults = CollectionDefaultsSection::default();
        defaults.method = Some("native".to_string());
        let result = store.set_collection_defaults("no-such-id", Some(&defaults));
        assert!(matches!(result, Err(MetadataStoreError::Validation(_))));
    }

    #[test]
    fn test_collection_defaults_corrupt_json_returns_corrupt_error() {
        let store = MetadataStore::open_in_memory().unwrap();
        let id = store.create_collection("Corrupt").unwrap();

        // Force a corrupt JSON payload via raw SQL.
        let conn = connection(&store);
        conn.execute(
            "UPDATE collections SET defaults_json = ?1 WHERE collection_id = ?2",
            params!["{not-valid-json", id],
        )
        .unwrap();
        drop(conn);

        let result = store.get_collection_defaults(&id);
        assert!(
            matches!(result, Err(MetadataStoreError::Corrupt(_))),
            "corrupt JSON should surface as Corrupt, got {result:?}"
        );
    }

    #[test]
    fn test_collection_defaults_cascades_on_collection_delete() {
        let store = MetadataStore::open_in_memory().unwrap();
        let id = store.create_collection("Scratch").unwrap();

        let mut defaults = CollectionDefaultsSection::default();
        defaults.method = Some("native".to_string());
        store.set_collection_defaults(&id, Some(&defaults)).unwrap();

        store.delete_collection(&id).unwrap();

        // After delete, reading defaults should error because the collection row is gone.
        // The error shape must match `set_collection_defaults` (Validation) so frontend
        // code sees a single surface for the missing-collection condition.
        let result = store.get_collection_defaults(&id);
        assert!(
            matches!(result, Err(MetadataStoreError::Validation(_))),
            "deleted collection defaults read should return Validation, got {result:?}"
        );
    }

    #[test]
    fn test_set_profile_favorite_toggles() {
        let store = MetadataStore::open_in_memory().unwrap();
        let profile = sample_profile();
        let path = std::path::Path::new("/profiles/elden-ring.toml");

        store
            .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
            .unwrap();

        store.set_profile_favorite("elden-ring", true).unwrap();

        let conn = connection(&store);
        let is_favorite: i64 = conn
            .query_row(
                "SELECT is_favorite FROM profiles WHERE current_filename = ?1",
                params!["elden-ring"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(is_favorite, 1);

        drop(conn);
        store.set_profile_favorite("elden-ring", false).unwrap();

        let conn = connection(&store);
        let is_favorite: i64 = conn
            .query_row(
                "SELECT is_favorite FROM profiles WHERE current_filename = ?1",
                params!["elden-ring"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(is_favorite, 0);
    }

    #[test]
    fn test_list_favorite_profiles_excludes_deleted() {
        let store = MetadataStore::open_in_memory().unwrap();
        let profile = sample_profile();

        store
            .observe_profile_write(
                "keep-me",
                &profile,
                std::path::Path::new("/profiles/keep-me.toml"),
                SyncSource::AppWrite,
                None,
            )
            .unwrap();
        store
            .observe_profile_write(
                "delete-me",
                &profile,
                std::path::Path::new("/profiles/delete-me.toml"),
                SyncSource::AppWrite,
                None,
            )
            .unwrap();

        store.set_profile_favorite("keep-me", true).unwrap();
        store.set_profile_favorite("delete-me", true).unwrap();
        store.observe_profile_delete("delete-me").unwrap();

        let favorites = store.list_favorite_profiles().unwrap();
        assert_eq!(favorites, vec!["keep-me".to_string()]);
    }

    // -------------------------------------------------------------------------
    // Phase 3: Usage insights tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_query_most_launched() {
        let store = MetadataStore::open_in_memory().unwrap();
        let report = clean_exit_report();

        // Profile A: 3 launches
        for _ in 0..3 {
            let op_id = store
                .record_launch_started(Some("profile-a"), "native", None)
                .unwrap();
            store
                .record_launch_finished(&op_id, Some(0), None, &report)
                .unwrap();
        }

        // Profile B: 1 launch
        let op_id = store
            .record_launch_started(Some("profile-b"), "native", None)
            .unwrap();
        store
            .record_launch_finished(&op_id, Some(0), None, &report)
            .unwrap();

        // Profile C: 2 launches
        for _ in 0..2 {
            let op_id = store
                .record_launch_started(Some("profile-c"), "native", None)
                .unwrap();
            store
                .record_launch_finished(&op_id, Some(0), None, &report)
                .unwrap();
        }

        let most_launched = store.query_most_launched(10).unwrap();

        assert_eq!(most_launched.len(), 3);
        assert_eq!(most_launched[0].0, "profile-a");
        assert_eq!(most_launched[0].1, 3);
        assert_eq!(most_launched[1].0, "profile-c");
        assert_eq!(most_launched[1].1, 2);
        assert_eq!(most_launched[2].0, "profile-b");
        assert_eq!(most_launched[2].1, 1);
    }

    #[test]
    fn test_query_failure_trends() {
        let store = MetadataStore::open_in_memory().unwrap();

        let clean_report = clean_exit_report();

        // Profile with failures: 1 success + 2 failures
        let op_ok = store
            .record_launch_started(Some("flaky-profile"), "native", None)
            .unwrap();
        store
            .record_launch_finished(&op_ok, Some(0), None, &clean_report)
            .unwrap();

        let failure_report = DiagnosticReport {
            severity: ValidationSeverity::Warning,
            summary: "Non-zero exit".to_string(),
            exit_info: ExitCodeInfo {
                code: Some(1),
                signal: None,
                signal_name: None,
                core_dumped: false,
                failure_mode: FailureMode::NonZeroExit,
                description: "Process exited with code 1".to_string(),
                severity: ValidationSeverity::Warning,
            },
            pattern_matches: vec![],
            suggestions: vec![],
            launch_method: "native".to_string(),
            log_tail_path: None,
            analyzed_at: "2026-01-01T00:00:00Z".to_string(),
        };

        for _ in 0..2 {
            let op_fail = store
                .record_launch_started(Some("flaky-profile"), "native", None)
                .unwrap();
            store
                .record_launch_finished(&op_fail, Some(1), None, &failure_report)
                .unwrap();
        }

        // Profile with no failures: 2 successes only
        for _ in 0..2 {
            let op_id = store
                .record_launch_started(Some("clean-profile"), "native", None)
                .unwrap();
            store
                .record_launch_finished(&op_id, Some(0), None, &clean_report)
                .unwrap();
        }

        let trends = store.query_failure_trends(30).unwrap();

        assert_eq!(trends.len(), 1, "only profiles with failures should appear");
        assert_eq!(trends[0].profile_name, "flaky-profile");
        assert_eq!(trends[0].successes, 1);
        assert_eq!(trends[0].failures, 2);
    }

    #[test]
    fn test_single_profile_usage_queries() {
        let store = MetadataStore::open_in_memory().unwrap();
        let clean_report = clean_exit_report();

        let ok = store
            .record_launch_started(Some("target-profile"), "native", None)
            .unwrap();
        store
            .record_launch_finished(&ok, Some(0), None, &clean_report)
            .unwrap();

        let failure_report = DiagnosticReport {
            severity: ValidationSeverity::Warning,
            summary: "Non-zero exit".to_string(),
            exit_info: ExitCodeInfo {
                code: Some(1),
                signal: None,
                signal_name: None,
                core_dumped: false,
                failure_mode: FailureMode::NonZeroExit,
                description: "Process exited with code 1".to_string(),
                severity: ValidationSeverity::Warning,
            },
            pattern_matches: vec![],
            suggestions: vec![],
            launch_method: "native".to_string(),
            log_tail_path: None,
            analyzed_at: "2026-01-01T00:00:00Z".to_string(),
        };

        let failed = store
            .record_launch_started(Some("target-profile"), "native", None)
            .unwrap();
        store
            .record_launch_finished(&failed, Some(1), None, &failure_report)
            .unwrap();

        let (failures, successes) = store
            .query_failure_trend_for_profile("target-profile", 30)
            .unwrap();
        assert_eq!(failures, 1);
        assert_eq!(successes, 1);

        let last_success = store
            .query_last_success_for_profile("target-profile")
            .unwrap();
        assert!(last_success.is_some());

        let total_launches = store
            .query_total_launches_for_profile("target-profile")
            .unwrap();
        assert_eq!(total_launches, 2);
    }

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

    // -------------------------------------------------------------------------
    // Version store tests
    // -------------------------------------------------------------------------

    fn insert_test_profile_row(conn: &Connection, profile_id: &str) {
        conn.execute(
            "INSERT INTO profiles (profile_id, current_filename, current_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                profile_id,
                format!("{profile_id}_file"),
                format!("/path/{profile_id}.toml"),
                "2024-01-01T00:00:00+00:00",
                "2024-01-01T00:00:00+00:00",
            ],
        )
        .unwrap();
    }

    #[test]
    fn verify_trainer_hash_second_hit_uses_cache() {
        let store = MetadataStore::open_in_memory().unwrap();
        {
            let conn = connection(&store);
            insert_test_profile_row(&conn, "profile-1");
        }
        let dir = tempdir().unwrap();
        let path = dir.path().join("trainer.exe");
        fs::write(&path, b"fake-trainer-bytes").unwrap();

        let first = store
            .verify_trainer_hash_for_profile_path("profile-1", &path)
            .unwrap()
            .expect("hash");
        assert!(!first.from_cache);

        let second = store
            .verify_trainer_hash_for_profile_path("profile-1", &path)
            .unwrap()
            .expect("hash");
        assert!(second.from_cache);
        assert_eq!(first.hash, second.hash);
    }

    #[test]
    fn trainer_hash_launch_check_first_baseline() {
        let store = MetadataStore::open_in_memory().unwrap();
        {
            let conn = connection(&store);
            insert_test_profile_row(&conn, "launch-pid-1");
        }
        let dir = tempdir().unwrap();
        let path = dir.path().join("trainer.exe");
        fs::write(&path, b"v1-bytes").unwrap();
        let out = store
            .with_sqlite_conn("trainer hash launch test", |conn| {
                crate::offline::trainer_hash_launch_check(conn, "launch-pid-1", &path, None)
            })
            .unwrap();
        assert!(matches!(
            out.baseline,
            crate::offline::TrainerHashBaselineResult::FirstBaselineRecorded
        ));
    }

    #[test]
    fn trainer_hash_launch_check_mismatch_after_content_change() {
        let store = MetadataStore::open_in_memory().unwrap();
        {
            let conn = connection(&store);
            insert_test_profile_row(&conn, "launch-pid-2");
        }
        let dir = tempdir().unwrap();
        let path = dir.path().join("trainer.exe");
        fs::write(&path, b"v1").unwrap();
        store
            .with_sqlite_conn("seed baseline", |conn| {
                crate::offline::trainer_hash_launch_check(conn, "launch-pid-2", &path, None)
            })
            .unwrap();
        fs::write(&path, b"v2-different").unwrap();
        let out = store
            .with_sqlite_conn("detect mismatch", |conn| {
                crate::offline::trainer_hash_launch_check(conn, "launch-pid-2", &path, None)
            })
            .unwrap();
        assert!(matches!(
            out.baseline,
            crate::offline::TrainerHashBaselineResult::Mismatch { .. }
        ));
    }

    #[test]
    fn launch_issues_from_trainer_hash_maps_mismatch_and_community() {
        use crate::launch::launch_issues_from_trainer_hash_outcome;
        use crate::offline::{
            TrainerHashBaselineResult, TrainerHashCommunityAdvisory, TrainerHashLaunchOutcome,
        };

        let out = TrainerHashLaunchOutcome {
            baseline: TrainerHashBaselineResult::Mismatch {
                stored_hash: "aa".repeat(32),
                current_hash: "bb".repeat(32),
            },
            community_advisory: Some(TrainerHashCommunityAdvisory {
                expected: "cc".repeat(32),
                current: "dd".repeat(32),
            }),
        };
        let issues = launch_issues_from_trainer_hash_outcome(out);
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].code.as_deref(), Some("trainer_hash_mismatch"));
        assert_eq!(
            issues[1].code.as_deref(),
            Some("trainer_hash_community_mismatch")
        );
    }

    #[test]
    fn test_version_snapshot_upsert_and_lookup_lifecycle() {
        let store = MetadataStore::open_in_memory().unwrap();
        {
            let conn = connection(&store);
            insert_test_profile_row(&conn, "lifecycle-profile");
        }

        store
            .upsert_version_snapshot(
                "lifecycle-profile",
                "99999",
                Some("build-abc"),
                Some("v1.2.3"),
                Some("deadbeef01234567deadbeef01234567deadbeef01234567deadbeef01234567"),
                Some("1.2.3"),
                "matched",
            )
            .unwrap();

        let snapshot = store
            .lookup_latest_version_snapshot("lifecycle-profile")
            .unwrap()
            .expect("snapshot should be present after upsert");

        assert_eq!(snapshot.profile_id, "lifecycle-profile");
        assert_eq!(snapshot.steam_app_id, "99999");
        assert_eq!(snapshot.steam_build_id.as_deref(), Some("build-abc"));
        assert_eq!(snapshot.trainer_version.as_deref(), Some("v1.2.3"));
        assert_eq!(snapshot.status, "matched");
        assert!(!snapshot.checked_at.is_empty());
    }

    #[test]
    fn test_version_snapshot_lookup_returns_latest() {
        let store = MetadataStore::open_in_memory().unwrap();
        {
            let conn = connection(&store);
            insert_test_profile_row(&conn, "latest-profile");
        }

        // Insert two snapshots with distinct checked_at values via raw SQL
        // so we can control ordering.
        {
            let conn = connection(&store);
            conn.execute(
                "INSERT INTO version_snapshots
                 (profile_id, steam_app_id, steam_build_id, status, checked_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    "latest-profile",
                    "11111",
                    "build-old",
                    "untracked",
                    "2024-01-01T00:00:00+00:00",
                ],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO version_snapshots
                 (profile_id, steam_app_id, steam_build_id, status, checked_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    "latest-profile",
                    "11111",
                    "build-new",
                    "matched",
                    "2024-06-01T00:00:00+00:00",
                ],
            )
            .unwrap();
        }

        let snapshot = store
            .lookup_latest_version_snapshot("latest-profile")
            .unwrap()
            .expect("snapshot should be present");

        assert_eq!(snapshot.steam_build_id.as_deref(), Some("build-new"));
        assert_eq!(snapshot.status, "matched");
    }

    #[test]
    fn test_version_snapshot_pruning_at_max_limit() {
        let store = MetadataStore::open_in_memory().unwrap();
        {
            let conn = connection(&store);
            insert_test_profile_row(&conn, "prune-profile");
        }

        // Insert MAX+1 rows — the prune step must keep exactly MAX.
        for i in 0..=MAX_VERSION_SNAPSHOTS_PER_PROFILE {
            store
                .upsert_version_snapshot(
                    "prune-profile",
                    "55555",
                    Some(&format!("build-{i:04}")),
                    None,
                    None,
                    None,
                    "untracked",
                )
                .unwrap();
        }

        let conn = connection(&store);
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM version_snapshots WHERE profile_id = 'prune-profile'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(
            count, MAX_VERSION_SNAPSHOTS_PER_PROFILE as i64,
            "row count must be exactly MAX after pruning"
        );
    }

    #[test]
    fn test_acknowledge_version_change_sets_matched() {
        let store = MetadataStore::open_in_memory().unwrap();
        {
            let conn = connection(&store);
            insert_test_profile_row(&conn, "ack-profile");
        }

        store
            .upsert_version_snapshot(
                "ack-profile",
                "77777",
                None,
                None,
                None,
                None,
                "game_updated",
            )
            .unwrap();

        // Confirm initial status is game_updated.
        let before = store
            .lookup_latest_version_snapshot("ack-profile")
            .unwrap()
            .unwrap();
        assert_eq!(before.status, "game_updated");

        store.acknowledge_version_change("ack-profile").unwrap();

        let after = store
            .lookup_latest_version_snapshot("ack-profile")
            .unwrap()
            .unwrap();
        assert_eq!(after.status, "matched");
    }

    #[test]
    fn test_load_version_snapshots_for_profiles_returns_latest_per_profile() {
        let store = MetadataStore::open_in_memory().unwrap();
        {
            let conn = connection(&store);
            insert_test_profile_row(&conn, "bulk-profile-a");
            insert_test_profile_row(&conn, "bulk-profile-b");
        }

        // Profile A: two snapshots — the second (game_updated) should win.
        {
            let conn = connection(&store);
            conn.execute(
                "INSERT INTO version_snapshots (profile_id, steam_app_id, status, checked_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    "bulk-profile-a",
                    "10001",
                    "untracked",
                    "2024-01-01T00:00:00+00:00",
                ],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO version_snapshots (profile_id, steam_app_id, status, checked_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    "bulk-profile-a",
                    "10001",
                    "game_updated",
                    "2024-06-01T00:00:00+00:00",
                ],
            )
            .unwrap();
        }

        // Profile B: one snapshot.
        {
            let conn = connection(&store);
            conn.execute(
                "INSERT INTO version_snapshots (profile_id, steam_app_id, status, checked_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    "bulk-profile-b",
                    "20002",
                    "matched",
                    "2024-03-01T00:00:00+00:00",
                ],
            )
            .unwrap();
        }

        let snapshots = store.load_version_snapshots_for_profiles().unwrap();

        assert_eq!(snapshots.len(), 2, "should return one row per profile");

        let snap_a = snapshots
            .iter()
            .find(|s| s.profile_id == "bulk-profile-a")
            .expect("profile-a snapshot must be present");
        let snap_b = snapshots
            .iter()
            .find(|s| s.profile_id == "bulk-profile-b")
            .expect("profile-b snapshot must be present");

        // MAX(id) picks the last-inserted row for profile-a, which is game_updated.
        assert_eq!(snap_a.status, "game_updated");
        assert_eq!(snap_b.status, "matched");
    }

    #[test]
    fn test_compute_correlation_status_update_in_progress() {
        // state_flags Some(non-4) → UpdateInProgress regardless of other inputs.
        assert!(matches!(
            compute_correlation_status("build1", Some("build1"), None, None, Some(0)),
            VersionCorrelationStatus::UpdateInProgress
        ));
        assert!(matches!(
            compute_correlation_status("build1", Some("build1"), None, None, Some(6)),
            VersionCorrelationStatus::UpdateInProgress
        ));
        // state_flags None (manifest not found) → falls through to comparison, not UpdateInProgress.
        assert!(matches!(
            compute_correlation_status("build1", Some("build1"), None, None, None),
            VersionCorrelationStatus::Matched
        ));
    }

    #[test]
    fn test_compute_correlation_status_untracked() {
        // No snapshot → Untracked (when state_flags is stable).
        assert!(matches!(
            compute_correlation_status("build1", None, None, None, Some(4)),
            VersionCorrelationStatus::Untracked
        ));
    }

    #[test]
    fn test_compute_correlation_status_matched() {
        assert!(matches!(
            compute_correlation_status(
                "build1",
                Some("build1"),
                Some("hash-a"),
                Some("hash-a"),
                Some(4)
            ),
            VersionCorrelationStatus::Matched
        ));
        // Both trainer hashes None → also matched.
        assert!(matches!(
            compute_correlation_status("build1", Some("build1"), None, None, Some(4)),
            VersionCorrelationStatus::Matched
        ));
    }

    #[test]
    fn test_compute_correlation_status_game_updated() {
        assert!(matches!(
            compute_correlation_status(
                "build-new",
                Some("build-old"),
                Some("hash-a"),
                Some("hash-a"),
                Some(4)
            ),
            VersionCorrelationStatus::GameUpdated
        ));
    }

    #[test]
    fn test_compute_correlation_status_trainer_changed() {
        assert!(matches!(
            compute_correlation_status(
                "build1",
                Some("build1"),
                Some("hash-new"),
                Some("hash-old"),
                Some(4)
            ),
            VersionCorrelationStatus::TrainerChanged
        ));
    }

    #[test]
    fn test_compute_correlation_status_both_changed() {
        assert!(matches!(
            compute_correlation_status(
                "build-new",
                Some("build-old"),
                Some("hash-new"),
                Some("hash-old"),
                Some(4)
            ),
            VersionCorrelationStatus::BothChanged
        ));
    }

    #[test]
    fn test_version_store_disabled_store_noop() {
        let store = MetadataStore::disabled();

        assert!(store
            .upsert_version_snapshot("any-profile", "12345", None, None, None, None, "untracked")
            .is_ok());
        let snapshot = store.lookup_latest_version_snapshot("any-profile").unwrap();
        assert!(snapshot.is_none());
        let snapshots = store.load_version_snapshots_for_profiles().unwrap();
        assert!(snapshots.is_empty());
        assert!(store.acknowledge_version_change("any-profile").is_ok());
    }
}
