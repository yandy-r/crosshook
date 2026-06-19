use directories::BaseDirs;
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};

use super::LutrisImportError;

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
    let base = BaseDirs::new()?;
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

/// Lists per-game Lutris YAML configs under `games/` (flat and balanced layouts).
pub fn list_game_configs(root: &Path) -> Vec<PathBuf> {
    let games_dir = root.join("games");
    if !games_dir.is_dir() {
        return Vec::new();
    }

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

        let path = entry.path();
        if path.is_file() && is_lutris_yaml(&path) {
            configs.push(path);
            continue;
        }

        if !path.is_dir() {
            continue;
        }

        let Ok(sub_entries) = std::fs::read_dir(&path) else {
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

            let sub_path = sub_entry.path();
            if sub_path.is_file() && is_lutris_yaml(&sub_path) {
                configs.push(sub_path);
            }
        }
    }

    configs.sort();
    configs
}

/// Reads Lutris game metadata from `<root>/pga.db` without modifying the database.
pub fn read_pga_games(root: &Path) -> Result<Vec<PgaGame>, LutrisImportError> {
    let db_path = root.join("pga.db");
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
