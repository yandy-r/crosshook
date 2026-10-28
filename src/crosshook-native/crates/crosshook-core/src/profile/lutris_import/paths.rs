use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};

use super::LutrisImportError;
use crate::fs_util::ensure_within_workspace;

const MAX_LUTRIS_LIBRARY_ENTRIES: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgaGame {
    pub name: String,
    pub slug: String,
    pub runner: String,
    pub directory: Option<String>,
    pub configpath: Option<String>,
}

/// Returns the first existing Lutris library root, in precedence order.
pub fn discover_lutris_root() -> Option<PathBuf> {
    let base = crate::platform::base_dirs()?;
    let home = base.home_dir();

    [
        base.config_dir().join("lutris"),
        base.data_local_dir().join("lutris"),
        home.join(".var/app/net.lutris.Lutris/config/lutris"),
        home.join(".var/app/net.lutris.Lutris/data/lutris"),
    ]
    .into_iter()
    .find(|candidate| candidate.is_dir())
}

/// Canonicalizes the library root once so joins and containment agree on the
/// same spelling even when the caller passes a relative or symlinked root.
/// On failure the supplied root is kept, preserving the missing-root behaviour.
fn canonical_root(root: &Path) -> PathBuf {
    std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf())
}

/// Lists per-game Lutris YAML configs under `games/` (flat and balanced layouts).
pub fn list_game_configs(root: &Path) -> Vec<PathBuf> {
    let root = canonical_root(root);
    let games_dir = root.join("games");
    // Contain `games/` before reading: an outward symlink must never grant the
    // scanner a directory outside the trusted library root.
    let games_dir = match ensure_within_workspace(&root, &games_dir) {
        Ok(resolved) if resolved.is_dir() => resolved,
        Ok(_) => {
            tracing::warn!(
                root = %root.display(),
                "Lutris games/ is not a directory; no configs scanned"
            );
            return Vec::new();
        }
        Err(error) => {
            tracing::warn!(
                root = %root.display(),
                error = %error,
                "Lutris games/ failed workspace containment; no configs scanned"
            );
            return Vec::new();
        }
    };

    let Ok(entries) = std::fs::read_dir(&games_dir) else {
        return Vec::new();
    };

    let mut configs = Vec::new();
    'scan: for entry in entries.flatten() {
        if configs.len() >= MAX_LUTRIS_LIBRARY_ENTRIES {
            tracing::warn!(
                limit = MAX_LUTRIS_LIBRARY_ENTRIES,
                root = %root.display(),
                "Lutris game config scan hit library entry cap; remaining configs ignored"
            );
            break 'scan;
        }

        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if file_type.is_file() && is_lutris_yaml(&path) {
            push_contained_config(&root, &path, &mut configs);
            continue;
        }

        if !file_type.is_dir() {
            continue;
        }

        // Balanced layout: one nested level. Contain the nested directory
        // before scanning it.
        let nested_dir = match ensure_within_workspace(&root, &path) {
            Ok(resolved) => resolved,
            Err(error) => {
                tracing::warn!(
                    root = %root.display(),
                    error = %error,
                    "Lutris nested games directory failed workspace containment; skipping"
                );
                continue;
            }
        };
        let Ok(sub_entries) = std::fs::read_dir(&nested_dir) else {
            continue;
        };
        for sub_entry in sub_entries.flatten() {
            if configs.len() >= MAX_LUTRIS_LIBRARY_ENTRIES {
                tracing::warn!(
                    limit = MAX_LUTRIS_LIBRARY_ENTRIES,
                    root = %root.display(),
                    "Lutris game config scan hit library entry cap; remaining configs ignored"
                );
                break 'scan;
            }

            let Ok(sub_file_type) = sub_entry.file_type() else {
                continue;
            };
            let sub_path = sub_entry.path();
            if sub_file_type.is_file() && is_lutris_yaml(&sub_path) {
                push_contained_config(&root, &sub_path, &mut configs);
            }
        }
    }

    configs.sort();
    configs
}

/// Validates `candidate` against the library root and appends the resolved
/// path; failures are skipped with a diagnostic instead of aborting the scan.
fn push_contained_config(root: &Path, candidate: &Path, configs: &mut Vec<PathBuf>) {
    match ensure_within_workspace(root, candidate) {
        Ok(resolved) => configs.push(resolved),
        Err(error) => {
            tracing::warn!(
                root = %root.display(),
                error = %error,
                "Lutris game config failed workspace containment; skipping"
            );
        }
    }
}

/// Reads Lutris game metadata from `<root>/pga.db` without modifying the database.
pub fn read_pga_games(root: &Path) -> Result<Vec<PgaGame>, LutrisImportError> {
    let root = canonical_root(root);
    let db_path = root.join("pga.db");
    // Contain pga.db before opening it read-only: an outward symlink must never
    // point the importer at a database outside the trusted library root.
    let db_path = ensure_within_workspace(&root, &db_path).map_err(|_| LutrisImportError::Pga {
        message: "Lutris database path could not be resolved inside the library root".into(),
    })?;
    let conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(
        |error| LutrisImportError::Pga {
            message: format!("failed to open '{}': {error}", db_path.display()),
        },
    )?;

    let mut stmt = conn
        .prepare("SELECT name, slug, runner, directory, configpath FROM games LIMIT ?1")
        .map_err(|error| LutrisImportError::Pga {
            message: error.to_string(),
        })?;

    let limit =
        i64::try_from(MAX_LUTRIS_LIBRARY_ENTRIES).expect("MAX_LUTRIS_LIBRARY_ENTRIES fits in i64");
    let rows = stmt
        .query_map([limit + 1], |row| {
            Ok(PgaGame {
                name: row.get(0)?,
                slug: row.get(1)?,
                runner: row.get(2)?,
                directory: row.get::<_, Option<String>>(3)?,
                configpath: row.get::<_, Option<String>>(4)?,
            })
        })
        .map_err(|error| LutrisImportError::Pga {
            message: error.to_string(),
        })?;

    let mut games = Vec::new();
    for row in rows {
        match row {
            Ok(game) => games.push(game),
            Err(_) => continue,
        }
    }

    if games.len() > MAX_LUTRIS_LIBRARY_ENTRIES {
        tracing::warn!(
            limit = MAX_LUTRIS_LIBRARY_ENTRIES,
            root = %root.display(),
            "Lutris pga.db query hit library entry cap; remaining games ignored"
        );
        games.truncate(MAX_LUTRIS_LIBRARY_ENTRIES);
    }

    Ok(games)
}

fn is_lutris_yaml(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext == "yml")
}
