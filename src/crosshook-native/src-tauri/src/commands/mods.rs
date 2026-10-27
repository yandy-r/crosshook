//! Thin IPC adapters over the profile mods service (Forgejo #28).
//!
//! All registry, detection and coexistence logic lives in
//! `crosshook_app::services::mods`; the adapter only extracts the managed
//! service, awaits the call and maps the sanitized [`AppError`] message to the
//! legacy IPC string.
//!
//! Error-string contract (frontend keys off prefixes): `metadata_unavailable:`
//! when the DB is down/unusable, `no_game_path:` for the detection scan gate.
//! Everything else is a plain user-facing message.

use crosshook_app::services::mods::{ModsService, ProfileModsResponse};
use crosshook_core::launch::LaunchValidationIssue;
use crosshook_core::mods::{DetectionScanReport, ProfileModInput, ProfileModRecord};
use tauri::State;

fn map_app_error(error: crosshook_app::AppError) -> String {
    error.message().to_string()
}

#[tauri::command]
pub async fn list_profile_mods(
    profile_name: String,
    service: State<'_, ModsService>,
) -> Result<ProfileModsResponse, String> {
    service.list(profile_name).await.map_err(map_app_error)
}

#[tauri::command]
pub async fn add_profile_mod(
    profile_name: String,
    input: ProfileModInput,
    service: State<'_, ModsService>,
) -> Result<ProfileModRecord, String> {
    service
        .add(profile_name, input)
        .await
        .map_err(map_app_error)
}

#[tauri::command]
pub async fn update_profile_mod(
    profile_name: String,
    mod_id: String,
    input: ProfileModInput,
    service: State<'_, ModsService>,
) -> Result<ProfileModRecord, String> {
    service
        .update(profile_name, mod_id, input)
        .await
        .map_err(map_app_error)
}

#[tauri::command]
pub async fn remove_profile_mod(
    profile_name: String,
    mod_id: String,
    service: State<'_, ModsService>,
) -> Result<(), String> {
    service
        .remove(profile_name, mod_id)
        .await
        .map_err(map_app_error)
}

#[tauri::command]
pub async fn detect_profile_mods(
    profile_name: String,
    service: State<'_, ModsService>,
) -> Result<DetectionScanReport, String> {
    service.detect(profile_name).await.map_err(map_app_error)
}

#[tauri::command]
pub async fn analyze_mod_coexistence(
    profile_name: String,
    service: State<'_, ModsService>,
) -> Result<Vec<LaunchValidationIssue>, String> {
    service.analyze(profile_name).await.map_err(map_app_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        // Async commands cannot be cast to fn pointers; naming them pins the
        // exported symbol (and thus the IPC command name).
        let _ = list_profile_mods;
        let _ = add_profile_mod;
        let _ = update_profile_mod;
        let _ = remove_profile_mod;
        let _ = detect_profile_mods;
        let _ = analyze_mod_coexistence;
    }
}
