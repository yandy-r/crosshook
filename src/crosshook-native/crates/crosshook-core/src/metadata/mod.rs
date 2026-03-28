mod db;
mod launch_history;
mod launcher_sync;
mod migrations;
mod models;
pub mod profile_sync;

pub use models::{DriftState, LaunchOutcome, MAX_DIAGNOSTIC_JSON_BYTES, MetadataStoreError, SyncReport, SyncSource};

use crate::launch::diagnostics::models::DiagnosticReport;
use crate::profile::{GameProfile, ProfileStore};
use directories::BaseDirs;
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct MetadataStore {
    conn: Option<Arc<Mutex<Connection>>>,
    available: bool,
}

impl MetadataStore {
    pub fn try_new() -> Result<Self, String> {
        let path = BaseDirs::new()
            .ok_or("home directory not found — CrossHook requires a user home directory")?
            .data_local_dir()
            .join("crosshook/metadata.db");
        Self::open(&path).map_err(|error| error.to_string())
    }

    pub fn with_path(path: &Path) -> Result<Self, MetadataStoreError> {
        Self::open(path)
    }

    pub fn open_in_memory() -> Result<Self, MetadataStoreError> {
        Self::open_with_connection(db::open_in_memory()?)
    }

    pub fn disabled() -> Self {
        Self {
            conn: None,
            available: false,
        }
    }

    fn open(path: &Path) -> Result<Self, MetadataStoreError> {
        Self::open_with_connection(db::open_at_path(path)?)
    }

    fn open_with_connection(conn: Connection) -> Result<Self, MetadataStoreError> {
        migrations::run_migrations(&conn)?;
        Ok(Self {
            conn: Some(Arc::new(Mutex::new(conn))),
            available: true,
        })
    }

    fn with_conn<F, T>(&self, action: &'static str, f: F) -> Result<T, MetadataStoreError>
    where
        F: FnOnce(&Connection) -> Result<T, MetadataStoreError>,
        T: Default,
    {
        if !self.available {
            return Ok(T::default());
        }

        let Some(conn) = &self.conn else {
            return Ok(T::default());
        };

        let guard = conn.lock().map_err(|_| {
            MetadataStoreError::Corrupt(format!("metadata store mutex poisoned while {action}"))
        })?;
        f(&guard)
    }

    fn with_conn_mut<F, T>(&self, action: &'static str, f: F) -> Result<T, MetadataStoreError>
    where
        F: FnOnce(&mut Connection) -> Result<T, MetadataStoreError>,
        T: Default,
    {
        if !self.available {
            return Ok(T::default());
        }

        let Some(conn) = &self.conn else {
            return Ok(T::default());
        };

        let mut guard = conn.lock().map_err(|_| {
            MetadataStoreError::Corrupt(format!("metadata store mutex poisoned while {action}"))
        })?;
        f(&mut guard)
    }

    pub fn observe_profile_write(
        &self,
        name: &str,
        profile: &GameProfile,
        path: &Path,
        source: SyncSource,
        source_profile_id: Option<&str>,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("observe a profile write", |conn| {
            profile_sync::observe_profile_write(
                conn,
                name,
                profile,
                path,
                source,
                source_profile_id,
            )
        })
    }

    pub fn lookup_profile_id(&self, name: &str) -> Result<Option<String>, MetadataStoreError> {
        self.with_conn("look up a profile id", |conn| {
            profile_sync::lookup_profile_id(conn, name)
        })
    }

    pub fn observe_profile_rename(
        &self,
        old_name: &str,
        new_name: &str,
        old_path: &Path,
        new_path: &Path,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("observe a profile rename", |conn| {
            profile_sync::observe_profile_rename(conn, old_name, new_name, old_path, new_path)
        })
    }

    pub fn observe_profile_delete(&self, name: &str) -> Result<(), MetadataStoreError> {
        self.with_conn("observe a profile delete", |conn| {
            profile_sync::observe_profile_delete(conn, name)
        })
    }

    pub fn sync_profiles_from_store(
        &self,
        store: &ProfileStore,
    ) -> Result<SyncReport, MetadataStoreError> {
        self.with_conn("sync profiles from store", |conn| {
            profile_sync::sync_profiles_from_store(conn, store)
        })
    }

    pub fn observe_launcher_exported(
        &self,
        profile_name: Option<&str>,
        slug: &str,
        display_name: &str,
        script_path: &str,
        desktop_entry_path: &str,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("observe a launcher export", |conn| {
            launcher_sync::observe_launcher_exported(
                conn,
                profile_name,
                slug,
                display_name,
                script_path,
                desktop_entry_path,
            )
        })
    }

    pub fn observe_launcher_deleted(
        &self,
        launcher_slug: &str,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("observe a launcher deletion", |conn| {
            launcher_sync::observe_launcher_deleted(conn, launcher_slug)
        })
    }

    pub fn observe_launcher_renamed(
        &self,
        old_slug: &str,
        new_slug: &str,
        new_display_name: &str,
        new_script_path: &str,
        new_desktop_entry_path: &str,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn_mut("observe a launcher rename", |conn| {
            launcher_sync::observe_launcher_renamed(
                conn,
                old_slug,
                new_slug,
                new_display_name,
                new_script_path,
                new_desktop_entry_path,
            )
        })
    }

    pub fn record_launch_started(
        &self,
        profile_name: Option<&str>,
        method: &str,
        log_path: Option<&str>,
    ) -> Result<String, MetadataStoreError> {
        self.with_conn("record a launch start", |conn| {
            launch_history::record_launch_started(conn, profile_name, method, log_path)
        })
    }

    pub fn record_launch_finished(
        &self,
        operation_id: &str,
        exit_code: Option<i32>,
        signal: Option<i32>,
        report: &DiagnosticReport,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("record a launch finish", |conn| {
            launch_history::record_launch_finished(conn, operation_id, exit_code, signal, report)
        })
    }

    pub fn sweep_abandoned_operations(&self) -> Result<usize, MetadataStoreError> {
        self.with_conn("sweep abandoned operations", |conn| {
            launch_history::sweep_abandoned_operations(conn)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::diagnostics::models::{
        ActionableSuggestion, DiagnosticReport, ExitCodeInfo, FailureMode,
    };
    use crate::launch::request::ValidationSeverity;
    use crate::profile::{
        GameProfile, GameSection, InjectionSection, LaunchSection, LauncherSection, ProfileStore,
        RuntimeSection, SteamSection, TrainerLoadingMode, TrainerSection,
    };
    use rusqlite::params;
    use std::fs;
    use std::os::unix::fs::{symlink, PermissionsExt};
    use tempfile::tempdir;

    fn sample_profile() -> GameProfile {
        GameProfile {
            game: GameSection {
                name: "Elden Ring".to_string(),
                executable_path: "/games/elden-ring/eldenring.exe".to_string(),
            },
            trainer: TrainerSection {
                path: "/trainers/elden-ring.exe".to_string(),
                kind: "fling".to_string(),
                loading_mode: TrainerLoadingMode::SourceDirectory,
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
            },
            launch: LaunchSection {
                method: "steam_applaunch".to_string(),
                ..Default::default()
            },
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
            "small report ({} bytes) must be under 4KB for this test",
            small_json_len
        );

        let op_id_small = store
            .record_launch_started(None, "native", None)
            .unwrap();
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
                title: format!("Suggestion title number {} with extra padding to push over 4KB boundary", i),
                description: format!(
                    "Suggestion description number {} with a lot of extra text to ensure that the serialized JSON grows large enough to exceed the 4096-byte limit imposed by MAX_DIAGNOSTIC_JSON_BYTES",
                    i
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
            "large report ({} bytes) must exceed 4KB for this test",
            large_json_len
        );

        let op_id_large = store
            .record_launch_started(None, "native", None)
            .unwrap();
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

        let operation_id = store
            .record_launch_started(None, "native", None)
            .unwrap();

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
            .observe_launcher_renamed("old", "new", "New Name", "/path/new.sh", "/path/new.desktop")
            .is_ok());

        let operation_id = store
            .record_launch_started(None, "native", None)
            .unwrap();
        assert!(operation_id.is_empty());

        assert!(store
            .record_launch_finished("any-id", Some(0), None, &report)
            .is_ok());

        let swept = store.sweep_abandoned_operations().unwrap();
        assert_eq!(swept, 0);
    }
}
