//! Regression tests for a symlinked default Steam root. The default native
//! Steam layout has `~/.steam/root` as a symlink to `~/.local/share/Steam`,
//! and `libraryfolders.vdf` lists the real path. Discovery must treat both as
//! one root, one library and one set of Proton installs.

use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use super::discovery::discover_steam_root_candidates_with_home;
use super::libraries::discover_steam_libraries;
use super::manifest::find_game_match;
use super::models::SteamAutoPopulateFieldState;
use super::proton::discovery::discover_compat_tools_with_roots;
use super::proton::resolve_proton_path;

const APP_ID: &str = "12345";
const PROTON_NAME: &str = "GE-Proton9-4";

struct SymlinkedSteamHome {
    _home: tempfile::TempDir,
    home: PathBuf,
    real_root: PathBuf,
    linked_root: PathBuf,
}

impl SymlinkedSteamHome {
    fn new() -> Self {
        let home_dir = tempfile::tempdir().expect("temp home");
        let home = home_dir.path().to_path_buf();
        let real_root = home.join(".local/share/Steam");
        let steamapps = real_root.join("steamapps");
        let game_dir = steamapps.join("common/Test Game");
        let proton_dir = steamapps.join("common").join(PROTON_NAME);

        fs::create_dir_all(&game_dir).expect("game dir");
        fs::create_dir_all(steamapps.join("compatdata").join(APP_ID)).expect("compatdata");
        fs::create_dir_all(&proton_dir).expect("proton dir");
        fs::create_dir_all(real_root.join("config")).expect("config dir");
        fs::write(game_dir.join("game.exe"), b"test").expect("game exe");
        fs::write(proton_dir.join("proton"), b"#!/bin/sh\n").expect("proton");
        fs::write(
            steamapps.join(format!("appmanifest_{APP_ID}.acf")),
            format!(
                "\"AppState\"\n{{\n  \"appid\" \"{APP_ID}\"\n  \"installdir\" \"Test Game\"\n}}\n"
            ),
        )
        .expect("manifest");
        fs::write(
            steamapps.join("libraryfolders.vdf"),
            format!(
                "\"libraryfolders\"\n{{\n  \"0\"\n  {{\n    \"path\" \"{}\"\n  }}\n}}\n",
                real_root.display()
            ),
        )
        .expect("libraryfolders.vdf");
        fs::write(
            real_root.join("config/config.vdf"),
            format!(
                "\"root\"\n{{\n  \"CompatToolMapping\"\n  {{\n    \"{APP_ID}\"\n    {{\n      \"name\" \"{PROTON_NAME}\"\n    }}\n  }}\n}}\n"
            ),
        )
        .expect("config.vdf");

        fs::create_dir_all(home.join(".steam")).expect(".steam dir");
        let linked_root = home.join(".steam/root");
        symlink(&real_root, &linked_root).expect("~/.steam/root symlink");

        Self {
            _home: home_dir,
            home,
            real_root,
            linked_root,
        }
    }

    fn canonical_root(&self) -> PathBuf {
        fs::canonicalize(&self.real_root).expect("canonical root")
    }

    fn discover_roots(&self) -> Vec<PathBuf> {
        let mut diagnostics = Vec::new();
        discover_steam_root_candidates_with_home("", Some(self.home.clone()), &mut diagnostics)
    }
}

fn game_exe_under(root: &Path) -> PathBuf {
    root.join("steamapps/common/Test Game/game.exe")
}

#[test]
fn symlinked_default_root_yields_a_single_canonical_root() {
    let steam = SymlinkedSteamHome::new();

    assert_eq!(steam.discover_roots(), vec![steam.canonical_root()]);
}

#[test]
fn symlinked_default_root_yields_a_single_library() {
    let steam = SymlinkedSteamHome::new();
    let mut diagnostics = Vec::new();

    let libraries = discover_steam_libraries(&steam.discover_roots(), &mut diagnostics);

    assert_eq!(libraries.len(), 1, "libraries: {libraries:?}");
    assert_eq!(libraries[0].path, steam.canonical_root());
}

#[test]
fn raw_symlink_and_real_roots_dedupe_to_one_library() {
    let steam = SymlinkedSteamHome::new();
    let mut diagnostics = Vec::new();

    let libraries = discover_steam_libraries(
        &[steam.linked_root.clone(), steam.real_root.clone()],
        &mut diagnostics,
    );

    assert_eq!(libraries.len(), 1, "libraries: {libraries:?}");
}

#[test]
fn app_id_is_found_for_executables_under_either_root_path() {
    let steam = SymlinkedSteamHome::new();
    let mut diagnostics = Vec::new();
    let libraries = discover_steam_libraries(&steam.discover_roots(), &mut diagnostics);

    for game_exe in [
        game_exe_under(&steam.real_root),
        game_exe_under(&steam.linked_root),
    ] {
        let selection = find_game_match(&game_exe, &libraries, &mut diagnostics);
        assert_eq!(
            selection.state,
            SteamAutoPopulateFieldState::Found,
            "{}: {diagnostics:#?}",
            game_exe.display()
        );
        assert_eq!(selection.matched.expect("match").app_id, APP_ID);
    }
}

#[test]
fn proton_install_is_listed_once() {
    let steam = SymlinkedSteamHome::new();
    let mut diagnostics = Vec::new();

    let from_discovered_roots =
        discover_compat_tools_with_roots(steam.discover_roots(), Vec::new(), &mut diagnostics);
    let from_raw_roots = discover_compat_tools_with_roots(
        vec![steam.linked_root.clone(), steam.real_root.clone()],
        Vec::new(),
        &mut diagnostics,
    );

    assert_eq!(from_discovered_roots.len(), 1, "{from_discovered_roots:?}");
    assert_eq!(from_raw_roots.len(), 1, "{from_raw_roots:?}");
}

#[test]
fn proton_resolution_is_unambiguous() {
    let steam = SymlinkedSteamHome::new();
    let mut diagnostics = Vec::new();

    let resolution = resolve_proton_path(APP_ID, &steam.discover_roots(), &mut diagnostics);

    assert_eq!(
        resolution.state,
        SteamAutoPopulateFieldState::Found,
        "{diagnostics:#?}"
    );
    assert_eq!(
        resolution.proton_path,
        steam
            .canonical_root()
            .join("steamapps/common")
            .join(PROTON_NAME)
            .join("proton")
    );
}
