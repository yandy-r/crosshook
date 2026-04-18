use std::path::PathBuf;

use crate::protonup::{ProtonUpInstallErrorKind, ProtonUpInstallResult};

/// Internal install error — richer than `ProtonUpInstallResult` for use inside
/// the orchestrator. Converted to `ProtonUpInstallResult` at the public boundary.
#[derive(Debug)]
pub enum InstallError {
    InvalidPath(String),
    PermissionDenied(String),
    NetworkError(String),
    ChecksumMissing(String),
    ChecksumFailed(String),
    AlreadyInstalled { path: PathBuf },
    DependencyMissing { reason: String },
    NoWritableInstallRoot,
    Cancelled,
    UntrustedUrl(String),
    Unknown(String),
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPath(message) => write!(f, "invalid path: {message}"),
            Self::PermissionDenied(message) => write!(f, "permission denied: {message}"),
            Self::NetworkError(message) => write!(f, "network error: {message}"),
            Self::ChecksumMissing(message) => write!(f, "checksum missing: {message}"),
            Self::ChecksumFailed(message) => write!(f, "checksum failed: {message}"),
            Self::AlreadyInstalled { path } => {
                write!(f, "already installed at {}", path.display())
            }
            Self::DependencyMissing { reason } => write!(f, "dependency missing: {reason}"),
            Self::NoWritableInstallRoot => {
                write!(
                    f,
                    "no writable Proton install root found; install Steam first"
                )
            }
            Self::Cancelled => write!(f, "install cancelled"),
            Self::UntrustedUrl(message) => write!(f, "untrusted URL: {message}"),
            Self::Unknown(message) => write!(f, "unknown error: {message}"),
        }
    }
}

impl InstallError {
    pub(super) fn to_result(&self) -> ProtonUpInstallResult {
        match self {
            Self::InvalidPath(message) => err(message, ProtonUpInstallErrorKind::InvalidPath),
            Self::PermissionDenied(message) => {
                err(message, ProtonUpInstallErrorKind::PermissionDenied)
            }
            Self::NetworkError(message) => err(message, ProtonUpInstallErrorKind::NetworkError),
            Self::ChecksumMissing(message) | Self::ChecksumFailed(message) => {
                err(message, ProtonUpInstallErrorKind::ChecksumFailed)
            }
            Self::AlreadyInstalled { path } => ProtonUpInstallResult {
                success: false,
                installed_path: Some(path.to_string_lossy().to_string()),
                error_kind: Some(ProtonUpInstallErrorKind::AlreadyInstalled),
                error_message: Some(format!("already installed at {}", path.display())),
            },
            Self::DependencyMissing { reason } => {
                err(reason, ProtonUpInstallErrorKind::DependencyMissing)
            }
            Self::NoWritableInstallRoot => err(
                "no writable Proton install root found; install Steam first",
                ProtonUpInstallErrorKind::InvalidPath,
            ),
            Self::Cancelled => err("install cancelled", ProtonUpInstallErrorKind::Cancelled),
            Self::UntrustedUrl(message) => err(message, ProtonUpInstallErrorKind::NetworkError),
            Self::Unknown(message) => err(message, ProtonUpInstallErrorKind::Unknown),
        }
    }
}

pub(super) fn err(
    message: impl Into<String>,
    kind: ProtonUpInstallErrorKind,
) -> ProtonUpInstallResult {
    ProtonUpInstallResult {
        success: false,
        installed_path: None,
        error_kind: Some(kind),
        error_message: Some(message.into()),
    }
}

pub(super) fn map_io_err(io_err: std::io::Error, context: &str) -> InstallError {
    if io_err.kind() == std::io::ErrorKind::PermissionDenied {
        InstallError::PermissionDenied(format!("{context}: {io_err}"))
    } else {
        InstallError::Unknown(format!("{context}: {io_err}"))
    }
}

pub(super) fn network_err(message: impl std::fmt::Display) -> InstallError {
    InstallError::NetworkError(message.to_string())
}
