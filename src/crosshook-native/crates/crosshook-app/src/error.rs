//! Sanitized presenter-level errors.

use crate::presenter::sanitize_message;

/// Stable machine-readable category for an [`AppError`]; the human message stays private.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppErrorKind {
    /// Caller input failed validation; the `field` names the offending input.
    Validation {
        /// Offending input field.
        field: &'static str,
    },
    /// Referenced item does not exist.
    NotFound,
    /// Item already exists or conflicts with existing state.
    Conflict,
    /// A store, host tool, or backend is unavailable.
    Unavailable,
    /// I/O or OS-level failure.
    Io,
    /// Work was cancelled (caller drop or shutdown race).
    Cancelled,
    /// Unexpected failure that maps to nothing more specific.
    Internal,
}

/// Presenter-facing error: a stable [`AppErrorKind`] plus a sanitized message.
/// Never embeds raw store paths, SQL, or panic payloads.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct AppError {
    kind: AppErrorKind,
    message: String,
}

impl AppError {
    fn new(kind: AppErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: sanitize_message(&message.into()),
        }
    }

    /// Creates a cancelled error for task shutdown races.
    pub(crate) fn cancelled() -> Self {
        Self::new(
            AppErrorKind::Cancelled,
            "task was cancelled during shutdown",
        )
    }

    /// Builds a validation error for `field`.
    pub fn validation(field: &'static str, message: impl Into<String>) -> Self {
        Self::new(AppErrorKind::Validation { field }, message)
    }

    /// Returns the stable error category.
    pub fn kind(&self) -> AppErrorKind {
        self.kind
    }

    /// Returns the sanitized human-readable message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl From<tokio::task::JoinError> for AppError {
    fn from(error: tokio::task::JoinError) -> Self {
        if error.is_cancelled() {
            Self::new(AppErrorKind::Cancelled, "background task was cancelled")
        } else {
            // Never surface raw panic payloads; they may embed user paths.
            Self::new(AppErrorKind::Internal, "background task failed")
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::new(AppErrorKind::Io, error.kind().to_string())
    }
}

impl From<crosshook_core::profile::ProfileStoreError> for AppError {
    fn from(error: crosshook_core::profile::ProfileStoreError) -> Self {
        use crosshook_core::profile::ProfileStoreError as E;
        match error {
            E::InvalidName(_)
            | E::InvalidLaunchOptimizationId(_)
            | E::InvalidCommandArgumentId(_)
            | E::InvalidLaunchPresetName(_)
            | E::ReservedLaunchPresetName(_) => Self::new(
                AppErrorKind::Validation { field: "profile" },
                error.to_string(),
            ),
            E::CommandArgumentValidation(validation) => Self::from(validation),
            E::NotFound(_) | E::LaunchPresetNotFound(_) => {
                Self::new(AppErrorKind::NotFound, error.to_string())
            }
            E::AlreadyExists(_) => Self::new(AppErrorKind::Conflict, error.to_string()),
            // `Io` may embed absolute paths; keep only the kind.
            E::Io(io) => Self::new(AppErrorKind::Io, io.kind().to_string()),
            E::TomlDe(_) | E::TomlSer(_) => Self::new(
                AppErrorKind::Validation { field: "profile" },
                error.to_string(),
            ),
        }
    }
}

impl From<crosshook_core::settings::SettingsStoreError> for AppError {
    fn from(error: crosshook_core::settings::SettingsStoreError) -> Self {
        use crosshook_core::settings::SettingsStoreError as E;
        match error {
            // `Io` may embed absolute paths; keep only the kind.
            E::Io(io) => Self::new(AppErrorKind::Io, io.kind().to_string()),
            E::TomlDe(_) | E::TomlSer(_) => Self::new(
                AppErrorKind::Validation { field: "settings" },
                error.to_string(),
            ),
        }
    }
}

impl From<crosshook_core::metadata::MetadataStoreError> for AppError {
    fn from(error: crosshook_core::metadata::MetadataStoreError) -> Self {
        use crosshook_core::metadata::MetadataStoreError as E;
        // Fixed strings only: Display impls may embed paths or SQL detail.
        match error {
            E::HomeDirectoryUnavailable => {
                Self::new(AppErrorKind::Unavailable, "home directory unavailable")
            }
            E::Validation(_) => Self::new(
                AppErrorKind::Validation { field: "metadata" },
                "metadata validation failed",
            ),
            E::SymlinkDetected(_) => Self::new(
                AppErrorKind::Validation { field: "metadata" },
                "invalid metadata path",
            ),
            E::Corrupt(_) => Self::new(AppErrorKind::Internal, "metadata database is corrupt"),
            E::Database { .. } => {
                Self::new(AppErrorKind::Internal, "metadata database operation failed")
            }
            E::Io { .. } => Self::new(AppErrorKind::Io, "metadata storage I/O failed"),
            E::NewerSchema { .. }
            | E::ReadOnlyNewerSchema
            | E::SQLiteReadonly
            | E::ReadOnlyDisabled { .. } => Self::new(
                AppErrorKind::Unavailable,
                "metadata database is unavailable",
            ),
        }
    }
}

impl From<crosshook_core::launch::ValidationError> for AppError {
    fn from(error: crosshook_core::launch::ValidationError) -> Self {
        Self::new(
            AppErrorKind::Validation { field: "launch" },
            error.to_string(),
        )
    }
}

#[cfg(test)]
mod tests;
