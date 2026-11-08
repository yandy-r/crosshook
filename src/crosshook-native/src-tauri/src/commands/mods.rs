//! Thin IPC adapters over the profile mods service.
//!
//! All registry, detection and coexistence logic lives in
//! `crosshook_app::services::mods`.
//!
//! Error-string contract (frontend keys off prefixes): `metadata_unavailable:`
//! when the DB is down/unusable, `no_game_path:` for the detection scan gate.
//! Everything else is a plain user-facing message.

use crosshook_app::services::ProfileModsResponse;
use crosshook_core::launch::LaunchValidationIssue;
use crosshook_core::mods::{DetectionScanReport, ProfileModInput, ProfileModRecord};

use super::{AppState, IpcResult};
use crate::error_map::ipc;

#[tauri::command]
pub async fn list_profile_mods(
    profile_name: String,
    app: AppState<'_>,
) -> IpcResult<ProfileModsResponse> {
    ipc(app.services().mods.list(profile_name).await)
}

#[tauri::command]
pub async fn add_profile_mod(
    profile_name: String,
    input: ProfileModInput,
    app: AppState<'_>,
) -> IpcResult<ProfileModRecord> {
    ipc(app.services().mods.add(profile_name, input).await)
}

#[tauri::command]
pub async fn update_profile_mod(
    profile_name: String,
    mod_id: String,
    input: ProfileModInput,
    app: AppState<'_>,
) -> IpcResult<ProfileModRecord> {
    ipc(app
        .services()
        .mods
        .update(profile_name, mod_id, input)
        .await)
}

#[tauri::command]
pub async fn remove_profile_mod(
    profile_name: String,
    mod_id: String,
    app: AppState<'_>,
) -> IpcResult<()> {
    ipc(app.services().mods.remove(profile_name, mod_id).await)
}

#[tauri::command]
pub async fn detect_profile_mods(
    profile_name: String,
    app: AppState<'_>,
) -> IpcResult<DetectionScanReport> {
    ipc(app.services().mods.detect(profile_name).await)
}

#[tauri::command]
pub async fn analyze_mod_coexistence(
    profile_name: String,
    app: AppState<'_>,
) -> IpcResult<Vec<LaunchValidationIssue>> {
    ipc(app.services().mods.analyze(profile_name).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = list_profile_mods;
        let _ = add_profile_mod;
        let _ = update_profile_mod;
        let _ = remove_profile_mod;
        let _ = detect_profile_mods;
        let _ = analyze_mod_coexistence;
    }
}
