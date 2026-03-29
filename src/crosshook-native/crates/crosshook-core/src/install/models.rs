use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::profile::{
    GameProfile, GameSection, LaunchSection, RuntimeSection, SteamSection, TrainerLoadingMode,
    TrainerSection,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct InstallGameRequest {
    #[serde(default)]
    pub profile_name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub installer_path: String,
    #[serde(default)]
    pub trainer_path: String,
    #[serde(default)]
    pub proton_path: String,
    #[serde(default)]
    pub prefix_path: String,
    #[serde(default)]
    pub installed_game_executable_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct InstallGameResult {
    #[serde(default)]
    pub succeeded: bool,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub helper_log_path: String,
    #[serde(default)]
    pub profile_name: String,
    #[serde(default)]
    pub needs_executable_confirmation: bool,
    #[serde(default)]
    pub discovered_game_executable_candidates: Vec<String>,
    #[serde(default)]
    pub profile: GameProfile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallGameError {
    Validation(InstallGameValidationError),
    RuntimeUnavailable,
    HomeDirectoryUnavailable,
    PrefixPathExistsAsFile { path: PathBuf },
    PrefixCreationFailed { path: PathBuf, message: String },
    LogAttachmentFailed { path: PathBuf, message: String },
    InstallerSpawnFailed { message: String },
    InstallerWaitFailed { message: String },
    InstallerExitedWithFailure { status: Option<i32> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallGameValidationError {
    ProfileNameRequired,
    ProfileNameInvalid,
    InstallerPathRequired,
    InstallerPathMissing,
    InstallerPathNotFile,
    InstallerPathNotWindowsExecutable,
    TrainerPathMissing,
    TrainerPathNotFile,
    ProtonPathRequired,
    ProtonPathMissing,
    ProtonPathNotExecutable,
    PrefixPathRequired,
    PrefixPathMissing,
    PrefixPathNotDirectory,
    InstalledGameExecutablePathMissing,
    InstalledGameExecutablePathNotFile,
}

impl InstallGameRequest {
    pub fn resolved_profile_name(&self) -> &str {
        self.profile_name.trim()
    }

    pub fn resolved_display_name(&self) -> &str {
        let display_name = self.display_name.trim();
        if !display_name.is_empty() {
            return display_name;
        }

        self.resolved_profile_name()
    }

    pub fn reviewable_profile(&self, prefix_path: &Path) -> GameProfile {
        GameProfile {
            game: GameSection {
                name: self.resolved_display_name().to_string(),
                executable_path: String::new(),
            },
            trainer: TrainerSection {
                path: self.trainer_path.trim().to_string(),
                kind: String::new(),
                loading_mode: TrainerLoadingMode::SourceDirectory,
            },
            injection: Default::default(),
            steam: SteamSection::default(),
            runtime: RuntimeSection {
                prefix_path: prefix_path.to_string_lossy().into_owned(),
                proton_path: self.proton_path.trim().to_string(),
                working_directory: String::new(),
            },
            launch: LaunchSection {
                method: "proton_run".to_string(),
                ..Default::default()
            },
            local_override: crate::profile::LocalOverrideSection::default(),
        }
    }
}

impl InstallGameError {
    pub fn message(&self) -> String {
        match self {
            Self::Validation(error) => error.message(),
            Self::RuntimeUnavailable => {
                "Install execution requires a Tokio runtime, but none was available.".to_string()
            }
            Self::HomeDirectoryUnavailable => {
                "CrossHook could not resolve the default prefix path because the home directory is unavailable.".to_string()
            }
            Self::PrefixPathExistsAsFile { path } => {
                format!(
                    "The selected prefix path '{}' already exists as a file. Choose a directory path instead.",
                    path.display()
                )
            }
            Self::PrefixCreationFailed { path, message } => {
                format!(
                    "Failed to create the prefix directory '{}': {message}",
                    path.display()
                )
            }
            Self::LogAttachmentFailed { path, message } => {
                format!("Failed to attach installer logs to '{}': {message}", path.display())
            }
            Self::InstallerSpawnFailed { message } => {
                format!("Failed to launch the installer through Proton: {message}")
            }
            Self::InstallerWaitFailed { message } => {
                format!("Failed to monitor the installer process: {message}")
            }
            Self::InstallerExitedWithFailure { status } => match status {
                Some(code) => format!(
                    "The installer exited unsuccessfully with status code {code}. Review the log file for details."
                ),
                None => {
                    "The installer exited unsuccessfully. Review the log file for details."
                        .to_string()
                }
            },
        }
    }
}

impl InstallGameValidationError {
    pub fn message(&self) -> String {
        match self {
            Self::ProfileNameRequired => "An install profile name is required.".to_string(),
            Self::ProfileNameInvalid => {
                "The install profile name contains invalid characters.".to_string()
            }
            Self::InstallerPathRequired => "An installer path is required.".to_string(),
            Self::InstallerPathMissing => "The installer path does not exist.".to_string(),
            Self::InstallerPathNotFile => "The installer path must be a file.".to_string(),
            Self::InstallerPathNotWindowsExecutable => {
                "The installer path must point to a Windows .exe file.".to_string()
            }
            Self::TrainerPathMissing => "The trainer path does not exist.".to_string(),
            Self::TrainerPathNotFile => "The trainer path must be a file.".to_string(),
            Self::ProtonPathRequired => "A Proton path is required.".to_string(),
            Self::ProtonPathMissing => "The Proton path does not exist.".to_string(),
            Self::ProtonPathNotExecutable => "The Proton path must be executable.".to_string(),
            Self::PrefixPathRequired => "A prefix path is required.".to_string(),
            Self::PrefixPathMissing => "The prefix path does not exist.".to_string(),
            Self::PrefixPathNotDirectory => "The prefix path must be a directory.".to_string(),
            Self::InstalledGameExecutablePathMissing => {
                "The final game executable path does not exist.".to_string()
            }
            Self::InstalledGameExecutablePathNotFile => {
                "The final game executable path must be a file.".to_string()
            }
        }
    }
}

impl fmt::Display for InstallGameValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl Error for InstallGameValidationError {}

impl fmt::Display for InstallGameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl Error for InstallGameError {}

impl From<InstallGameValidationError> for InstallGameError {
    fn from(value: InstallGameValidationError) -> Self {
        Self::Validation(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn reviewable_profile_uses_install_details_without_persisting_runtime_target() {
        let temp_dir = tempdir().expect("temp dir");
        let prefix_path = temp_dir.path().join("prefix");

        let request = InstallGameRequest {
            profile_name: "god-of-war-ragnarok".to_string(),
            display_name: "God of War Ragnarok".to_string(),
            installer_path: "/mnt/media/setup.exe".to_string(),
            trainer_path: "/mnt/trainers/gowr.exe".to_string(),
            proton_path: "/home/user/.steam/root/steamapps/common/Proton - Experimental/proton"
                .to_string(),
            prefix_path: prefix_path.to_string_lossy().into_owned(),
            installed_game_executable_path: String::new(),
        };

        let profile = request.reviewable_profile(&prefix_path);

        assert_eq!(profile.game.name, "God of War Ragnarok");
        assert!(profile.game.executable_path.is_empty());
        assert_eq!(profile.trainer.path, "/mnt/trainers/gowr.exe");
        assert_eq!(
            profile.runtime.prefix_path,
            prefix_path.to_string_lossy().into_owned()
        );
        assert_eq!(
            profile.runtime.proton_path,
            "/home/user/.steam/root/steamapps/common/Proton - Experimental/proton"
        );
        assert!(profile.runtime.working_directory.is_empty());
        assert_eq!(profile.launch.method, "proton_run");
    }
}
