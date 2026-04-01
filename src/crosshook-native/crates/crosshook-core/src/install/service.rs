use std::fs;
use std::path::{Path, PathBuf};

use directories::BaseDirs;
use tokio::process::Command;
use tokio::runtime::Handle;

use crate::launch::runtime_helpers::{
    apply_host_environment, apply_runtime_proton_environment, apply_working_directory,
    attach_log_stdio, new_direct_proton_command,
};
use crate::profile::validate_name;

use super::discovery::discover_game_executable_candidates;
use super::models::{
    InstallGameError, InstallGameRequest, InstallGameResult, InstallGameValidationError,
};

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

pub fn install_game(
    request: &InstallGameRequest,
    log_path: &Path,
) -> Result<InstallGameResult, InstallGameError> {
    validate_install_request(request)?;

    let prefix_path = PathBuf::from(request.prefix_path.trim());
    provision_prefix(&prefix_path)?;
    let runtime_handle = Handle::try_current().map_err(|_| InstallGameError::RuntimeUnavailable)?;

    let mut command = build_install_command(request, &prefix_path, log_path)?;
    let mut child = command
        .spawn()
        .map_err(|error| InstallGameError::InstallerSpawnFailed {
            message: error.to_string(),
        })?;

    let status = runtime_handle.block_on(child.wait()).map_err(|error| {
        InstallGameError::InstallerWaitFailed {
            message: error.to_string(),
        }
    })?;

    if !status.success() {
        return Err(InstallGameError::InstallerExitedWithFailure {
            status: status.code(),
        });
    }

    let discovered_game_executable_candidates = discover_game_executable_candidates(
        &prefix_path,
        request.resolved_profile_name(),
        request.resolved_display_name(),
        request.installer_path.trim(),
    );
    let confirmed_game_executable_path =
        resolve_confirmed_game_executable_path(request, &discovered_game_executable_candidates);
    let profile = build_reviewable_profile(
        request,
        &prefix_path,
        confirmed_game_executable_path.as_deref(),
    );

    Ok(InstallGameResult {
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
    })
}

fn build_install_command(
    request: &InstallGameRequest,
    prefix_path: &Path,
    log_path: &Path,
) -> Result<Command, InstallGameError> {
    let mut command = new_direct_proton_command(request.proton_path.trim());
    command.arg(request.installer_path.trim());
    apply_host_environment(&mut command);
    let prefix_path_string = prefix_path.to_string_lossy().into_owned();
    apply_runtime_proton_environment(&mut command, &prefix_path_string, "");
    apply_working_directory(&mut command, "", Path::new(request.installer_path.trim()));
    attach_log_stdio(&mut command, log_path).map_err(|error| {
        InstallGameError::LogAttachmentFailed {
            path: log_path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    Ok(command)
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
    let base_dirs = BaseDirs::new().ok_or(InstallGameError::HomeDirectoryUnavailable)?;
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
    if path.is_empty() {
        return Err(InstallGameValidationError::InstallerPathRequired);
    }

    let path = Path::new(path);
    if !path.exists() {
        return Err(InstallGameValidationError::InstallerPathMissing);
    }
    if !path.is_file() {
        return Err(InstallGameValidationError::InstallerPathNotFile);
    }
    if !is_windows_executable(path) {
        return Err(InstallGameValidationError::InstallerPathNotWindowsExecutable);
    }

    Ok(())
}

fn validate_optional_trainer_path(path: &str) -> Result<(), InstallGameValidationError> {
    if path.is_empty() {
        return Ok(());
    }

    let path = Path::new(path);
    if !path.exists() {
        return Err(InstallGameValidationError::TrainerPathMissing);
    }
    if !path.is_file() {
        return Err(InstallGameValidationError::TrainerPathNotFile);
    }

    Ok(())
}

fn validate_optional_custom_cover_art_path(path: &str) -> Result<(), InstallGameValidationError> {
    if path.is_empty() {
        return Ok(());
    }

    let path = Path::new(path);
    if !path.exists() {
        return Err(InstallGameValidationError::CustomCoverArtPathMissing);
    }
    if !path.is_file() {
        return Err(InstallGameValidationError::CustomCoverArtPathNotFile);
    }

    Ok(())
}

fn validate_proton_path(path: &str) -> Result<(), InstallGameValidationError> {
    if path.is_empty() {
        return Err(InstallGameValidationError::ProtonPathRequired);
    }

    let path = Path::new(path);
    if !path.exists() {
        return Err(InstallGameValidationError::ProtonPathMissing);
    }
    if !is_executable_file(path) {
        return Err(InstallGameValidationError::ProtonPathNotExecutable);
    }

    Ok(())
}

fn validate_optional_installed_game_executable_path(
    path: &str,
) -> Result<(), InstallGameValidationError> {
    if path.is_empty() {
        return Ok(());
    }

    let path = Path::new(path);
    if !path.exists() {
        return Err(InstallGameValidationError::InstalledGameExecutablePathMissing);
    }
    if !path.is_file() {
        return Err(InstallGameValidationError::InstalledGameExecutablePathNotFile);
    }

    Ok(())
}

fn validate_prefix_path(path: &str) -> Result<(), InstallGameValidationError> {
    if path.is_empty() {
        return Err(InstallGameValidationError::PrefixPathRequired);
    }

    let path = Path::new(path);
    if let Ok(metadata) = fs::metadata(path) {
        if !metadata.is_dir() {
            return Err(InstallGameValidationError::PrefixPathNotDirectory);
        }
    }

    Ok(())
}

fn resolve_confirmed_game_executable_path(
    request: &InstallGameRequest,
    discovered_candidates: &[PathBuf],
) -> Option<PathBuf> {
    let configured_path = request.installed_game_executable_path.trim();
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

fn is_executable_file(path: &Path) -> bool {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    fn write_executable_script(path: &Path, body: &str) {
        fs::write(path, body).expect("write executable script");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(path).expect("script metadata").permissions();
            permissions.set_mode(permissions.mode() | 0o111);
            fs::set_permissions(path, permissions).expect("set executable permissions");
        }
    }

    fn valid_request(temp_dir: &Path) -> InstallGameRequest {
        let installer_path = temp_dir.join("setup.exe");
        let trainer_path = temp_dir.join("trainer.exe");
        let proton_path = temp_dir.join("proton");
        let prefix_path = temp_dir.join("prefix");

        fs::write(&installer_path, b"installer").expect("installer file");
        fs::write(&trainer_path, b"trainer").expect("trainer file");
        write_executable_script(
            &proton_path,
            "#!/bin/sh\nprefix_path=\"$WINEPREFIX\"\nmkdir -p \"$prefix_path/drive_c/Games/Example Game\"\ncat <<'EOF' > \"$prefix_path/drive_c/Games/Example Game/ExampleGame.exe\"\ngame\nEOF\nexit 0\n",
        );

        InstallGameRequest {
            profile_name: "example-game".to_string(),
            display_name: "Example Game".to_string(),
            installer_path: installer_path.to_string_lossy().into_owned(),
            trainer_path: trainer_path.to_string_lossy().into_owned(),
            proton_path: proton_path.to_string_lossy().into_owned(),
            prefix_path: prefix_path.to_string_lossy().into_owned(),
            installed_game_executable_path: String::new(),
            custom_cover_art_path: String::new(),
        }
    }

    #[test]
    fn resolve_default_prefix_path_uses_data_local_root_and_slugifies_name() {
        let temp_dir = tempdir().expect("temp dir");
        let profile_name = "God of War Ragnarok";

        let prefix_root = resolve_default_prefix_path_from_data_local_dir(temp_dir.path());
        let prefix_path = prefix_root.join(slugify_profile_name(profile_name));

        assert_eq!(
            prefix_path,
            temp_dir
                .path()
                .join(DEFAULT_PREFIX_ROOT_SEGMENT)
                .join("god-of-war-ragnarok")
        );
    }

    #[test]
    fn validate_install_request_returns_specific_field_errors() {
        let temp_dir = tempdir().expect("temp dir");
        let base_request = valid_request(temp_dir.path());

        assert!(validate_install_request(&base_request).is_ok());

        let mut request = base_request.clone();
        request.profile_name = String::new();
        assert!(matches!(
            validate_install_request(&request),
            Err(InstallGameValidationError::ProfileNameRequired)
        ));

        let mut request = base_request.clone();
        request.installer_path = temp_dir
            .path()
            .join("setup.txt")
            .to_string_lossy()
            .into_owned();
        fs::write(&request.installer_path, b"installer").expect("installer txt");
        assert!(matches!(
            validate_install_request(&request),
            Err(InstallGameValidationError::InstallerPathNotWindowsExecutable)
        ));

        let mut request = base_request.clone();
        request.proton_path = temp_dir
            .path()
            .join("proton-dir")
            .to_string_lossy()
            .into_owned();
        fs::create_dir_all(&request.proton_path).expect("proton dir");
        assert!(matches!(
            validate_install_request(&request),
            Err(InstallGameValidationError::ProtonPathNotExecutable)
        ));

        let mut request = base_request.clone();
        request.installed_game_executable_path = temp_dir
            .path()
            .join("candidate")
            .to_string_lossy()
            .into_owned();
        fs::create_dir_all(&request.installed_game_executable_path).expect("candidate dir");
        assert!(matches!(
            validate_install_request(&request),
            Err(InstallGameValidationError::InstalledGameExecutablePathNotFile)
        ));

        let mut request = base_request;
        request.custom_cover_art_path = temp_dir.path().to_string_lossy().into_owned();
        assert!(matches!(
            validate_install_request(&request),
            Err(InstallGameValidationError::CustomCoverArtPathNotFile)
        ));
    }

    #[test]
    fn install_game_creates_prefix_and_prefers_discovered_executable() {
        let temp_dir = tempdir().expect("temp dir");
        let request = valid_request(temp_dir.path());
        let prefix_path = PathBuf::from(&request.prefix_path);
        let log_path = temp_dir.path().join("install.log");

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime");
        let result = runtime
            .block_on(async move {
                tokio::task::spawn_blocking(move || install_game(&request, &log_path))
                    .await
                    .expect("join install task")
            })
            .expect("install game");

        assert!(prefix_path.is_dir());
        assert!(result.succeeded);
        assert_eq!(result.profile_name, "example-game");
        assert!(result.needs_executable_confirmation);
        let expected_game_path = prefix_path
            .join("drive_c/Games/Example Game/ExampleGame.exe")
            .to_string_lossy()
            .into_owned();
        assert_eq!(
            result
                .discovered_game_executable_candidates
                .first()
                .map(|path| path.as_str()),
            Some(expected_game_path.as_str())
        );
        assert_eq!(result.profile.game.executable_path, expected_game_path);
        assert_eq!(
            result.profile.runtime.working_directory,
            prefix_path
                .join("drive_c/Games/Example Game")
                .to_string_lossy()
                .into_owned()
        );
        assert_eq!(result.profile.launch.method, "proton_run");
        assert_eq!(result.profile.game.name, "Example Game");
        assert_eq!(
            result.profile.trainer.path,
            temp_dir
                .path()
                .join("trainer.exe")
                .to_string_lossy()
                .into_owned()
        );
    }
}
