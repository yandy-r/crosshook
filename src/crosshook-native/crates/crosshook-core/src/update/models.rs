use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UpdateGameRequest {
    #[serde(default)]
    pub profile_name: String,
    #[serde(default)]
    pub updater_path: String,
    #[serde(default)]
    pub proton_path: String,
    #[serde(default)]
    pub prefix_path: String,
    #[serde(default)]
    pub steam_client_install_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UpdateGameResult {
    #[serde(default)]
    pub succeeded: bool,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub helper_log_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateGameError {
    Validation(UpdateGameValidationError),
    RuntimeUnavailable,
    LogAttachmentFailed { path: PathBuf, message: String },
    UpdaterSpawnFailed { message: String },
    UpdaterWaitFailed { message: String },
    UpdaterExitedWithFailure { status: Option<i32> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateGameValidationError {
    UpdaterPathRequired,
    UpdaterPathMissing,
    UpdaterPathNotFile,
    UpdaterPathNotWindowsExecutable,
    ProtonPathRequired,
    ProtonPathMissing,
    ProtonPathNotExecutable,
    PrefixPathRequired,
    PrefixPathMissing,
    PrefixPathNotDirectory,
}

impl UpdateGameValidationError {
    pub fn message(&self) -> String {
        match self {
            Self::UpdaterPathRequired => "The updater executable path is required.".to_string(),
            Self::UpdaterPathMissing => "The updater executable path does not exist.".to_string(),
            Self::UpdaterPathNotFile => "The updater executable path must be a file.".to_string(),
            Self::UpdaterPathNotWindowsExecutable => {
                "The updater executable path must point to a Windows .exe file.".to_string()
            }
            Self::ProtonPathRequired => "A Proton path is required.".to_string(),
            Self::ProtonPathMissing => "The Proton path does not exist.".to_string(),
            Self::ProtonPathNotExecutable => {
                "The Proton path does not point to an executable file.".to_string()
            }
            Self::PrefixPathRequired => "A prefix path is required.".to_string(),
            Self::PrefixPathMissing => "The prefix path does not exist.".to_string(),
            Self::PrefixPathNotDirectory => "The prefix path must be a directory.".to_string(),
        }
    }
}

impl fmt::Display for UpdateGameValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl Error for UpdateGameValidationError {}

impl UpdateGameError {
    pub fn message(&self) -> String {
        match self {
            Self::Validation(error) => error.message(),
            Self::RuntimeUnavailable => {
                "Update execution requires a Tokio runtime, but none was available.".to_string()
            }
            Self::LogAttachmentFailed { path, message } => {
                format!(
                    "Failed to attach updater logs to '{}': {message}",
                    path.display()
                )
            }
            Self::UpdaterSpawnFailed { message } => {
                format!("Failed to launch the updater through Proton: {message}")
            }
            Self::UpdaterWaitFailed { message } => {
                format!("Failed to monitor the updater process: {message}")
            }
            Self::UpdaterExitedWithFailure { status } => match status {
                Some(code) => format!(
                    "The updater exited unsuccessfully with status code {code}. Review the log file for details."
                ),
                None => {
                    "The updater exited unsuccessfully. Review the log file for details."
                        .to_string()
                }
            },
        }
    }
}

impl fmt::Display for UpdateGameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl Error for UpdateGameError {}

impl From<UpdateGameValidationError> for UpdateGameError {
    fn from(value: UpdateGameValidationError) -> Self {
        Self::Validation(value)
    }
}
