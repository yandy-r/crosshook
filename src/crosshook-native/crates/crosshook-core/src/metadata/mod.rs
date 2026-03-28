mod db;
mod migrations;
mod models;
pub mod profile_sync;

pub use models::{MetadataStoreError, SyncReport, SyncSource};

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
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
