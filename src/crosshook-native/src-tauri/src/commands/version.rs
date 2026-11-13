//! Thin IPC adapters for Steam build/trainer version correlation.

use crosshook_app::services::VersionCheckResult;

use super::{AppState, IpcResult};
use crate::error_map::ipc;

/// Check the current version correlation status for a named profile.
///
/// Reads the Steam appmanifest for the profile's App ID to get the live build ID
/// and StateFlags, then compares against the latest recorded snapshot. Returns
/// `UpdateInProgress` when Steam is actively updating the game (StateFlags != 4).
#[tauri::command]
pub async fn check_version_status(
    name: String,
    app: AppState<'_>,
) -> IpcResult<VersionCheckResult> {
    ipc(app.services().version.check_status(name).await)
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
    app: AppState<'_>,
) -> IpcResult<()> {
    ipc(app
        .services()
        .version
        .set_trainer_version(name, version)
        .await)
}

/// Mark the latest version change for a named profile as acknowledged.
///
/// Sets the latest snapshot's status to `"matched"`, indicating the user has
/// verified the trainer still works with the updated game version.
#[tauri::command]
pub async fn acknowledge_version_change(name: String, app: AppState<'_>) -> IpcResult<()> {
    ipc(app.services().version.acknowledge_change(name).await)
}
