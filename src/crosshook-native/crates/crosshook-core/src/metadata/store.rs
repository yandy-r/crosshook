use std::path::Path;
use std::sync::{Arc, Mutex};

use directories::BaseDirs;
use rusqlite::Connection;

use super::{db, migrations};
use crate::metadata::MetadataStoreError;

#[derive(Clone)]
pub struct MetadataStore {
    pub(super) conn: Option<Arc<Mutex<Connection>>>,
    pub(super) available: bool,
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

    pub fn is_available(&self) -> bool {
        self.available && self.conn.is_some()
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

    pub(super) fn with_conn<F, T>(
        &self,
        action: &'static str,
        f: F,
    ) -> Result<T, MetadataStoreError>
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

    pub(super) fn with_conn_mut<F, T>(
        &self,
        action: &'static str,
        f: F,
    ) -> Result<T, MetadataStoreError>
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

    /// Runs `f` with a shared SQLite `Connection` lock. Unlike [`Self::with_conn`], the return
    /// type does not need [`Default`] (used for batch health + offline on one connection).
    pub fn with_sqlite_conn<R, F>(
        &self,
        action: &'static str,
        f: F,
    ) -> Result<R, MetadataStoreError>
    where
        F: FnOnce(&rusqlite::Connection) -> Result<R, MetadataStoreError>,
    {
        if !self.available {
            return Err(MetadataStoreError::Corrupt(
                "metadata store unavailable".to_string(),
            ));
        }
        let Some(conn) = &self.conn else {
            return Err(MetadataStoreError::Corrupt(
                "metadata store connection missing".to_string(),
            ));
        };
        let guard = conn.lock().map_err(|_| {
            MetadataStoreError::Corrupt(format!("metadata store mutex poisoned while {action}"))
        })?;
        f(&guard)
    }
}
