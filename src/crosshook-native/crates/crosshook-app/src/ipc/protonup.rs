//! `protonup:install:progress` event of a supervised Proton install.

use crosshook_core::protonup::progress::ProtonInstallProgress;

use super::WireEvent;

/// Event carrying one install progress snapshot.
pub const PROTONUP_INSTALL_PROGRESS_EVENT: &str = "protonup:install:progress";

/// The wire event for one progress snapshot.
pub fn progress_event(progress: &ProtonInstallProgress) -> Option<WireEvent> {
    WireEvent::new(PROTONUP_INSTALL_PROGRESS_EVENT, progress)
}

#[cfg(test)]
mod tests;
