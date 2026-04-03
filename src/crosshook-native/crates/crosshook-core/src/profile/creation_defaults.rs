//! Apply app-level defaults when creating a new profile from settings.

use crate::profile::models::{GameProfile, TrainerLoadingMode};
use crate::settings::AppSettingsData;

/// Fills empty/default profile fields from `settings` (new-profile creation only).
pub fn apply_profile_creation_defaults_from_settings(
    profile: &mut GameProfile,
    settings: &AppSettingsData,
) {
    let proton = settings.default_proton_path.trim();
    if !proton.is_empty() && profile.runtime.proton_path.trim().is_empty() {
        profile.runtime.proton_path = proton.to_string();
    }

    let method = settings.default_launch_method.trim();
    if !method.is_empty() && profile.launch.method.trim().is_empty() {
        profile.launch.method = method.to_string();
    }

    let mode = parse_trainer_loading_mode(&settings.default_trainer_loading_mode);
    if profile.trainer.loading_mode == TrainerLoadingMode::default() && mode != TrainerLoadingMode::default()
    {
        profile.trainer.loading_mode = mode;
    }
}

fn parse_trainer_loading_mode(raw: &str) -> TrainerLoadingMode {
    match raw.trim() {
        "copy_to_prefix" => TrainerLoadingMode::CopyToPrefix,
        _ => TrainerLoadingMode::SourceDirectory,
    }
}
