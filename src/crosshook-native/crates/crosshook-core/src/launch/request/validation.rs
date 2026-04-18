use super::error::ValidationError;
use super::issues::LaunchValidationIssue;
use super::models::{
    is_inside_gamescope_session, looks_like_windows_executable, LaunchRequest, METHOD_NATIVE,
    METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
};
use super::path_probe::{
    path_exists_visible_or_host, path_is_file_visible_or_host, require_directory,
    require_executable_file,
};
use crate::launch::optimizations::{
    is_command_available, resolve_launch_directives, resolve_launch_directives_for_method,
};
use crate::profile::GamescopeConfig;

/// Keys the user may not override via `custom_env_vars` (runtime-managed).
const RESERVED_CUSTOM_ENV_KEYS: &[&str] = &[
    "WINEPREFIX",
    "STEAM_COMPAT_DATA_PATH",
    "STEAM_COMPAT_CLIENT_INSTALL_PATH",
];

/// Validates one custom env entry and returns the canonical **trimmed** key for emission paths.
fn validate_custom_env_entry(key: &str, value: &str) -> Result<String, ValidationError> {
    let trimmed_key = key.trim();
    if trimmed_key.is_empty() {
        return Err(ValidationError::CustomEnvVarKeyEmpty);
    }
    if key.contains('=') {
        return Err(ValidationError::CustomEnvVarKeyContainsEquals);
    }
    if key.contains('\0') {
        return Err(ValidationError::CustomEnvVarKeyContainsNul);
    }
    if value.contains('\0') {
        return Err(ValidationError::CustomEnvVarValueContainsNul);
    }
    if RESERVED_CUSTOM_ENV_KEYS.contains(&trimmed_key) {
        return Err(ValidationError::CustomEnvVarReservedKey(
            trimmed_key.to_string(),
        ));
    }
    Ok(trimmed_key.to_string())
}

fn validate_custom_env(request: &LaunchRequest) -> Result<(), ValidationError> {
    for (key, value) in &request.custom_env_vars {
        validate_custom_env_entry(key, value)?;
    }
    Ok(())
}

fn collect_custom_env_issues(request: &LaunchRequest, issues: &mut Vec<LaunchValidationIssue>) {
    for (key, value) in &request.custom_env_vars {
        if let Err(err) = validate_custom_env_entry(key, value) {
            issues.push(err.issue());
        }
    }
}

fn collect_gamescope_issues(
    request: &LaunchRequest,
    config: &GamescopeConfig,
    issues: &mut Vec<LaunchValidationIssue>,
) {
    let method = request.resolved_method();
    if method != METHOD_PROTON_RUN && method != METHOD_STEAM_APPLAUNCH {
        issues.push(ValidationError::GamescopeNotSupportedForMethod(method.to_string()).issue());
        return;
    }

    if !is_command_available("gamescope") {
        issues.push(ValidationError::GamescopeBinaryMissing.issue());
    }

    if config.internal_width.is_some() != config.internal_height.is_some() {
        issues.push(
            ValidationError::GamescopeResolutionPairIncomplete {
                pair: "internal".into(),
            }
            .issue(),
        );
    }

    if config.output_width.is_some() != config.output_height.is_some() {
        issues.push(
            ValidationError::GamescopeResolutionPairIncomplete {
                pair: "output".into(),
            }
            .issue(),
        );
    }

    if let Some(v) = config.fsr_sharpness {
        if v > 20 {
            issues.push(ValidationError::GamescopeFsrSharpnessOutOfRange(v).issue());
        }
    }

    if config.fullscreen && config.borderless {
        issues.push(ValidationError::GamescopeFullscreenBorderlessConflict.issue());
    }

    if is_inside_gamescope_session() && !config.allow_nested {
        issues.push(ValidationError::GamescopeNestedSession.issue());
    }
}

pub fn validate_all(request: &LaunchRequest) -> Vec<LaunchValidationIssue> {
    let method_trimmed = request.method.trim();
    if !method_trimmed.is_empty()
        && method_trimmed != METHOD_STEAM_APPLAUNCH
        && method_trimmed != METHOD_PROTON_RUN
        && method_trimmed != METHOD_NATIVE
    {
        return vec![ValidationError::UnsupportedMethod(method_trimmed.to_string()).issue()];
    }

    let mut issues = Vec::new();
    collect_custom_env_issues(request, &mut issues);
    match request.resolved_method() {
        METHOD_STEAM_APPLAUNCH => collect_steam_issues(request, &mut issues),
        METHOD_PROTON_RUN => collect_proton_issues(request, &mut issues),
        METHOD_NATIVE => collect_native_issues(request, &mut issues),
        other => issues.push(ValidationError::UnsupportedMethod(other.to_string()).issue()),
    }
    let gamescope_config = request.effective_gamescope_config();
    if gamescope_config.enabled {
        collect_gamescope_issues(request, &gamescope_config, &mut issues);
    }
    issues
}

pub fn validate(request: &LaunchRequest) -> Result<(), ValidationError> {
    match request.method.trim() {
        "" | METHOD_STEAM_APPLAUNCH | METHOD_PROTON_RUN | METHOD_NATIVE => {}
        value => return Err(ValidationError::UnsupportedMethod(value.to_string())),
    }

    validate_custom_env(request)?;

    match request.resolved_method() {
        METHOD_STEAM_APPLAUNCH => validate_steam_applaunch(request),
        METHOD_PROTON_RUN => validate_proton_run(request),
        METHOD_NATIVE => validate_native(request),
        other => Err(ValidationError::UnsupportedMethod(other.to_string())),
    }
}

fn validate_steam_applaunch(request: &LaunchRequest) -> Result<(), ValidationError> {
    require_game_path_if_needed(request, false)?;
    require_trainer_paths_if_needed(request)?;

    if request.steam.app_id.trim().is_empty() {
        return Err(ValidationError::SteamAppIdRequired);
    }

    require_directory(
        request.steam.compatdata_path.trim(),
        ValidationError::SteamCompatDataPathRequired,
        ValidationError::SteamCompatDataPathMissing,
        ValidationError::SteamCompatDataPathNotDirectory,
    )?;

    require_executable_file(
        request.steam.proton_path.trim(),
        ValidationError::SteamProtonPathRequired,
        ValidationError::SteamProtonPathMissing,
        ValidationError::SteamProtonPathNotExecutable,
    )?;

    if request.steam.steam_client_install_path.trim().is_empty() {
        return Err(ValidationError::SteamClientInstallPathRequired);
    }

    if !request.launch_trainer_only {
        // Align with `build_steam_launch_options_command`: steam_applaunch uses the same
        // optimization IDs as `proton_run` for the Launch Options prefix (unknown IDs,
        // conflicts, PATH deps). Trainer-only launches intentionally ignore the game's
        // optimization wrapper stack to match exported trainer launchers.
        resolve_launch_directives_for_method(
            &request.optimizations.enabled_option_ids,
            METHOD_PROTON_RUN,
        )?;
    }

    Ok(())
}

fn validate_proton_run(request: &LaunchRequest) -> Result<(), ValidationError> {
    require_game_path_if_needed(request, true)?;
    require_trainer_paths_if_needed(request)?;

    require_directory(
        request.runtime.prefix_path.trim(),
        ValidationError::RuntimePrefixPathRequired,
        ValidationError::RuntimePrefixPathMissing,
        ValidationError::RuntimePrefixPathNotDirectory,
    )?;

    require_executable_file(
        request.runtime.proton_path.trim(),
        ValidationError::RuntimeProtonPathRequired,
        ValidationError::RuntimeProtonPathMissing,
        ValidationError::RuntimeProtonPathNotExecutable,
    )?;

    if !request.launch_trainer_only {
        resolve_launch_directives(request)?;
    }

    Ok(())
}

fn validate_native(request: &LaunchRequest) -> Result<(), ValidationError> {
    if request.launch_trainer_only {
        return Err(ValidationError::NativeTrainerLaunchUnsupported);
    }

    require_game_path_if_needed(request, true)?;

    if looks_like_windows_executable(&request.game_path) {
        return Err(ValidationError::NativeWindowsExecutableNotSupported);
    }

    reject_launch_optimizations_for_method(request, METHOD_NATIVE)?;

    Ok(())
}

fn collect_steam_issues(request: &LaunchRequest, issues: &mut Vec<LaunchValidationIssue>) {
    if let Err(e) = require_game_path_if_needed(request, false) {
        issues.push(e.issue());
    }
    if let Err(e) = require_trainer_paths_if_needed(request) {
        issues.push(e.issue());
    }

    if request.steam.app_id.trim().is_empty() {
        issues.push(ValidationError::SteamAppIdRequired.issue());
    }

    if let Err(e) = require_directory(
        request.steam.compatdata_path.trim(),
        ValidationError::SteamCompatDataPathRequired,
        ValidationError::SteamCompatDataPathMissing,
        ValidationError::SteamCompatDataPathNotDirectory,
    ) {
        issues.push(e.issue());
    }

    if let Err(e) = require_executable_file(
        request.steam.proton_path.trim(),
        ValidationError::SteamProtonPathRequired,
        ValidationError::SteamProtonPathMissing,
        ValidationError::SteamProtonPathNotExecutable,
    ) {
        issues.push(e.issue());
    }

    if request.steam.steam_client_install_path.trim().is_empty() {
        issues.push(ValidationError::SteamClientInstallPathRequired.issue());
    }

    if !request.launch_trainer_only {
        if let Err(e) = resolve_launch_directives_for_method(
            &request.optimizations.enabled_option_ids,
            METHOD_PROTON_RUN,
        ) {
            issues.push(e.issue());
        }
    }

    if request.network_isolation
        && !request.launch_game_only
        && !crate::launch::runtime_helpers::is_unshare_net_available()
    {
        issues.push(ValidationError::UnshareNetUnavailable.issue());
    }
}

fn collect_proton_issues(request: &LaunchRequest, issues: &mut Vec<LaunchValidationIssue>) {
    if let Err(e) = require_game_path_if_needed(request, true) {
        issues.push(e.issue());
    }
    if let Err(e) = require_trainer_paths_if_needed(request) {
        issues.push(e.issue());
    }

    if let Err(e) = require_directory(
        request.runtime.prefix_path.trim(),
        ValidationError::RuntimePrefixPathRequired,
        ValidationError::RuntimePrefixPathMissing,
        ValidationError::RuntimePrefixPathNotDirectory,
    ) {
        issues.push(e.issue());
    }

    if let Err(e) = require_executable_file(
        request.runtime.proton_path.trim(),
        ValidationError::RuntimeProtonPathRequired,
        ValidationError::RuntimeProtonPathMissing,
        ValidationError::RuntimeProtonPathNotExecutable,
    ) {
        issues.push(e.issue());
    }

    if !request.launch_trainer_only {
        if let Err(e) = resolve_launch_directives(request) {
            issues.push(e.issue());
        }
    }

    if request.network_isolation
        && !request.launch_game_only
        && !crate::launch::runtime_helpers::is_unshare_net_available()
    {
        issues.push(ValidationError::UnshareNetUnavailable.issue());
    }
}

fn collect_native_issues(request: &LaunchRequest, issues: &mut Vec<LaunchValidationIssue>) {
    if request.launch_trainer_only {
        issues.push(ValidationError::NativeTrainerLaunchUnsupported.issue());
    }

    if let Err(e) = require_game_path_if_needed(request, true) {
        issues.push(e.issue());
    }

    if looks_like_windows_executable(&request.game_path) {
        issues.push(ValidationError::NativeWindowsExecutableNotSupported.issue());
    }

    if let Err(e) = reject_launch_optimizations_for_method(request, METHOD_NATIVE) {
        issues.push(e.issue());
    }
}

fn reject_launch_optimizations_for_method(
    request: &LaunchRequest,
    method: &str,
) -> Result<(), ValidationError> {
    if request.optimizations.enabled_option_ids.is_empty() {
        return Ok(());
    }

    Err(ValidationError::LaunchOptimizationsUnsupportedForMethod(
        method.to_string(),
    ))
}

fn require_game_path_if_needed(
    request: &LaunchRequest,
    must_exist: bool,
) -> Result<(), ValidationError> {
    if request.launch_trainer_only {
        return Ok(());
    }

    let game_path = request.game_path.trim();
    if game_path.is_empty() {
        return Err(ValidationError::GamePathRequired);
    }

    if must_exist {
        if !path_exists_visible_or_host(&request.game_path) {
            return Err(ValidationError::GamePathMissing);
        }
        if !path_is_file_visible_or_host(&request.game_path) {
            return Err(ValidationError::GamePathNotFile);
        }
    }

    Ok(())
}

fn require_trainer_paths_if_needed(request: &LaunchRequest) -> Result<(), ValidationError> {
    if request.launch_game_only {
        return Ok(());
    }

    if request.trainer_path.trim().is_empty() {
        return Err(ValidationError::TrainerPathRequired);
    }

    let trainer_host_path = request.trainer_host_path.trim();
    if trainer_host_path.is_empty() {
        return Err(ValidationError::TrainerHostPathRequired);
    }

    if !path_exists_visible_or_host(&request.trainer_host_path) {
        return Err(ValidationError::TrainerHostPathMissing);
    }
    if !path_is_file_visible_or_host(&request.trainer_host_path) {
        return Err(ValidationError::TrainerHostPathNotFile);
    }

    Ok(())
}
