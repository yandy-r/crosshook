use crosshook_core::export::{
    check_launcher_for_profile as check_launcher_for_profile_core,
    delete_launcher_by_slug as delete_launcher_by_slug_core,
    export_launchers as export_launchers_core, validate as validate_launcher_export_core,
    LauncherDeleteResult, LauncherInfo, LauncherRenameResult, SteamExternalLauncherExportRequest,
    SteamExternalLauncherExportResult,
};
use crosshook_core::profile::ProfileStore;
use tauri::State;

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

#[tauri::command]
pub fn check_launcher_exists(
    display_name: String,
    steam_app_id: String,
    trainer_path: String,
    target_home_path: String,
    steam_client_install_path: String,
) -> LauncherInfo {
    crosshook_core::export::check_launcher_exists(
        &display_name,
        &steam_app_id,
        &trainer_path,
        &target_home_path,
        &steam_client_install_path,
    )
}

#[tauri::command]
pub fn check_launcher_for_profile(
    name: String,
    store: State<'_, ProfileStore>,
) -> Result<LauncherInfo, String> {
    let profile = store.load(&name).map_err(|error| error.to_string())?;
    Ok(check_launcher_for_profile_core(&profile, "", ""))
}

#[tauri::command]
pub fn delete_launcher(
    display_name: String,
    steam_app_id: String,
    trainer_path: String,
    target_home_path: String,
    steam_client_install_path: String,
) -> Result<LauncherDeleteResult, String> {
    crosshook_core::export::delete_launcher_files(
        &display_name,
        &steam_app_id,
        &trainer_path,
        &target_home_path,
        &steam_client_install_path,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_launcher_by_slug(
    launcher_slug: String,
    target_home_path: String,
    steam_client_install_path: String,
) -> Result<LauncherDeleteResult, String> {
    delete_launcher_by_slug_core(
        &launcher_slug,
        &target_home_path,
        &steam_client_install_path,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn rename_launcher(
    old_launcher_slug: String,
    new_display_name: String,
    new_launcher_icon_path: String,
    target_home_path: String,
    steam_client_install_path: String,
    method: String,
    trainer_path: String,
    prefix_path: String,
    proton_path: String,
    steam_app_id: String,
    launcher_name: String,
) -> Result<LauncherRenameResult, String> {
    let request = SteamExternalLauncherExportRequest {
        method,
        launcher_name,
        trainer_path,
        launcher_icon_path: new_launcher_icon_path.clone(),
        prefix_path,
        proton_path,
        steam_app_id,
        steam_client_install_path: steam_client_install_path.clone(),
        target_home_path: target_home_path.clone(),
    };
    crosshook_core::export::rename_launcher_files(
        &old_launcher_slug,
        &new_display_name,
        &new_launcher_icon_path,
        &target_home_path,
        &steam_client_install_path,
        &request,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_launchers(
    target_home_path: String,
    steam_client_install_path: String,
) -> Vec<LauncherInfo> {
    crosshook_core::export::list_launchers(&target_home_path, &steam_client_install_path)
}

#[tauri::command]
pub fn find_orphaned_launchers(
    known_profile_slugs: Vec<String>,
    target_home_path: String,
    steam_client_install_path: String,
) -> Vec<LauncherInfo> {
    crosshook_core::export::find_orphaned_launchers(
        &known_profile_slugs,
        &target_home_path,
        &steam_client_install_path,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = validate_launcher_export
            as fn(SteamExternalLauncherExportRequest) -> Result<(), String>;
        let _ = export_launchers
            as fn(
                SteamExternalLauncherExportRequest,
            ) -> Result<SteamExternalLauncherExportResult, String>;
        let _ = check_launcher_exists as fn(String, String, String, String, String) -> LauncherInfo;
        let _ = check_launcher_for_profile
            as fn(String, State<'_, ProfileStore>) -> Result<LauncherInfo, String>;
        let _ = delete_launcher
            as fn(String, String, String, String, String) -> Result<LauncherDeleteResult, String>;
        let _ = delete_launcher_by_slug
            as fn(String, String, String) -> Result<LauncherDeleteResult, String>;
    }
}
