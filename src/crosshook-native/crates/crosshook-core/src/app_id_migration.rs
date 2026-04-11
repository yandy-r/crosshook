//! One-time migration of XDG roots from the legacy Tauri app identifier to the current one.
//!
//! When `identifier` in `tauri.conf.json` changes, Tauri may resolve per-app paths under
//! `~/.config/<identifier>/`, etc. CrossHook core stores data under `.../crosshook/` inside those
//! roots; this module moves the entire legacy app-id directory to the new name when the
//! destination is absent or empty.

use std::fs;
use std::path::{Path, PathBuf};

/// Legacy Tauri `identifier` segment used before Flathub-compliant app ID adoption.
pub const LEGACY_TAURI_APP_ID_DIR: &str = "com.crosshook.native";
/// Current Tauri `identifier` directory segment (must match `tauri.conf.json`).
pub const CURRENT_TAURI_APP_ID_DIR: &str = "io.github.yandy-r.CrossHook";

fn dir_is_empty(path: &Path) -> Result<bool, std::io::Error> {
    let mut it = fs::read_dir(path)?;
    Ok(it.next().is_none())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let dest = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &dest)?;
        } else {
            fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

/// Moves `old_root` to `new_root` if `old_root` exists as a directory and `new_root` is missing
/// or an empty directory. If `new_root` exists and is non-empty, migration is skipped.
///
/// On failure of `rename` (e.g. cross-device), falls back to recursive copy then removes `old_root`.
pub fn migrate_one_app_id_root(old_root: &Path, new_root: &Path) -> Result<(), String> {
    if !old_root.exists() {
        return Ok(());
    }
    if !old_root.is_dir() {
        return Ok(());
    }

    if new_root.exists() {
        if dir_is_empty(new_root).map_err(|e| e.to_string())? {
            fs::remove_dir(new_root).map_err(|e| e.to_string())?;
        } else {
            tracing::info!(
                from = %old_root.display(),
                to = %new_root.display(),
                "skipping app-id migration: destination exists and is not empty"
            );
            return Ok(());
        }
    }

    match fs::rename(old_root, new_root) {
        Ok(()) => {
            tracing::info!(
                from = %old_root.display(),
                to = %new_root.display(),
                "migrated Tauri app-id directory (rename)"
            );
            Ok(())
        }
        Err(rename_err) => {
            tracing::warn!(
                error = %rename_err,
                from = %old_root.display(),
                to = %new_root.display(),
                "app-id directory rename failed; trying copy+remove"
            );
            copy_dir_recursive(old_root, new_root).map_err(|e| e.to_string())?;
            fs::remove_dir_all(old_root).map_err(|e| e.to_string())?;
            tracing::info!(
                from = %old_root.display(),
                to = %new_root.display(),
                "migrated Tauri app-id directory (copy+remove)"
            );
            Ok(())
        }
    }
}

/// Runs migration for config, local data, and cache roots. Best-effort: logs warnings and continues.
#[cfg(target_os = "linux")]
pub fn migrate_legacy_tauri_app_id_xdg_directories() {
    let Some(dirs) = directories::BaseDirs::new() else {
        tracing::warn!("app-id migration skipped: could not resolve base directories");
        return;
    };

    let pairs: [(PathBuf, PathBuf); 3] = [
        (
            dirs.config_dir().join(LEGACY_TAURI_APP_ID_DIR),
            dirs.config_dir().join(CURRENT_TAURI_APP_ID_DIR),
        ),
        (
            dirs.data_local_dir().join(LEGACY_TAURI_APP_ID_DIR),
            dirs.data_local_dir().join(CURRENT_TAURI_APP_ID_DIR),
        ),
        (
            dirs.cache_dir().join(LEGACY_TAURI_APP_ID_DIR),
            dirs.cache_dir().join(CURRENT_TAURI_APP_ID_DIR),
        ),
    ];

    for (old, new) in pairs {
        if let Err(e) = migrate_one_app_id_root(&old, &new) {
            tracing::warn!(
                error = %e,
                from = %old.display(),
                to = %new.display(),
                "app-id migration failed for one XDG root"
            );
            eprintln!(
                "CrossHook: app-id migration failed {} -> {}: {e}",
                old.display(),
                new.display()
            );
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub fn migrate_legacy_tauri_app_id_xdg_directories() {}

/// Test helper: run the same triple of migrations using arbitrary base paths (e.g. temp dirs).
pub fn migrate_legacy_tauri_app_id_xdg_directories_for_roots(
    config_dir: &Path,
    data_local_dir: &Path,
    cache_dir: &Path,
) -> Vec<String> {
    let pairs = [
        (
            config_dir.join(LEGACY_TAURI_APP_ID_DIR),
            config_dir.join(CURRENT_TAURI_APP_ID_DIR),
        ),
        (
            data_local_dir.join(LEGACY_TAURI_APP_ID_DIR),
            data_local_dir.join(CURRENT_TAURI_APP_ID_DIR),
        ),
        (
            cache_dir.join(LEGACY_TAURI_APP_ID_DIR),
            cache_dir.join(CURRENT_TAURI_APP_ID_DIR),
        ),
    ];

    let mut errors = Vec::new();
    for (old, new) in pairs {
        if let Err(e) = migrate_one_app_id_root(&old, &new) {
            errors.push(format!("{} -> {}: {e}", old.display(), new.display()));
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn migrates_when_destination_missing() {
        let t = tempdir().unwrap();
        let cfg = t.path().join("config");
        let data = t.path().join("data");
        let cache = t.path().join("cache");
        let old = cfg.join(LEGACY_TAURI_APP_ID_DIR);
        fs::create_dir_all(old.join("nested")).unwrap();
        fs::write(old.join("nested/a.txt"), b"hi").unwrap();

        let errs = migrate_legacy_tauri_app_id_xdg_directories_for_roots(&cfg, &data, &cache);
        assert!(errs.is_empty(), "{errs:?}");

        let new = cfg.join(CURRENT_TAURI_APP_ID_DIR);
        assert!(new.join("nested/a.txt").exists());
        assert!(!old.exists());
    }

    #[test]
    fn no_op_when_source_missing() {
        let t = tempdir().unwrap();
        let cfg = t.path().join("config");
        let data = t.path().join("data");
        let cache = t.path().join("cache");
        fs::create_dir_all(&cfg).unwrap();

        let errs = migrate_legacy_tauri_app_id_xdg_directories_for_roots(&cfg, &data, &cache);
        assert!(errs.is_empty(), "{errs:?}");
        assert!(!cfg.join(CURRENT_TAURI_APP_ID_DIR).exists());
    }

    #[test]
    fn skips_when_destination_non_empty() {
        let t = tempdir().unwrap();
        let cfg = t.path().join("config");
        let data = t.path().join("data");
        let cache = t.path().join("cache");
        let old = cfg.join(LEGACY_TAURI_APP_ID_DIR);
        let new = cfg.join(CURRENT_TAURI_APP_ID_DIR);
        fs::create_dir_all(old.join("from_old")).unwrap();
        fs::create_dir_all(new.join("already")).unwrap();
        fs::write(new.join("already/b.txt"), b"x").unwrap();

        let errs = migrate_legacy_tauri_app_id_xdg_directories_for_roots(&cfg, &data, &cache);
        assert!(errs.is_empty(), "{errs:?}");
        assert!(old.exists());
        assert!(new.join("already/b.txt").exists());
        assert!(!new.join("from_old").exists());
    }

    #[test]
    fn migrates_when_destination_exists_empty() {
        let t = tempdir().unwrap();
        let cfg = t.path().join("config");
        let data = t.path().join("data");
        let cache = t.path().join("cache");
        let old = cfg.join(LEGACY_TAURI_APP_ID_DIR);
        let new = cfg.join(CURRENT_TAURI_APP_ID_DIR);
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("x.toml"), b"x").unwrap();
        fs::create_dir_all(&new).unwrap();

        let errs = migrate_legacy_tauri_app_id_xdg_directories_for_roots(&cfg, &data, &cache);
        assert!(errs.is_empty(), "{errs:?}");
        assert!(new.join("x.toml").exists());
        assert!(!old.exists());
    }

    #[test]
    fn one_root_failure_does_not_stop_others() {
        let t = tempdir().unwrap();
        let cfg = t.path().join("config");
        let data = t.path().join("data");
        let cache = t.path().join("cache");
        // Block data migration: new exists and is non-empty
        let old_d = data.join(LEGACY_TAURI_APP_ID_DIR);
        let new_d = data.join(CURRENT_TAURI_APP_ID_DIR);
        fs::create_dir_all(old_d.join("a")).unwrap();
        fs::create_dir_all(new_d.join("b")).unwrap();
        fs::write(new_d.join("b/keep.txt"), b"1").unwrap();

        // Config migrates OK
        let old_c = cfg.join(LEGACY_TAURI_APP_ID_DIR);
        fs::create_dir_all(&old_c).unwrap();
        fs::write(old_c.join("settings.toml"), b"").unwrap();

        let errs = migrate_legacy_tauri_app_id_xdg_directories_for_roots(&cfg, &data, &cache);
        assert!(errs.is_empty());

        assert!(cfg.join(CURRENT_TAURI_APP_ID_DIR).join("settings.toml").exists());
        assert!(old_d.exists());
        assert!(new_d.join("b/keep.txt").exists());
    }
}
