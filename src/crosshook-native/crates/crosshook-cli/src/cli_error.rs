use std::error::Error;

// Exit code constants — EXIT_USAGE_ERROR is handled by clap directly;
// EXIT_SUCCESS and EXIT_STEAM_NOT_FOUND are reserved for future use.
#[allow(dead_code)]
pub(crate) const EXIT_SUCCESS: i32 = 0;
pub(crate) const EXIT_GENERAL_ERROR: i32 = 1;
#[allow(dead_code)]
pub(crate) const EXIT_USAGE_ERROR: i32 = 2;
pub(crate) const EXIT_PROFILE_NOT_FOUND: i32 = 3;
pub(crate) const EXIT_LAUNCH_FAILURE: i32 = 4;
#[allow(dead_code)]
pub(crate) const EXIT_STEAM_NOT_FOUND: i32 = 5;

pub(crate) enum CliError {
    ProfileNotFound(String),
    LaunchFailure(String),
    General(String),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProfileNotFound(msg) => write!(f, "{msg}"),
            Self::LaunchFailure(msg) => write!(f, "{msg}"),
            Self::General(msg) => write!(f, "{msg}"),
        }
    }
}

impl CliError {
    pub(crate) fn exit_code(&self) -> i32 {
        match self {
            Self::ProfileNotFound(_) => EXIT_PROFILE_NOT_FOUND,
            Self::LaunchFailure(_) => EXIT_LAUNCH_FAILURE,
            Self::General(_) => EXIT_GENERAL_ERROR,
        }
    }
}

impl From<Box<dyn Error>> for CliError {
    fn from(error: Box<dyn Error>) -> Self {
        Self::General(error.to_string())
    }
}

impl From<String> for CliError {
    fn from(msg: String) -> Self {
        Self::General(msg)
    }
}

impl From<&str> for CliError {
    fn from(msg: &str) -> Self {
        Self::General(msg.to_string())
    }
}

impl From<serde_json::Error> for CliError {
    fn from(error: serde_json::Error) -> Self {
        Self::General(error.to_string())
    }
}

impl From<crosshook_core::export::diagnostics::DiagnosticBundleError> for CliError {
    fn from(error: crosshook_core::export::diagnostics::DiagnosticBundleError) -> Self {
        Self::General(error.to_string())
    }
}
