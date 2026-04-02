use crosshook_core::export::launcher::sanitize_launcher_slug;
use crosshook_core::export::{
    check_launcher_exists_for_request as check_launcher_exists_for_request_core,
    check_launcher_for_profile as check_launcher_for_profile_core,
    delete_launcher_by_slug as delete_launcher_by_slug_core,
    export_launchers as export_launchers_core, validate as validate_launcher_export_core,
    LauncherDeleteResult, LauncherInfo, LauncherRenameResult, SteamExternalLauncherExportRequest,
    SteamExternalLauncherExportResult,
};
use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::{resolve_launch_method, GameProfile, GamescopeConfig, ProfileStore};
use std::collections::HashMap;
use tauri::State;

fn build_export_request_for_profile(
    profile: &GameProfile,
    profile_name: Option<&str>,
    target_home_path: &str,
    steam_client_install_path: &str,
) -> Result<SteamExternalLauncherExportRequest, String> {
    let method = resolve_launch_method(profile).to_string();
    if method == "native" {
        return Err("Profile launch method does not support launcher export".to_string());
    }

    let prefix_path = if method == "steam_applaunch" {
        profile.steam.compatdata_path.clone()
    } else {
        profile.runtime.prefix_path.clone()
    };
    let proton_path = if method == "steam_applaunch" {
        profile.steam.proton_path.clone()
    } else {
        profile.runtime.proton_path.clone()
    };

    Ok(SteamExternalLauncherExportRequest {
        method,
        launcher_name: profile.steam.launcher.display_name.clone(),
        trainer_path: profile.trainer.path.clone(),
        trainer_loading_mode: profile.trainer.loading_mode,
        launcher_icon_path: profile.steam.launcher.icon_path.clone(),
        prefix_path,
        proton_path,
        steam_app_id: profile.steam.app_id.clone(),
        steam_client_install_path: steam_client_install_path.to_string(),
        target_home_path: target_home_path.to_string(),
        profile_name: profile_name.map(|name| name.to_string()),
        gamescope: profile.launch.trainer_gamescope.clone(),
    })
}

fn apply_stale_flags(launchers: &mut [LauncherInfo], stale_by_slug: &HashMap<String, bool>) {
    for launcher in launchers {
        launcher.is_stale = stale_by_slug
            .get(&launcher.launcher_slug)
            .copied()
            .unwrap_or(false);
    }
}

fn stale_flags_by_profile_slug(
    store: &ProfileStore,
    target_home_path: &str,
    steam_client_install_path: &str,
) -> HashMap<String, bool> {
    let profile_names = match store.list() {
        Ok(names) => names,
        Err(error) => {
            tracing::warn!(%error, "failed to list profiles for launcher stale detection");
            return HashMap::new();
        }
    };

    let mut stale_by_slug = HashMap::new();
    for profile_name in profile_names {
        let profile = match store.load(&profile_name) {
            Ok(profile) => profile,
            Err(error) => {
                tracing::warn!(%error, profile_name, "failed to load profile while computing launcher staleness");
                continue;
            }
        };

        if resolve_launch_method(&profile) == "native" {
            continue;
        }

        let launcher_info = match check_launcher_for_profile_core(
            &profile,
            target_home_path,
            steam_client_install_path,
        ) {
            Ok(info) => info,
            Err(error) => {
                tracing::warn!(
                    %error,
                    profile_name,
                    "failed to compute launcher stale status for profile"
                );
                continue;
            }
        };

        if launcher_info.launcher_slug.is_empty() {
            continue;
        }
        stale_by_slug
            .entry(launcher_info.launcher_slug)
            .and_modify(|is_stale| *is_stale = *is_stale || launcher_info.is_stale)
            .or_insert(launcher_info.is_stale);
    }

    stale_by_slug
}

fn find_profile_name_for_launcher_slug(
    store: &ProfileStore,
    launcher_slug: &str,
    target_home_path: &str,
    steam_client_install_path: &str,
) -> Result<Option<String>, String> {
    let profile_names = store.list().map_err(|error| error.to_string())?;
    for profile_name in profile_names {
        let profile = match store.load(&profile_name) {
            Ok(profile) => profile,
            Err(error) => {
                tracing::warn!(%error, profile_name, "failed to load profile while matching launcher slug");
                continue;
            }
        };

        let request = match build_export_request_for_profile(
            &profile,
            Some(&profile_name),
            target_home_path,
            steam_client_install_path,
        ) {
            Ok(request) => request,
            Err(_) => continue,
        };
        let launcher_info = match check_launcher_exists_for_request_core(
            &request.launcher_name,
            &request,
        ) {
            Ok(info) => info,
            Err(error) => {
                tracing::warn!(%error, profile_name, "failed to compute launcher slug while matching profile");
                continue;
            }
        };

        if launcher_info.launcher_slug == launcher_slug {
            return Ok(Some(profile_name));
        }
    }

    Ok(None)
}

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
    gamescope: GamescopeConfig,
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
        gamescope,
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
    store: State<'_, ProfileStore>,
) -> Vec<LauncherInfo> {
    let mut launchers =
        crosshook_core::export::list_launchers(&target_home_path, &steam_client_install_path);
    let stale_by_slug =
        stale_flags_by_profile_slug(&store, &target_home_path, &steam_client_install_path);
    apply_stale_flags(&mut launchers, &stale_by_slug);
    launchers
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

/// Re-exports a launcher by slug using the profile that currently owns that slug.
#[tauri::command]
pub fn reexport_launcher_by_slug(
    launcher_slug: String,
    target_home_path: String,
    steam_client_install_path: String,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<SteamExternalLauncherExportResult, String> {
    let profile_name = find_profile_name_for_launcher_slug(
        &store,
        &launcher_slug,
        &target_home_path,
        &steam_client_install_path,
    )?
    .ok_or_else(|| format!("No profile found for launcher slug '{launcher_slug}'"))?;

    let profile = store
        .load(&profile_name)
        .map_err(|error| error.to_string())?;
    let request = build_export_request_for_profile(
        &profile,
        Some(&profile_name),
        &target_home_path,
        &steam_client_install_path,
    )?;
    validate_launcher_export_core(&request).map_err(|error| error.to_string())?;
    let result = export_launchers_core(&request).map_err(|error| error.to_string())?;

    if let Err(e) = metadata_store.observe_launcher_exported(
        request.profile_name.as_deref(),
        &result.launcher_slug,
        &result.display_name,
        &result.script_path,
        &result.desktop_entry_path,
    ) {
        tracing::warn!(%e, launcher_slug = %result.launcher_slug, "metadata sync after reexport_launcher_by_slug failed");
    }

    Ok(result)
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
        let _ = reexport_launcher_by_slug
            as fn(
                String,
                String,
                String,
                State<'_, ProfileStore>,
                State<'_, MetadataStore>,
            ) -> Result<SteamExternalLauncherExportResult, String>;
        let _ = list_launchers as fn(String, String, State<'_, ProfileStore>) -> Vec<LauncherInfo>;
        let _ = preview_launcher_script
            as fn(SteamExternalLauncherExportRequest) -> Result<String, String>;
        let _ = preview_launcher_desktop
            as fn(SteamExternalLauncherExportRequest) -> Result<String, String>;
    }

    #[test]
    fn apply_stale_flags_sets_matching_slug_status() {
        let mut launchers = vec![
            LauncherInfo {
                launcher_slug: "alpha".to_string(),
                is_stale: false,
                ..Default::default()
            },
            LauncherInfo {
                launcher_slug: "beta".to_string(),
                is_stale: false,
                ..Default::default()
            },
        ];
        let stale_by_slug =
            HashMap::from([("alpha".to_string(), true), ("beta".to_string(), false)]);

        apply_stale_flags(&mut launchers, &stale_by_slug);

        assert!(launchers[0].is_stale);
        assert!(!launchers[1].is_stale);
    }

    #[test]
    fn apply_stale_flags_keeps_unmatched_launchers_not_stale() {
        let mut launchers = vec![LauncherInfo {
            launcher_slug: "orphan".to_string(),
            is_stale: false,
            ..Default::default()
        }];
        let stale_by_slug = HashMap::from([("known".to_string(), true)]);

        apply_stale_flags(&mut launchers, &stale_by_slug);

        assert!(!launchers[0].is_stale);
    }
}
