//! Read-only directory detection of installed mods.

use std::path::{Path, PathBuf};

use crosshook_core::mods::{mark_already_registered, scan_game_directory, DetectionScanReport};

use super::ModsService;
use crate::services::run_blocking;
use crate::{AppError, AppErrorKind};

/// Message the Tauri adapter keeps for the frontend's `no_game_path:` prefix.
const NO_GAME_PATH: &str = "no_game_path: set the game executable path to enable detection";

fn no_game_path() -> AppError {
    AppError::new(
        AppErrorKind::Validation { field: "game_path" },
        NO_GAME_PATH,
    )
}

/// Directory containing the game executable, if the path has one.
fn scan_root(executable_path: &str) -> Result<PathBuf, AppError> {
    let trimmed = executable_path.trim();
    if trimmed.is_empty() {
        return Err(no_game_path());
    }
    match Path::new(trimmed).parent() {
        Some(parent) if !parent.as_os_str().is_empty() => Ok(parent.to_path_buf()),
        _ => Err(no_game_path()),
    }
}

impl ModsService {
    /// Scans the game directory of `profile_name` for known mods. Works with
    /// the metadata database down: `already_registered` enrichment is best
    /// effort and every failure leaves the flags `false`.
    pub async fn detect(&self, profile_name: String) -> Result<DetectionScanReport, AppError> {
        let profiles = self.profiles.clone();
        let metadata = self.metadata.clone();
        run_blocking(move || {
            let profile = profiles.load(&profile_name)?;
            let root = scan_root(&profile.effective_profile().game.executable_path)?;
            let mut report = scan_game_directory(&root)
                .map_err(|error| AppError::new(AppErrorKind::Io, error.to_string()))?;
            // Best-effort enrichment; with the DB down every flag stays false.
            if metadata.is_available() {
                let existing = metadata
                    .lookup_profile_id(&profile_name)
                    .ok()
                    .flatten()
                    .and_then(|profile_id| metadata.list_profile_mods(&profile_id).ok())
                    .unwrap_or_default();
                mark_already_registered(&mut report, &existing, &root);
            }
            Ok(report)
        })
        .await
    }
}
