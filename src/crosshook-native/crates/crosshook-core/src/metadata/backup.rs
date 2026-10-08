//! Pre-migration snapshots. Only exact CrossHook-owned regular files are retained.
use std::fs::{self, File, OpenOptions, Permissions};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use chrono::{NaiveDateTime, Utc};
use rusqlite::Connection;

use super::MetadataStoreError;

fn io(action: &'static str, path: &Path, source: std::io::Error) -> MetadataStoreError {
    MetadataStoreError::Io {
        action,
        path: path.to_owned(),
        source,
    }
}

/// Newest first, using timestamp and collision suffix, not mutable filesystem mtime.
pub(super) fn files(db_path: &Path) -> Vec<PathBuf> {
    let Some(parent) = db_path.parent() else {
        return Vec::new();
    };
    let Some(base) = db_path.file_name().and_then(|n| n.to_str()) else {
        return Vec::new();
    };
    let prefix = format!("{base}.bak-v");
    let Ok(entries) = fs::read_dir(parent) else {
        return Vec::new();
    };
    let mut owned = Vec::new();
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            continue;
        }
        let name = entry.file_name();
        let Some(rest) = name.to_str().and_then(|name| name.strip_prefix(&prefix)) else {
            continue;
        };
        let Some((version, tail)) = rest.split_once('-') else {
            continue;
        };
        let Ok(version_number) = version.parse::<u32>() else {
            continue;
        };
        if version_number == 0 || version_number.to_string() != version {
            continue;
        }
        let (stamp, collision) = match tail.split_once('-') {
            Some((stamp, suffix)) => {
                let Ok(n) = suffix.parse::<u32>() else {
                    continue;
                };
                if n == 0 || n.to_string() != suffix {
                    continue;
                }
                (stamp, n)
            }
            None => (tail, 0),
        };
        if stamp.len() != 16 || NaiveDateTime::parse_from_str(stamp, "%Y%m%dT%H%M%SZ").is_err() {
            continue;
        }
        owned.push((stamp.to_owned(), collision, entry.path()));
    }
    owned.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(b.2.cmp(&a.2)));
    owned.into_iter().map(|(_, _, path)| path).collect()
}

pub(super) fn snapshot(
    conn: &Connection,
    path: &Path,
    version: u32,
) -> Result<PathBuf, MetadataStoreError> {
    let base = path
        .file_name()
        .ok_or_else(|| MetadataStoreError::Validation("missing database filename".into()))?
        .to_string_lossy();
    let parent = path
        .parent()
        .ok_or_else(|| MetadataStoreError::Validation("missing database parent".into()))?;
    let staging = tempfile::Builder::new()
        .prefix(".metadata-backup-")
        .permissions(Permissions::from_mode(0o700))
        .tempdir_in(parent)
        .map_err(|error| io("create metadata backup staging directory", parent, error))?;
    let stage = staging.path().join("snapshot.db");
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&stage)
        .map_err(|error| io("create staged metadata backup", &stage, error))?;
    // FULL ensures SQLite syncs VACUUM output; stage stays private until complete.
    conn.pragma_update(None, "synchronous", "FULL")?;
    let stage_text = stage
        .to_str()
        .ok_or_else(|| MetadataStoreError::Validation("backup path is not UTF-8".into()))?;
    conn.execute("VACUUM INTO ?1", [stage_text])?;
    if output
        .metadata()
        .map_err(|error| io("inspect metadata backup", &stage, error))?
        .len()
        == 0
    {
        return Err(MetadataStoreError::Corrupt("empty metadata backup".into()));
    }
    output
        .sync_all()
        .map_err(|error| io("sync metadata backup", &stage, error))?;
    let target = publish(
        &stage,
        path,
        version,
        &Utc::now().format("%Y%m%dT%H%M%SZ").to_string(),
        &base,
    )?;
    if let Err(error) = sync_directory(parent) {
        discard(&target);
        return Err(error);
    }
    Ok(target)
}

// Hard-link publication is atomic, same-filesystem, and never follows/replaces a target.
fn publish(
    stage: &Path,
    path: &Path,
    version: u32,
    stamp: &str,
    base: &str,
) -> Result<PathBuf, MetadataStoreError> {
    let mut collision = 0_u32;
    loop {
        let suffix = if collision == 0 {
            String::new()
        } else {
            format!("-{collision}")
        };
        let target = path.with_file_name(format!("{base}.bak-v{version}-{stamp}{suffix}"));
        match fs::hard_link(stage, &target) {
            Ok(()) => return Ok(target),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                collision = collision
                    .checked_add(1)
                    .ok_or_else(|| io("publish metadata backup", &target, error))?;
            }
            Err(error) => return Err(io("publish metadata backup", &target, error)),
        }
    }
}

fn sync_directory(parent: &Path) -> Result<(), MetadataStoreError> {
    File::open(parent)
        .and_then(|dir| dir.sync_all())
        .map_err(|error| io("sync metadata backup directory", parent, error))
}

pub(super) fn discard(path: &Path) {
    if let Err(error) = fs::remove_file(path) {
        tracing::warn!(%error, path = %path.display(), "Failed to remove partial metadata backup");
    }
}

pub(super) fn retain_two(path: &Path) {
    let mut removed = false;
    for old in files(path).into_iter().skip(2) {
        match fs::remove_file(&old) {
            Ok(()) => removed = true,
            Err(error) => {
                tracing::warn!(%error, path = %old.display(), "Failed to prune metadata backup");
            }
        }
    }
    if let (true, Some(parent)) = (removed, path.parent()) {
        if let Err(error) = sync_directory(parent) {
            tracing::warn!(%error, "Failed to sync metadata backup retention");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn publication_preserves_decoy_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let stage = dir.path().join("stage");
        fs::write(&stage, b"snapshot").unwrap();
        let victim = dir.path().join("victim");
        fs::write(&victim, b"untouched").unwrap();
        let path = dir.path().join("metadata.db");
        let decoy = dir.path().join("metadata.db.bak-v26-20260101T000000Z");
        std::os::unix::fs::symlink(&victim, &decoy).unwrap();
        let output = publish(&stage, &path, 26, "20260101T000000Z", "metadata.db").unwrap();
        assert_eq!(
            output.file_name().unwrap(),
            "metadata.db.bak-v26-20260101T000000Z-1"
        );
        assert!(decoy.is_symlink());
        assert_eq!(fs::read(&victim).unwrap(), b"untouched");
        assert_eq!(fs::read(output).unwrap(), b"snapshot");
    }
}
