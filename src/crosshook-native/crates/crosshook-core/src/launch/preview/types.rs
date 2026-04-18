use serde::Serialize;

use crate::launch::request::{
    LaunchRequest, LaunchValidationIssue, METHOD_NATIVE, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
};
use crate::profile::TrainerLoadingMode;

/// Source category for a preview environment variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvVarSource {
    /// Core Proton runtime vars (STEAM_COMPAT_DATA_PATH, WINEPREFIX, etc.)
    ProtonRuntime,
    /// Vars from launch optimization toggles (PROTON_NO_STEAMINPUT, etc.)
    LaunchOptimization,
    /// Passthrough from host (HOME, DISPLAY, PATH, etc.)
    Host,
    /// Steam-specific Proton vars for steam_applaunch
    SteamProton,
    /// Profile `launch.custom_env_vars` (wins over launch optimizations on key conflict)
    ProfileCustom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolvedLaunchMethod {
    SteamApplaunch,
    ProtonRun,
    Native,
}

impl ResolvedLaunchMethod {
    pub(super) fn from_request(request: &LaunchRequest) -> Self {
        match request.resolved_method() {
            METHOD_STEAM_APPLAUNCH => Self::SteamApplaunch,
            METHOD_PROTON_RUN => Self::ProtonRun,
            METHOD_NATIVE => Self::Native,
            _ => Self::Native,
        }
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::SteamApplaunch => METHOD_STEAM_APPLAUNCH,
            Self::ProtonRun => METHOD_PROTON_RUN,
            Self::Native => METHOD_NATIVE,
        }
    }
}

/// A single environment variable that will be set during launch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreviewEnvVar {
    pub key: String,
    pub value: String,
    pub source: EnvVarSource,
}

/// Proton runtime setup details (non-native methods only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProtonSetup {
    pub wine_prefix_path: String,
    pub compat_data_path: String,
    pub steam_client_install_path: String,
    pub proton_executable: String,
    /// Path to `umu-run` if it will be used for this launch, otherwise `None`.
    pub umu_run_path: Option<String>,
}

/// Trainer configuration details for the preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreviewTrainerInfo {
    pub path: String,
    pub host_path: String,
    pub loading_mode: TrainerLoadingMode,
    /// The Windows-side path when copy_to_prefix mode is used.
    pub staged_path: Option<String>,
}

/// Validation summary for the preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreviewValidation {
    pub issues: Vec<LaunchValidationIssue>,
}

/// Complete dry-run preview result returned to the frontend.
///
/// Sections that depend on independent computations use `Option<T>` so
/// the preview can return partial results when one section fails (e.g.,
/// directive resolution fails but validation and game info are still useful).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LaunchPreview {
    /// The effective launch method after inference.
    pub resolved_method: ResolvedLaunchMethod,

    /// All validation results collected (not short-circuited).
    pub validation: PreviewValidation,

    /// Environment variables that will be set.
    /// None when environment collection fails (e.g., directive resolution error).
    pub environment: Option<Vec<PreviewEnvVar>>,

    /// WINE/Proton env vars that will be cleared before launch.
    pub cleared_variables: Vec<String>,

    /// Wrapper command chain (e.g. ["mangohud", "gamemoderun"]).
    /// None when directive resolution fails.
    pub wrappers: Option<Vec<String>>,

    /// Human-readable effective command string.
    /// None when directive resolution fails.
    pub effective_command: Option<String>,

    /// Error message when directive resolution or command building fails.
    /// Allows the frontend to show what went wrong alongside partial results.
    pub directives_error: Option<String>,

    /// Steam Launch Options %command% string (steam_applaunch only).
    pub steam_launch_options: Option<String>,

    /// Proton environment setup details.
    pub proton_setup: Option<ProtonSetup>,

    /// Resolved working directory.
    pub working_directory: String,

    /// Full game executable path.
    pub game_executable: String,

    /// Just the file name portion.
    pub game_executable_name: String,

    /// Trainer details (None for game-only or native launches).
    pub trainer: Option<PreviewTrainerInfo>,

    /// ISO 8601 timestamp when the preview was generated.
    pub generated_at: String,

    /// Pre-rendered display text for clipboard copy.
    pub display_text: String,

    /// Whether gamescope will be active for this launch.
    pub gamescope_active: bool,

    /// Diagnostic: how the umu decision was resolved for this preview.
    /// Only populated for `proton_run` method; `None` for `steam_applaunch` and `native`.
    pub umu_decision: Option<UmuDecisionPreview>,
}

/// Explains why the preview will (or will not) invoke `umu-run`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UmuDecisionPreview {
    /// The `umu_preference` value the backend received on the request.
    pub requested_preference: crate::settings::UmuPreference,
    /// `Some(path)` when `umu-run` was discovered on the backend's `PATH`.
    pub umu_run_path_on_backend_path: Option<String>,
    /// Final decision: `true` → emit `umu-run`, `false` → emit direct Proton.
    pub will_use_umu: bool,
    /// Human-readable reason the preview modal can surface.
    pub reason: String,
    /// Coverage status of the profile's app id in the umu-database CSV.
    /// `Unknown` when no app id is available or no CSV source is reachable.
    pub csv_coverage: crate::umu_database::CsvCoverage,
}
