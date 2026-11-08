use crosshook_app::ipc::settings::{self, AppSettingsIpcData};
use crosshook_app::services::SettingsSaveRequest;
use crosshook_core::settings::{LibraryViewMode, RecentFilesData};

use super::{AppState, IpcResult};
use crate::error_map::ipc;

#[tauri::command]
pub async fn settings_load(app: AppState<'_>) -> IpcResult<AppSettingsIpcData> {
    let services = app.services();
    ipc(settings::load(&services.settings, &services.ui_settings).await)
}

/// Saves the settings form. The SteamGridDB key is never touched; use
/// `settings_save_steamgriddb_key`.
#[tauri::command]
pub async fn settings_save(data: SettingsSaveRequest, app: AppState<'_>) -> IpcResult<()> {
    ipc(app.services().settings.save_request(data).await.map(drop))
}

/// Write-only command for updating the SteamGridDB API key.
///
/// Pass `Some(key)` to set the key, or `None` to clear it.
#[tauri::command]
pub async fn settings_save_steamgriddb_key(
    key: Option<String>,
    app: AppState<'_>,
) -> IpcResult<()> {
    ipc(app.services().settings.save_steamgriddb_key(key).await)
}

/// Persists the Library view mode through the throttled `[ui]` writer.
#[tauri::command]
pub async fn settings_set_library_view_mode(
    mode: LibraryViewMode,
    app: AppState<'_>,
) -> IpcResult<()> {
    app.services().ui_settings.set_library_view_mode(mode);
    Ok(())
}

#[tauri::command]
pub async fn recent_files_load(app: AppState<'_>) -> IpcResult<RecentFilesData> {
    ipc(app.services().recent_files.load().await)
}

#[tauri::command]
pub async fn recent_files_save(data: RecentFilesData, app: AppState<'_>) -> IpcResult<()> {
    ipc(app.services().recent_files.save(data).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = settings_load;
        let _ = settings_save;
        let _ = settings_save_steamgriddb_key;
        let _ = settings_set_library_view_mode;
        let _ = recent_files_load;
        let _ = recent_files_save;
    }
}
