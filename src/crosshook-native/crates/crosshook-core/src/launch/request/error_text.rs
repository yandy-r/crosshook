use super::error::ValidationError;

impl ValidationError {
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
            Self::UnshareNetUnavailable => {
                "Network isolation (unshare --net) is not available on this system.".to_string()
            }
            Self::OfflineReadinessInsufficient { score, reasons } => {
                let detail = if reasons.is_empty() {
                    String::new()
                } else {
                    format!(" {}", reasons.join("; "))
                };
                format!("Offline readiness score is {score}/100 (below 60).{detail}")
            }
            Self::LowDiskSpaceAdvisory {
                available_mb,
                threshold_mb,
                ..
            } => format!(
                "Low disk space detected: {available_mb} MiB available (recommended minimum {threshold_mb} MiB)."
            ),
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
            Self::UnshareNetUnavailable => {
                "Kernel policy or missing capabilities may block unshare --net on this system. The trainer will launch without network isolation."
                    .to_string()
            }
            Self::OfflineReadinessInsufficient { .. } => {
                "Review trainer files, game paths, and Proton prefix in the profile. This warning is informational; you can still launch."
                    .to_string()
            }
            Self::LowDiskSpaceAdvisory { mount_path, .. } => {
                format!(
                    "Free up space on the filesystem backing '{mount_path}' before launching to reduce crash and staging failures. This warning is informational."
                )
            }
        }
    }
}
