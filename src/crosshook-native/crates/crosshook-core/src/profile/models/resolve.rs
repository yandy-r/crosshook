use crate::launch::request::{METHOD_NATIVE, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH};

use super::legacy::looks_like_windows_executable;
use super::profile::GameProfile;

pub fn resolve_launch_method(profile: &GameProfile) -> &str {
    let method = profile.launch.method.trim();

    if matches!(
        method,
        METHOD_STEAM_APPLAUNCH | METHOD_PROTON_RUN | METHOD_NATIVE
    ) {
        return method;
    }

    if profile.steam.enabled {
        return METHOD_STEAM_APPLAUNCH;
    }

    if looks_like_windows_executable(&profile.game.executable_path) {
        return METHOD_PROTON_RUN;
    }

    METHOD_NATIVE
}

/// Returns the effective Steam App ID to use for art/metadata resolution.
///
/// Priority: `steam.app_id` (non-empty) → `runtime.steam_app_id`.
/// This field is media-only and does NOT affect how games launch (BR-9).
pub fn resolve_art_app_id(profile: &GameProfile) -> &str {
    let steam = profile.steam.app_id.trim();
    if !steam.is_empty() {
        return steam;
    }
    profile.runtime.steam_app_id.trim()
}

/// Validates a Steam App ID string.
///
/// Accepts: pure ASCII decimal digits, 1–12 characters.
/// Accepts: empty string (means "not set").
/// Rejects: non-digit characters, strings longer than 12 digits.
pub fn validate_steam_app_id(value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Ok(());
    }
    if value.len() > 12 {
        return Err(format!(
            "Steam App ID must be at most 12 digits, got {}",
            value.len()
        ));
    }
    if !value.chars().all(|c| c.is_ascii_digit()) {
        return Err("Steam App ID must contain only numeric digits (0-9)".to_string());
    }
    Ok(())
}
