//! Canonical profile normalization matching Classic TypeScript (`profileNormalize.ts`).
//!
//! Classic normalizes draft profiles in TypeScript before calling `profile_save`.
//! Running the exact same normalization rules in `crosshook-core` (and invoking
//! [`normalize_profile`] from `crosshook_app::services::profiles::ProfileWriter`)
//! guarantees that Classic and native UIs write byte-identical `profiles/*.toml`
//! files when sharing one data directory.

use std::collections::{BTreeMap, HashSet};

use crate::launch::catalog::OptimizationCatalog;
use crate::launch::request::METHOD_STEAM_APPLAUNCH;
use crate::profile::models::{
    resolve_launch_method, GameProfile, InjectionNormalizePolicy, LaunchCommandArgumentsSection,
    LaunchOptimizationsSection,
};

/// Suffix automatically appended to exported trainer launcher display names.
pub const AUTOMATIC_LAUNCHER_SUFFIX: &str = " - Trainer";

/// Maximum character length preserved for `runtime.umu_store` and `runtime.umu_codename`.
pub const UMU_RUNTIME_HINT_MAX_LENGTH: usize = 128;

/// Strips a trailing `" - Trainer"` suffix (after trimming) so saved profiles
/// never accumulate repeated suffixes across edits and exports.
pub fn strip_automatic_launcher_suffix(value: &str) -> String {
    let trimmed = value.trim();
    match trimmed.strip_suffix(AUTOMATIC_LAUNCHER_SUFFIX) {
        Some(stripped) => stripped.trim_end().to_string(),
        None => trimmed.to_string(),
    }
}

/// Derives a display name from a Unix or Windows path by taking its final path
/// segment and removing the last `.ext` suffix (matching JS `/\.[^.]+$/`).
fn derive_display_name_from_path(path: &str) -> String {
    let normalized = path.trim();
    if normalized.is_empty() {
        return String::new();
    }

    let segment = normalized.rsplit(['/', '\\']).next().unwrap_or("");
    let stem = match segment.rfind('.') {
        Some(dot) if dot + 1 < segment.len() => &segment[..dot],
        _ => segment,
    };
    stem.trim().to_string()
}

/// Derives `game.name` for save: trimmed `game.name`, falling back to the stem
/// of `game.executable_path`.
pub fn derive_game_name(profile: &GameProfile) -> String {
    let name = profile.game.name.trim();
    if !name.is_empty() {
        return name.to_string();
    }
    derive_display_name_from_path(&profile.game.executable_path)
}

/// Derives `steam.launcher.display_name` for save: stripped launcher display
/// name, falling back to [`derive_game_name`], then the stripped stem of
/// `trainer.path`.
pub fn derive_launcher_display_name(profile: &GameProfile) -> String {
    let explicit = strip_automatic_launcher_suffix(&profile.steam.launcher.display_name);
    if !explicit.is_empty() {
        return explicit;
    }
    let game_name = derive_game_name(profile);
    if !game_name.is_empty() {
        return game_name;
    }
    strip_automatic_launcher_suffix(&derive_display_name_from_path(&profile.trainer.path))
}

/// Normalizes a `runtime.umu_store` or `runtime.umu_codename` hint:
/// trims whitespace, drops the value if it contains any ASCII control character
/// (`< 0x20` or `0x7F`), caps at [`UMU_RUNTIME_HINT_MAX_LENGTH`] Unicode scalar
/// values, and optionally lowercases.
pub fn normalize_umu_runtime_hint(value: &str, lowercase: bool) -> String {
    let trimmed = value.trim();
    if trimmed
        .chars()
        .any(|ch| (ch as u32) < 32 || (ch as u32) == 127)
    {
        return String::new();
    }
    let capped: String = trimmed.chars().take(UMU_RUNTIME_HINT_MAX_LENGTH).collect();
    if lowercase {
        capped.to_lowercase()
    } else {
        capped
    }
}

/// Deduplicates launch optimization IDs preserving first-occurrence order.
///
/// When `catalog` is `Some`, unknown IDs are filtered out (matching Classic's
/// `catalogLoaded = true` mode). When `None`, IDs pass through leniently
/// (matching `catalogLoaded = false`).
pub fn normalize_launch_optimization_ids(
    ids: &[String],
    catalog: Option<&OptimizationCatalog>,
) -> Vec<String> {
    let mut normalized = Vec::with_capacity(ids.len());
    let mut seen = HashSet::with_capacity(ids.len());

    for option_id in ids {
        if let Some(cat) = catalog {
            if !cat.is_known_id(option_id) {
                continue;
            }
        }
        if seen.insert(option_id.as_str()) {
            normalized.push(option_id.clone());
        }
    }

    normalized
}

fn normalize_command_argument_ids(ids: &[String]) -> Vec<String> {
    let mut normalized = Vec::with_capacity(ids.len());
    let mut seen = HashSet::with_capacity(ids.len());

    for argument_id in ids {
        let trimmed = argument_id.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            normalized.push(trimmed.to_string());
        }
    }

    normalized
}

fn normalize_command_arguments_for_edit(
    args: &LaunchCommandArgumentsSection,
) -> LaunchCommandArgumentsSection {
    LaunchCommandArgumentsSection {
        enabled_argument_ids: normalize_command_argument_ids(&args.enabled_argument_ids),
        custom_args: args
            .custom_args
            .iter()
            .map(|arg| arg.trim().to_string())
            .collect(),
        extra: args.extra.clone(),
    }
}

/// Normalizes `launch.command_arguments` before persistence: trims and
/// deduplicates `enabled_argument_ids`, trims `custom_args`, and drops blank
/// custom rows.
pub fn normalize_command_arguments_for_save(
    args: &LaunchCommandArgumentsSection,
) -> LaunchCommandArgumentsSection {
    let mut for_edit = normalize_command_arguments_for_edit(args);
    for_edit.custom_args.retain(|arg| !arg.is_empty());
    for_edit
}

fn normalize_launch_presets(
    raw_presets: &BTreeMap<String, LaunchOptimizationsSection>,
    active_preset: &str,
    catalog: Option<&OptimizationCatalog>,
) -> (BTreeMap<String, LaunchOptimizationsSection>, String) {
    let mut presets = BTreeMap::new();
    for (key, section) in raw_presets {
        let name = key.trim();
        if name.is_empty() {
            continue;
        }
        presets.insert(
            name.to_string(),
            LaunchOptimizationsSection {
                enabled_option_ids: normalize_launch_optimization_ids(
                    &section.enabled_option_ids,
                    catalog,
                ),
                extra: section.extra.clone(),
            },
        );
    }
    (presets, active_preset.trim().to_string())
}

/// Normalizes a profile draft for editing (matching Classic's
/// `normalizeProfileForEdit` with lenient optimization ID handling).
pub fn normalize_profile_for_edit(profile: GameProfile) -> GameProfile {
    normalize_profile_for_edit_with_catalog(profile, None)
}

/// Normalizes a profile draft for editing, optionally filtering optimization
/// IDs against `catalog`.
pub fn normalize_profile_for_edit_with_catalog(
    mut profile: GameProfile,
    catalog: Option<&OptimizationCatalog>,
) -> GameProfile {
    let method = resolve_launch_method(&profile).to_string();
    let (presets, active_preset) = normalize_launch_presets(
        &profile.launch.presets,
        &profile.launch.active_preset,
        catalog,
    );

    let mut enabled_option_ids = normalize_launch_optimization_ids(
        &profile.launch.optimizations.enabled_option_ids,
        catalog,
    );
    if !active_preset.is_empty() {
        if let Some(preset) = presets.get(&active_preset) {
            enabled_option_ids = preset.enabled_option_ids.clone();
        }
    }

    profile.trainer.kind = profile.trainer.kind.trim().to_string();
    profile
        .injection
        .reconcile(InjectionNormalizePolicy::ClassicEditor);
    profile.steam.enabled = method == METHOD_STEAM_APPLAUNCH;
    profile.steam.launcher.display_name =
        strip_automatic_launcher_suffix(&profile.steam.launcher.display_name);

    profile.runtime.prefix_path = profile.runtime.prefix_path.trim().to_string();
    profile.runtime.proton_path = profile.runtime.proton_path.trim().to_string();
    profile.runtime.working_directory = profile.runtime.working_directory.trim().to_string();
    profile.runtime.umu_store = normalize_umu_runtime_hint(&profile.runtime.umu_store, true);
    profile.runtime.umu_codename = normalize_umu_runtime_hint(&profile.runtime.umu_codename, false);

    profile.launch.method = method;
    profile.launch.presets = presets;
    profile.launch.active_preset = active_preset;
    profile.launch.optimizations.enabled_option_ids = enabled_option_ids;
    profile.launch.command_arguments =
        normalize_command_arguments_for_edit(&profile.launch.command_arguments);

    profile
}

/// Normalizes a profile before persistence (matching Classic's
/// `normalizeProfileForSave`).
///
/// Called by `crosshook_app::services::profiles::ProfileWriter::save` on every
/// full save so native and Classic callers cannot write divergent TOML.
pub fn normalize_profile(profile: GameProfile) -> GameProfile {
    normalize_profile_with_catalog(profile, None)
}

/// Normalizes a profile before persistence, optionally filtering optimization
/// IDs against `catalog`.
pub fn normalize_profile_with_catalog(
    profile: GameProfile,
    catalog: Option<&OptimizationCatalog>,
) -> GameProfile {
    let mut normalized = normalize_profile_for_edit_with_catalog(profile, catalog);

    let derived_game_name = derive_game_name(&normalized);
    let derived_launcher_display_name = derive_launcher_display_name(&normalized);

    normalized.game.name = derived_game_name;
    normalized.steam.launcher.display_name = derived_launcher_display_name;
    normalized.launch.command_arguments =
        normalize_command_arguments_for_save(&normalized.launch.command_arguments);

    normalized
}

#[cfg(test)]
mod tests;
