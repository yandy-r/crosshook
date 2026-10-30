//! Install-game: request validation, the default prefix path and the
//! installer run.
//!
//! [`InstallService::start`] validates and provisions through core
//! (`prepare_install`), then runs the lifecycle-bound installer command as a
//! supervised `install-game` job that holds the prefix's exclusivity key
//! (D5). The job's completion carries the reviewable [`InstallGameResult`]
//! or the failure text; app shutdown ends the installer (D3).

mod start;

use std::path::PathBuf;

use crosshook_core::install::{
    install_default_prefix_path, validate_install_request, InstallGameError, InstallGameRequest,
    InstallGameValidationError,
};

use super::run_blocking;
use crate::jobs::{JobRegistry, LOG_DIR};
use crate::{AppError, AppErrorKind};

/// Registry kind for installer jobs.
pub const INSTALL_GAME_JOB_KIND: &str = "install-game";

/// Toolkit-independent install-game operations.
#[derive(Clone)]
pub struct InstallService {
    jobs: JobRegistry,
    log_dir: PathBuf,
}

impl InstallService {
    /// Builds the service over the shared job registry; installer logs go to
    /// the shared log directory.
    pub fn new(jobs: JobRegistry) -> Self {
        Self {
            jobs,
            log_dir: PathBuf::from(LOG_DIR),
        }
    }

    /// Builds the service with installer logs created in `log_dir`.
    #[cfg(test)]
    pub(crate) fn with_log_dir(jobs: JobRegistry, log_dir: PathBuf) -> Self {
        Self { jobs, log_dir }
    }

    #[cfg(test)]
    /// The job registry the service's jobs run on.
    pub(crate) fn jobs(&self) -> &JobRegistry {
        &self.jobs
    }

    /// The default prefix path for `profile_name`
    /// (`<prefix root>/<slugified name>`).
    ///
    /// # Errors
    /// `Validation { field: "profile_name" }` for an empty or invalid name;
    /// `Unavailable` when the prefix root cannot be resolved.
    pub async fn default_prefix_path(&self, profile_name: String) -> Result<String, AppError> {
        run_blocking(move || {
            install_default_prefix_path(&profile_name)
                .map(|path| path.to_string_lossy().into_owned())
                .map_err(install_error)
        })
        .await
    }

    /// Validates `request` without provisioning anything.
    ///
    /// # Errors
    /// `Validation { field }` naming the first invalid input, with core's
    /// message.
    pub async fn validate(&self, request: InstallGameRequest) -> Result<(), AppError> {
        run_blocking(move || validate_install_request(&request).map_err(validation_error)).await
    }
}

static_assertions::assert_impl_all!(InstallService: Send, Sync);

/// Maps a core install error to an [`AppError`] carrying core's message.
fn install_error(error: InstallGameError) -> AppError {
    let message = error.message();
    match error {
        InstallGameError::Validation(validation) => validation_error(validation),
        InstallGameError::PrefixPathExistsAsFile { .. } => {
            AppError::validation("prefix_path", message)
        }
        InstallGameError::HomeDirectoryUnavailable => {
            AppError::new(AppErrorKind::Unavailable, message)
        }
        InstallGameError::PrefixCreationFailed { .. }
        | InstallGameError::InstallerSpawnFailed { .. }
        | InstallGameError::InstallerExitedWithFailure { .. } => {
            AppError::new(AppErrorKind::Io, message)
        }
    }
}

/// Maps a core validation error to `Validation { field }` with its message.
fn validation_error(error: InstallGameValidationError) -> AppError {
    AppError::validation(validation_field(&error), error.message())
}

/// The request field a validation error is about.
fn validation_field(error: &InstallGameValidationError) -> &'static str {
    use InstallGameValidationError as E;
    match error {
        E::ProfileNameRequired | E::ProfileNameInvalid => "profile_name",
        E::InstallerPathRequired
        | E::InstallerPathNotAbsolute
        | E::InstallerPathMissing
        | E::InstallerPathNotFile
        | E::InstallerPathNotWindowsExecutable => "installer_path",
        E::TrainerPathMissing | E::TrainerPathNotFile => "trainer_path",
        E::ProtonPathRequired
        | E::ProtonPathNotAbsolute
        | E::ProtonPathMissing
        | E::ProtonPathNotExecutable => "proton_path",
        E::PrefixPathRequired
        | E::PrefixPathNotAbsolute
        | E::PrefixPathMissing
        | E::PrefixPathNotDirectory => "prefix_path",
        E::InstalledGameExecutablePathMissing | E::InstalledGameExecutablePathNotFile => {
            "installed_game_executable_path"
        }
        E::CustomCoverArtPathMissing | E::CustomCoverArtPathNotFile => "custom_cover_art_path",
        E::CustomPortraitArtPathMissing | E::CustomPortraitArtPathNotFile => {
            "custom_portrait_art_path"
        }
        E::CustomBackgroundArtPathMissing | E::CustomBackgroundArtPathNotFile => {
            "custom_background_art_path"
        }
    }
}

#[cfg(test)]
mod tests;
