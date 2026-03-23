use crosshook_core::export::{
    export_launchers as export_launchers_core, validate as validate_launcher_export_core,
    SteamExternalLauncherExportRequest, SteamExternalLauncherExportResult,
};

#[tauri::command]
pub fn validate_launcher_export(request: SteamExternalLauncherExportRequest) -> Result<(), String> {
    validate_launcher_export_core(&request).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn export_launchers(
    request: SteamExternalLauncherExportRequest,
) -> Result<SteamExternalLauncherExportResult, String> {
    export_launchers_core(&request).map_err(|error| error.to_string())
}
