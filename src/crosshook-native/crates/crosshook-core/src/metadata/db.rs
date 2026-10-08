use super::{backup, migrations, MetadataStatus, MetadataStoreError};
use migrations::SUPPORTED_MAX_VERSION;
use rusqlite::{Connection, OpenFlags, TransactionBehavior};
use std::fs::{self, OpenOptions, Permissions};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

pub(super) struct Opened {
    pub conn: Option<Connection>,
    pub status: MetadataStatus,
}

fn raw_open(path: &Path, read_only: bool) -> Result<Connection, MetadataStoreError> {
    let flags = if read_only {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    } | OpenFlags::SQLITE_OPEN_NOFOLLOW
        | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(path, flags)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(conn)
}

pub(super) fn version(conn: &Connection) -> Result<u32, MetadataStoreError> {
    Ok(conn.pragma_query_value(None, "user_version", |row| row.get(0))?)
}

pub(super) fn readonly(path: &Path, status: MetadataStatus) -> Opened {
    match raw_open(path, true).and_then(|conn| {
        conn.pragma_update(None, "query_only", true)?;
        Ok(conn)
    }) {
        Ok(conn) => Opened {
            conn: Some(conn),
            status,
        },
        Err(error) => {
            tracing::error!(%error, "Failed to open metadata database read-only");
            Opened {
                conn: None,
                status: MetadataStatus::Disabled {
                    reason: "read-only open failed".into(),
                },
            }
        }
    }
}

pub(super) fn newer_status(found: u32) -> MetadataStatus {
    MetadataStatus::NewerSchema {
        found,
        supported: SUPPORTED_MAX_VERSION,
    }
}

pub(super) fn open_at_path(path: &Path) -> Result<Opened, MetadataStoreError> {
    open_with_hooks(path, |_, _| Ok(()), |_| Ok(()))
}

// Hook controls the peek/RW-open boundary in deterministic race tests.
fn open_with_hooks<F, G>(
    path: &Path,
    mut after_snapshot: F,
    mut before_migrations: G,
) -> Result<Opened, MetadataStoreError>
where
    F: FnMut(u32, usize) -> Result<(), MetadataStoreError>,
    G: FnMut(&Path) -> Result<(), MetadataStoreError>,
{
    for attempt in 0..2 {
        let existing = match fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(MetadataStoreError::SymlinkDetected(path.to_owned()))
            }
            Ok(_) => true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(source) => {
                return Err(MetadataStoreError::Io {
                    action: "inspect metadata database",
                    path: path.to_owned(),
                    source,
                })
            }
        };
        // Ordinary READ_ONLY, not immutable: live WAL must remain visible.
        let peek = if existing {
            Some(raw_open(path, true)?)
        } else {
            None
        };
        let from = match &peek {
            Some(conn) => version(conn)?,
            None => 0,
        };
        let wal = match &peek {
            Some(conn) => conn
                .pragma_query_value(None, "journal_mode", |row| row.get::<_, String>(0))?
                .eq_ignore_ascii_case("wal"),
            None => false,
        };
        if from > SUPPORTED_MAX_VERSION {
            return Ok(readonly(path, newer_status(from)));
        }
        let snapshot = if from > 0 && from < SUPPORTED_MAX_VERSION {
            match backup::snapshot(
                peek.as_ref()
                    .ok_or_else(|| MetadataStoreError::Corrupt("missing backup source".into()))?,
                path,
                from,
            ) {
                Ok(target) => Some(target),
                Err(error) => {
                    tracing::error!(%error, "Metadata backup failed; migrations disabled");
                    return Ok(readonly(
                        path,
                        MetadataStatus::Disabled {
                            reason: "backup failed".into(),
                        },
                    ));
                }
            }
        } else {
            None
        };
        drop(peek);
        after_snapshot(from, attempt)?;
        if !existing {
            if let Some(parent) = path.parent() {
                fs::DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(parent)
                    .map_err(|source| MetadataStoreError::Io {
                        action: "create metadata directory",
                        path: parent.to_owned(),
                        source,
                    })?;
            }
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)
            {
                Ok(_) => (),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(source) => {
                    return Err(MetadataStoreError::Io {
                        action: "create private metadata database",
                        path: path.to_owned(),
                        source,
                    })
                }
            }
        }
        let mut conn = raw_open(path, false)?;
        configure_local(&conn)?;
        if !wal {
            // Rollback-journal DBs: exclusive locking retains the lock across COMMIT,
            // so the persistent WAL switch cannot race another binary's upgrade.
            conn.pragma_update(None, "locking_mode", "EXCLUSIVE")?;
        }
        // No persistent pragma, chmod, or header write occurs before this recheck.
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = version(&tx)?;
        if current != from || current > SUPPORTED_MAX_VERSION {
            tx.rollback()?;
            drop(conn);
            if let Some(snapshot) = snapshot {
                backup::discard(&snapshot);
            }
            if current > SUPPORTED_MAX_VERSION {
                return Ok(readonly(path, newer_status(current)));
            }
            continue;
        }
        secure(path, 0o600)?;
        before_migrations(path)?;
        // One transaction covers the whole ladder, version bumps and application_id.
        migrations::run_migrations(&tx)?;
        tx.pragma_update(None, "application_id", 0x43484B00_i32)?;
        let check: String = tx.pragma_query_value(None, "quick_check", |row| row.get(0))?;
        if check != "ok" {
            return Err(MetadataStoreError::Corrupt(check));
        }
        tx.commit()?;
        if !wal {
            let mode: String =
                conn.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))?;
            if !mode.eq_ignore_ascii_case("wal") {
                return Err(MetadataStoreError::Corrupt(format!(
                    "expected journal_mode wal, got {mode}"
                )));
            }
            // WAL entered under EXCLUSIVE keeps the lock; reopen to share normally.
            drop(conn);
            conn = raw_open(path, false)?;
            configure_local(&conn)?;
        }
        backup::retain_two(path);
        return Ok(Opened {
            conn: Some(conn),
            status: MetadataStatus::Ok,
        });
    }
    Ok(readonly(
        path,
        MetadataStatus::Disabled {
            reason: "schema changed during startup".into(),
        },
    ))
}

fn secure(path: &Path, mode: u32) -> Result<(), MetadataStoreError> {
    fs::set_permissions(path, Permissions::from_mode(mode)).map_err(|source| {
        MetadataStoreError::Io {
            action: "secure metadata permissions",
            path: path.to_owned(),
            source,
        }
    })
}

pub(super) fn open_in_memory() -> Result<Connection, MetadataStoreError> {
    let conn = Connection::open_in_memory()?;
    configure_local(&conn)?;
    let tx = rusqlite::Transaction::new_unchecked(&conn, TransactionBehavior::Immediate)?;
    migrations::run_migrations(&tx)?;
    tx.pragma_update(None, "application_id", 0x43484B00_i32)?;
    tx.commit()?;
    Ok(conn)
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn configure_local(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=5000; PRAGMA secure_delete=ON;")?;
    Ok(())
}

#[cfg(test)]
#[path = "db_tests.rs"]
mod tests;
