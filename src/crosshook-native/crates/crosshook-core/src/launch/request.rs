use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::optimizations::{
    is_command_available, resolve_launch_directives, resolve_launch_directives_for_method,
};
use crate::profile::{GamescopeConfig, MangoHudConfig, TrainerLoadingMode};

pub const METHOD_STEAM_APPLAUNCH: &str = "steam_applaunch";
pub const METHOD_PROTON_RUN: &str = "proton_run";
pub const METHOD_NATIVE: &str = "native";

/// Returns `true` if the current process is running inside a gamescope compositor session.
pub fn is_inside_gamescope_session() -> bool {
    std::env::var("GAMESCOPE_WAYLAND_DISPLAY").is_ok()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LaunchRequest {
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub game_path: String,
    #[serde(default)]
    pub trainer_path: String,
    #[serde(default)]
    pub trainer_host_path: String,
    #[serde(default)]
    pub trainer_loading_mode: TrainerLoadingMode,
    #[serde(default)]
    pub steam: SteamLaunchConfig,
    #[serde(default)]
    pub runtime: RuntimeLaunchConfig,
    #[serde(default)]
    pub optimizations: LaunchOptimizationsRequest,
    #[serde(default)]
    pub launch_trainer_only: bool,
    #[serde(default)]
    pub launch_game_only: bool,
    #[serde(default)]
    pub profile_name: Option<String>,
    #[serde(
        rename = "custom_env_vars",
        default,
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub custom_env_vars: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "GamescopeConfig::is_default")]
    pub gamescope: GamescopeConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trainer_gamescope: Option<GamescopeConfig>,
    #[serde(default, skip_serializing_if = "MangoHudConfig::is_default")]
    pub mangohud: MangoHudConfig,
}

pub type SteamLaunchRequest = LaunchRequest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SteamLaunchConfig {
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub compatdata_path: String,
    #[serde(default)]
    pub proton_path: String,
    #[serde(default)]
    pub steam_client_install_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RuntimeLaunchConfig {
    #[serde(default)]
    pub prefix_path: String,
    #[serde(default)]
    pub proton_path: String,
    #[serde(default)]
    pub working_directory: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LaunchOptimizationsRequest {
    #[serde(
        rename = "enabled_option_ids",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub enabled_option_ids: Vec<String>,
}

impl LaunchRequest {
    pub fn effective_trainer_gamescope(&self) -> &GamescopeConfig {
        self.trainer_gamescope
            .as_ref()
            .filter(|config| config.enabled)
            .unwrap_or(&self.gamescope)
    }

    pub fn effective_gamescope_config(&self) -> &GamescopeConfig {
        if self.launch_trainer_only {
            self.effective_trainer_gamescope()
        } else {
            &self.gamescope
        }
    }

    pub fn resolved_method(&self) -> &str {
        match self.method.trim() {
            METHOD_STEAM_APPLAUNCH => METHOD_STEAM_APPLAUNCH,
            METHOD_PROTON_RUN => METHOD_PROTON_RUN,
            METHOD_NATIVE => METHOD_NATIVE,
            _ if !self.steam.app_id.trim().is_empty() => METHOD_STEAM_APPLAUNCH,
            _ if looks_like_windows_executable(&self.game_path) => METHOD_PROTON_RUN,
            _ => METHOD_NATIVE,
        }
    }

    pub fn game_executable_name(&self) -> String {
        let trimmed_path = self.game_path.trim();

        if trimmed_path.is_empty() {
            return String::new();
        }

        let separator_index = trimmed_path
            .char_indices()
            .rev()
            .find_map(|(index, character)| matches!(character, '/' | '\\').then_some(index));

        match separator_index {
            Some(index) if index + 1 < trimmed_path.len() => trimmed_path[index + 1..].to_string(),
            Some(_) => String::new(),
            None => trimmed_path.to_string(),
        }
    }

    pub fn log_target_slug(&self) -> String {
        let game_executable_name = self.game_executable_name();
        let source = match self.resolved_method() {
            METHOD_STEAM_APPLAUNCH => self.steam.app_id.trim(),
            _ => game_executable_name.trim(),
        };

        let fallback = match self.resolved_method() {
            METHOD_STEAM_APPLAUNCH => "steam",
            METHOD_PROTON_RUN => "proton",
            METHOD_NATIVE => "native",
            _ => "launch",
        };

        let candidate = if source.is_empty() { fallback } else { source };
        let slug = candidate
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() {
                    character.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect::<String>();

        let trimmed = slug.trim_matches('-');
        if trimmed.is_empty() {
            fallback.to_string()
        } else {
            trimmed.to_string()
        }
    }

    pub fn should_copy_trainer_to_prefix(&self) -> bool {
        self.trainer_loading_mode == TrainerLoadingMode::CopyToPrefix
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationSeverity {
    Fatal,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchValidationIssue {
    pub message: String,
    pub help: String,
    pub severity: ValidationSeverity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    GamePathRequired,
    GamePathMissing,
    GamePathNotFile,
    TrainerPathRequired,
    TrainerHostPathRequired,
    TrainerHostPathMissing,
    TrainerHostPathNotFile,
    SteamAppIdRequired,
    SteamCompatDataPathRequired,
    SteamCompatDataPathMissing,
    SteamCompatDataPathNotDirectory,
    SteamProtonPathRequired,
    SteamProtonPathMissing,
    SteamProtonPathNotExecutable,
    SteamClientInstallPathRequired,
    RuntimePrefixPathRequired,
    RuntimePrefixPathMissing,
    RuntimePrefixPathNotDirectory,
    RuntimeProtonPathRequired,
    RuntimeProtonPathMissing,
    RuntimeProtonPathNotExecutable,
    UnknownLaunchOptimization(String),
    DuplicateLaunchOptimization(String),
    LaunchOptimizationsUnsupportedForMethod(String),
    LaunchOptimizationNotSupportedForMethod {
        option_id: String,
        method: String,
    },
    IncompatibleLaunchOptimizations {
        first: String,
        second: String,
    },
    LaunchOptimizationDependencyMissing {
        option_id: String,
        dependency: String,
    },
    NativeWindowsExecutableNotSupported,
    NativeTrainerLaunchUnsupported,
    UnsupportedMethod(String),
    /// Custom env key is empty or only whitespace.
    CustomEnvVarKeyEmpty,
    /// Custom env key contains `=`, which is invalid for environment variable names.
    CustomEnvVarKeyContainsEquals,
    /// Custom env key contains a NUL byte.
    CustomEnvVarKeyContainsNul,
    /// Custom env value contains a NUL byte.
    CustomEnvVarValueContainsNul,
    /// Custom env must not override runtime-reserved keys.
    CustomEnvVarReservedKey(String),
    /// The `gamescope` binary is not installed or not found on PATH.
    GamescopeBinaryMissing,
    /// Gamescope is only supported for `proton_run` and `steam_applaunch` launches.
    GamescopeNotSupportedForMethod(String),
    /// Running inside an existing gamescope session without `allow_nested`.
    GamescopeNestedSession,
    /// Only one of width/height was set for a resolution pair.
    GamescopeResolutionPairIncomplete {
        pair: String,
    },
    /// FSR sharpness value is outside the valid range (0–20).
    GamescopeFsrSharpnessOutOfRange(u8),
    /// Fullscreen and borderless are mutually exclusive in gamescope.
    GamescopeFullscreenBorderlessConflict,
    /// Offline readiness score is below the advisory threshold; launch still proceeds.
    OfflineReadinessInsufficient {
        score: u8,
        reasons: Vec<String>,
    },
}

impl ValidationError {
    pub fn issue(&self) -> LaunchValidationIssue {
        LaunchValidationIssue {
            message: self.message(),
            help: self.help(),
            severity: self.severity(),
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::GamePathRequired => "A game executable path is required.".to_string(),
            Self::GamePathMissing => {
                "The selected game executable path does not exist.".to_string()
            }
            Self::GamePathNotFile => {
                "The selected game executable path must be a file.".to_string()
            }
            Self::TrainerPathRequired => {
                "A trainer path is required for trainer launch.".to_string()
            }
            Self::TrainerHostPathRequired => {
                "A trainer host path is required for trainer launch.".to_string()
            }
            Self::TrainerHostPathMissing => "The trainer host path does not exist.".to_string(),
            Self::TrainerHostPathNotFile => "The trainer host path must be a file.".to_string(),
            Self::SteamAppIdRequired => "Steam app launch requires a Steam App ID.".to_string(),
            Self::SteamCompatDataPathRequired => {
                "Steam app launch requires a compatdata path.".to_string()
            }
            Self::SteamCompatDataPathMissing => {
                "The Steam compatdata path does not exist.".to_string()
            }
            Self::SteamCompatDataPathNotDirectory => {
                "The Steam compatdata path must be a directory.".to_string()
            }
            Self::SteamProtonPathRequired => "Steam app launch requires a Proton path.".to_string(),
            Self::SteamProtonPathMissing => "The Steam Proton path does not exist.".to_string(),
            Self::SteamProtonPathNotExecutable => {
                "The Steam Proton path must be executable.".to_string()
            }
            Self::SteamClientInstallPathRequired => {
                "Steam app launch requires a Steam client install path.".to_string()
            }
            Self::RuntimePrefixPathRequired => {
                "Proton launch requires a runtime prefix path.".to_string()
            }
            Self::RuntimePrefixPathMissing => "The runtime prefix path does not exist.".to_string(),
            Self::RuntimePrefixPathNotDirectory => {
                "The runtime prefix path must be a directory.".to_string()
            }
            Self::RuntimeProtonPathRequired => {
                "Proton launch requires a runtime Proton path.".to_string()
            }
            Self::RuntimeProtonPathMissing => "The runtime Proton path does not exist.".to_string(),
            Self::RuntimeProtonPathNotExecutable => {
                "The runtime Proton path must be executable.".to_string()
            }
            Self::UnknownLaunchOptimization(option_id) => {
                format!("Unknown launch optimization '{option_id}'.")
            }
            Self::DuplicateLaunchOptimization(option_id) => {
                format!("Launch optimization '{option_id}' was selected more than once.")
            }
            Self::LaunchOptimizationsUnsupportedForMethod(method) => {
                format!("Launch optimizations are only supported for proton_run launches, not '{method}'.")
            }
            Self::LaunchOptimizationNotSupportedForMethod { option_id, method } => {
                format!(
                    "Launch optimization '{option_id}' is not supported for '{method}' launches."
                )
            }
            Self::IncompatibleLaunchOptimizations { first, second } => {
                format!("Launch optimizations '{first}' and '{second}' cannot be enabled together.")
            }
            Self::LaunchOptimizationDependencyMissing {
                option_id,
                dependency,
            } => {
                format!("Launch optimization '{option_id}' requires '{dependency}' to be installed and available on PATH.")
            }
            Self::NativeWindowsExecutableNotSupported => {
                "Native launch only supports Linux-native executables, not Windows .exe files."
                    .to_string()
            }
            Self::NativeTrainerLaunchUnsupported => {
                "Native launch does not support the two-step trainer launch workflow.".to_string()
            }
            Self::UnsupportedMethod(method) => {
                format!(
                    "Unsupported launch method '{method}'. Use steam_applaunch, proton_run, or native."
                )
            }
            Self::CustomEnvVarKeyEmpty => {
                "A custom environment variable key cannot be empty.".to_string()
            }
            Self::CustomEnvVarKeyContainsEquals => {
                "Custom environment variable keys cannot contain '='.".to_string()
            }
            Self::CustomEnvVarKeyContainsNul => {
                "Custom environment variable keys cannot contain NUL bytes.".to_string()
            }
            Self::CustomEnvVarValueContainsNul => {
                "Custom environment variable values cannot contain NUL bytes.".to_string()
            }
            Self::CustomEnvVarReservedKey(key) => {
                format!(
                    "The environment variable '{key}' is reserved and cannot be set via custom env vars."
                )
            }
            Self::GamescopeBinaryMissing => {
                "gamescope is not installed or not found on PATH.".to_string()
            }
            Self::GamescopeNotSupportedForMethod(method) => {
                format!("Gamescope is only supported for proton_run and steam_applaunch launches, not '{method}'.")
            }
            Self::GamescopeNestedSession => {
                "Running inside an existing gamescope session. Gamescope will be auto-skipped unless allow_nested is enabled.".to_string()
            }
            Self::GamescopeResolutionPairIncomplete { pair } => {
                format!("Both width and height must be set for {pair} resolution.")
            }
            Self::GamescopeFsrSharpnessOutOfRange(v) => {
                format!("FSR sharpness {v} is out of range (0–20).")
            }
            Self::GamescopeFullscreenBorderlessConflict => {
                "Fullscreen and borderless cannot both be enabled in gamescope.".to_string()
            }
            Self::OfflineReadinessInsufficient { score, reasons } => {
                let detail = if reasons.is_empty() {
                    String::new()
                } else {
                    format!(" {}", reasons.join("; "))
                };
                format!(
                    "Offline readiness score is {score}/100 (below 60).{detail}"
                )
            }
        }
    }

    pub fn help(&self) -> String {
        match self {
            Self::GamePathRequired => {
                "Browse to the game executable before launching. For Steam or Proton launches, this is usually the game's .exe under steamapps/common/."
                    .to_string()
            }
            Self::GamePathMissing => {
                "The saved game path no longer exists. Re-browse to the current executable or verify the game files."
                    .to_string()
            }
            Self::GamePathNotFile => {
                "Select the game executable file itself, not the containing directory."
                    .to_string()
            }
            Self::TrainerPathRequired => {
                "Select the trainer executable before starting the trainer step."
                    .to_string()
            }
            Self::TrainerHostPathRequired => {
                "Save the trainer executable path in the profile so CrossHook can locate the host-side trainer file."
                    .to_string()
            }
            Self::TrainerHostPathMissing => {
                "The saved trainer file was moved or deleted. Re-browse to the trainer executable."
                    .to_string()
            }
            Self::TrainerHostPathNotFile => {
                "Select the trainer executable file itself, not a directory."
                    .to_string()
            }
            Self::SteamAppIdRequired => {
                "Use Auto-Populate or enter the game's Steam App ID from Steam or the appmanifest."
                    .to_string()
            }
            Self::SteamCompatDataPathRequired => {
                "Launch the game through Steam once, then use Auto-Populate or browse to the game's compatdata directory."
                    .to_string()
            }
            Self::SteamCompatDataPathMissing => {
                "Launch the game through Steam at least once to create the compatibility data directory."
                    .to_string()
            }
            Self::SteamCompatDataPathNotDirectory => {
                "Select the compatdata folder for the game, not a file inside it."
                    .to_string()
            }
            Self::SteamProtonPathRequired => {
                "Choose the Proton tool Steam should use for this game. Auto-Populate can detect installed Proton versions."
                    .to_string()
            }
            Self::SteamProtonPathMissing => {
                "The configured Proton version may have been removed. Re-select an installed Proton tool or use Auto-Populate."
                    .to_string()
            }
            Self::SteamProtonPathNotExecutable => {
                "Point this field at the Proton 'proton' executable and make sure it still has execute permission."
                    .to_string()
            }
            Self::SteamClientInstallPathRequired => {
                "Set the Steam client install path to your real Steam root, such as ~/.local/share/Steam or ~/.steam/root."
                    .to_string()
            }
            Self::RuntimePrefixPathRequired => {
                "Choose the Proton prefix for this profile. If Steam creates it, launch the game once or use Auto-Populate first."
                    .to_string()
            }
            Self::RuntimePrefixPathMissing => {
                "The saved prefix path no longer exists. Re-select the prefix directory or launch the game once to recreate it."
                    .to_string()
            }
            Self::RuntimePrefixPathNotDirectory => {
                "Select the prefix directory itself, not a file inside it."
                    .to_string()
            }
            Self::RuntimeProtonPathRequired => {
                "Choose the Proton executable that should run this game and trainer."
                    .to_string()
            }
            Self::RuntimeProtonPathMissing => {
                "The configured Proton version may have been removed. Re-select an installed Proton tool."
                    .to_string()
            }
            Self::RuntimeProtonPathNotExecutable => {
                "Point this field at the Proton 'proton' executable and make sure it has execute permission."
                    .to_string()
            }
            Self::UnknownLaunchOptimization(option_id) => {
                format!(
                    "Remove '{option_id}' from the profile or update CrossHook to a version that supports it."
                )
            }
            Self::DuplicateLaunchOptimization(option_id) => {
                format!(
                    "Open Launch Optimizations and keep '{option_id}' selected only once."
                )
            }
            Self::LaunchOptimizationsUnsupportedForMethod(method) => {
                format!(
                    "Switch the profile to 'proton_run' to use launch optimizations, or clear the selected optimizations for '{method}'."
                )
            }
            Self::LaunchOptimizationNotSupportedForMethod { option_id, method } => {
                format!(
                    "Disable '{option_id}' or change the profile to a launch method that supports it instead of '{method}'."
                )
            }
            Self::IncompatibleLaunchOptimizations { first, second } => {
                format!("Disable either '{first}' or '{second}' before launching.")
            }
            Self::LaunchOptimizationDependencyMissing {
                option_id,
                dependency,
            } => {
                format!(
                    "Install '{dependency}' and make sure it is available on PATH, or disable '{option_id}'."
                )
            }
            Self::NativeWindowsExecutableNotSupported => {
                "Switch the profile to 'proton_run' for Windows games, or choose a Linux-native executable."
                    .to_string()
            }
            Self::NativeTrainerLaunchUnsupported => {
                "Use 'steam_applaunch' or 'proton_run' for trainer workflows. Native launch only starts the game executable."
                    .to_string()
            }
            Self::UnsupportedMethod(_) => {
                "Change the profile launch method to 'steam_applaunch', 'proton_run', or 'native'."
                    .to_string()
            }
            Self::CustomEnvVarKeyEmpty => {
                "Remove the empty entry or enter a valid variable name in Profile custom environment variables."
                    .to_string()
            }
            Self::CustomEnvVarKeyContainsEquals => {
                "Edit the key so it is a single token without '='; use separate keys for each assignment."
                    .to_string()
            }
            Self::CustomEnvVarKeyContainsNul => {
                "Remove NUL characters from the key; paste plain text only."
                    .to_string()
            }
            Self::CustomEnvVarValueContainsNul => {
                "Remove NUL characters from the value; paste plain text only."
                    .to_string()
            }
            Self::CustomEnvVarReservedKey(_) => {
                "Remove this key from custom env vars; CrossHook sets WINEPREFIX and Steam compat paths from your profile at launch."
                    .to_string()
            }
            Self::GamescopeBinaryMissing => {
                "Install gamescope from your distribution's package manager.".to_string()
            }
            Self::GamescopeNotSupportedForMethod(_) => {
                "Switch the launch method to proton_run or steam_applaunch to use gamescope."
                    .to_string()
            }
            Self::GamescopeNestedSession => {
                "Enable 'Allow nested' in the gamescope configuration to override.".to_string()
            }
            Self::GamescopeResolutionPairIncomplete { .. } => {
                "Set both dimensions or leave both empty for auto-detection.".to_string()
            }
            Self::GamescopeFsrSharpnessOutOfRange(_) => {
                "Set FSR sharpness to a value between 0 (sharpest) and 20 (softest).".to_string()
            }
            Self::GamescopeFullscreenBorderlessConflict => {
                "Choose either fullscreen or borderless, not both.".to_string()
            }
            Self::OfflineReadinessInsufficient { .. } => {
                "Review trainer files, game paths, and Proton prefix in the profile. This warning is informational; you can still launch."
                    .to_string()
            }
        }
    }

    pub fn severity(&self) -> ValidationSeverity {
        match self {
            Self::GamescopeNestedSession | Self::OfflineReadinessInsufficient { .. } => {
                ValidationSeverity::Warning
            }
            _ => ValidationSeverity::Fatal,
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl Error for ValidationError {}

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
    if RESERVED_CUSTOM_ENV_KEYS
        .iter()
        .any(|reserved| *reserved == trimmed_key)
    {
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
        collect_gamescope_issues(request, gamescope_config, &mut issues);
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

    // Align with `build_steam_launch_options_command`: steam_applaunch uses the same optimization
    // IDs as `proton_run` for the Launch Options prefix (unknown IDs, conflicts, PATH deps).
    resolve_launch_directives_for_method(
        &request.optimizations.enabled_option_ids,
        METHOD_PROTON_RUN,
    )?;

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

    resolve_launch_directives(request)?;

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

    if let Err(e) = resolve_launch_directives_for_method(
        &request.optimizations.enabled_option_ids,
        METHOD_PROTON_RUN,
    ) {
        issues.push(e.issue());
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

    if let Err(e) = resolve_launch_directives(request) {
        issues.push(e.issue());
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
        let path = Path::new(game_path);
        if !path.exists() {
            return Err(ValidationError::GamePathMissing);
        }
        if !path.is_file() {
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

    let trainer_host = Path::new(trainer_host_path);
    if !trainer_host.exists() {
        return Err(ValidationError::TrainerHostPathMissing);
    }
    if !trainer_host.is_file() {
        return Err(ValidationError::TrainerHostPathNotFile);
    }

    Ok(())
}

pub(crate) fn require_directory<'a>(
    value: &'a str,
    required_error: ValidationError,
    missing_error: ValidationError,
    not_directory_error: ValidationError,
) -> Result<&'a Path, ValidationError> {
    if value.is_empty() {
        return Err(required_error);
    }

    let path = Path::new(value);
    if !path.exists() {
        return Err(missing_error);
    }
    if !path.is_dir() {
        return Err(not_directory_error);
    }

    Ok(path)
}

pub(crate) fn require_executable_file(
    value: &str,
    required_error: ValidationError,
    missing_error: ValidationError,
    not_executable_error: ValidationError,
) -> Result<(), ValidationError> {
    if value.is_empty() {
        return Err(required_error);
    }

    let path = Path::new(value);
    if !path.exists() {
        return Err(missing_error);
    }
    if !is_executable_file(path) {
        return Err(not_executable_error);
    }

    Ok(())
}

pub(crate) fn is_executable_file(path: &Path) -> bool {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}

fn looks_like_windows_executable(path: &str) -> bool {
    path.trim().to_ascii_lowercase().ends_with(".exe")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    struct RequestFixture {
        _temp_dir: tempfile::TempDir,
        game_path: String,
        trainer_path: String,
        compatdata_path: String,
        proton_path: String,
        steam_client_install_path: String,
    }

    fn write_executable_file(path: &Path) {
        fs::write(path, b"test").expect("write file");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).expect("chmod");
        }
    }

    fn fixture() -> RequestFixture {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let compatdata = temp_dir.path().join("compat");
        let proton = temp_dir.path().join("proton");
        let game = temp_dir.path().join("game.sh");
        let trainer = temp_dir.path().join("trainer.exe");
        let steam_client = temp_dir.path().join("steam");

        fs::create_dir_all(&compatdata).expect("compatdata dir");
        fs::create_dir_all(&steam_client).expect("steam client dir");
        write_executable_file(&proton);
        write_executable_file(&game);
        fs::write(&trainer, b"trainer").expect("trainer file");

        RequestFixture {
            _temp_dir: temp_dir,
            game_path: game.to_string_lossy().into_owned(),
            trainer_path: trainer.to_string_lossy().into_owned(),
            compatdata_path: compatdata.to_string_lossy().into_owned(),
            proton_path: proton.to_string_lossy().into_owned(),
            steam_client_install_path: steam_client.to_string_lossy().into_owned(),
        }
    }

    fn steam_request() -> (tempfile::TempDir, LaunchRequest) {
        let RequestFixture {
            _temp_dir,
            game_path,
            trainer_path,
            compatdata_path,
            proton_path,
            steam_client_install_path,
        } = fixture();
        (
            _temp_dir,
            LaunchRequest {
                method: METHOD_STEAM_APPLAUNCH.to_string(),
                game_path,
                trainer_path: trainer_path.clone(),
                trainer_host_path: trainer_path,
                trainer_loading_mode: TrainerLoadingMode::SourceDirectory,
                steam: SteamLaunchConfig {
                    app_id: "12345".to_string(),
                    compatdata_path,
                    proton_path,
                    steam_client_install_path,
                },
                runtime: RuntimeLaunchConfig::default(),
                optimizations: LaunchOptimizationsRequest::default(),
                launch_trainer_only: false,
                launch_game_only: false,
                profile_name: None,
                ..Default::default()
            },
        )
    }

    fn proton_request() -> (tempfile::TempDir, LaunchRequest) {
        let (temp_dir, mut request) = steam_request();
        request.method = METHOD_PROTON_RUN.to_string();
        request.game_path = request.game_path.replace("game.sh", "game.exe");
        fs::write(&request.game_path, b"game").expect("game exe");
        request.runtime = RuntimeLaunchConfig {
            prefix_path: request.steam.compatdata_path.clone(),
            proton_path: request.steam.proton_path.clone(),
            working_directory: String::new(),
        };
        request.steam = SteamLaunchConfig::default();
        (temp_dir, request)
    }

    fn native_request() -> (tempfile::TempDir, LaunchRequest) {
        let (temp_dir, mut request) = steam_request();
        request.method = METHOD_NATIVE.to_string();
        request.trainer_path.clear();
        request.trainer_host_path.clear();
        request.steam = SteamLaunchConfig::default();
        (temp_dir, request)
    }

    #[test]
    fn validates_steam_applaunch_request() {
        let (_temp_dir, request) = steam_request();
        assert_eq!(validate(&request), Ok(()));
    }

    #[test]
    fn steam_applaunch_rejects_unknown_launch_optimization() {
        let (_temp_dir, mut request) = steam_request();
        request.optimizations.enabled_option_ids = vec!["unknown_toggle".to_string()];

        assert_eq!(
            validate(&request),
            Err(ValidationError::UnknownLaunchOptimization(
                "unknown_toggle".to_string()
            ))
        );
    }

    #[test]
    fn steam_applaunch_validate_all_collects_launch_optimization_issue() {
        let (_temp_dir, mut request) = steam_request();
        request.optimizations.enabled_option_ids = vec!["unknown_toggle".to_string()];

        let issues = validate_all(&request);
        assert!(
            issues
                .iter()
                .any(|issue| issue.message.contains("unknown_toggle")),
            "expected optimization issue in: {issues:?}"
        );
    }

    #[test]
    fn allows_game_only_steam_launch_without_trainer_paths() {
        let (_temp_dir, mut request) = steam_request();
        request.launch_game_only = true;
        request.trainer_path.clear();
        request.trainer_host_path.clear();

        assert_eq!(validate(&request), Ok(()));
    }

    #[test]
    fn validates_proton_run_request() {
        let (_temp_dir, request) = proton_request();
        assert_eq!(validate(&request), Ok(()));
    }

    #[test]
    fn proton_run_rejects_unknown_launch_optimization() {
        let (_temp_dir, mut request) = proton_request();
        request.optimizations.enabled_option_ids = vec!["unknown_toggle".to_string()];

        assert_eq!(
            validate(&request),
            Err(ValidationError::UnknownLaunchOptimization(
                "unknown_toggle".to_string()
            ))
        );
    }

    #[test]
    fn proton_run_rejects_duplicate_launch_optimizations() {
        let (_temp_dir, mut request) = proton_request();
        request.optimizations.enabled_option_ids = vec![
            "disable_steam_input".to_string(),
            "disable_steam_input".to_string(),
        ];

        assert_eq!(
            validate(&request),
            Err(ValidationError::DuplicateLaunchOptimization(
                "disable_steam_input".to_string()
            ))
        );
    }

    #[test]
    fn proton_run_rejects_conflicting_launch_optimizations() {
        let (_temp_dir, mut request) = proton_request();
        request.optimizations.enabled_option_ids = vec![
            "use_gamemode".to_string(),
            "use_game_performance".to_string(),
        ];

        assert_eq!(
            validate(&request),
            Err(ValidationError::IncompatibleLaunchOptimizations {
                first: "use_gamemode".to_string(),
                second: "use_game_performance".to_string(),
            })
        );
    }

    #[test]
    fn proton_run_requires_runtime_prefix_path() {
        let (_temp_dir, mut request) = proton_request();
        request.runtime.prefix_path.clear();

        assert_eq!(
            validate(&request),
            Err(ValidationError::RuntimePrefixPathRequired)
        );
    }

    #[test]
    fn native_requires_linux_native_executable() {
        let (_temp_dir, mut request) = native_request();
        request.game_path = request.game_path.replace("game.sh", "game.exe");
        fs::write(&request.game_path, b"game").expect("game exe");

        assert_eq!(
            validate(&request),
            Err(ValidationError::NativeWindowsExecutableNotSupported)
        );
    }

    #[test]
    fn native_rejects_trainer_only_launches() {
        let (_temp_dir, mut request) = native_request();
        request.launch_trainer_only = true;

        assert_eq!(
            validate(&request),
            Err(ValidationError::NativeTrainerLaunchUnsupported)
        );
    }

    #[test]
    fn native_rejects_launch_optimizations() {
        let (_temp_dir, mut request) = native_request();
        request.optimizations.enabled_option_ids = vec!["disable_steam_input".to_string()];

        assert_eq!(
            validate(&request),
            Err(ValidationError::LaunchOptimizationsUnsupportedForMethod(
                METHOD_NATIVE.to_string()
            ))
        );
    }

    #[test]
    fn rejects_unsupported_method() {
        let (_temp_dir, mut request) = steam_request();
        request.method = "direct".to_string();

        assert_eq!(
            validate(&request),
            Err(ValidationError::UnsupportedMethod("direct".to_string()))
        );
    }

    #[test]
    fn validation_error_help_explains_missing_steam_compatdata_path() {
        assert_eq!(
            ValidationError::SteamCompatDataPathMissing.help(),
            "Launch the game through Steam at least once to create the compatibility data directory."
        );
    }

    #[test]
    fn validation_error_help_explains_missing_launch_optimization_dependency() {
        assert_eq!(
            ValidationError::LaunchOptimizationDependencyMissing {
                option_id: "use_gamemode".to_string(),
                dependency: "gamemoderun".to_string(),
            }
            .help(),
            "Install 'gamemoderun' and make sure it is available on PATH, or disable 'use_gamemode'."
        );
    }

    #[test]
    fn validation_error_severity_is_fatal_for_current_variants() {
        assert_eq!(
            ValidationError::NativeWindowsExecutableNotSupported.severity(),
            ValidationSeverity::Fatal
        );
        assert_eq!(
            ValidationError::UnsupportedMethod("direct".to_string()).severity(),
            ValidationSeverity::Fatal
        );
    }

    #[test]
    fn validation_error_issue_packages_message_help_and_severity() {
        assert_eq!(
            ValidationError::UnsupportedMethod("direct".to_string()).issue(),
            LaunchValidationIssue {
                message:
                    "Unsupported launch method 'direct'. Use steam_applaunch, proton_run, or native."
                        .to_string(),
                help:
                    "Change the profile launch method to 'steam_applaunch', 'proton_run', or 'native'."
                        .to_string(),
                severity: ValidationSeverity::Fatal,
            }
        );
    }

    #[test]
    fn request_uses_last_path_segment_for_executable_name() {
        let request = LaunchRequest {
            game_path: r"Z:\Games\Test Game\game.exe".to_string(),
            optimizations: LaunchOptimizationsRequest::default(),
            ..LaunchRequest::default()
        };

        assert_eq!(request.game_executable_name(), "game.exe");
    }

    #[test]
    fn log_target_slug_prefers_game_name_for_non_steam_methods() {
        let request = LaunchRequest {
            method: METHOD_PROTON_RUN.to_string(),
            game_path: "/games/Example Game/game.exe".to_string(),
            optimizations: LaunchOptimizationsRequest::default(),
            ..LaunchRequest::default()
        };

        assert_eq!(request.log_target_slug(), "game-exe");
    }

    #[test]
    fn validate_all_returns_empty_for_valid_steam_request() {
        let (_temp_dir, request) = steam_request();
        let issues = validate_all(&request);
        assert!(issues.is_empty(), "expected no issues, got: {issues:?}");
    }

    #[test]
    fn validate_all_collects_multiple_issues() {
        let (_temp_dir, mut request) = steam_request();
        request.game_path.clear();
        request.steam.app_id.clear();
        request.steam.compatdata_path.clear();
        request.steam.proton_path.clear();
        request.steam.steam_client_install_path.clear();

        let issues = validate_all(&request);
        assert!(
            issues.len() >= 4,
            "expected at least 4 issues, got {}: {issues:?}",
            issues.len()
        );

        let messages: Vec<&str> = issues.iter().map(|i| i.message.as_str()).collect();
        assert!(messages.iter().any(|m| m.contains("game executable path")));
        assert!(messages.iter().any(|m| m.contains("Steam App ID")));
        assert!(messages.iter().any(|m| m.contains("compatdata path")));
        assert!(messages.iter().any(|m| m.contains("Proton path")));
    }

    #[test]
    fn validate_all_proton_collects_directive_error_alongside_path_issues() {
        let (_temp_dir, mut request) = proton_request();
        request.runtime.prefix_path.clear();
        request.optimizations.enabled_option_ids = vec!["unknown_toggle".to_string()];

        let issues = validate_all(&request);
        assert!(
            issues.len() >= 2,
            "expected at least 2 issues, got {}: {issues:?}",
            issues.len()
        );

        let messages: Vec<&str> = issues.iter().map(|i| i.message.as_str()).collect();
        assert!(
            messages.iter().any(|m| m.contains("prefix path")),
            "expected prefix path issue in: {messages:?}"
        );
        assert!(
            messages.iter().any(|m| m.contains("unknown_toggle")),
            "expected directive error issue in: {messages:?}"
        );
    }

    #[test]
    fn proton_run_validates_with_custom_env_vars() {
        let (_temp_dir, mut request) = proton_request();
        request
            .custom_env_vars
            .insert("DXVK_ASYNC".to_string(), "1".to_string());
        assert_eq!(validate(&request), Ok(()));
        assert!(validate_all(&request).is_empty());
    }

    #[test]
    fn validate_rejects_reserved_custom_env_key() {
        let (_temp_dir, mut request) = proton_request();
        request
            .custom_env_vars
            .insert("WINEPREFIX".to_string(), "/tmp/evil".to_string());
        assert_eq!(
            validate(&request),
            Err(ValidationError::CustomEnvVarReservedKey(
                "WINEPREFIX".to_string()
            ))
        );
    }

    #[test]
    fn validate_rejects_custom_env_key_with_equals() {
        let (_temp_dir, mut request) = proton_request();
        request
            .custom_env_vars
            .insert("A=B".to_string(), "1".to_string());
        assert_eq!(
            validate(&request),
            Err(ValidationError::CustomEnvVarKeyContainsEquals)
        );
    }

    #[test]
    fn validate_rejects_whitespace_only_custom_env_key() {
        let (_temp_dir, mut request) = proton_request();
        request
            .custom_env_vars
            .insert("   ".to_string(), "1".to_string());
        assert_eq!(
            validate(&request),
            Err(ValidationError::CustomEnvVarKeyEmpty)
        );
    }

    #[test]
    fn validate_rejects_nul_in_custom_env_key_and_value() {
        let (_temp_dir, mut request) = proton_request();
        request
            .custom_env_vars
            .insert("A\0B".to_string(), "1".to_string());
        assert_eq!(
            validate(&request),
            Err(ValidationError::CustomEnvVarKeyContainsNul)
        );

        let (_temp_dir, mut request) = proton_request();
        request
            .custom_env_vars
            .insert("FOO".to_string(), "bar\0baz".to_string());
        assert_eq!(
            validate(&request),
            Err(ValidationError::CustomEnvVarValueContainsNul)
        );
    }

    #[test]
    fn validate_all_collects_multiple_custom_env_issues() {
        let (_temp_dir, mut request) = steam_request();
        request.custom_env_vars = BTreeMap::from([
            ("WINEPREFIX".to_string(), "1".to_string()),
            ("BAD=KEY".to_string(), "1".to_string()),
        ]);
        let issues = validate_all(&request);
        assert_eq!(issues.len(), 2);
    }

    #[test]
    fn offline_readiness_insufficient_is_warning_severity() {
        let err = ValidationError::OfflineReadinessInsufficient {
            score: 40,
            reasons: vec!["missing hash".to_string()],
        };
        assert_eq!(err.severity(), ValidationSeverity::Warning);
        let issue = err.issue();
        assert_eq!(issue.severity, ValidationSeverity::Warning);
        assert!(issue.message.contains("40"));
    }

    #[test]
    fn gamescope_validation_passes_for_steam_applaunch() {
        let (_td, mut request) = steam_request();
        request.gamescope = crate::profile::GamescopeConfig {
            enabled: true,
            internal_width: Some(1920),
            internal_height: Some(1080),
            ..Default::default()
        };
        let issues = validate_all(&request);
        let gamescope_method_issue = issues.iter().any(|i| {
            i.message
                .contains("only supported for proton_run and steam_applaunch")
        });
        assert!(
            !gamescope_method_issue,
            "steam_applaunch should not emit GamescopeNotSupportedForMethod"
        );
    }

    #[test]
    fn gamescope_validation_rejected_for_native() {
        let (_td, mut request) = native_request();
        request.gamescope = crate::profile::GamescopeConfig {
            enabled: true,
            ..Default::default()
        };
        let issues = validate_all(&request);
        let gamescope_method_issue = issues.iter().any(|i| {
            i.message
                .contains("only supported for proton_run and steam_applaunch")
        });
        assert!(
            gamescope_method_issue,
            "native method should emit GamescopeNotSupportedForMethod"
        );
    }

    #[test]
    fn trainer_only_validation_uses_trainer_gamescope_before_main_gamescope() {
        let (_td, mut request) = proton_request();
        request.launch_trainer_only = true;
        request.launch_game_only = false;
        request.gamescope = crate::profile::GamescopeConfig::default();
        request.trainer_gamescope = Some(crate::profile::GamescopeConfig {
            enabled: true,
            internal_width: Some(1920),
            internal_height: None,
            ..Default::default()
        });

        let issues = validate_all(&request);
        assert!(
            issues
                .iter()
                .any(|issue| issue.message.contains("Both width and height must be set for internal resolution.")),
            "expected trainer gamescope validation issue in: {issues:?}"
        );
    }
}
