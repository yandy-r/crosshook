use std::fs;
use std::path::{Path, PathBuf};

use super::models::{SteamAutoPopulateFieldState, SteamGameMatch, SteamLibrary};
use super::vdf::parse_vdf;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SteamGameMatchSelection {
    pub state: SteamAutoPopulateFieldState,
    pub matched: Option<SteamGameMatch>,
}

pub fn find_game_match(
    game_path: impl AsRef<Path>,
    libraries: &[SteamLibrary],
    diagnostics: &mut Vec<String>,
) -> SteamGameMatchSelection {
    let game_path = normalize_path(game_path.as_ref());
    if game_path.as_os_str().is_empty() {
        diagnostics.push("No game executable path was provided.".to_string());
        return SteamGameMatchSelection::default();
    }

    let mut matches = Vec::new();

    for library in libraries {
        let manifest_paths = safe_manifest_paths(&library.steamapps_path, diagnostics);
        for manifest_path in manifest_paths {
            let normalized_manifest_path = normalize_path(&manifest_path);

            match parse_manifest(&manifest_path) {
                Ok((steam_app_id, install_dir_name)) => {
                    if steam_app_id.is_empty() || install_dir_name.is_empty() {
                        continue;
                    }

                    let install_dir_path = library
                        .steamapps_path
                        .join("common")
                        .join(&install_dir_name);

                    if !path_is_same_or_child(&game_path, &install_dir_path) {
                        continue;
                    }

                    matches.push(SteamGameMatch {
                        app_id: steam_app_id,
                        library_path: library.path.clone(),
                        install_dir_path,
                        manifest_path: normalized_manifest_path,
                    });
                }
                Err(error) => diagnostics.push(format!(
                    "Failed to parse app manifest '{}': {error}",
                    normalized_manifest_path.display()
                )),
            }
        }
    }

    let matches = dedupe_matches(matches);
    if matches.is_empty() {
        diagnostics
            .push("No Steam app manifest matched the selected game executable path.".to_string());
        return SteamGameMatchSelection {
            state: SteamAutoPopulateFieldState::NotFound,
            matched: None,
        };
    }

    if matches.len() == 1 {
        let matched = matches.into_iter().next().expect("single match");
        diagnostics.push(format!(
            "Matched Steam App ID {} using manifest: {}",
            matched.app_id,
            matched.manifest_path.display()
        ));
        return SteamGameMatchSelection {
            state: SteamAutoPopulateFieldState::Found,
            matched: Some(matched),
        };
    }

    diagnostics.push(
        "Multiple Steam manifests matched the selected executable. Auto-populate will not guess the Steam App ID."
            .to_string(),
    );
    for matched in &matches {
        diagnostics.push(format!(
            "Conflicting manifest candidate: App ID {} in {}",
            matched.app_id,
            matched.library_path.display()
        ));
    }

    SteamGameMatchSelection {
        state: SteamAutoPopulateFieldState::Ambiguous,
        matched: None,
    }
}

pub fn compatdata_path_for_match(match_: &SteamGameMatch) -> PathBuf {
    match_
        .library_path
        .join("steamapps")
        .join("compatdata")
        .join(&match_.app_id)
}

fn parse_manifest(manifest_path: &Path) -> Result<(String, String), String> {
    let content = fs::read_to_string(manifest_path)
        .map_err(|error| format!("unable to read manifest: {error}"))?;
    let manifest_root = parse_vdf(&content).map_err(|error| error.to_string())?;
    let app_state_node = manifest_root
        .get_child("AppState")
        .unwrap_or(&manifest_root);

    let steam_app_id = app_state_node
        .get_child("appid")
        .and_then(|node| node.value.as_ref())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| extract_app_id_from_manifest_path(manifest_path))
        .unwrap_or_default();

    let install_dir_name = app_state_node
        .get_child("installdir")
        .and_then(|node| node.value.as_ref())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_default();

    Ok((steam_app_id, install_dir_name))
}

fn extract_app_id_from_manifest_path(manifest_path: &Path) -> Option<String> {
    let file_name = manifest_path.file_stem()?.to_str()?;
    const PREFIX: &str = "appmanifest_";

    file_name
        .strip_prefix(PREFIX)
        .map(|value| value.to_string())
}

fn safe_manifest_paths(steamapps_path: &Path, diagnostics: &mut Vec<String>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let Ok(entries) = fs::read_dir(steamapps_path) else {
        diagnostics.push(format!(
            "Unable to enumerate manifests in {}",
            steamapps_path.display()
        ));
        return paths;
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if !is_appmanifest_path(&path) {
            continue;
        }
        paths.push(path);
    }

    paths.sort();
    paths
}

fn is_appmanifest_path(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };

    file_name.starts_with("appmanifest_") && file_name.ends_with(".acf")
}

fn path_is_same_or_child(path: &Path, root: &Path) -> bool {
    if path == root {
        return true;
    }

    path.starts_with(root)
}

fn normalize_path(path: &Path) -> PathBuf {
    let trimmed = path.as_os_str().to_string_lossy().trim().to_string();
    if trimmed.is_empty() {
        return PathBuf::new();
    }

    PathBuf::from(trimmed)
}

fn dedupe_matches(matches: Vec<SteamGameMatch>) -> Vec<SteamGameMatch> {
    let mut unique = Vec::new();

    for matched in matches {
        if unique.iter().any(|existing: &SteamGameMatch| {
            existing.app_id == matched.app_id && existing.library_path == matched.library_path
        }) {
            continue;
        }

        unique.push(matched);
    }

    unique
}

#[cfg(test)]
mod tests {
    use super::{compatdata_path_for_match, find_game_match};
    use crate::steam::models::{SteamAutoPopulateFieldState, SteamLibrary};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn write_manifest(path: &PathBuf, app_id: &str, install_dir: &str) {
        fs::write(
            path,
            format!(
                r#"
                "AppState"
                {{
                  "appid" "{app_id}"
                  "installdir" "{install_dir}"
                }}
                "#
            ),
        )
        .expect("manifest");
    }

    #[test]
    fn finds_single_matching_manifest() {
        let temp_root = tempdir().expect("root");
        let steamapps = temp_root.path().join("steamapps");
        let common = steamapps.join("common");
        let game_dir = common.join("Game");
        let game_exe = game_dir.join("game.exe");

        fs::create_dir_all(&game_dir).expect("game dir");
        fs::create_dir_all(steamapps.join("compatdata/12345")).expect("compatdata");
        write_manifest(&steamapps.join("appmanifest_12345.acf"), "12345", "Game");
        fs::write(&game_exe, b"test").expect("exe");

        let libraries = vec![SteamLibrary {
            path: temp_root.path().to_path_buf(),
            steamapps_path: steamapps.clone(),
        }];

        let mut diagnostics = Vec::new();
        let selection = find_game_match(&game_exe, &libraries, &mut diagnostics);

        assert_eq!(selection.state, SteamAutoPopulateFieldState::Found);
        let matched = selection.matched.expect("match");
        assert_eq!(matched.app_id, "12345");
        assert_eq!(matched.install_dir_path, game_dir);
        assert_eq!(
            compatdata_path_for_match(&matched),
            steamapps.join("compatdata/12345")
        );
        assert!(diagnostics
            .iter()
            .any(|entry| entry.contains("Matched Steam App ID")));
    }

    #[test]
    fn falls_back_to_manifest_filename_for_missing_app_id() {
        let temp_root = tempdir().expect("root");
        let steamapps = temp_root.path().join("steamapps");
        let common = steamapps.join("common");
        let game_dir = common.join("Fallback");
        let game_exe = game_dir.join("game.exe");

        fs::create_dir_all(&game_dir).expect("game dir");
        write_manifest(&steamapps.join("appmanifest_67890.acf"), "", "Fallback");
        fs::write(&game_exe, b"test").expect("exe");

        let libraries = vec![SteamLibrary {
            path: temp_root.path().to_path_buf(),
            steamapps_path: steamapps,
        }];

        let mut diagnostics = Vec::new();
        let selection = find_game_match(&game_exe, &libraries, &mut diagnostics);

        assert_eq!(selection.state, SteamAutoPopulateFieldState::Found);
        assert_eq!(selection.matched.expect("match").app_id, "67890");
    }

    #[test]
    fn reports_ambiguous_matches() {
        let temp_root = tempdir().expect("root");
        let steamapps = temp_root.path().join("steamapps");
        let game_dir = steamapps.join("common/SharedGame");

        fs::create_dir_all(&game_dir).expect("game dir");
        write_manifest(&steamapps.join("appmanifest_1.acf"), "1", "SharedGame");
        write_manifest(&steamapps.join("appmanifest_2.acf"), "2", "SharedGame");
        let game_exe = game_dir.join("game.exe");
        fs::write(&game_exe, b"test").expect("exe");

        let libraries = vec![SteamLibrary {
            path: temp_root.path().to_path_buf(),
            steamapps_path: steamapps,
        }];

        let mut diagnostics = Vec::new();
        let selection = find_game_match(&game_exe, &libraries, &mut diagnostics);

        assert_eq!(selection.state, SteamAutoPopulateFieldState::Ambiguous);
        assert!(selection.matched.is_none());
        assert!(diagnostics
            .iter()
            .any(|entry| entry.contains("Multiple Steam manifests matched")));
    }

    #[test]
    fn reports_not_found_for_unmatched_game_path() {
        let temp_root = tempdir().expect("root");
        let steamapps = temp_root.path().join("steamapps");
        fs::create_dir_all(&steamapps).expect("steamapps");

        let libraries = vec![SteamLibrary {
            path: temp_root.path().to_path_buf(),
            steamapps_path: steamapps,
        }];

        let mut diagnostics = Vec::new();
        let selection = find_game_match("/games/not-steam/game.exe", &libraries, &mut diagnostics);

        assert_eq!(selection.state, SteamAutoPopulateFieldState::NotFound);
        assert!(selection.matched.is_none());
        assert!(diagnostics
            .iter()
            .any(|entry| entry.contains("No Steam app manifest matched")));
    }
}
