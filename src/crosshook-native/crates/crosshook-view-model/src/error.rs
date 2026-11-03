//! Service error mirror used by every view-model.
//!
//! [`ServiceError`] mirrors `crosshook_app::AppError` so the default build
//! needs no `crosshook-app` (which pulls in tokio). With the `app` feature,
//! `From<&AppError>` converts one exhaustively.

/// Stable category of a [`ServiceError`]; mirrors `crosshook_app::AppErrorKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServiceErrorKind {
    /// Caller input failed validation; `field` names the backend input id.
    Validation {
        /// Backend field id, e.g. `"profiles_directory"`.
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

/// Error returned by a service call, as a view-model sees it.
///
/// `message` is already sanitized by `crosshook-app`; view-models show it
/// verbatim and never re-sanitize (that would read `$HOME`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceError {
    /// Stable category.
    pub kind: ServiceErrorKind,
    /// Sanitized, user-safe message.
    pub message: String,
}

impl ServiceError {
    /// Builds an error of `kind` with `message`.
    pub fn new(kind: ServiceErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// Builds a validation error for backend field id `field`.
    pub fn validation(field: &'static str, message: impl Into<String>) -> Self {
        Self::new(ServiceErrorKind::Validation { field }, message)
    }
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ServiceError {}

#[cfg(feature = "app")]
impl From<&crosshook_app::AppError> for ServiceError {
    fn from(error: &crosshook_app::AppError) -> Self {
        use crosshook_app::AppErrorKind as K;
        // Exhaustive on purpose: a new AppErrorKind variant must fail to
        // compile here instead of silently mapping to something else.
        let kind = match error.kind() {
            K::Validation { field } => ServiceErrorKind::Validation { field },
            K::NotFound => ServiceErrorKind::NotFound,
            K::Conflict => ServiceErrorKind::Conflict,
            K::Unavailable => ServiceErrorKind::Unavailable,
            K::Io => ServiceErrorKind::Io,
            K::Cancelled => ServiceErrorKind::Cancelled,
            K::Internal => ServiceErrorKind::Internal,
        };
        Self::new(kind, error.message())
    }
}

#[cfg(feature = "app")]
impl From<crosshook_app::AppError> for ServiceError {
    fn from(error: crosshook_app::AppError) -> Self {
        Self::from(&error)
    }
}

#[cfg(all(test, feature = "app"))]
mod tests;
