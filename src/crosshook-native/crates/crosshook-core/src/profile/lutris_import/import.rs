use std::collections::HashMap;
use std::path::PathBuf;

use crate::profile::ProfileStore;

use super::error::LutrisImportError;
use super::map::{is_wine_or_proton_runner, map_lutris_to_profile, MappedLutrisInput};
use super::parse::{parse_lutris_yaml, LutrisGameConfig};
use super::paths::{discover_lutris_root, list_game_configs, read_pga_games, PgaGame};
use super::types::{
    LutrisImportEntry, LutrisImportEntryResult, LutrisImportOutcome, LutrisImportPreview,
    LutrisImportResult,
};

pub fn preview_lutris_import(
    root: Option<PathBuf>,
    profile_store: Option<&ProfileStore>,
) -> Result<LutrisImportPreview, LutrisImportError> {
    let lutris_root = match root {
        Some(path) if path.is_dir() => Some(path),
        Some(_) => None,
        None => discover_lutris_root(),
    };

    let Some(lutris_root) = lutris_root else {
        return Ok(LutrisImportPreview {
            entries: Vec::new(),
            lutris_root: None,
            diagnostics: vec!["Lutris library directory was not found.".to_string()],
        });
    };

    let mut diagnostics = Vec::new();
    let pga_games = match read_pga_games(&lutris_root) {
        Ok(games) => games,
        Err(error) => {
            diagnostics.push(error.to_string());
            Vec::new()
        }
    };

    let pga_by_configpath: HashMap<String, PgaGame> = pga_games
        .into_iter()
        .filter_map(|game| game.configpath.clone().map(|configpath| (configpath, game)))
        .collect();

    let existing_names = profile_store
        .and_then(|store| store.list().ok())
        .unwrap_or_default();

    let mut entries = Vec::new();
    for config_path in list_game_configs(&lutris_root) {
        let config_stem = config_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();

        let pga = pga_by_configpath.get(&config_stem).cloned();

        let config = match parse_lutris_yaml(&config_path) {
            Ok(parsed) => parsed,
            Err(error) => {
                diagnostics.push(format!("Skipped '{}': {error}", config_path.display()));
                continue;
            }
        };

        if let Some(pga_row) = &pga {
            if !is_wine_or_proton_runner(&pga_row.runner) {
                continue;
            }
        } else if !yaml_indicates_wine_or_proton(&config) {
            continue;
        }

        let mapped = map_lutris_to_profile(MappedLutrisInput {
            config,
            pga,
            lutris_root: lutris_root.clone(),
            source_path: config_path.clone(),
        });

        let base_name = sanitize_profile_name(&mapped.game_name);
        let suggested_name = derive_unique_profile_name(&base_name, &existing_names, &entries);

        entries.push(LutrisImportEntry {
            source_path: config_path,
            suggested_name,
            game_name: mapped.game_name,
            runner: mapped.runner,
            mapped: mapped.profile,
            warnings: mapped.warnings,
            importable: mapped.importable,
        });
    }

    Ok(LutrisImportPreview {
        entries,
        lutris_root: Some(lutris_root),
        diagnostics,
    })
}

pub fn apply_lutris_import(
    store: &ProfileStore,
    entries: Vec<LutrisImportEntry>,
) -> LutrisImportResult {
    let mut results = Vec::new();
    let mut imported_count = 0usize;
    let mut skipped_count = 0usize;
    let mut failed_count = 0usize;
    let mut reserved_names = store.list().unwrap_or_default();

    for entry in entries {
        if !entry.importable {
            skipped_count += 1;
            results.push(LutrisImportEntryResult {
                entry: entry.clone(),
                outcome: LutrisImportOutcome::Skipped,
                profile_name: None,
                profile_path: None,
                error: Some("Entry is not importable.".to_string()),
            });
            continue;
        }

        let profile_name = derive_unique_profile_name(&entry.suggested_name, &reserved_names, &[]);

        match store.save(&profile_name, &entry.mapped) {
            Ok(()) => {
                reserved_names.push(profile_name.clone());
                imported_count += 1;
                let profile_path = store
                    .profile_path(&profile_name)
                    .map(|path| path.to_path_buf())
                    .ok();
                results.push(LutrisImportEntryResult {
                    entry,
                    outcome: LutrisImportOutcome::Imported,
                    profile_name: Some(profile_name),
                    profile_path,
                    error: None,
                });
            }
            Err(error) => {
                failed_count += 1;
                results.push(LutrisImportEntryResult {
                    entry,
                    outcome: LutrisImportOutcome::Failed,
                    profile_name: None,
                    profile_path: None,
                    error: Some(error.to_string()),
                });
            }
        }
    }

    LutrisImportResult {
        results,
        imported_count,
        skipped_count,
        failed_count,
    }
}

fn yaml_indicates_wine_or_proton(config: &LutrisGameConfig) -> bool {
    config.wine.is_some()
        || config
            .runner
            .as_deref()
            .is_some_and(is_wine_or_proton_runner)
}

const MAX_UNIQUE_NAME_ATTEMPTS: u32 = 1000;

fn sanitize_profile_name(name: &str) -> String {
    let mut slug = String::with_capacity(name.len());
    let mut last_was_separator = false;

    for ch in name.trim().chars() {
        if ch.is_alphanumeric() {
            for lower in ch.to_lowercase() {
                slug.push(lower);
            }
            last_was_separator = false;
        } else if !last_was_separator {
            slug.push('-');
            last_was_separator = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "lutris-game".to_string()
    } else {
        slug
    }
}

fn derive_unique_profile_name(
    base: &str,
    existing: &[String],
    pending: &[LutrisImportEntry],
) -> String {
    let mut taken: Vec<String> = existing.to_vec();
    taken.extend(pending.iter().map(|entry| entry.suggested_name.clone()));

    if !taken.iter().any(|name| name == base) {
        return base.to_string();
    }

    for index in 2..=MAX_UNIQUE_NAME_ATTEMPTS {
        let candidate = format!("{base}-{index}");
        if !taken.iter().any(|name| name == &candidate) {
            return candidate;
        }
    }

    let fallback = format!("{base}-copy");
    if !taken.iter().any(|name| name == &fallback) {
        return fallback;
    }

    for index in 2..=MAX_UNIQUE_NAME_ATTEMPTS {
        let candidate = format!("{base}-copy-{index}");
        if !taken.iter().any(|name| name == &candidate) {
            return candidate;
        }
    }

    format!("{base}-copy-{}", MAX_UNIQUE_NAME_ATTEMPTS + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    fn write_sample_lutris_tree(root: &Path) {
        let games_dir = root.join("games");
        fs::create_dir_all(&games_dir).unwrap();
        fs::write(
            games_dir.join("sample-game.yml"),
            r#"name: Sample Game
game:
  exe: /games/sample/game.exe
  prefix: ~/Games/sample-prefix
wine:
  version: lutris-GE-Proton8-14-x86_64
  esync: false
system:
  env:
    WINEDLLOVERRIDES: d3d11=
"#,
        )
        .unwrap();

        let db_path = root.join("pga.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE games (
                id INTEGER PRIMARY KEY,
                name TEXT,
                slug TEXT,
                runner TEXT,
                directory TEXT,
                configpath TEXT
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO games (name, slug, runner, directory, configpath)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                "Sample Game",
                "sample-game",
                "wine",
                "/games/sample",
                "sample-game"
            ],
        )
        .unwrap();
    }

    #[test]
    fn preview_is_read_only() {
        let temp = tempdir().unwrap();
        write_sample_lutris_tree(temp.path());

        let preview = preview_lutris_import(Some(temp.path().to_path_buf()), None).unwrap();
        assert_eq!(preview.entries.len(), 1);
        assert!(preview.lutris_root.is_some());
        assert!(!temp.path().join("profiles").exists());
    }

    #[test]
    fn apply_writes_importable_profiles() {
        let temp = tempdir().unwrap();
        write_sample_lutris_tree(temp.path());
        let profiles_dir = temp.path().join("profiles");
        fs::create_dir_all(&profiles_dir).unwrap();

        let preview = preview_lutris_import(Some(temp.path().to_path_buf()), None).unwrap();
        let store = ProfileStore::with_base_path(profiles_dir.clone());
        let result = apply_lutris_import(&store, preview.entries);
        assert_eq!(result.imported_count, 1);
        assert!(profiles_dir.join("sample-game.toml").exists());
    }
}
