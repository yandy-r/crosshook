//! Mutating launcher operations: delete.

use crate::profile::GameProfile;

use super::fs_ops::{remove_file_if_exists, verify_crosshook_file};
use super::paths::{derive_launcher_paths, derive_launcher_paths_from_slug};
use super::types::{LauncherDeleteResult, LauncherStoreError};
use super::{DESKTOP_ENTRY_WATERMARK, SCRIPT_WATERMARK};

pub(super) fn delete_launcher_at_paths(
    script_path: String,
    desktop_entry_path: String,
) -> Result<LauncherDeleteResult, LauncherStoreError> {
    let mut result = LauncherDeleteResult {
        script_path: script_path.clone(),
        desktop_entry_path: desktop_entry_path.clone(),
        ..Default::default()
    };

    match verify_crosshook_file(&desktop_entry_path, DESKTOP_ENTRY_WATERMARK)? {
        Some(reason) => {
            result.desktop_entry_skipped_reason = Some(reason);
        }
        None => {
            result.desktop_entry_deleted = remove_file_if_exists(&desktop_entry_path)?;
        }
    }

    match verify_crosshook_file(&script_path, SCRIPT_WATERMARK)? {
        Some(reason) => {
            result.script_skipped_reason = Some(reason);
        }
        None => {
            result.script_deleted = remove_file_if_exists(&script_path)?;
        }
    }

    Ok(result)
}

pub fn delete_launcher_files(
    display_name: &str,
    steam_app_id: &str,
    trainer_path: &str,
    target_home_path: &str,
    steam_client_install_path: &str,
) -> Result<LauncherDeleteResult, LauncherStoreError> {
    let (_resolved_name, _slug, script_path, desktop_entry_path) = derive_launcher_paths(
        display_name,
        steam_app_id,
        trainer_path,
        target_home_path,
        steam_client_install_path,
    );

    delete_launcher_at_paths(script_path, desktop_entry_path)
}

pub fn delete_launcher_by_slug(
    launcher_slug: &str,
    target_home_path: &str,
    steam_client_install_path: &str,
) -> Result<LauncherDeleteResult, LauncherStoreError> {
    let (script_path, desktop_entry_path) =
        derive_launcher_paths_from_slug(launcher_slug, target_home_path, steam_client_install_path);
    delete_launcher_at_paths(script_path, desktop_entry_path)
}

pub fn delete_launcher_for_profile(
    profile: &GameProfile,
    target_home_path: &str,
    steam_client_install_path: &str,
) -> Result<LauncherDeleteResult, LauncherStoreError> {
    let display_name = &profile.steam.launcher.display_name;
    let steam_app_id = &profile.steam.app_id;
    let trainer_path = &profile.trainer.path;

    delete_launcher_files(
        display_name,
        steam_app_id,
        trainer_path,
        target_home_path,
        steam_client_install_path,
    )
}
