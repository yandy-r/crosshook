use std::fs;

use crate::launch::runtime_helpers::path_resolution::{
    attach_log_stdio, resolve_steam_client_install_path_with_home,
};

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

#[test]
fn attach_log_stdio_appends_to_existing_log() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let log_path = temp_dir.path().join("game.log");
    fs::write(&log_path, "").expect("pre-create log");

    let mut command = tokio::process::Command::new("true");
    attach_log_stdio(&mut command, &log_path).expect("attach existing log");
}

#[test]
fn attach_log_stdio_fails_when_log_is_missing_and_does_not_create_it() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let log_path = temp_dir.path().join("missing.log");
    let nested_log_path = temp_dir.path().join("missing-dir").join("game.log");

    let mut command = tokio::process::Command::new("true");
    assert!(attach_log_stdio(&mut command, &log_path).is_err());
    assert!(!log_path.exists());

    assert!(attach_log_stdio(&mut command, &nested_log_path).is_err());
    assert!(!nested_log_path.parent().expect("parent").exists());
}

#[test]
fn attach_log_stdio_refuses_symlink_and_leaves_target_untouched() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let target_path = temp_dir.path().join("victim.txt");
    fs::write(&target_path, "original").expect("write target");
    let log_path = temp_dir.path().join("game.log");
    std::os::unix::fs::symlink(&target_path, &log_path).expect("plant symlink");

    let mut command = tokio::process::Command::new("true");
    assert!(attach_log_stdio(&mut command, &log_path).is_err());
    assert_eq!(fs::read_to_string(&target_path).unwrap(), "original");
}
