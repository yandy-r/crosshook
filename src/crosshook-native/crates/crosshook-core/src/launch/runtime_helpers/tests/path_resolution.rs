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

/// Test that path normalization properly handles Flatpak host paths.
/// In a real Flatpak environment, `/run/host/home/user/.local/share/Steam` would map to
/// the host filesystem path `/home/user/.local/share/Steam`. The normalization function
/// strips the `/run/host` prefix, and the validation should work with the normalized path.
///
/// This test creates a Steam client root at a local temp path, then passes it as if it
/// were a Flatpak host path (with `/run/host` prefix). The validation should normalize
/// the path and validate it correctly.
///
/// Note: This test validates the normalization logic; full Flatpak host filesystem
/// probing requires running in an actual Flatpak sandbox with host filesystem access.
#[test]
fn resolve_steam_client_install_path_normalizes_flatpak_host_paths() {
    let temp_home = tempfile::tempdir().expect("temp home");
    let steam_root = temp_home.path().join(".local/share/Steam");
    write_steam_client_root(&steam_root);

    // Simulate a Flatpak host path by prepending `/run/host`
    let flatpak_style_path = format!("/run/host{}", steam_root.to_string_lossy());

    let resolved = resolve_steam_client_install_path_with_home(
        &flatpak_style_path,
        None,
        Some(temp_home.path().to_path_buf()),
    );

    // The resolved path should be the normalized path (without `/run/host` prefix)
    assert_eq!(resolved, Some(steam_root.to_string_lossy().into_owned()));
}
