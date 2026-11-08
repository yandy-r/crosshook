use std::collections::BTreeMap;

use crosshook_app::services::LauncherRenameRequest;
use crosshook_core::export::{
    LauncherDeleteResult, LauncherInfo, LauncherRenameResult, SteamExternalLauncherExportRequest,
    SteamExternalLauncherExportResult,
};
use crosshook_core::profile::GamescopeConfig;

use super::{AppState, IpcResult};
use crate::error_map::ipc;

/// Validates whether a launcher export request has the required trainer/runtime inputs.
#[tauri::command]
pub async fn validate_launcher_export(
    request: SteamExternalLauncherExportRequest,
    app: AppState<'_>,
) -> IpcResult<()> {
    ipc(app.services().launcher_export.validate(request).await)
}

/// Exports the launcher shell script and desktop entry for the provided request.
#[tauri::command]
pub async fn export_launchers(
    request: SteamExternalLauncherExportRequest,
    app: AppState<'_>,
) -> IpcResult<SteamExternalLauncherExportResult> {
    ipc(app.services().launcher_export.export(request).await)
}

#[tauri::command]
pub async fn check_launcher_exists(
    request: SteamExternalLauncherExportRequest,
    app: AppState<'_>,
) -> IpcResult<LauncherInfo> {
    ipc(app.services().launcher_export.check_exists(request).await)
}

/// Loads a saved profile and checks whether its exported launcher files exist on disk.
#[tauri::command]
pub async fn check_launcher_for_profile(
    name: String,
    app: AppState<'_>,
) -> IpcResult<LauncherInfo> {
    ipc(app.services().launcher_export.check_for_profile(name).await)
}

#[tauri::command]
pub async fn delete_launcher(
    display_name: String,
    steam_app_id: String,
    trainer_path: String,
    target_home_path: String,
    steam_client_install_path: String,
    app: AppState<'_>,
) -> IpcResult<LauncherDeleteResult> {
    ipc(app
        .services()
        .launcher_export
        .delete(
            display_name,
            steam_app_id,
            trainer_path,
            target_home_path,
            steam_client_install_path,
        )
        .await)
}

#[tauri::command]
pub async fn delete_launcher_by_slug(
    launcher_slug: String,
    target_home_path: String,
    steam_client_install_path: String,
    app: AppState<'_>,
) -> IpcResult<LauncherDeleteResult> {
    ipc(app
        .services()
        .launcher_export
        .delete_by_slug(launcher_slug, target_home_path, steam_client_install_path)
        .await)
}

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
    app: AppState<'_>,
) -> IpcResult<LauncherRenameResult> {
    ipc(app
        .services()
        .launcher_export
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
        .await)
}

/// Lists launcher files found under the resolved launcher directory.
#[tauri::command]
pub async fn list_launchers(
    target_home_path: String,
    steam_client_install_path: String,
    app: AppState<'_>,
) -> IpcResult<Vec<LauncherInfo>> {
    Ok(app
        .services()
        .launcher_export
        .list(target_home_path, steam_client_install_path)
        .await)
}

/// Lists launcher files whose slugs do not match the supplied known profile slugs.
#[tauri::command]
pub async fn find_orphaned_launchers(
    known_profile_slugs: Vec<String>,
    target_home_path: String,
    steam_client_install_path: String,
    app: AppState<'_>,
) -> IpcResult<Vec<LauncherInfo>> {
    Ok(app
        .services()
        .launcher_export
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
    app: AppState<'_>,
) -> IpcResult<SteamExternalLauncherExportResult> {
    ipc(app
        .services()
        .launcher_export
        .reexport_by_slug(launcher_slug, target_home_path, steam_client_install_path)
        .await)
}

/// Generates the trainer launcher script content for clipboard copy.
/// Does NOT write to disk.
#[tauri::command]
pub async fn preview_launcher_script(
    request: SteamExternalLauncherExportRequest,
    app: AppState<'_>,
) -> IpcResult<String> {
    ipc(app.services().launcher_export.preview_script(request).await)
}

/// Generates the desktop entry content for clipboard copy.
/// Does NOT write to disk.
#[tauri::command]
pub async fn preview_launcher_desktop(
    request: SteamExternalLauncherExportRequest,
    app: AppState<'_>,
) -> IpcResult<String> {
    ipc(app
        .services()
        .launcher_export
        .preview_desktop(request)
        .await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
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
    }
}
