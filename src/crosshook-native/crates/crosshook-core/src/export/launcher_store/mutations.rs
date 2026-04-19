//! Mutating launcher operations: delete and rename.

use std::path::Path;

use crate::export::launcher::{
    build_desktop_entry_content, build_trainer_script_content, combine_host_unix_path,
    resolve_target_home_path, sanitize_launcher_slug, write_host_text_file,
    SteamExternalLauncherExportRequest,
};
use crate::profile::GameProfile;

use super::fs_ops::{
    is_regular_file_safe, remove_file_if_exists, remove_old_launcher_file, verify_crosshook_file,
};
use super::paths::{derive_launcher_paths, derive_launcher_paths_from_slug};
use super::types::{LauncherDeleteResult, LauncherRenameResult, LauncherStoreError};
use super::{DESKTOP_ENTRY_WATERMARK, SCRIPT_WATERMARK};

/// Verifies that a destination path is safe to write to:
/// - If the path doesn't exist, it's safe
/// - If it exists and is a regular file with the CrossHook watermark, it's safe
/// - Otherwise (symlink, directory, or non-CrossHook file), it's unsafe
fn verify_destination_safe(path: &str, watermark: &str) -> Result<(), String> {
    // If destination doesn't exist, it's safe to write
    if !is_regular_file_safe(path) && !Path::new(path).exists() {
        return Ok(());
    }

    // If destination exists, verify it's a CrossHook-owned regular file
    match verify_crosshook_file(path, watermark) {
        Ok(None) => Ok(()), // Safe to overwrite
        Ok(Some(reason)) => Err(reason),
        Err(e) => Err(format!("Failed to verify destination: {e}")),
    }
}

pub(super) fn delete_launcher_at_paths(
    script_path: String,
    desktop_entry_path: String,
) -> Result<LauncherDeleteResult, LauncherStoreError> {
    let mut result = LauncherDeleteResult {
        script_path: script_path.clone(),
        desktop_entry_path: desktop_entry_path.clone(),
        ..Default::default()
    };

    // Delete desktop entry first (user-visible artifact), with watermark verification
    match verify_crosshook_file(&desktop_entry_path, DESKTOP_ENTRY_WATERMARK)? {
        Some(reason) => {
            result.desktop_entry_skipped_reason = Some(reason);
        }
        None => {
            result.desktop_entry_deleted = remove_file_if_exists(&desktop_entry_path)?;
        }
    }

    // Delete script, with watermark verification
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

/// Renames launcher files from one slug to another, rewriting file content with updated
/// display names and paths. Uses a write-then-delete strategy because both `.sh` and
/// `.desktop` files embed display names and paths as plaintext.
///
/// When the slug is unchanged, files are rewritten in place and the old paths are not deleted.
pub fn rename_launcher_files(
    old_launcher_slug: &str,
    new_display_name: &str,
    new_launcher_icon_path: &str,
    target_home_path: &str,
    steam_client_install_path: &str,
    request: &SteamExternalLauncherExportRequest,
) -> Result<LauncherRenameResult, LauncherStoreError> {
    let new_slug = sanitize_launcher_slug(new_display_name);
    let home = resolve_target_home_path(target_home_path, steam_client_install_path);

    // Construct old file paths from old_launcher_slug
    let old_script_path = combine_host_unix_path(
        &home,
        ".local/share/crosshook/launchers",
        &format!("{old_launcher_slug}-trainer.sh"),
    );
    let old_desktop_entry_path = combine_host_unix_path(
        &home,
        ".local/share/applications",
        &format!("crosshook-{old_launcher_slug}-trainer.desktop"),
    );

    // Construct new file paths from new_slug
    let new_script_path = combine_host_unix_path(
        &home,
        ".local/share/crosshook/launchers",
        &format!("{new_slug}-trainer.sh"),
    );
    let new_desktop_entry_path = combine_host_unix_path(
        &home,
        ".local/share/applications",
        &format!("crosshook-{new_slug}-trainer.desktop"),
    );

    // Check if old files exist. If neither exists, return early with renamed: false
    let old_script_exists = is_regular_file_safe(&old_script_path);
    let old_desktop_exists = is_regular_file_safe(&old_desktop_entry_path);

    if !old_script_exists && !old_desktop_exists {
        return Ok(LauncherRenameResult {
            old_slug: old_launcher_slug.to_string(),
            new_slug,
            new_script_path,
            new_desktop_entry_path,
            script_renamed: false,
            desktop_entry_renamed: false,
            old_script_cleanup_warning: None,
            old_desktop_entry_cleanup_warning: None,
        });
    }

    // Before writing, verify that destination paths are safe:
    // - If destination exists, it must be a regular file with the CrossHook watermark
    // - Symlinks and non-CrossHook files should be rejected
    // - Skip verification if old and new paths are the same (slug unchanged)
    let mut old_script_cleanup_warning = None;
    let mut old_desktop_entry_cleanup_warning = None;
    let slug_changed = old_launcher_slug != new_slug;

    if slug_changed && old_script_exists {
        if let Err(reason) = verify_destination_safe(&new_script_path, SCRIPT_WATERMARK) {
            old_script_cleanup_warning =
                Some(format!("Refusing to overwrite new script path: {reason}"));
        }
    }

    if slug_changed && old_desktop_exists {
        if let Err(reason) =
            verify_destination_safe(&new_desktop_entry_path, DESKTOP_ENTRY_WATERMARK)
        {
            old_desktop_entry_cleanup_warning = Some(format!(
                "Refusing to overwrite new desktop entry path: {reason}"
            ));
        }
    }

    // If either destination is unsafe, return early without writing
    if old_script_cleanup_warning.is_some() || old_desktop_entry_cleanup_warning.is_some() {
        return Ok(LauncherRenameResult {
            old_slug: old_launcher_slug.to_string(),
            new_slug,
            new_script_path,
            new_desktop_entry_path,
            script_renamed: false,
            desktop_entry_renamed: false,
            old_script_cleanup_warning,
            old_desktop_entry_cleanup_warning,
        });
    }

    // Generate new file content
    let new_script_content = build_trainer_script_content(request, new_display_name);
    let new_desktop_content = build_desktop_entry_content(
        new_display_name,
        &new_slug,
        &new_script_path,
        new_launcher_icon_path,
    );

    // Write new files
    let mut script_renamed = false;
    if old_script_exists {
        write_host_text_file(&new_script_path, &new_script_content, 0o755)?;
        script_renamed = true;
    }

    let mut desktop_entry_renamed = false;
    if old_desktop_exists {
        write_host_text_file(&new_desktop_entry_path, &new_desktop_content, 0o644)?;
        desktop_entry_renamed = true;
    }

    // Reset cleanup warnings before attempting to delete old files
    old_script_cleanup_warning = None;
    old_desktop_entry_cleanup_warning = None;

    // If old paths differ from new paths (slug changed), delete old files
    if slug_changed {
        if old_script_exists {
            old_script_cleanup_warning =
                remove_old_launcher_file(&old_script_path, SCRIPT_WATERMARK, "script");
        }
        if old_desktop_exists {
            old_desktop_entry_cleanup_warning = remove_old_launcher_file(
                &old_desktop_entry_path,
                DESKTOP_ENTRY_WATERMARK,
                "desktop entry",
            );
        }
    }

    Ok(LauncherRenameResult {
        old_slug: old_launcher_slug.to_string(),
        new_slug,
        new_script_path,
        new_desktop_entry_path,
        script_renamed,
        desktop_entry_renamed,
        old_script_cleanup_warning,
        old_desktop_entry_cleanup_warning,
    })
}
