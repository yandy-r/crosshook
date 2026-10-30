use std::fs;
use std::path::{Path, PathBuf};

use tokio::process::Command;

use crate::launch::request::{path_exists_visible_or_host, path_is_executable_visible_or_host};
use crate::launch::runtime_helpers::proton_command::build_direct_proton_command_lifecycle_with;
use crate::launch::runtime_helpers::{
    apply_working_directory, host_environment_map, merge_runtime_proton_into_map,
    resolve_effective_working_directory,
};
use crate::platform::{
    normalize_flatpak_host_path, normalized_path_exists_on_host, normalized_path_is_dir,
    normalized_path_is_dir_on_host, normalized_path_is_file,
};
use crate::profile::validate_name;

use super::discovery::discover_game_executable_candidates;
use super::models::{
    InstallGameError, InstallGameRequest, InstallGameResult, InstallGameValidationError,
};

/// An installer command ready to spawn, plus the provisioned prefix it targets.
///
/// The command is lifecycle-bound (`--watch-bus` under Flatpak) and has no
/// stdio attached; the caller owns stdio, process group, spawn and supervision.
#[derive(Debug)]
pub struct PreparedInstall {
    pub command: Command,
    pub prefix_path: PathBuf,
}

const DEFAULT_PREFIX_ROOT_SEGMENT: &str = "crosshook/prefixes";

pub fn resolve_default_prefix_path(profile_name: &str) -> Result<PathBuf, InstallGameError> {
    validate_profile_name(profile_name)?;
    Ok(resolve_prefix_root()?.join(slugify_profile_name(profile_name)))
}

pub fn validate_install_request(
    request: &InstallGameRequest,
) -> Result<(), InstallGameValidationError> {
    validate_profile_name(request.resolved_profile_name())?;
    validate_installer_path(request.installer_path.trim())?;
    validate_optional_trainer_path(request.trainer_path.trim())?;
    validate_optional_custom_cover_art_path(request.custom_cover_art_path.trim())?;
    validate_optional_custom_portrait_art_path(request.custom_portrait_art_path.trim())?;
    validate_optional_custom_background_art_path(request.custom_background_art_path.trim())?;
    validate_proton_path(request.proton_path.trim())?;
    validate_prefix_path(request.prefix_path.trim())?;
    validate_optional_installed_game_executable_path(
        request.installed_game_executable_path.trim(),
    )?;

    Ok(())
}

pub fn install_default_prefix_path(profile_name: &str) -> Result<PathBuf, InstallGameError> {
    resolve_default_prefix_path(profile_name)
}

/// Validates the request, provisions the prefix and builds the installer
/// command (`<proton> run <installer>`) without attaching stdio.
pub fn prepare_install(request: &InstallGameRequest) -> Result<PreparedInstall, InstallGameError> {
    validate_install_request(request)?;

    let prefix_path = PathBuf::from(normalize_flatpak_host_path(&request.prefix_path));
    provision_prefix(&prefix_path)?;
    let command = build_install_command(request, &prefix_path, crate::platform::is_flatpak());
    Ok(PreparedInstall {
        command,
        prefix_path,
    })
}

/// Builds the reviewable result after the installer exited successfully:
/// discovers executables in the prefix and prepares the generated profile.
pub fn finish_install(
    request: &InstallGameRequest,
    prefix_path: &Path,
    log_path: &Path,
) -> InstallGameResult {
    let discovered_game_executable_candidates = discover_game_executable_candidates(
        prefix_path,
        request.resolved_profile_name(),
        request.resolved_display_name(),
        normalize_flatpak_host_path(&request.installer_path).trim(),
    );
    let confirmed_game_executable_path =
        resolve_confirmed_game_executable_path(request, &discovered_game_executable_candidates);
    let profile = build_reviewable_profile(
        request,
        prefix_path,
        confirmed_game_executable_path.as_deref(),
    );

    InstallGameResult {
        succeeded: true,
        message: "Installer completed. Review the generated profile.".to_string(),
        helper_log_path: log_path.to_string_lossy().into_owned(),
        profile_name: request.resolved_profile_name().to_string(),
        needs_executable_confirmation: true,
        discovered_game_executable_candidates: discovered_game_executable_candidates
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect(),
        profile,
    }
}

fn build_install_command(
    request: &InstallGameRequest,
    prefix_path: &Path,
    flatpak: bool,
) -> Command {
    let normalized_installer_path = normalize_flatpak_host_path(&request.installer_path);
    let mut env = host_environment_map();
    let prefix_path_string = prefix_path.to_string_lossy().into_owned();
    merge_runtime_proton_into_map(&mut env, &prefix_path_string, "");
    let effective_working_directory =
        resolve_effective_working_directory("", Path::new(normalized_installer_path.trim()));
    let mut command = build_direct_proton_command_lifecycle_with(
        request.proton_path.trim(),
        &env,
        effective_working_directory.as_deref(),
        flatpak,
    );
    command.arg(normalized_installer_path.trim());
    if !flatpak {
        apply_working_directory(
            &mut command,
            "",
            Path::new(normalized_installer_path.trim()),
        );
    }
    command
}

fn provision_prefix(prefix_path: &Path) -> Result<(), InstallGameError> {
    if let Ok(metadata) = fs::metadata(prefix_path) {
        if !metadata.is_dir() {
            return Err(InstallGameError::PrefixPathExistsAsFile {
                path: prefix_path.to_path_buf(),
            });
        }
        return Ok(());
    }

    fs::create_dir_all(prefix_path).map_err(|error| InstallGameError::PrefixCreationFailed {
        path: prefix_path.to_path_buf(),
        message: error.to_string(),
    })
}

fn resolve_prefix_root() -> Result<PathBuf, InstallGameError> {
    resolve_prefix_root_with(crate::flatpak_migration::host_prefix_root())
}

pub(crate) fn resolve_prefix_root_with(
    host_override: Option<PathBuf>,
) -> Result<PathBuf, InstallGameError> {
    if let Some(path) = host_override {
        return Ok(path);
    }
    let base_dirs =
        crate::platform::base_dirs().ok_or(InstallGameError::HomeDirectoryUnavailable)?;
    Ok(resolve_default_prefix_path_from_data_local_dir(
        base_dirs.data_local_dir(),
    ))
}

fn resolve_default_prefix_path_from_data_local_dir(data_local_dir: &Path) -> PathBuf {
    data_local_dir.join(DEFAULT_PREFIX_ROOT_SEGMENT)
}

fn validate_profile_name(profile_name: &str) -> Result<(), InstallGameValidationError> {
    if profile_name.trim().is_empty() {
        return Err(InstallGameValidationError::ProfileNameRequired);
    }

    validate_name(profile_name).map_err(|_| InstallGameValidationError::ProfileNameInvalid)
}

fn validate_installer_path(path: &str) -> Result<(), InstallGameValidationError> {
    let normalized_path = normalize_flatpak_host_path(path);
    if normalized_path.is_empty() {
        return Err(InstallGameValidationError::InstallerPathRequired);
    }

    let path = Path::new(normalized_path.trim());
    if !path.is_absolute() {
        return Err(InstallGameValidationError::InstallerPathNotAbsolute);
    }
    if !normalized_path_is_file(normalized_path.trim()) {
        if !path.exists() {
            return Err(InstallGameValidationError::InstallerPathMissing);
        }
        return Err(InstallGameValidationError::InstallerPathNotFile);
    }
    if !is_windows_executable(path) {
        return Err(InstallGameValidationError::InstallerPathNotWindowsExecutable);
    }

    Ok(())
}

fn validate_optional_trainer_path(path: &str) -> Result<(), InstallGameValidationError> {
    let normalized_path = normalize_flatpak_host_path(path);
    if normalized_path.is_empty() {
        return Ok(());
    }

    let path = Path::new(normalized_path.trim());
    if !normalized_path_is_file(normalized_path.trim()) {
        if !path.exists() {
            return Err(InstallGameValidationError::TrainerPathMissing);
        }
        return Err(InstallGameValidationError::TrainerPathNotFile);
    }

    Ok(())
}

fn validate_optional_custom_cover_art_path(path: &str) -> Result<(), InstallGameValidationError> {
    let normalized_path = normalize_flatpak_host_path(path);
    if normalized_path.is_empty() {
        return Ok(());
    }

    let path = Path::new(normalized_path.trim());
    if !normalized_path_is_file(normalized_path.trim()) {
        if !path.exists() {
            return Err(InstallGameValidationError::CustomCoverArtPathMissing);
        }
        return Err(InstallGameValidationError::CustomCoverArtPathNotFile);
    }

    Ok(())
}

fn validate_optional_custom_portrait_art_path(
    path: &str,
) -> Result<(), InstallGameValidationError> {
    let normalized_path = normalize_flatpak_host_path(path);
    if normalized_path.is_empty() {
        return Ok(());
    }

    let path = Path::new(normalized_path.trim());
    if !normalized_path_is_file(normalized_path.trim()) {
        if !path.exists() {
            return Err(InstallGameValidationError::CustomPortraitArtPathMissing);
        }
        return Err(InstallGameValidationError::CustomPortraitArtPathNotFile);
    }

    Ok(())
}

fn validate_optional_custom_background_art_path(
    path: &str,
) -> Result<(), InstallGameValidationError> {
    let normalized_path = normalize_flatpak_host_path(path);
    if normalized_path.is_empty() {
        return Ok(());
    }

    let path = Path::new(normalized_path.trim());
    if !normalized_path_is_file(normalized_path.trim()) {
        if !path.exists() {
            return Err(InstallGameValidationError::CustomBackgroundArtPathMissing);
        }
        return Err(InstallGameValidationError::CustomBackgroundArtPathNotFile);
    }

    Ok(())
}

fn validate_proton_path(path: &str) -> Result<(), InstallGameValidationError> {
    let normalized_path = normalize_flatpak_host_path(path);
    if normalized_path.is_empty() {
        return Err(InstallGameValidationError::ProtonPathRequired);
    }
    if !Path::new(normalized_path.trim()).is_absolute() {
        return Err(InstallGameValidationError::ProtonPathNotAbsolute);
    }

    if path_is_executable_visible_or_host(path) {
        return Ok(());
    }

    if !path_exists_visible_or_host(path) {
        return Err(InstallGameValidationError::ProtonPathMissing);
    }
    Err(InstallGameValidationError::ProtonPathNotExecutable)
}

fn validate_optional_installed_game_executable_path(
    path: &str,
) -> Result<(), InstallGameValidationError> {
    let normalized_path = normalize_flatpak_host_path(path);
    if normalized_path.is_empty() {
        return Ok(());
    }

    let path = Path::new(normalized_path.trim());
    if !normalized_path_is_file(normalized_path.trim()) {
        if !path.exists() {
            return Err(InstallGameValidationError::InstalledGameExecutablePathMissing);
        }
        return Err(InstallGameValidationError::InstalledGameExecutablePathNotFile);
    }

    Ok(())
}

fn validate_prefix_path(path: &str) -> Result<(), InstallGameValidationError> {
    let normalized_path = normalize_flatpak_host_path(path);
    if normalized_path.is_empty() {
        return Err(InstallGameValidationError::PrefixPathRequired);
    }

    let trimmed = normalized_path.trim();
    let path = Path::new(trimmed);
    if !path.is_absolute() {
        return Err(InstallGameValidationError::PrefixPathNotAbsolute);
    }
    if !normalized_path_is_dir(trimmed)
        && !normalized_path_is_dir_on_host(trimmed)
        && (path.exists() || normalized_path_exists_on_host(trimmed))
    {
        return Err(InstallGameValidationError::PrefixPathNotDirectory);
    }

    Ok(())
}

fn resolve_confirmed_game_executable_path(
    request: &InstallGameRequest,
    discovered_candidates: &[PathBuf],
) -> Option<PathBuf> {
    let normalized_configured_path =
        normalize_flatpak_host_path(&request.installed_game_executable_path);
    let configured_path = normalized_configured_path.trim();
    if !configured_path.is_empty() {
        let path = PathBuf::from(configured_path);
        return Some(path);
    }

    discovered_candidates.first().cloned()
}

fn build_reviewable_profile(
    request: &InstallGameRequest,
    prefix_path: &Path,
    confirmed_game_executable_path: Option<&Path>,
) -> crate::profile::GameProfile {
    let mut profile = request.reviewable_profile(prefix_path);

    if let Some(executable_path) = confirmed_game_executable_path {
        profile.game.executable_path = executable_path.to_string_lossy().into_owned();
        profile.runtime.working_directory = executable_path
            .parent()
            .map(|parent| parent.to_string_lossy().into_owned())
            .unwrap_or_default();
    }

    profile
}

fn slugify_profile_name(profile_name: &str) -> String {
    let slug = profile_name
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "install".to_string()
    } else {
        slug
    }
}

fn is_windows_executable(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
