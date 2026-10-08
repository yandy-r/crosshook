use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use directories::BaseDirs;
use rusqlite::{Connection, Transaction, TransactionBehavior};

use super::{backup, db, migrations, MetadataStatus, MetadataStoreError};

#[derive(Clone)]
pub struct MetadataStore {
    pub(super) conn: Option<Arc<Mutex<Connection>>>,
    pub(super) available: bool,
    read_only: bool,
    initial_status: MetadataStatus,
    path: Option<PathBuf>,
}

impl MetadataStore {
    pub fn try_new() -> Result<Self, String> {
        let path = BaseDirs::new()
            .ok_or("home directory not found — CrossHook requires a user home directory")?
            .data_local_dir()
            .join("crosshook/metadata.db");
        Self::with_path(&path).map_err(|error| error.to_string())
    }

    pub fn with_path(path: &Path) -> Result<Self, MetadataStoreError> {
        let opened = db::open_at_path(path)?;
        Ok(Self {
            available: opened.conn.is_some(),
            conn: opened.conn.map(|conn| Arc::new(Mutex::new(conn))),
            read_only: opened.status != MetadataStatus::Ok,
            initial_status: opened.status,
            path: Some(path.to_owned()),
        })
    }

    pub fn open_in_memory() -> Result<Self, MetadataStoreError> {
        Ok(Self {
            conn: Some(Arc::new(Mutex::new(db::open_in_memory()?))),
            available: true,
            read_only: false,
            initial_status: MetadataStatus::Ok,
            path: None,
        })
    }

    pub fn disabled() -> Self {
        Self {
            conn: None,
            available: false,
            read_only: true,
            initial_status: MetadataStatus::Disabled {
                reason: "metadata store unavailable".into(),
            },
            path: None,
        }
    }

    pub fn is_available(&self) -> bool {
        self.available && self.conn.is_some()
    }

    /// Sanitized IPC health; detailed errors belong in logs, never in this value.
    pub fn status(&self) -> MetadataStatus {
        let Some(conn) = &self.conn else {
            return self.initial_status.clone();
        };
        let Ok(guard) = conn.lock() else {
            return MetadataStatus::Disabled {
                reason: "metadata store unavailable".into(),
            };
        };
        match db::version(&guard) {
            Ok(found) if found > migrations::SUPPORTED_MAX_VERSION => db::newer_status(found),
            Ok(_) => self.initial_status.clone(),
            Err(error) => {
                tracing::error!(%error, "Failed to read metadata schema status");
                MetadataStatus::Disabled {
                    reason: "schema status unavailable".into(),
                }
            }
        }
    }

    /// Exact owned regular backup files, newest first. No filesystem paths cross IPC.
    pub fn backup_files(&self) -> Vec<PathBuf> {
        self.path.as_deref().map(backup::files).unwrap_or_default()
    }

    fn readonly_error(&self) -> MetadataStoreError {
        match &self.initial_status {
            MetadataStatus::NewerSchema { .. } => MetadataStoreError::ReadOnlyNewerSchema,
            MetadataStatus::Disabled { reason } => MetadataStoreError::ReadOnlyDisabled {
                reason: reason.clone(),
            },
            MetadataStatus::Ok => MetadataStoreError::SQLiteReadonly,
        }
    }

    fn map_error<T>(
        &self,
        conn: &Connection,
        result: Result<T, MetadataStoreError>,
    ) -> Result<T, MetadataStoreError> {
        result.map_err(|error| match &error {
            MetadataStoreError::Database { source, .. }
                if source.sqlite_error_code() == Some(rusqlite::ffi::ErrorCode::ReadOnly) =>
            {
                if db::version(conn).is_ok_and(|found| found > migrations::SUPPORTED_MAX_VERSION) {
                    MetadataStoreError::ReadOnlyNewerSchema
                } else {
                    self.readonly_error()
                }
            }
            _ => error,
        })
    }

    // Shared closures may write even when their names suggest reads. Take SQLite's
    // writer lock before checking user_version; keep it until the closure completes.
    // Actual READ_ONLY connections stay readable and let SQLite reject writes.
    fn run_shared<R>(
        &self,
        conn: &Connection,
        f: impl FnOnce(&Connection) -> Result<R, MetadataStoreError>,
    ) -> Result<R, MetadataStoreError> {
        let query_only: bool = conn.pragma_query_value(None, "query_only", |row| row.get(0))?;
        if self.read_only || query_only {
            return self.map_error(conn, f(conn));
        }
        let tx = self.map_error(
            conn,
            Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(Into::into),
        )?;
        if db::version(&tx)? > migrations::SUPPORTED_MAX_VERSION {
            tx.rollback()?;
            conn.pragma_update(None, "query_only", true)?;
            return self.map_error(conn, f(conn));
        }
        let value = self.map_error(&tx, f(&tx))?;
        tx.commit()?;
        Ok(value)
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
        self.run_shared(&guard, f)
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
        if self.read_only {
            return Err(self.readonly_error());
        }
        let mut guard = conn.lock().map_err(|_| {
            MetadataStoreError::Corrupt(format!("metadata store mutex poisoned while {action}"))
        })?;
        let mut tx = super::util::WriteTransaction::begin(&mut guard)?;
        if db::version(&tx)? > migrations::SUPPORTED_MAX_VERSION {
            return Err(MetadataStoreError::ReadOnlyNewerSchema);
        }
        let result = f(&mut tx);
        let value = self.map_error(&tx, result)?;
        tx.commit()?;
        Ok(value)
    }

    /// Shared SQLite access, preserving readable newer-schema stores and typed write failures.
    pub fn with_sqlite_conn<R, F>(
        &self,
        action: &'static str,
        f: F,
    ) -> Result<R, MetadataStoreError>
    where
        F: FnOnce(&Connection) -> Result<R, MetadataStoreError>,
    {
        if !self.available {
            return Err(self.readonly_error());
        }
        let Some(conn) = &self.conn else {
            return Err(self.readonly_error());
        };
        let guard = conn.lock().map_err(|_| {
            MetadataStoreError::Corrupt(format!("metadata store mutex poisoned while {action}"))
        })?;
        self.run_shared(&guard, f)
    }
}
