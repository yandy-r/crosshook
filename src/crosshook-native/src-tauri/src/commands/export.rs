use crosshook_app::services::{LauncherExportService, LauncherRenameRequest};
use crosshook_app::AppError;
use crosshook_core::export::{
    LauncherDeleteResult, LauncherInfo, LauncherRenameResult, SteamExternalLauncherExportRequest,
    SteamExternalLauncherExportResult,
};
use crosshook_core::profile::GamescopeConfig;
use std::collections::BTreeMap;
use tauri::State;

fn map_app_error(error: AppError) -> String {
    error.message().to_string()
}

/// Validates whether a launcher export request has the required trainer/runtime inputs.
#[tauri::command]
pub async fn validate_launcher_export(
    request: SteamExternalLauncherExportRequest,
    service: State<'_, LauncherExportService>,
) -> Result<(), String> {
    service.validate(request).await.map_err(map_app_error)
}

/// Exports the launcher shell script and desktop entry for the provided request.
#[tauri::command]
pub async fn export_launchers(
    request: SteamExternalLauncherExportRequest,
    service: State<'_, LauncherExportService>,
) -> Result<SteamExternalLauncherExportResult, String> {
    service.export(request).await.map_err(map_app_error)
}

/// Checks whether the launcher files derived from the supplied profile fields exist on disk.
#[tauri::command]
pub async fn check_launcher_exists(
    request: SteamExternalLauncherExportRequest,
    service: State<'_, LauncherExportService>,
) -> Result<LauncherInfo, String> {
    service.check_exists(request).await.map_err(map_app_error)
}

/// Loads a saved profile and checks whether its exported launcher files exist on disk.
#[tauri::command]
pub async fn check_launcher_for_profile(
    name: String,
    service: State<'_, LauncherExportService>,
) -> Result<LauncherInfo, String> {
    service.check_for_profile(name).await.map_err(map_app_error)
}

/// Deletes the launcher files derived from the supplied profile fields.
#[tauri::command]
pub async fn delete_launcher(
    display_name: String,
    steam_app_id: String,
    trainer_path: String,
    target_home_path: String,
    steam_client_install_path: String,
    service: State<'_, LauncherExportService>,
) -> Result<LauncherDeleteResult, String> {
    service
        .delete(
            display_name,
            steam_app_id,
            trainer_path,
            target_home_path,
            steam_client_install_path,
        )
        .await
        .map_err(map_app_error)
}

/// Deletes launcher files directly from a known launcher slug.
#[tauri::command]
pub async fn delete_launcher_by_slug(
    launcher_slug: String,
    target_home_path: String,
    steam_client_install_path: String,
    service: State<'_, LauncherExportService>,
) -> Result<LauncherDeleteResult, String> {
    service
        .delete_by_slug(launcher_slug, target_home_path, steam_client_install_path)
        .await
        .map_err(map_app_error)
}

/// Rewrites launcher files for a renamed launcher and optionally cleans up old paths.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn rename_launcher(
    old_launcher_slug: String,
    new_display_name: String,
    new_launcher_icon_path: String,
    target_home_path: String,
    steam_client_install_path: String,
    method: String,
    trainer_path: String,
    trainer_loading_mode: String,
    prefix_path: String,
    proton_path: String,
    steam_app_id: String,
    launcher_name: String,
    network_isolation: bool,
    gamescope: GamescopeConfig,
    custom_env_vars: BTreeMap<String, String>,
    service: State<'_, LauncherExportService>,
) -> Result<LauncherRenameResult, String> {
    service
        .rename(LauncherRenameRequest {
            old_launcher_slug,
            new_display_name,
            new_launcher_icon_path,
            target_home_path,
            steam_client_install_path,
            method,
            trainer_path,
            trainer_loading_mode,
            prefix_path,
            proton_path,
            steam_app_id,
            launcher_name,
            network_isolation,
            gamescope,
            custom_env_vars,
        })
        .await
        .map_err(map_app_error)
}

/// Lists launcher files found under the resolved launcher directory.
#[tauri::command]
pub async fn list_launchers(
    target_home_path: String,
    steam_client_install_path: String,
    service: State<'_, LauncherExportService>,
) -> Result<Vec<LauncherInfo>, String> {
    Ok(service
        .list(target_home_path, steam_client_install_path)
        .await)
}

/// Lists launcher files whose slugs do not match the supplied known profile slugs.
#[tauri::command]
pub async fn find_orphaned_launchers(
    known_profile_slugs: Vec<String>,
    target_home_path: String,
    steam_client_install_path: String,
    service: State<'_, LauncherExportService>,
) -> Result<Vec<LauncherInfo>, String> {
    Ok(service
        .find_orphaned(
            known_profile_slugs,
            target_home_path,
            steam_client_install_path,
        )
        .await)
}

/// Re-exports a launcher by slug using the profile that currently owns that slug.
#[tauri::command]
pub async fn reexport_launcher_by_slug(
    launcher_slug: String,
    target_home_path: String,
    steam_client_install_path: String,
    service: State<'_, LauncherExportService>,
) -> Result<SteamExternalLauncherExportResult, String> {
    service
        .reexport_by_slug(launcher_slug, target_home_path, steam_client_install_path)
        .await
        .map_err(map_app_error)
}

/// Generates the trainer launcher script content for clipboard copy.
/// Does NOT write to disk.
#[tauri::command]
pub async fn preview_launcher_script(
    request: SteamExternalLauncherExportRequest,
    service: State<'_, LauncherExportService>,
) -> Result<String, String> {
    service.preview_script(request).await.map_err(map_app_error)
}

/// Generates the desktop entry content for clipboard copy.
/// Does NOT write to disk.
#[tauri::command]
pub async fn preview_launcher_desktop(
    request: SteamExternalLauncherExportRequest,
    service: State<'_, LauncherExportService>,
) -> Result<String, String> {
    service
        .preview_desktop(request)
        .await
        .map_err(map_app_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        // Async commands cannot be cast to fn pointers; naming them pins the
        // exported symbol (and thus the IPC command name).
        let _ = validate_launcher_export;
        let _ = export_launchers;
        let _ = check_launcher_exists;
        let _ = check_launcher_for_profile;
        let _ = delete_launcher;
        let _ = delete_launcher_by_slug;
        let _ = rename_launcher;
        let _ = list_launchers;
        let _ = find_orphaned_launchers;
        let _ = reexport_launcher_by_slug;
        let _ = preview_launcher_script;
        let _ = preview_launcher_desktop;
        let _ = crate::commands::lutris::lutris_prepare_import;
        let _ = crate::commands::lutris::lutris_import_profiles;
    }

    #[test]
    fn app_error_maps_to_its_plain_message() {
        let error = AppError::validation("request", "invalid trainer loading mode");
        assert_eq!(map_app_error(error), "invalid trainer loading mode");
    }

    fn sorted_keys<T: serde::Serialize>(value: &T) -> Vec<String> {
        let json = serde_json::to_value(value).unwrap();
        let mut keys: Vec<String> = json.as_object().unwrap().keys().cloned().collect();
        keys.sort_unstable();
        keys
    }

    #[test]
    fn launcher_result_payloads_keep_snake_case_keys() {
        assert_eq!(
            sorted_keys(&LauncherInfo::default()),
            [
                "desktop_entry_exists",
                "desktop_entry_path",
                "display_name",
                "is_stale",
                "launcher_slug",
                "script_exists",
                "script_path",
            ]
        );
        assert_eq!(
            sorted_keys(&LauncherDeleteResult::default()),
            [
                "desktop_entry_deleted",
                "desktop_entry_path",
                "desktop_entry_skipped_reason",
                "script_deleted",
                "script_path",
                "script_skipped_reason",
            ]
        );
        assert_eq!(
            sorted_keys(&LauncherRenameResult::default()),
            [
                "desktop_entry_renamed",
                "new_desktop_entry_path",
                "new_script_path",
                "new_slug",
                "old_desktop_entry_cleanup_warning",
                "old_script_cleanup_warning",
                "old_slug",
                "script_renamed",
            ]
        );
        assert_eq!(
            sorted_keys(&SteamExternalLauncherExportResult::default()),
            [
                "desktop_entry_path",
                "display_name",
                "launcher_slug",
                "script_path",
            ]
        );
    }

    #[test]
    fn export_request_payload_deserializes_from_frontend_shape() {
        let request: SteamExternalLauncherExportRequest =
            serde_json::from_value(serde_json::json!({
                "method": "proton_run",
                "launcher_name": "Game",
                "trainer_path": "/t.exe",
                "launcher_icon_path": "",
                "prefix_path": "/p",
                "proton_path": "/proton",
                "steam_app_id": "1",
                "steam_client_install_path": "",
                "target_home_path": "/home/u",
            }))
            .unwrap();
        assert_eq!(request.method, "proton_run");
        assert_eq!(request.launcher_name, "Game");
        assert!(request.profile_name.is_none());
        assert!(request.custom_env_vars.is_empty());
    }
}
