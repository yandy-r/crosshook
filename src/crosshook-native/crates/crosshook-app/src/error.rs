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
    pub(crate) fn new(kind: AppErrorKind, message: impl Into<String>) -> Self {
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

    /// Maps `MetadataStoreError::Validation(msg)` from the collections, mods
    /// and version stores to `Validation { field }` while keeping `msg`
    /// (user-entered names only; no paths). The message equals
    /// `MetadataStoreError::Validation`'s `Display` text so adapters reproduce
    /// today's IPC strings; [`AppError::new`] still sanitizes it. Every other
    /// variant delegates to the fixed-message `From<MetadataStoreError>`.
    pub(crate) fn from_metadata_validation(
        field: &'static str,
        error: crosshook_core::metadata::MetadataStoreError,
    ) -> Self {
        use crosshook_core::metadata::MetadataStoreError as E;
        match error {
            E::Validation(message) => Self::new(
                AppErrorKind::Validation { field },
                format!("metadata validation error: {message}"),
            ),
            other => Self::from(other),
        }
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
            // Fixed strings only: TOML Display echoes source lines, which
            // hold user paths that may live outside `$HOME`.
            E::TomlDe(error) => {
                tracing::warn!(%error, "profile TOML parse failed");
                Self::new(
                    AppErrorKind::Validation { field: "profile" },
                    "profile file is not valid TOML",
                )
            }
            E::TomlSer(error) => {
                tracing::warn!(%error, "profile TOML serialization failed");
                Self::new(
                    AppErrorKind::Validation { field: "profile" },
                    "failed to serialize profile",
                )
            }
        }
    }
}

impl From<crosshook_core::profile::CommunityExchangeError> for AppError {
    fn from(error: crosshook_core::profile::CommunityExchangeError) -> Self {
        use crosshook_core::profile::CommunityExchangeError as E;
        match error {
            // The manifest path is the caller's input; leave it out.
            E::Io {
                action, message, ..
            } => Self::new(AppErrorKind::Io, format!("failed to {action}: {message}")),
            E::Json { message, .. } => Self::new(
                AppErrorKind::Validation {
                    field: "community_profile",
                },
                format!("failed to parse community profile: {message}"),
            ),
            E::InvalidManifest { .. } | E::UnsupportedSchemaVersion { .. } => Self::new(
                AppErrorKind::Validation {
                    field: "community_profile",
                },
                error.to_string(),
            ),
            E::ProfileStore { message } => Self::new(AppErrorKind::Io, message),
        }
    }
}

impl From<crosshook_core::settings::SettingsStoreError> for AppError {
    fn from(error: crosshook_core::settings::SettingsStoreError) -> Self {
        use crosshook_core::settings::SettingsStoreError as E;
        // Fixed strings only: TOML Display echoes source lines, which hold
        // user paths that may live outside `$HOME`.
        match error {
            // `Io` may embed absolute paths; keep only the kind.
            E::Io(io) => Self::new(AppErrorKind::Io, io.kind().to_string()),
            E::TomlDe(_) => Self::new(
                AppErrorKind::Validation { field: "settings" },
                "settings file is not valid TOML",
            ),
            E::TomlSer(_) => Self::new(AppErrorKind::Internal, "failed to serialize settings"),
        }
    }
}

impl From<crosshook_core::settings::RecentFilesStoreError> for AppError {
    fn from(error: crosshook_core::settings::RecentFilesStoreError) -> Self {
        use crosshook_core::settings::RecentFilesStoreError as E;
        // Fixed strings only: TOML Display echoes source lines (recent paths).
        match error {
            // `Io` may embed absolute paths; keep only the kind.
            E::Io(io) => Self::new(AppErrorKind::Io, io.kind().to_string()),
            E::TomlDe(_) => Self::new(
                AppErrorKind::Validation {
                    field: "recent_files",
                },
                "recent files data is not valid TOML",
            ),
            E::TomlSer(_) => Self::new(AppErrorKind::Internal, "failed to serialize recent files"),
        }
    }
}

impl From<crosshook_core::export::DiagnosticBundleError> for AppError {
    fn from(error: crosshook_core::export::DiagnosticBundleError) -> Self {
        use crosshook_core::export::DiagnosticBundleError as E;
        // Fixed strings only: `Io` carries the absolute path and archive
        // messages may echo entry paths.
        match error {
            E::Io { action, source, .. } => Self::new(
                AppErrorKind::Io,
                format!("failed to {action}: {}", source.kind()),
            ),
            E::Archive(_) => Self::new(
                AppErrorKind::Internal,
                "failed to write the diagnostic archive",
            ),
            E::ProfileStore(_) => Self::new(
                AppErrorKind::Internal,
                "failed to read profiles for the diagnostic bundle",
            ),
        }
    }
}

impl From<crosshook_core::fs_util::WorkspacePathError> for AppError {
    fn from(error: crosshook_core::fs_util::WorkspacePathError) -> Self {
        use crosshook_core::fs_util::WorkspacePathError as E;
        // Fixed strings only: the candidate path is caller input.
        match error {
            E::InvalidRoot | E::Traversal | E::OutsideWorkspace => Self::new(
                AppErrorKind::Validation { field: "path" },
                "path is not inside the allowed workspace",
            ),
            E::NotFound => Self::new(AppErrorKind::NotFound, "path was not found"),
            E::Io { .. } => Self::new(AppErrorKind::Io, "failed to resolve path"),
        }
    }
}

impl From<crosshook_core::community::CommunityTapError> for AppError {
    fn from(error: crosshook_core::community::CommunityTapError) -> Self {
        use crosshook_core::community::CommunityTapError as E;
        match error {
            // User-entered tap fields only; no host paths.
            E::EmptyTapUrl
            | E::InvalidTapUrl(_)
            | E::InvalidBranch(_)
            | E::InvalidPinnedCommit(_) => {
                Self::new(AppErrorKind::Validation { field: "tap" }, error.to_string())
            }
            // Fixed strings: Io/Git carry workspace paths, argv and stderr.
            E::Io { .. } | E::Git { .. } => {
                Self::new(AppErrorKind::Io, "failed to sync community tap")
            }
            E::Index(_) => Self::new(AppErrorKind::Io, "failed to index community taps"),
            E::Cancelled => Self::new(AppErrorKind::Cancelled, "community tap sync was cancelled"),
        }
    }
}

impl From<crosshook_core::profile::LutrisImportError> for AppError {
    fn from(error: crosshook_core::profile::LutrisImportError) -> Self {
        use crosshook_core::profile::LutrisImportError as E;
        // Fixed strings only: variants embed Lutris library paths.
        match error {
            E::Io { .. } => Self::new(AppErrorKind::Io, "failed to read Lutris library"),
            E::Yaml { .. } => Self::new(
                AppErrorKind::Validation { field: "lutris" },
                "failed to parse Lutris config",
            ),
            E::Pga { .. } => Self::new(AppErrorKind::Io, "failed to read Lutris pga.db"),
            E::ProfileStore { .. } => Self::new(AppErrorKind::Io, "failed to access profiles"),
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
            E::IntegrityCheckFailed { .. } => Self::new(
                AppErrorKind::Validation { field: "revision" },
                "config revision integrity check failed",
            ),
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
