//! Fixed, path-free user-facing texts and the pure mappers that choose them.
//!
//! Underlying error details are logged with `tracing` by the callers; nothing
//! here ever formats a filesystem path or URL from an error value.

use crosshook_core::protonup::install::InstallError;
use crosshook_core::protonup::progress::Phase;
use crosshook_core::protonup::uninstall::UninstallError;
use crosshook_core::protonup::{
    ProtonUpInstallErrorKind, ProtonUpInstallRequest, ProtonUpInstallResult,
};

use super::identity::IdentityError;

pub const MSG_OFFLINE: &str = "offline mode is enabled";
pub const MSG_OP_NOT_FOUND: &str = "install operation was not found";
pub const MSG_INSTALL_FAILED: &str = "Proton install failed";
pub const MSG_INSTALL_CANCELLED: &str = "Proton install cancelled";
pub const UNINSTALL_SYSTEM: &str = "refusing to delete a system path";
pub const UNINSTALL_OUTSIDE: &str = "path is not inside a known Proton install root";
pub const UNINSTALL_NOT_FOUND: &str = "Proton tool was not found";
pub const UNINSTALL_FAILED: &str = "failed to remove the Proton tool";

/// Validation text (field `version`) for a failed catalog identity check.
pub fn identity_message(error: IdentityError, request: &ProtonUpInstallRequest) -> String {
    match error {
        IdentityError::Missing => format!(
            "requested version '{}' for provider '{}' was not found in the server catalog",
            request.version, request.provider
        ),
        IdentityError::Mismatch => {
            "install request/version mismatch with server catalog identity".to_string()
        }
        IdentityError::MetadataMismatch => {
            "install metadata did not match server-resolved catalog entry".to_string()
        }
    }
}

/// Fixed text for an uninstall refusal or failure.
pub fn uninstall_error_text(error: &UninstallError) -> &'static str {
    match error {
        UninstallError::SystemPathRefused(_) => UNINSTALL_SYSTEM,
        UninstallError::PathOutsideKnownRoots(_) => UNINSTALL_OUTSIDE,
        UninstallError::NotFound(_) => UNINSTALL_NOT_FOUND,
        UninstallError::Io(_) => UNINSTALL_FAILED,
    }
}

/// Structured legacy install result for `error`, with the same kind core
/// would report and a fixed, path-free message.
pub fn install_error_result(error: &InstallError) -> ProtonUpInstallResult {
    let (kind, message) = match error {
        InstallError::InvalidPath(_) => (
            ProtonUpInstallErrorKind::InvalidPath,
            "install path is invalid",
        ),
        InstallError::PermissionDenied(_) => (
            ProtonUpInstallErrorKind::PermissionDenied,
            "permission denied while installing",
        ),
        InstallError::NetworkError(_) | InstallError::UntrustedUrl(_) => (
            ProtonUpInstallErrorKind::NetworkError,
            "network error while installing",
        ),
        InstallError::ChecksumMissing(_) | InstallError::ChecksumFailed(_) => (
            ProtonUpInstallErrorKind::ChecksumFailed,
            "checksum verification failed",
        ),
        InstallError::AlreadyInstalled { path } => {
            return ProtonUpInstallResult {
                success: false,
                installed_path: Some(path.to_string_lossy().into_owned()),
                error_kind: Some(ProtonUpInstallErrorKind::AlreadyInstalled),
                error_message: Some("Proton version is already installed".to_string()),
            };
        }
        InstallError::DependencyMissing { .. } => (
            ProtonUpInstallErrorKind::DependencyMissing,
            "install dependency is missing",
        ),
        InstallError::NoWritableInstallRoot => (
            ProtonUpInstallErrorKind::InvalidPath,
            "no writable Proton install root found; install Steam first",
        ),
        InstallError::Cancelled => (ProtonUpInstallErrorKind::Cancelled, "install cancelled"),
        InstallError::Unknown(_) => (ProtonUpInstallErrorKind::Unknown, "install failed"),
    };
    ProtonUpInstallResult {
        success: false,
        installed_path: None,
        error_kind: Some(kind),
        error_message: Some(message.to_string()),
    }
}

/// Message to forward with a progress event of `phase`.
///
/// `Failed` and `Cancelled` always use the fixed texts. Other phases keep the
/// `original` message only when it contains no `/` (so no path or URL leaks).
pub fn progress_message(phase: &Phase, original: Option<&str>) -> Option<String> {
    match phase {
        Phase::Failed => Some(MSG_INSTALL_FAILED.to_string()),
        Phase::Cancelled => Some(MSG_INSTALL_CANCELLED.to_string()),
        Phase::Resolving
        | Phase::Downloading
        | Phase::Verifying
        | Phase::Extracting
        | Phase::Finalizing
        | Phase::Done => original
            .filter(|message| !message.contains('/'))
            .map(str::to_string),
    }
}
