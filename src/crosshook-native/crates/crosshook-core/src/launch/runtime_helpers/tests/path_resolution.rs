use std::fs;

use crate::launch::runtime_helpers::path_resolution::resolve_steam_client_install_path_with_home;

use super::support::write_steam_client_root;

#[test]
fn resolve_steam_client_install_path_accepts_valid_configured_root() {
    let temp_home = tempfile::tempdir().expect("temp home");
    let configured_root = temp_home.path().join("steam-root");
    write_steam_client_root(&configured_root);

    let resolved = resolve_steam_client_install_path_with_home(
        configured_root.to_string_lossy().as_ref(),
        None,
        Some(temp_home.path().to_path_buf()),
    );

    assert_eq!(
        resolved,
        Some(configured_root.to_string_lossy().into_owned())
    );
}

#[test]
fn resolve_steam_client_install_path_rejects_library_root_and_falls_back_to_default() {
    let temp_home = tempfile::tempdir().expect("temp home");
    let library_root = temp_home.path().join("SteamLibrary");
    let default_root = temp_home.path().join(".local/share/Steam");
    fs::create_dir_all(library_root.join("steamapps")).expect("library steamapps");
    write_steam_client_root(&default_root);

    let resolved = resolve_steam_client_install_path_with_home(
        library_root.to_string_lossy().as_ref(),
        None,
        Some(temp_home.path().to_path_buf()),
    );

    assert_eq!(resolved, Some(default_root.to_string_lossy().into_owned()));
}
