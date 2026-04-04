use serde::{Deserialize, Serialize};

/// State of a prefix dependency package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DependencyState {
    #[default]
    Unknown,
    Installed,
    Missing,
    InstallFailed,
    CheckFailed,
    UserSkipped,
}

/// Status of a single prefix dependency for IPC/UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefixDependencyStatus {
    pub package_name: String,
    pub state: DependencyState,
    pub checked_at: Option<String>,
    pub installed_at: Option<String>,
    pub last_error: Option<String>,
}

/// Result of detecting winetricks/protontricks binary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryDetectionResult {
    pub found: bool,
    pub binary_path: Option<String>,
    pub binary_name: String,
    pub tool_type: Option<PrefixDepsTool>,
    pub source: String,
}

/// Resolved dependency tool identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrefixDepsTool {
    Winetricks,
    Protontricks,
}

/// Errors specific to prefix dependency operations.
#[derive(Debug)]
pub enum PrefixDepsError {
    BinaryNotFound { tool: String },
    PrefixNotInitialized { path: String },
    ValidationError(String),
    ProcessFailed { exit_code: Option<i32>, stderr: String },
    Timeout { seconds: u64 },
    AlreadyInstalling { prefix_path: String },
    Database { action: &'static str, source: rusqlite::Error },
}

impl std::fmt::Display for PrefixDepsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BinaryNotFound { tool } => write!(f, "{tool} binary not found"),
            Self::PrefixNotInitialized { path } => {
                write!(f, "WINE prefix not initialized at {path}")
            }
            Self::ValidationError(msg) => write!(f, "validation error: {msg}"),
            Self::ProcessFailed { exit_code, .. } => {
                write!(
                    f,
                    "process failed (exit code: {exit_code:?}): <stderr omitted>"
                )
            }
            Self::Timeout { seconds } => write!(f, "operation timed out after {seconds}s"),
            Self::AlreadyInstalling { prefix_path } => {
                write!(f, "installation already in progress for prefix: {prefix_path}")
            }
            Self::Database { action, source } => {
                write!(f, "database error during {action}: {source}")
            }
        }
    }
}

impl std::error::Error for PrefixDepsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Database { source, .. } => Some(source),
            _ => None,
        }
    }
}
