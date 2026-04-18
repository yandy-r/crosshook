//! Low-level filesystem primitives: watermark verification, file removal, desktop name parsing.

use std::fs;
use std::io;

use crate::export::launcher::strip_trainer_suffix;

/// Verifies that a file is safe to delete:
/// 1. Must be a regular file (not a symlink or directory)
/// 2. Must contain the CrossHook watermark
///
/// Returns `Ok(None)` if safe to delete or the file does not exist.
/// Returns `Ok(Some(reason))` if the file should be skipped.
pub(super) fn verify_crosshook_file(
    path: &str,
    watermark: &str,
) -> Result<Option<String>, io::Error> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };

    if !metadata.file_type().is_file() {
        return Ok(Some(format!("Not a regular file: {path}")));
    }

    let content = fs::read_to_string(path)?;
    if !content.contains(watermark) {
        return Ok(Some(format!("Missing CrossHook watermark in: {path}")));
    }

    Ok(None)
}

/// Attempts to remove a file. Returns `true` if the file was actually removed,
/// `false` if it did not exist. Treats `ErrorKind::NotFound` as success (idempotent).
pub(super) fn remove_file_if_exists(path: &str) -> Result<bool, io::Error> {
    if path.is_empty() {
        return Ok(false);
    }

    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub(super) fn remove_old_launcher_file(
    path: &str,
    watermark: &str,
    artifact_label: &str,
) -> Option<String> {
    match verify_crosshook_file(path, watermark) {
        Ok(Some(reason)) => {
            let warning = format!("Skipped old {artifact_label} cleanup: {reason}");
            tracing::warn!(
                path = %path,
                artifact = artifact_label,
                reason = %reason,
                "skipping old launcher cleanup during rename"
            );
            Some(warning)
        }
        Ok(None) => {
            if let Err(error) = remove_file_if_exists(path) {
                let warning = format!("Failed to remove old {artifact_label} at {path}: {error}");
                tracing::warn!(
                    path = %path,
                    artifact = artifact_label,
                    %error,
                    "failed to remove old launcher during rename"
                );
                Some(warning)
            } else {
                None
            }
        }
        Err(error) => {
            let warning = format!("Failed to verify old {artifact_label} at {path}: {error}");
            tracing::warn!(
                path = %path,
                artifact = artifact_label,
                %error,
                "failed to verify old launcher during rename"
            );
            Some(warning)
        }
    }
}

pub(super) fn extract_display_name_from_desktop(
    desktop_path: &str,
) -> Result<Option<String>, io::Error> {
    let content = fs::read_to_string(desktop_path)?;
    Ok(parse_display_name_from_desktop_content(&content))
}

pub(super) fn parse_display_name_from_desktop_content(content: &str) -> Option<String> {
    for line in content.lines() {
        if let Some(name_value) = line.strip_prefix("Name=") {
            return Some(strip_trainer_suffix(name_value));
        }
    }

    None
}
