//! Read-only launcher queries: existence checks, listing, and orphan detection.

use std::fs;
use std::io;

use crate::export::launcher::{
    build_trainer_script_content, combine_host_unix_path, resolve_target_home_path,
    SteamExternalLauncherExportRequest,
};
use crate::profile::{resolve_launch_method, GameProfile};
use crate::settings::UmuPreference;

use super::fs_ops::{extract_display_name_from_desktop, is_regular_file_safe};
use super::paths::derive_launcher_paths;
use super::types::{LauncherInfo, LauncherStoreError};

pub fn check_launcher_exists(
    display_name: &str,
    steam_app_id: &str,
    trainer_path: &str,
    target_home_path: &str,
    steam_client_install_path: &str,
) -> Result<LauncherInfo, LauncherStoreError> {
    let (resolved_name, slug, script_path, desktop_entry_path) = derive_launcher_paths(
        display_name,
        steam_app_id,
        trainer_path,
        target_home_path,
        steam_client_install_path,
    );

    let script_exists = is_regular_file_safe(&script_path);
    let desktop_entry_exists = is_regular_file_safe(&desktop_entry_path);

    // Check staleness: compare Name= line in .desktop against expected
    let is_stale = if desktop_entry_exists {
        match extract_display_name_from_desktop(&desktop_entry_path)? {
            Some(actual_name) => actual_name != resolved_name,
            None => true,
        }
    } else {
        false
    };

    Ok(LauncherInfo {
        display_name: resolved_name,
        launcher_slug: slug,
        script_path,
        desktop_entry_path,
        script_exists,
        desktop_entry_exists,
        is_stale,
    })
}

pub fn check_launcher_exists_for_request(
    display_name: &str,
    request: &SteamExternalLauncherExportRequest,
) -> Result<LauncherInfo, LauncherStoreError> {
    let (resolved_name, slug, script_path, desktop_entry_path) = derive_launcher_paths(
        display_name,
        &request.steam_app_id,
        &request.trainer_path,
        &request.target_home_path,
        &request.steam_client_install_path,
    );

    let script_exists = is_regular_file_safe(&script_path);
    let desktop_entry_exists = is_regular_file_safe(&desktop_entry_path);

    let script_is_stale = if script_exists {
        let expected_script = build_trainer_script_content(request, &resolved_name);
        fs::read_to_string(&script_path)
            .map(|actual_script| actual_script != expected_script)
            .unwrap_or(true)
    } else {
        false
    };

    let desktop_is_stale = if desktop_entry_exists {
        match extract_display_name_from_desktop(&desktop_entry_path)? {
            Some(actual_name) => actual_name != resolved_name,
            None => true,
        }
    } else {
        false
    };

    Ok(LauncherInfo {
        display_name: resolved_name,
        launcher_slug: slug,
        script_path,
        desktop_entry_path,
        script_exists,
        desktop_entry_exists,
        is_stale: script_is_stale || desktop_is_stale,
    })
}

pub fn check_launcher_for_profile(
    profile: &GameProfile,
    target_home_path: &str,
    steam_client_install_path: &str,
    global_umu_preference: UmuPreference,
) -> Result<LauncherInfo, LauncherStoreError> {
    let resolved_method = resolve_launch_method(profile);

    if resolved_method == "native" {
        return Ok(LauncherInfo::default());
    }

    check_launcher_exists_for_request(
        &profile.steam.launcher.display_name,
        &SteamExternalLauncherExportRequest {
            method: resolved_method.to_string(),
            launcher_name: profile.steam.launcher.display_name.clone(),
            trainer_path: profile.trainer.path.clone(),
            trainer_loading_mode: profile.trainer.loading_mode,
            launcher_icon_path: profile.steam.launcher.icon_path.clone(),
            prefix_path: if resolved_method == "steam_applaunch" {
                profile.steam.compatdata_path.clone()
            } else {
                profile.runtime.prefix_path.clone()
            },
            proton_path: if resolved_method == "steam_applaunch" {
                profile.steam.proton_path.clone()
            } else {
                profile.runtime.proton_path.clone()
            },
            steam_app_id: profile.steam.app_id.clone(),
            steam_client_install_path: steam_client_install_path.to_string(),
            target_home_path: target_home_path.to_string(),
            profile_name: None,
            runtime_steam_app_id: profile.runtime.steam_app_id.clone(),
            umu_game_id: profile.runtime.umu_game_id.clone(),
            umu_preference: profile
                .runtime
                .umu_preference
                .unwrap_or(global_umu_preference),
            network_isolation: profile.launch.network_isolation,
            gamescope: profile.launch.resolved_trainer_gamescope(),
        },
    )
}

/// Lists all launchers found in the launchers directory.
///
/// Scans `{home}/.local/share/crosshook/launchers/` for files ending in `-trainer.sh`,
/// derives the slug from each filename, and checks for a matching `.desktop` entry.
/// Attempts to extract the display name from the `Name=` line in the `.desktop` file.
/// `is_stale` is reported as `false` for these results because the function does not
/// have the profile metadata required to compute freshness against an expected name.
pub fn list_launchers(
    target_home_path: &str,
    steam_client_install_path: &str,
) -> Vec<LauncherInfo> {
    let home = resolve_target_home_path(target_home_path, steam_client_install_path);
    let launchers_dir = combine_host_unix_path(&home, ".local/share/crosshook/launchers", "");

    if launchers_dir.is_empty() {
        return Vec::new();
    }

    let dir_entries = match fs::read_dir(&launchers_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            tracing::warn!(
                path = %launchers_dir,
                %error,
                "failed to read launcher directory"
            );
            return Vec::new();
        }
    };

    let mut launchers: Vec<LauncherInfo> = Vec::new();

    for entry in dir_entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                tracing::warn!(%error, "failed to read launcher directory entry");
                continue;
            }
        };
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        if !file_name_str.ends_with("-trainer.sh") {
            continue;
        }

        // Derive slug by stripping the -trainer.sh suffix
        let slug = file_name_str
            .strip_suffix("-trainer.sh")
            .unwrap_or_default()
            .to_string();

        if slug.is_empty() {
            continue;
        }

        let script_path =
            combine_host_unix_path(&home, ".local/share/crosshook/launchers", &file_name_str);

        // Only process entries that are regular files (skip symlinks and directories)
        if !is_regular_file_safe(&script_path) {
            continue;
        }

        let desktop_entry_path = combine_host_unix_path(
            &home,
            ".local/share/applications",
            &format!("crosshook-{slug}-trainer.desktop"),
        );

        let script_exists = is_regular_file_safe(&script_path);
        let desktop_entry_exists = is_regular_file_safe(&desktop_entry_path);

        // Try to extract display name from the Name= line in the .desktop file
        let display_name = if desktop_entry_exists {
            match extract_display_name_from_desktop(&desktop_entry_path) {
                Ok(Some(display_name)) => display_name,
                Ok(None) => slug.clone(),
                Err(error) => {
                    tracing::warn!(
                        path = %desktop_entry_path,
                        %error,
                        "failed to inspect launcher desktop entry"
                    );
                    slug.clone()
                }
            }
        } else {
            slug.clone()
        };

        launchers.push(LauncherInfo {
            display_name,
            launcher_slug: slug,
            script_path,
            desktop_entry_path,
            script_exists,
            desktop_entry_exists,
            is_stale: false,
        });
    }

    launchers.sort_by(|a, b| a.launcher_slug.cmp(&b.launcher_slug));
    launchers
}

/// Returns launchers that don't match any known profile slug.
pub fn find_orphaned_launchers(
    known_profile_slugs: &[String],
    target_home_path: &str,
    steam_client_install_path: &str,
) -> Vec<LauncherInfo> {
    let all_launchers = list_launchers(target_home_path, steam_client_install_path);
    all_launchers
        .into_iter()
        .filter(|launcher| !known_profile_slugs.contains(&launcher.launcher_slug))
        .collect()
}
