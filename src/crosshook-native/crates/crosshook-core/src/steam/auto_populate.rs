use std::path::Path;

use super::diagnostics::DiagnosticCollector;
use super::discovery::discover_steam_root_candidates;
use super::libraries::discover_steam_libraries;
use super::manifest::{compatdata_path_for_match, find_game_match};
use super::models::{
    SteamAutoPopulateFieldState, SteamAutoPopulateRequest, SteamAutoPopulateResult, SteamLibrary,
};
use super::proton::resolve_proton_path;

pub fn attempt_auto_populate(request: &SteamAutoPopulateRequest) -> SteamAutoPopulateResult {
    let mut collector = DiagnosticCollector::default();
    let normalized_game_path = normalize_path(&request.game_path);

    if normalized_game_path.as_os_str().is_empty() {
        collector.add_diagnostic("No game executable path was provided for Steam auto-populate.");
        add_default_manual_hints(&mut collector, &[], &[], "");
        return result_with(
            SteamAutoPopulateFieldState::NotFound,
            String::new(),
            SteamAutoPopulateFieldState::NotFound,
            Default::default(),
            SteamAutoPopulateFieldState::NotFound,
            Default::default(),
            collector,
        );
    }

    collector.add_diagnostic(format!(
        "Normalized game executable path: {}",
        normalized_game_path.display()
    ));
    if !normalized_game_path.is_file() {
        collector.add_diagnostic(
            "The normalized game path does not currently exist on the host filesystem. CrossHook will still attempt a manifest match.",
        );
    }

    let steam_root_candidates = discover_steam_root_candidates(
        &request.steam_client_install_path,
        &mut collector.diagnostics,
    );
    let libraries = discover_steam_libraries(&steam_root_candidates, &mut collector.diagnostics);
    let match_selection = find_game_match(
        &normalized_game_path,
        &libraries,
        &mut collector.diagnostics,
    );

    let app_id_state = match_selection.state;
    let steam_app_id = match_selection
        .matched
        .as_ref()
        .map(|matched| matched.app_id.clone())
        .unwrap_or_default();

    let mut compatdata_state = SteamAutoPopulateFieldState::NotFound;
    let mut compatdata_path = Default::default();

    if let Some(matched) = match_selection.matched.as_ref() {
        let candidate_compatdata_path = compatdata_path_for_match(matched);
        if candidate_compatdata_path.is_dir() {
            compatdata_state = SteamAutoPopulateFieldState::Found;
            compatdata_path = candidate_compatdata_path.clone();
            collector.add_diagnostic(format!(
                "Detected compatdata path: {}",
                candidate_compatdata_path.display()
            ));
        } else {
            collector.add_diagnostic(format!(
                "Derived compatdata path does not exist yet: {}",
                candidate_compatdata_path.display()
            ));
        }
    }

    let mut proton_state = SteamAutoPopulateFieldState::NotFound;
    let mut proton_path = Default::default();

    if let Some(matched) = match_selection.matched.as_ref() {
        let proton_resolution = resolve_proton_path(
            &matched.app_id,
            &steam_root_candidates,
            &mut collector.diagnostics,
        );
        proton_state = proton_resolution.state;
        proton_path = proton_resolution.proton_path;
    }

    add_default_manual_hints(
        &mut collector,
        &steam_root_candidates,
        &libraries,
        &steam_app_id,
    );

    if match_selection.matched.is_none() && app_id_state == SteamAutoPopulateFieldState::NotFound {
        collector.add_hint(
            "Select the game executable from inside a Steam library under steamapps/common so CrossHook can match it against Steam manifests.",
        );
    }

    if compatdata_state != SteamAutoPopulateFieldState::Found && !steam_app_id.is_empty() {
        for library in &libraries {
            collector.add_hint(format!(
                "Compatdata is usually under: {}",
                library
                    .path
                    .join("steamapps")
                    .join("compatdata")
                    .join(&steam_app_id)
                    .display()
            ));
        }
    }

    if proton_state != SteamAutoPopulateFieldState::Found {
        for steam_root_candidate in &steam_root_candidates {
            collector.add_hint(format!(
                "Proton is usually under: {}",
                steam_root_candidate
                    .join("steamapps")
                    .join("common")
                    .display()
            ));
            collector.add_hint(format!(
                "Custom Proton tools are usually under: {}",
                steam_root_candidate.join("compatibilitytools.d").display()
            ));
        }

        for system_root in [
            "/usr/share/steam/compatibilitytools.d",
            "/usr/local/share/steam/compatibilitytools.d",
            "/usr/share/steam/compatibilitytools",
            "/usr/local/share/steam/compatibilitytools",
        ] {
            collector.add_hint(format!(
                "System Steam compat tools may also be under: {system_root}"
            ));
        }
    }

    result_with(
        app_id_state,
        steam_app_id,
        compatdata_state,
        compatdata_path,
        proton_state,
        proton_path,
        collector,
    )
}

fn result_with(
    app_id_state: SteamAutoPopulateFieldState,
    app_id: String,
    compatdata_state: SteamAutoPopulateFieldState,
    compatdata_path: std::path::PathBuf,
    proton_state: SteamAutoPopulateFieldState,
    proton_path: std::path::PathBuf,
    collector: DiagnosticCollector,
) -> SteamAutoPopulateResult {
    let (diagnostics, manual_hints) = collector.finalize();

    SteamAutoPopulateResult {
        app_id_state,
        app_id,
        compatdata_state,
        compatdata_path,
        proton_state,
        proton_path,
        diagnostics,
        manual_hints,
    }
}

fn add_default_manual_hints(
    collector: &mut DiagnosticCollector,
    steam_root_candidates: &[std::path::PathBuf],
    libraries: &[SteamLibrary],
    steam_app_id: &str,
) {
    if steam_root_candidates.is_empty() {
        collector.add_hint(
            "Steam installs are usually under ~/.steam/root, ~/.local/share/Steam, or ~/.var/app/com.valvesoftware.Steam/data/Steam.",
        );
    }

    if libraries.is_empty() {
        collector.add_hint(
            "No Steam libraries were detected. Make sure Steam has been started at least once and libraryfolders.vdf is readable.",
        );
    }

    if !steam_app_id.is_empty() {
        collector.add_hint(format!(
            "If Steam matched the game, the App ID should be {steam_app_id}."
        ));
    }
}

fn normalize_path(path: &Path) -> std::path::PathBuf {
    let trimmed = path.as_os_str().to_string_lossy().trim().to_string();
    if trimmed.is_empty() {
        return Default::default();
    }

    trimmed.into()
}

#[cfg(test)]
mod tests {
    use super::attempt_auto_populate;
    use crate::steam::{SteamAutoPopulateFieldState, SteamAutoPopulateRequest};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn auto_populate_finds_app_id_compatdata_and_proton() {
        let steam_root = tempdir().expect("steam root");
        let steamapps = steam_root.path().join("steamapps");
        let game_dir = steamapps.join("common/Test Game");
        let game_exe = game_dir.join("game.exe");
        let compatdata = steamapps.join("compatdata/12345");
        let proton_dir = steamapps.join("common/GE-Proton-9-4");

        fs::create_dir_all(&game_dir).expect("game dir");
        fs::create_dir_all(&compatdata).expect("compatdata dir");
        fs::create_dir_all(&proton_dir).expect("proton dir");
        fs::create_dir_all(steam_root.path().join("config")).expect("config dir");

        fs::write(&game_exe, b"test").expect("game exe");
        fs::write(
            steamapps.join("appmanifest_12345.acf"),
            r#"
            "AppState"
            {
              "appid" "12345"
              "installdir" "Test Game"
            }
            "#,
        )
        .expect("manifest");
        fs::write(proton_dir.join("proton"), b"#!/bin/sh\n").expect("proton file");
        fs::write(
            steam_root.path().join("config/config.vdf"),
            r#"
            "root"
            {
              "CompatToolMapping"
              {
                "12345"
                {
                  "name" "GE-Proton-9-4"
                }
              }
            }
            "#,
        )
        .expect("config.vdf");

        let result = attempt_auto_populate(&SteamAutoPopulateRequest {
            game_path: game_exe,
            steam_client_install_path: steam_root.path().to_path_buf(),
        });

        assert_eq!(result.app_id_state, SteamAutoPopulateFieldState::Found);
        assert_eq!(result.app_id, "12345");
        assert_eq!(result.compatdata_state, SteamAutoPopulateFieldState::Found);
        assert_eq!(result.compatdata_path, compatdata);
        assert_eq!(result.proton_state, SteamAutoPopulateFieldState::Found);
        assert_eq!(result.proton_path, proton_dir.join("proton"));
        assert!(result.has_any_match());
    }
}
