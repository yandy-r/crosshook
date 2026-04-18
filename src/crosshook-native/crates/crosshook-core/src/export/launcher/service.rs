use super::content::{build_desktop_entry_content, build_trainer_script_content};
use super::names::{resolve_display_name, sanitize_launcher_slug};
use super::paths::{combine_host_unix_path, write_host_text_file};
use super::resolve_target_home_path;
use super::types::{
    validate, SteamExternalLauncherExportError, SteamExternalLauncherExportRequest,
    SteamExternalLauncherExportResult,
};

pub fn export_launchers(
    request: &SteamExternalLauncherExportRequest,
) -> Result<SteamExternalLauncherExportResult, SteamExternalLauncherExportError> {
    let resolved = validated_launcher_paths(request)?;

    write_host_text_file(
        &resolved.script_path,
        &build_trainer_script_content(request, &resolved.display_name),
        0o755,
    )?;
    write_host_text_file(
        &resolved.desktop_entry_path,
        &build_desktop_entry_content(
            &resolved.display_name,
            &resolved.launcher_slug,
            &resolved.script_path,
            &request.launcher_icon_path,
        ),
        0o644,
    )?;

    Ok(SteamExternalLauncherExportResult {
        display_name: resolved.display_name,
        launcher_slug: resolved.launcher_slug,
        script_path: resolved.script_path,
        desktop_entry_path: resolved.desktop_entry_path,
    })
}

pub(super) struct ResolvedLauncherPaths {
    pub(super) display_name: String,
    pub(super) launcher_slug: String,
    pub(super) script_path: String,
    pub(super) desktop_entry_path: String,
}

pub(super) fn validated_launcher_paths(
    request: &SteamExternalLauncherExportRequest,
) -> Result<ResolvedLauncherPaths, SteamExternalLauncherExportError> {
    validate(request).map_err(SteamExternalLauncherExportError::InvalidRequest)?;

    let display_name = resolve_display_name(
        &request.launcher_name,
        &request.steam_app_id,
        &request.trainer_path,
    );
    let launcher_slug = sanitize_launcher_slug(&display_name);
    let target_home_path = resolve_target_home_path(
        &request.target_home_path,
        &request.steam_client_install_path,
    );

    if target_home_path.trim().is_empty() {
        return Err(SteamExternalLauncherExportError::CouldNotResolveHomePath);
    }

    let script_path = combine_host_unix_path(
        &target_home_path,
        ".local/share/crosshook/launchers",
        &format!("{launcher_slug}-trainer.sh"),
    );
    let desktop_entry_path = combine_host_unix_path(
        &target_home_path,
        ".local/share/applications",
        &format!("crosshook-{launcher_slug}-trainer.desktop"),
    );

    Ok(ResolvedLauncherPaths {
        display_name,
        launcher_slug,
        script_path,
        desktop_entry_path,
    })
}
