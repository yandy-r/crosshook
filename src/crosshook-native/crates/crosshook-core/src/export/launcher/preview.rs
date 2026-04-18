use super::content::{build_desktop_entry_content, build_trainer_script_content};
use super::service::validated_launcher_paths;
use super::types::{SteamExternalLauncherExportError, SteamExternalLauncherExportRequest};

/// Generates the trainer launcher script content with placement comment headers,
/// suitable for clipboard copy. Does NOT write to disk.
pub fn preview_trainer_script_content(
    request: &SteamExternalLauncherExportRequest,
) -> Result<String, SteamExternalLauncherExportError> {
    let resolved = validated_launcher_paths(request)?;
    let body = build_trainer_script_content(request, &resolved.display_name);
    Ok(format!(
        "# Save this file to: {}\n\
         # Make executable: chmod +x {}\n\
         \n\
         {body}",
        resolved.script_path, resolved.script_path
    ))
}

/// Generates the desktop entry content with placement comment headers,
/// suitable for clipboard copy. Does NOT write to disk.
pub fn preview_desktop_entry_content(
    request: &SteamExternalLauncherExportRequest,
) -> Result<String, SteamExternalLauncherExportError> {
    let resolved = validated_launcher_paths(request)?;
    let body = build_desktop_entry_content(
        &resolved.display_name,
        &resolved.launcher_slug,
        &resolved.script_path,
        &request.launcher_icon_path,
    );
    Ok(format!(
        "# Save this file to: {}\n\
         # Permissions: 644 (not executable)\n\
         \n\
         {body}",
        resolved.desktop_entry_path
    ))
}
