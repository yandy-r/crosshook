//! Best-effort exported-launcher cleanup for profile delete and rename
//! (moved from the Tauri shell). Deletion is confined to core's launcher
//! store, which only removes CrossHook-watermarked files.

use crosshook_core::export::{
    delete_launcher_for_profile, LauncherDeleteResult, LauncherStoreError,
};
use crosshook_core::profile::GameProfile;

const STEAM_COMPATDATA_MARKER: &str = "/steamapps/compatdata/";
const STEAM_ROOT_SUFFIXES: [&str; 2] = ["/.local/share/Steam", "/.steam/root"];

/// Steam client root derived from `steam.compatdata_path`; empty when unknown.
pub(super) fn derive_steam_client_install_path(profile: &GameProfile) -> String {
    let compatdata_path = profile.steam.compatdata_path.trim().replace('\\', "/");
    compatdata_path
        .split_once(STEAM_COMPATDATA_MARKER)
        .map(|(steam_root, _)| steam_root.to_string())
        .unwrap_or_default()
}

/// Home directory owning `steam_client_install_path`, falling back to `$HOME`.
pub(super) fn derive_target_home_path(steam_client_install_path: &str) -> String {
    let normalized = steam_client_install_path.trim().replace('\\', "/");

    for suffix in STEAM_ROOT_SUFFIXES {
        if let Some(home_path) = normalized.strip_suffix(suffix) {
            return home_path.to_string();
        }
    }

    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() => home,
        _ => {
            tracing::warn!(
                "HOME is unset or empty and Steam client path did not match known patterns; derived home for launcher cleanup will be empty"
            );
            String::new()
        }
    }
}

/// Deletes the launcher files exported for `profile`. Native profiles have no
/// exported launchers and return `Ok(None)`.
pub(super) fn cleanup_launchers_for_profile_delete(
    profile_name: &str,
    profile: &GameProfile,
) -> Result<Option<LauncherDeleteResult>, LauncherStoreError> {
    if profile.launch.method == "native" {
        tracing::debug!(
            profile_name,
            "skipping launcher cleanup for native profile delete"
        );
        return Ok(None);
    }

    let steam_client_install_path = derive_steam_client_install_path(profile);
    let target_home_path = derive_target_home_path(&steam_client_install_path);

    delete_launcher_for_profile(profile, &target_home_path, &steam_client_install_path).map(Some)
}
