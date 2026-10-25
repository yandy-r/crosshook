//! Fatal bootstrap errors; degraded optional features stay in the report.

use crate::AppError;
use std::fmt;

/// Required persistent store that failed to initialize.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreKind {
    /// User preferences.
    Settings,
    /// Game profiles.
    Profiles,
    /// Recently used files.
    RecentFiles,
    /// Community tap subscriptions.
    CommunityTaps,
}

impl fmt::Display for StoreKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Settings => "settings",
            Self::Profiles => "profiles",
            Self::RecentFiles => "recent files",
            Self::CommunityTaps => "community taps",
        })
    }
}

/// A fatal failure preventing application startup. Never exits the process.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BootstrapError {
    /// A required store is unavailable.
    #[error("{store} store could not be initialized: {detail}")]
    StoreInit {
        /// Store that failed.
        store: StoreKind,
        /// Raw diagnostic for logs, not direct UI display.
        detail: String,
    },
    /// The application runtime could not be created.
    #[error("application runtime unavailable: {0}")]
    Runtime(AppError),
}

impl BootstrapError {
    /// Home-path-redacted message suitable for a shell's fatal-error dialog.
    pub fn user_message(&self) -> String {
        crate::presenter::sanitize_message(&self.to_string())
    }
}

#[cfg(test)]
mod tests;
