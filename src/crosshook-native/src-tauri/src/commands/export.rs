use crosshook_core::export::{
    check_launcher_exists_for_request as check_launcher_exists_for_request_core,
    check_launcher_for_profile as check_launcher_for_profile_core,
    delete_launcher_by_slug as delete_launcher_by_slug_core,
    export_launchers as export_launchers_core, validate as validate_launcher_export_core,
    LauncherDeleteResult, LauncherInfo, LauncherRenameResult, SteamExternalLauncherExportRequest,
    SteamExternalLauncherExportResult,
};
use crosshook_core::export::launcher::sanitize_launcher_slug;
use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::ProfileStore;
use tauri::State;

/// Validates whether a launcher export request has the required trainer/runtime inputs.
#[tauri::command]
pub fn validate_launcher_export(request: SteamExternalLauncherExportRequest) -> Result<(), String> {
    validate_launcher_export_core(&request).map_err(|error| error.to_string())
}

/// Exports the launcher shell script and desktop entry for the provided request.
#[tauri::command]
pub fn export_launchers(
    request: SteamExternalLauncherExportRequest,
    metadata_store: State<'_, MetadataStore>,
) -> Result<SteamExternalLauncherExportResult, String> {
    let result = export_launchers_core(&request).map_err(|error| error.to_string())?;

    if let Err(e) = metadata_store.observe_launcher_exported(
        request.profile_name.as_deref(),
        &result.launcher_slug,
        &result.display_name,
        &result.script_path,
        &result.desktop_entry_path,
    ) {
        tracing::warn!(%e, launcher_slug = %result.launcher_slug, "metadata sync after export_launchers failed");
    }

    Ok(result)
}

/// Checks whether the launcher files derived from the supplied profile fields exist on disk.
#[tauri::command]
pub fn check_launcher_exists(
    request: SteamExternalLauncherExportRequest,
) -> Result<LauncherInfo, String> {
    check_launcher_exists_for_request_core(&request.launcher_name, &request)
        .map_err(|error| error.to_string())
}

/// Loads a saved profile and checks whether its exported launcher files exist on disk.
#[tauri::command]
pub fn check_launcher_for_profile(
    name: String,
    store: State<'_, ProfileStore>,
) -> Result<LauncherInfo, String> {
    let profile = store.load(&name).map_err(|error| error.to_string())?;
    check_launcher_for_profile_core(&profile, "", "").map_err(|error| error.to_string())
}

/// Deletes the launcher files derived from the supplied profile fields.
#[tauri::command]
pub fn delete_launcher(
    display_name: String,
    steam_app_id: String,
    trainer_path: String,
    target_home_path: String,
    steam_client_install_path: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<LauncherDeleteResult, String> {
    let result = crosshook_core::export::delete_launcher_files(
        &display_name,
        &steam_app_id,
        &trainer_path,
        &target_home_path,
        &steam_client_install_path,
    )
    .map_err(|error| error.to_string())?;

    let slug = sanitize_launcher_slug(&display_name);
    if let Err(e) = metadata_store.observe_launcher_deleted(&slug) {
        tracing::warn!(%e, launcher_slug = %slug, "metadata sync after delete_launcher failed");
    }

    Ok(result)
}

/// Deletes launcher files directly from a known launcher slug.
#[tauri::command]
pub fn delete_launcher_by_slug(
    launcher_slug: String,
    target_home_path: String,
    steam_client_install_path: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<LauncherDeleteResult, String> {
    let result = delete_launcher_by_slug_core(
        &launcher_slug,
        &target_home_path,
        &steam_client_install_path,
    )
    .map_err(|error| error.to_string())?;

    if let Err(e) = metadata_store.observe_launcher_deleted(&launcher_slug) {
        tracing::warn!(%e, launcher_slug = %launcher_slug, "metadata sync after delete_launcher_by_slug failed");
    }

    Ok(result)
}

/// Rewrites launcher files for a renamed launcher and optionally cleans up old paths.
#[tauri::command]
pub fn rename_launcher(
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
    metadata_store: State<'_, MetadataStore>,
) -> Result<LauncherRenameResult, String> {
    let request = SteamExternalLauncherExportRequest {
        method,
        launcher_name,
        trainer_path,
        trainer_loading_mode: trainer_loading_mode
            .parse()
            .map_err(|_| "invalid trainer loading mode".to_string())?,
        launcher_icon_path: new_launcher_icon_path.clone(),
        prefix_path,
        proton_path,
        steam_app_id,
        steam_client_install_path: steam_client_install_path.clone(),
        target_home_path: target_home_path.clone(),
        profile_name: None,
    };
    let result = crosshook_core::export::rename_launcher_files(
        &old_launcher_slug,
        &new_display_name,
        &new_launcher_icon_path,
        &target_home_path,
        &steam_client_install_path,
        &request,
    )
    .map_err(|error| error.to_string())?;

    if let Err(e) = metadata_store.observe_launcher_renamed(
        &result.old_slug,
        &result.new_slug,
        &new_display_name,
        &result.new_script_path,
        &result.new_desktop_entry_path,
    ) {
        tracing::warn!(%e, old_slug = %result.old_slug, new_slug = %result.new_slug, "metadata sync after rename_launcher failed");
    }

    Ok(result)
}

/// Lists launcher files found under the resolved launcher directory.
#[tauri::command]
pub fn list_launchers(
    target_home_path: String,
    steam_client_install_path: String,
) -> Vec<LauncherInfo> {
    crosshook_core::export::list_launchers(&target_home_path, &steam_client_install_path)
}

/// Lists launcher files whose slugs do not match the supplied known profile slugs.
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

/// Generates the trainer launcher script content for clipboard copy.
/// Does NOT write to disk.
#[tauri::command]
pub fn preview_launcher_script(
    request: SteamExternalLauncherExportRequest,
) -> Result<String, String> {
    crosshook_core::export::preview_trainer_script_content(&request)
        .map_err(|error| error.to_string())
}

/// Generates the desktop entry content for clipboard copy.
/// Does NOT write to disk.
#[tauri::command]
pub fn preview_launcher_desktop(
    request: SteamExternalLauncherExportRequest,
) -> Result<String, String> {
    crosshook_core::export::preview_desktop_entry_content(&request)
        .map_err(|error| error.to_string())
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
                State<'_, MetadataStore>,
            ) -> Result<SteamExternalLauncherExportResult, String>;
        let _ = check_launcher_exists
            as fn(SteamExternalLauncherExportRequest) -> Result<LauncherInfo, String>;
        let _ = check_launcher_for_profile
            as fn(String, State<'_, ProfileStore>) -> Result<LauncherInfo, String>;
        let _ = delete_launcher
            as fn(
                String,
                String,
                String,
                String,
                String,
                State<'_, MetadataStore>,
            ) -> Result<LauncherDeleteResult, String>;
        let _ = delete_launcher_by_slug
            as fn(
                String,
                String,
                String,
                State<'_, MetadataStore>,
            ) -> Result<LauncherDeleteResult, String>;
        let _ = preview_launcher_script
            as fn(SteamExternalLauncherExportRequest) -> Result<String, String>;
        let _ = preview_launcher_desktop
            as fn(SteamExternalLauncherExportRequest) -> Result<String, String>;
    }
}
