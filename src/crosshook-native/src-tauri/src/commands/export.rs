use crosshook_core::export::{
    LauncherDeleteResult, LauncherInfo, SteamExternalLauncherExportRequest,
    SteamExternalLauncherExportResult,
};

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
