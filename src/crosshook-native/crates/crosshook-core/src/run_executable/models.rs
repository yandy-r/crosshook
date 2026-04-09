use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RunExecutableRequest {
    #[serde(default)]
    pub executable_path: String,
    #[serde(default)]
    pub proton_path: String,
    /// Optional. Empty string means "auto-resolve a throwaway prefix under
    /// `~/.local/share/crosshook/prefixes/_run-adhoc/<slug>`".
    #[serde(default)]
    pub prefix_path: String,
    #[serde(default)]
    pub working_directory: String,
    #[serde(default)]
    pub steam_client_install_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RunExecutableResult {
    /// `true` once the Proton wrapper has been *spawned* successfully.
    /// This field intentionally does NOT reflect the eventual exit status of
    /// the Windows executable — that arrives asynchronously via the
    /// `run-executable-complete` event after the wrapper child exits. The UI
    /// uses this acknowledgement only to know that the spawn handshake
    /// succeeded and the log/prefix paths below are valid.
    #[serde(default)]
    pub succeeded: bool,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub helper_log_path: String,
    /// Echoes the actual prefix path used to launch the executable so the UI
    /// can surface the auto-resolved location when the user left
    /// `prefix_path` empty.
    #[serde(default)]
    pub resolved_prefix_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunExecutableError {
    Validation(RunExecutableValidationError),
    RuntimeUnavailable,
    LogAttachmentFailed { path: PathBuf, message: String },
    PrefixCreationFailed { path: PathBuf, message: String },
    HomeDirectoryUnavailable,
    RunnerSpawnFailed { message: String },
    RunnerWaitFailed { message: String },
    RunnerExitedWithFailure { status: Option<i32> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunExecutableValidationError {
    ExecutablePathRequired,
    ExecutablePathMissing,
    ExecutablePathNotFile,
    ExecutablePathNotWindowsExecutable,
    ProtonPathRequired,
    ProtonPathMissing,
    ProtonPathNotExecutable,
    PrefixPathMissing,
    PrefixPathNotDirectory,
}

impl RunExecutableValidationError {
    pub fn message(&self) -> String {
        match self {
            Self::ExecutablePathRequired => "The executable path is required.".to_string(),
            Self::ExecutablePathMissing => "The executable path does not exist.".to_string(),
            Self::ExecutablePathNotFile => "The executable path must be a file.".to_string(),
            Self::ExecutablePathNotWindowsExecutable => {
                "The executable path must point to a Windows .exe or .msi file.".to_string()
            }
            Self::ProtonPathRequired => "A Proton path is required.".to_string(),
            Self::ProtonPathMissing => "The Proton path does not exist.".to_string(),
            Self::ProtonPathNotExecutable => {
                "The Proton path does not point to an executable file.".to_string()
            }
            Self::PrefixPathMissing => "The prefix path does not exist.".to_string(),
            Self::PrefixPathNotDirectory => "The prefix path must be a directory.".to_string(),
        }
    }

    /// Field name (matches the snake_case fields on
    /// [`RunExecutableRequest`]) that this validation error attaches to.
    /// Returned to the frontend so it can render field-level error UI
    /// without parsing message text.
    pub fn field(&self) -> &'static str {
        match self {
            Self::ExecutablePathRequired
            | Self::ExecutablePathMissing
            | Self::ExecutablePathNotFile
            | Self::ExecutablePathNotWindowsExecutable => "executable_path",
            Self::ProtonPathRequired | Self::ProtonPathMissing | Self::ProtonPathNotExecutable => {
                "proton_path"
            }
            Self::PrefixPathMissing | Self::PrefixPathNotDirectory => "prefix_path",
        }
    }
}

impl fmt::Display for RunExecutableValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl Error for RunExecutableValidationError {}

impl RunExecutableError {
    pub fn message(&self) -> String {
        match self {
            Self::Validation(error) => error.message(),
            Self::RuntimeUnavailable => {
                "Run-executable execution requires a Tokio runtime, but none was available."
                    .to_string()
            }
            Self::LogAttachmentFailed { path, message } => {
                format!(
                    "Failed to attach run-executable logs to '{}': {message}",
                    path.display()
                )
            }
            Self::PrefixCreationFailed { path, message } => {
                format!(
                    "Failed to create the throwaway Proton prefix at '{}': {message}",
                    path.display()
                )
            }
            Self::HomeDirectoryUnavailable => {
                "Unable to determine a home directory for the throwaway Proton prefix.".to_string()
            }
            Self::RunnerSpawnFailed { message } => {
                format!("Failed to launch the executable through Proton: {message}")
            }
            Self::RunnerWaitFailed { message } => {
                format!("Failed to monitor the run-executable process: {message}")
            }
            Self::RunnerExitedWithFailure { status } => match status {
                Some(code) => format!(
                    "The executable exited unsuccessfully with status code {code}. Review the log file for details."
                ),
                None => {
                    "The executable exited unsuccessfully. Review the log file for details."
                        .to_string()
                }
            },
        }
    }
}

impl fmt::Display for RunExecutableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl Error for RunExecutableError {}

impl From<RunExecutableValidationError> for RunExecutableError {
    fn from(value: RunExecutableValidationError) -> Self {
        Self::Validation(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_request_through_serde() {
        let original = RunExecutableRequest {
            executable_path: "/mnt/media/setup.exe".to_string(),
            proton_path: "/proton/proton".to_string(),
            prefix_path: String::new(),
            working_directory: "/mnt/media".to_string(),
            steam_client_install_path: String::new(),
        };

        let json = serde_json::to_string(&original).expect("serialize request");
        let parsed: RunExecutableRequest =
            serde_json::from_str(&json).expect("deserialize request");

        assert_eq!(parsed, original);
    }

    #[test]
    fn deserializes_request_with_missing_optional_fields() {
        let json = r#"{"executable_path":"/x/setup.exe","proton_path":"/proton/proton"}"#;
        let parsed: RunExecutableRequest =
            serde_json::from_str(json).expect("deserialize partial request");

        assert_eq!(parsed.executable_path, "/x/setup.exe");
        assert_eq!(parsed.proton_path, "/proton/proton");
        assert!(parsed.prefix_path.is_empty());
        assert!(parsed.working_directory.is_empty());
        assert!(parsed.steam_client_install_path.is_empty());
    }

    #[test]
    fn validation_error_messages_are_user_facing() {
        // The frontend renders these strings verbatim from the
        // structured `RunCommandError::Validation { message, .. }`
        // payload returned by the Tauri command layer; it does NOT
        // round-trip them through any JS-side constants table.
        assert_eq!(
            RunExecutableValidationError::ExecutablePathRequired.message(),
            "The executable path is required."
        );
        assert_eq!(
            RunExecutableValidationError::ExecutablePathMissing.message(),
            "The executable path does not exist."
        );
        assert_eq!(
            RunExecutableValidationError::ExecutablePathNotFile.message(),
            "The executable path must be a file."
        );
        assert_eq!(
            RunExecutableValidationError::ExecutablePathNotWindowsExecutable.message(),
            "The executable path must point to a Windows .exe or .msi file."
        );
        assert_eq!(
            RunExecutableValidationError::ProtonPathRequired.message(),
            "A Proton path is required."
        );
        assert_eq!(
            RunExecutableValidationError::ProtonPathMissing.message(),
            "The Proton path does not exist."
        );
        assert_eq!(
            RunExecutableValidationError::ProtonPathNotExecutable.message(),
            "The Proton path does not point to an executable file."
        );
        assert_eq!(
            RunExecutableValidationError::PrefixPathMissing.message(),
            "The prefix path does not exist."
        );
        assert_eq!(
            RunExecutableValidationError::PrefixPathNotDirectory.message(),
            "The prefix path must be a directory."
        );
    }

    #[test]
    fn validation_error_converts_into_run_executable_error() {
        let validation = RunExecutableValidationError::ExecutablePathRequired;
        let wrapped: RunExecutableError = validation.clone().into();
        assert_eq!(wrapped, RunExecutableError::Validation(validation));
    }

    #[test]
    fn validation_error_field_matches_request_field_names() {
        assert_eq!(
            RunExecutableValidationError::ExecutablePathRequired.field(),
            "executable_path"
        );
        assert_eq!(
            RunExecutableValidationError::ExecutablePathMissing.field(),
            "executable_path"
        );
        assert_eq!(
            RunExecutableValidationError::ExecutablePathNotFile.field(),
            "executable_path"
        );
        assert_eq!(
            RunExecutableValidationError::ExecutablePathNotWindowsExecutable.field(),
            "executable_path"
        );
        assert_eq!(
            RunExecutableValidationError::ProtonPathRequired.field(),
            "proton_path"
        );
        assert_eq!(
            RunExecutableValidationError::ProtonPathMissing.field(),
            "proton_path"
        );
        assert_eq!(
            RunExecutableValidationError::ProtonPathNotExecutable.field(),
            "proton_path"
        );
        assert_eq!(
            RunExecutableValidationError::PrefixPathMissing.field(),
            "prefix_path"
        );
        assert_eq!(
            RunExecutableValidationError::PrefixPathNotDirectory.field(),
            "prefix_path"
        );
    }
}
