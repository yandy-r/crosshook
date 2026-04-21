//! First-run detection: decide whether the sandbox is empty and the host has data.

use std::path::{Path, PathBuf};

use crate::flatpak_migration::FlatpakMigrationError;
use crate::fs_util::dir_is_empty;

pub(crate) fn host_config_dir(home: &Path) -> PathBuf {
    home.join(".config").join("crosshook")
}

pub(crate) fn host_data_dir(home: &Path) -> PathBuf {
    home.join(".local/share").join("crosshook")
}

pub(crate) fn needs_first_run(
    sandbox_config_root: &Path,
    host_config_root: &Path,
) -> Result<bool, FlatpakMigrationError> {
    // Host must exist and be non-empty.
    let host_populated = match dir_is_empty(host_config_root) {
        Ok(empty) => !empty,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
        Err(err) => {
            return Err(FlatpakMigrationError::Io {
                path: host_config_root.to_path_buf(),
                source: err,
            });
        }
    };
    if !host_populated {
        return Ok(false);
    }
    // Sandbox missing → needs migration. Sandbox empty → needs migration. Sandbox populated → skip.
    match dir_is_empty(sandbox_config_root) {
        Ok(empty) => Ok(empty),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(err) => Err(FlatpakMigrationError::Io {
            path: sandbox_config_root.to_path_buf(),
            source: err,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn populate(dir: &Path) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("sentinel.txt"), b"x").unwrap();
    }

    #[test]
    fn host_present_sandbox_empty_returns_true() {
        let host = tempdir().unwrap();
        let sandbox = tempdir().unwrap();
        populate(host.path());
        assert!(needs_first_run(sandbox.path(), host.path()).unwrap());
    }

    #[test]
    fn host_missing_returns_false() {
        let host = tempdir().unwrap().path().join("missing");
        let sandbox = tempdir().unwrap();
        assert!(!needs_first_run(sandbox.path(), &host).unwrap());
    }

    #[test]
    fn host_empty_returns_false() {
        let host = tempdir().unwrap();
        let sandbox = tempdir().unwrap();
        // both exist, both empty
        assert!(!needs_first_run(sandbox.path(), host.path()).unwrap());
    }

    #[test]
    fn both_populated_returns_false() {
        let host = tempdir().unwrap();
        let sandbox = tempdir().unwrap();
        populate(host.path());
        populate(sandbox.path());
        assert!(!needs_first_run(sandbox.path(), host.path()).unwrap());
    }

    #[test]
    fn host_present_sandbox_missing_returns_true() {
        let host = tempdir().unwrap();
        let sandbox_parent = tempdir().unwrap();
        populate(host.path());
        let sandbox_missing = sandbox_parent.path().join("never-created");
        assert!(needs_first_run(&sandbox_missing, host.path()).unwrap());
    }

    #[test]
    fn host_config_dir_derives_correct_path() {
        let home = Path::new("/home/user");
        assert_eq!(
            host_config_dir(home),
            PathBuf::from("/home/user/.config/crosshook")
        );
    }

    #[test]
    fn host_data_dir_derives_correct_path() {
        let home = Path::new("/home/user");
        assert_eq!(
            host_data_dir(home),
            PathBuf::from("/home/user/.local/share/crosshook")
        );
    }
}
