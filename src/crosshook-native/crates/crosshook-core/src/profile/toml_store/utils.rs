use std::path::Path;

use crate::profile::GameProfile;

use super::error::ProfileStoreError;

/// TOML key under `[launch.presets]` for a bundled catalog preset (`bundled/<preset_id>`).
pub fn bundled_optimization_preset_toml_key(preset_id: &str) -> String {
    format!("bundled/{}", preset_id.trim())
}

pub(super) fn validate_manual_launch_preset_name(raw: &str) -> Result<String, ProfileStoreError> {
    let name = raw.trim();
    if name.is_empty() {
        return Err(ProfileStoreError::InvalidLaunchPresetName(
            "preset name must not be empty".to_string(),
        ));
    }
    if name.starts_with("bundled/") {
        return Err(ProfileStoreError::ReservedLaunchPresetName(
            name.to_string(),
        ));
    }
    Ok(name.to_string())
}

/// Serializes a `GameProfile` to a valid TOML string with comment headers
/// indicating where to save the file for sharing.
///
/// The returned string is valid TOML — comment headers use `#` syntax and are
/// ignored by TOML parsers, so the output can be saved directly as a `.toml` profile.
pub fn profile_to_shareable_toml(
    name: &str,
    profile: &GameProfile,
) -> Result<String, toml::ser::Error> {
    let toml_body = toml::to_string_pretty(profile)?;
    Ok(format!(
        "# CrossHook Profile: {name}\n\
         # https://github.com/yandy-r/crosshook\n\
         #\n\
         # To use this profile, save this file as:\n\
         #   ~/.config/crosshook/profiles/{name}.toml\n\
         #\n\
         # Then select the profile in CrossHook.\n\
         \n\
         {toml_body}"
    ))
}

pub fn validate_name(name: &str) -> Result<String, ProfileStoreError> {
    const WINDOWS_RESERVED_PATH_CHARACTERS: [char; 9] =
        ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        return Err(ProfileStoreError::InvalidName(name.to_string()));
    }
    if trimmed.chars().any(|character| character.is_control()) {
        return Err(ProfileStoreError::InvalidName(name.to_string()));
    }

    if Path::new(trimmed).is_absolute()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains(':')
    {
        return Err(ProfileStoreError::InvalidName(name.to_string()));
    }

    if trimmed
        .chars()
        .any(|character| WINDOWS_RESERVED_PATH_CHARACTERS.contains(&character))
    {
        return Err(ProfileStoreError::InvalidName(name.to_string()));
    }

    Ok(trimmed.to_string())
}

/// Strips a trailing `(Copy)` or `(Copy N)` suffix from a profile name, returning
/// the base name. Non-copy parenthesized suffixes (e.g. `"Game (Special Edition)"`)
/// are left intact.
///
/// Returns the full trimmed input if no copy suffix is detected.
///
/// # Examples (from tests)
/// - `"Name (Copy)"` -> `"Name"`
/// - `"Name (Copy 3)"` -> `"Name"`
/// - `"Game (Special Edition)"` -> `"Game (Special Edition)"` (unchanged)
/// - `"(Copy)"` -> `""` (empty -- caller must handle)
pub(super) fn strip_copy_suffix(name: &str) -> &str {
    let trimmed = name.trim_end();

    if let Some(before_paren) = trimmed.strip_suffix(')') {
        if let Some(pos) = before_paren.rfind('(') {
            let inside = before_paren[pos + 1..].trim();
            if inside == "Copy"
                || inside
                    .strip_prefix("Copy ")
                    .is_some_and(|n| n.parse::<u32>().is_ok())
            {
                return trimmed[..pos].trim_end();
            }
        }
    }

    trimmed
}
