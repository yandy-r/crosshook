use std::error::Error;
use std::fmt;

use super::issues::{LaunchValidationIssue, ValidationSeverity};

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
    /// `unshare --net` was requested but is not available on this system.
    UnshareNetUnavailable,
    /// Available disk space at the launch prefix mount is below warning threshold.
    LowDiskSpaceAdvisory {
        available_mb: u64,
        threshold_mb: u64,
        mount_path: String,
    },
}

impl ValidationError {
    /// Returns a stable snake_case identifier for this error variant.
    ///
    /// **Frontend coupling**: consumed by `src/utils/mapValidationToNode.ts` (prefix matching).
    /// When adding a new variant, update that file's mapping table. Unmapped codes default to
    /// the `'launch'` summary node.
    pub fn code(&self) -> &'static str {
        match self {
            Self::GamePathRequired => "game_path_required",
            Self::GamePathMissing => "game_path_missing",
            Self::GamePathNotFile => "game_path_not_file",
            Self::TrainerPathRequired => "trainer_path_required",
            Self::TrainerHostPathRequired => "trainer_host_path_required",
            Self::TrainerHostPathMissing => "trainer_host_path_missing",
            Self::TrainerHostPathNotFile => "trainer_host_path_not_file",
            Self::SteamAppIdRequired => "steam_app_id_required",
            Self::SteamCompatDataPathRequired => "steam_compat_data_path_required",
            Self::SteamCompatDataPathMissing => "steam_compat_data_path_missing",
            Self::SteamCompatDataPathNotDirectory => "steam_compat_data_path_not_directory",
            Self::SteamProtonPathRequired => "steam_proton_path_required",
            Self::SteamProtonPathMissing => "steam_proton_path_missing",
            Self::SteamProtonPathNotExecutable => "steam_proton_path_not_executable",
            Self::SteamClientInstallPathRequired => "steam_client_install_path_required",
            Self::RuntimePrefixPathRequired => "runtime_prefix_path_required",
            Self::RuntimePrefixPathMissing => "runtime_prefix_path_missing",
            Self::RuntimePrefixPathNotDirectory => "runtime_prefix_path_not_directory",
            Self::RuntimeProtonPathRequired => "runtime_proton_path_required",
            Self::RuntimeProtonPathMissing => "runtime_proton_path_missing",
            Self::RuntimeProtonPathNotExecutable => "runtime_proton_path_not_executable",
            Self::UnknownLaunchOptimization(_) => "unknown_launch_optimization",
            Self::DuplicateLaunchOptimization(_) => "duplicate_launch_optimization",
            Self::LaunchOptimizationsUnsupportedForMethod(_) => {
                "launch_optimizations_unsupported_for_method"
            }
            Self::LaunchOptimizationNotSupportedForMethod { .. } => {
                "launch_optimization_not_supported_for_method"
            }
            Self::IncompatibleLaunchOptimizations { .. } => "incompatible_launch_optimizations",
            Self::LaunchOptimizationDependencyMissing { .. } => {
                "launch_optimization_dependency_missing"
            }
            Self::NativeWindowsExecutableNotSupported => "native_windows_executable_not_supported",
            Self::NativeTrainerLaunchUnsupported => "native_trainer_launch_unsupported",
            Self::UnsupportedMethod(_) => "unsupported_method",
            Self::CustomEnvVarKeyEmpty => "custom_env_var_key_empty",
            Self::CustomEnvVarKeyContainsEquals => "custom_env_var_key_contains_equals",
            Self::CustomEnvVarKeyContainsNul => "custom_env_var_key_contains_nul",
            Self::CustomEnvVarValueContainsNul => "custom_env_var_value_contains_nul",
            Self::CustomEnvVarReservedKey(_) => "custom_env_var_reserved_key",
            Self::GamescopeBinaryMissing => "gamescope_binary_missing",
            Self::GamescopeNotSupportedForMethod(_) => "gamescope_not_supported_for_method",
            Self::GamescopeNestedSession => "gamescope_nested_session",
            Self::GamescopeResolutionPairIncomplete { .. } => {
                "gamescope_resolution_pair_incomplete"
            }
            Self::GamescopeFsrSharpnessOutOfRange(_) => "gamescope_fsr_sharpness_out_of_range",
            Self::GamescopeFullscreenBorderlessConflict => {
                "gamescope_fullscreen_borderless_conflict"
            }
            Self::OfflineReadinessInsufficient { .. } => "offline_readiness_insufficient",
            Self::UnshareNetUnavailable => "unshare_net_unavailable",
            Self::LowDiskSpaceAdvisory { .. } => "low_disk_space_advisory",
        }
    }

    pub fn issue(&self) -> LaunchValidationIssue {
        LaunchValidationIssue {
            message: self.message(),
            help: self.help(),
            severity: self.severity(),
            code: Some(self.code().to_string()),
            trainer_hash_stored: None,
            trainer_hash_current: None,
            trainer_sha256_community: None,
        }
    }

    pub fn severity(&self) -> ValidationSeverity {
        match self {
            Self::GamescopeNestedSession
            | Self::UnshareNetUnavailable
            | Self::OfflineReadinessInsufficient { .. }
            | Self::LowDiskSpaceAdvisory { .. } => ValidationSeverity::Warning,
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
