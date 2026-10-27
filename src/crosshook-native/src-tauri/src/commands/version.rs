//! Thin IPC adapters for Steam build/trainer version correlation (Forgejo #34).
//! All logic lives in [`crosshook_app::services::version`].
//!
//! Error strings reach the frontend unchanged via [`map_error`] (the service
//! preserves the legacy texts, e.g. `profile '{name}' is not registered in the
//! metadata store`).

use crosshook_app::services::{VersionCheckResult, VersionService, VersionSnapshotInfo};
use crosshook_app::AppError;
use tauri::State;

fn map_error(error: AppError) -> String {
    error.message().to_string()
}

/// Check the current version correlation status for a named profile.
///
/// Reads the Steam appmanifest for the profile's App ID to get the live build ID
/// and StateFlags, then compares against the latest recorded snapshot. Returns
/// `UpdateInProgress` when Steam is actively updating the game (StateFlags != 4).
#[tauri::command]
pub async fn check_version_status(
    name: String,
    service: State<'_, VersionService>,
) -> Result<VersionCheckResult, String> {
    service.check_status(name).await.map_err(map_error)
}

/// Return the latest recorded version snapshot for a named profile, if any.
#[tauri::command]
pub async fn get_version_snapshot(
    name: String,
    service: State<'_, VersionService>,
) -> Result<Option<VersionSnapshotInfo>, String> {
    service.snapshot(name).await.map_err(map_error)
}

/// Record a manual trainer version hint for a named profile.
///
/// Upserts a new snapshot row with `status = "untracked"` and the provided
/// version string. The `steam_app_id` is preserved from the latest existing
/// snapshot, or left empty if no snapshot has been recorded yet.
#[tauri::command]
pub async fn set_trainer_version(
    name: String,
    version: String,
    service: State<'_, VersionService>,
) -> Result<(), String> {
    service
        .set_trainer_version(name, version)
        .await
        .map_err(map_error)
}

/// Mark the latest version change for a named profile as acknowledged.
///
/// Sets the latest snapshot's status to `"matched"`, indicating the user has
/// verified the trainer still works with the updated game version.
#[tauri::command]
pub async fn acknowledge_version_change(
    name: String,
    service: State<'_, VersionService>,
) -> Result<(), String> {
    service.acknowledge_change(name).await.map_err(map_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        // Async commands cannot be cast to fn pointers; naming them pins the
        // exported symbol (and thus the IPC command name).
        let _ = check_version_status;
        let _ = get_version_snapshot;
        let _ = set_trainer_version;
        let _ = acknowledge_version_change;
    }
}
