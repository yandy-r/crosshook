use std::path::Path;

use tokio::process::Command;

use crate::launch::runtime_helpers::proton_command::build_direct_proton_command_lifecycle_with;
use crate::launch::runtime_helpers::{
    apply_working_directory, attach_log_stdio, host_environment_map, merge_runtime_proton_into_map,
    resolve_effective_working_directory,
};
use crate::platform::{
    normalize_flatpak_host_path, normalized_path_is_dir, normalized_path_is_executable_file,
    normalized_path_is_file,
};

use super::models::{
    UpdateGameError, UpdateGameRequest, UpdateGameResult, UpdateGameValidationError,
};

pub fn validate_update_request(
    request: &UpdateGameRequest,
) -> Result<(), UpdateGameValidationError> {
    validate_updater_path(request.updater_path.trim())?;
    validate_proton_path(request.proton_path.trim())?;
    validate_prefix_path(request.prefix_path.trim())?;

    Ok(())
}

/// Builds the lifecycle-bound updater command (`<proton> run <updater>`).
///
/// No stdio is attached; the caller owns stdio, process group, spawn and
/// supervision.
pub fn build_update_command(request: &UpdateGameRequest) -> Result<Command, UpdateGameError> {
    Ok(build_update_command_with(
        request,
        crate::platform::is_flatpak(),
    ))
}

fn build_update_command_with(request: &UpdateGameRequest, flatpak: bool) -> Command {
    let normalized_updater_path = normalize_flatpak_host_path(&request.updater_path);
    let mut env = host_environment_map();
    merge_runtime_proton_into_map(
        &mut env,
        request.prefix_path.trim(),
        request.steam_client_install_path.trim(),
    );
    let effective_working_directory =
        resolve_effective_working_directory("", Path::new(normalized_updater_path.trim()));
    let mut command = build_direct_proton_command_lifecycle_with(
        request.proton_path.trim(),
        &env,
        effective_working_directory.as_deref(),
        flatpak,
    );
    command.arg(normalized_updater_path.trim());
    if !flatpak {
        apply_working_directory(&mut command, "", Path::new(normalized_updater_path.trim()));
    }
    command
}

pub fn update_game(
    request: &UpdateGameRequest,
    log_path: &Path,
) -> Result<(UpdateGameResult, tokio::process::Child), UpdateGameError> {
    validate_update_request(request)?;

    let mut command = build_update_command(request)?;
    attach_log_stdio(&mut command, log_path).map_err(|error| {
        UpdateGameError::LogAttachmentFailed {
            path: log_path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    let child = command
        .spawn()
        .map_err(|error| UpdateGameError::UpdaterSpawnFailed {
            message: error.to_string(),
        })?;

    // `succeeded` indicates the update process was launched successfully, not that the
    // updater itself has finished. The actual process exit status is communicated
    // asynchronously via the `update-complete` event.
    let result = UpdateGameResult {
        succeeded: true,
        message: "Update process launched.".to_string(),
        helper_log_path: log_path.display().to_string(),
    };

    Ok((result, child))
}

fn validate_updater_path(path: &str) -> Result<(), UpdateGameValidationError> {
    let normalized_path = normalize_flatpak_host_path(path);
    if normalized_path.is_empty() {
        return Err(UpdateGameValidationError::UpdaterPathRequired);
    }

    let path = Path::new(normalized_path.trim());
    if !normalized_path_is_file(normalized_path.trim()) {
        if !path.exists() {
            return Err(UpdateGameValidationError::UpdaterPathMissing);
        }
        return Err(UpdateGameValidationError::UpdaterPathNotFile);
    }
    if !is_windows_executable(path) {
        return Err(UpdateGameValidationError::UpdaterPathNotWindowsExecutable);
    }

    Ok(())
}

fn validate_proton_path(path: &str) -> Result<(), UpdateGameValidationError> {
    let normalized_path = normalize_flatpak_host_path(path);
    if normalized_path.is_empty() {
        return Err(UpdateGameValidationError::ProtonPathRequired);
    }

    let path = Path::new(normalized_path.trim());
    if !normalized_path_is_executable_file(normalized_path.trim()) {
        if !path.exists() {
            return Err(UpdateGameValidationError::ProtonPathMissing);
        }
        return Err(UpdateGameValidationError::ProtonPathNotExecutable);
    }

    Ok(())
}

fn validate_prefix_path(path: &str) -> Result<(), UpdateGameValidationError> {
    let normalized_path = normalize_flatpak_host_path(path);
    if normalized_path.is_empty() {
        return Err(UpdateGameValidationError::PrefixPathRequired);
    }

    let path = Path::new(normalized_path.trim());
    if !normalized_path_is_dir(normalized_path.trim()) {
        if !path.exists() {
            return Err(UpdateGameValidationError::PrefixPathMissing);
        }
        return Err(UpdateGameValidationError::PrefixPathNotDirectory);
    }

    Ok(())
}

fn is_windows_executable(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
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

    fn valid_request(temp_dir: &Path) -> UpdateGameRequest {
        let updater_path = temp_dir.join("updater.exe");
        let proton_path = temp_dir.join("proton");
        let prefix_path = temp_dir.join("prefix");

        fs::write(&updater_path, b"updater").expect("updater file");
        write_executable_script(&proton_path, "#!/bin/sh\nexit 0\n");
        fs::create_dir_all(&prefix_path).expect("prefix directory");

        UpdateGameRequest {
            profile_name: "example-game".to_string(),
            updater_path: updater_path.to_string_lossy().into_owned(),
            proton_path: proton_path.to_string_lossy().into_owned(),
            prefix_path: prefix_path.to_string_lossy().into_owned(),
            steam_client_install_path: String::new(),
        }
    }

    #[test]
    fn validate_update_request_accepts_valid_request() {
        let temp_dir = tempdir().expect("temp dir");
        let request = valid_request(temp_dir.path());

        assert!(validate_update_request(&request).is_ok());
    }

    #[test]
    fn validate_update_request_rejects_empty_updater_path() {
        let temp_dir = tempdir().expect("temp dir");
        let mut request = valid_request(temp_dir.path());
        request.updater_path = String::new();

        assert!(matches!(
            validate_update_request(&request),
            Err(UpdateGameValidationError::UpdaterPathRequired)
        ));
    }

    #[test]
    fn validate_update_request_rejects_missing_updater_path() {
        let temp_dir = tempdir().expect("temp dir");
        let mut request = valid_request(temp_dir.path());
        request.updater_path = temp_dir
            .path()
            .join("nonexistent.exe")
            .to_string_lossy()
            .into_owned();

        assert!(matches!(
            validate_update_request(&request),
            Err(UpdateGameValidationError::UpdaterPathMissing)
        ));
    }

    #[test]
    fn validate_update_request_rejects_directory_as_updater_path() {
        let temp_dir = tempdir().expect("temp dir");
        let mut request = valid_request(temp_dir.path());
        let dir_path = temp_dir.path().join("updater-dir.exe");
        fs::create_dir_all(&dir_path).expect("updater dir");
        request.updater_path = dir_path.to_string_lossy().into_owned();

        assert!(matches!(
            validate_update_request(&request),
            Err(UpdateGameValidationError::UpdaterPathNotFile)
        ));
    }

    #[test]
    fn validate_update_request_rejects_non_exe_updater_path() {
        let temp_dir = tempdir().expect("temp dir");
        let mut request = valid_request(temp_dir.path());
        let txt_path = temp_dir.path().join("updater.txt");
        fs::write(&txt_path, b"not an exe").expect("txt file");
        request.updater_path = txt_path.to_string_lossy().into_owned();

        assert!(matches!(
            validate_update_request(&request),
            Err(UpdateGameValidationError::UpdaterPathNotWindowsExecutable)
        ));
    }

    #[test]
    fn validate_update_request_rejects_empty_proton_path() {
        let temp_dir = tempdir().expect("temp dir");
        let mut request = valid_request(temp_dir.path());
        request.proton_path = String::new();

        assert!(matches!(
            validate_update_request(&request),
            Err(UpdateGameValidationError::ProtonPathRequired)
        ));
    }

    #[test]
    fn validate_update_request_rejects_missing_proton_path() {
        let temp_dir = tempdir().expect("temp dir");
        let mut request = valid_request(temp_dir.path());
        request.proton_path = temp_dir
            .path()
            .join("nonexistent-proton")
            .to_string_lossy()
            .into_owned();

        assert!(matches!(
            validate_update_request(&request),
            Err(UpdateGameValidationError::ProtonPathMissing)
        ));
    }

    #[test]
    fn validate_update_request_rejects_non_executable_proton_path() {
        let temp_dir = tempdir().expect("temp dir");
        let mut request = valid_request(temp_dir.path());
        let non_exec_path = temp_dir.path().join("proton-no-exec");
        fs::write(&non_exec_path, b"not executable").expect("non-exec file");
        request.proton_path = non_exec_path.to_string_lossy().into_owned();

        assert!(matches!(
            validate_update_request(&request),
            Err(UpdateGameValidationError::ProtonPathNotExecutable)
        ));
    }

    #[test]
    fn validate_update_request_rejects_empty_prefix_path() {
        let temp_dir = tempdir().expect("temp dir");
        let mut request = valid_request(temp_dir.path());
        request.prefix_path = String::new();

        assert!(matches!(
            validate_update_request(&request),
            Err(UpdateGameValidationError::PrefixPathRequired)
        ));
    }

    #[test]
    fn validate_update_request_rejects_missing_prefix_path() {
        let temp_dir = tempdir().expect("temp dir");
        let mut request = valid_request(temp_dir.path());
        request.prefix_path = temp_dir
            .path()
            .join("nonexistent-prefix")
            .to_string_lossy()
            .into_owned();

        assert!(matches!(
            validate_update_request(&request),
            Err(UpdateGameValidationError::PrefixPathMissing)
        ));
    }

    #[test]
    fn validate_update_request_rejects_file_as_prefix_path() {
        let temp_dir = tempdir().expect("temp dir");
        let mut request = valid_request(temp_dir.path());
        let file_path = temp_dir.path().join("prefix-file");
        fs::write(&file_path, b"not a directory").expect("prefix file");
        request.prefix_path = file_path.to_string_lossy().into_owned();

        assert!(matches!(
            validate_update_request(&request),
            Err(UpdateGameValidationError::PrefixPathNotDirectory)
        ));
    }

    #[test]
    fn build_update_command_constructs_command_with_correct_proton_path() {
        let temp_dir = tempdir().expect("temp dir");
        let request = valid_request(temp_dir.path());
        let command = build_update_command(&request).unwrap();
        let debug_output = format!("{command:?}");

        // The command should reference the proton binary
        assert!(
            debug_output.contains(&request.proton_path),
            "Command should reference the proton path, got: {debug_output}"
        );
    }

    #[test]
    fn build_update_command_normalizes_flatpak_host_mounted_paths() {
        let temp_dir = tempdir().expect("temp dir");
        let mut request = valid_request(temp_dir.path());
        let normalized_proton_path = request.proton_path.clone();
        let normalized_updater_path = request.updater_path.clone();
        request.proton_path = format!("/run/host{}", request.proton_path);
        request.updater_path = format!("/run/host{}", request.updater_path);
        let command = build_update_command(&request).unwrap();
        let debug_output = format!("{command:?}");

        assert!(
            debug_output.contains(&normalized_proton_path),
            "Command should reference the normalized proton path, got: {debug_output}"
        );
        assert!(
            debug_output.contains(&normalized_updater_path),
            "Command should reference the normalized updater path, got: {debug_output}"
        );
        assert!(
            !debug_output.contains("/run/host"),
            "Command should not retain the Flatpak host prefix, got: {debug_output}"
        );
    }

    fn command_argv(command: &Command) -> Vec<String> {
        let std_command = command.as_std();
        std::iter::once(std_command.get_program())
            .chain(std_command.get_args())
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn build_update_command_flatpak_argv_is_lifecycle_bound_proton_run() {
        let temp_dir = tempdir().expect("temp dir");
        let request = valid_request(temp_dir.path());

        let command = build_update_command_with(&request, true);
        let argv = command_argv(&command);

        assert_eq!(
            &argv[..4],
            ["flatpak-spawn", "--host", "--watch-bus", "--clear-env"]
        );
        let expected_directory = format!("--directory={}", temp_dir.path().display());
        assert!(
            argv.contains(&expected_directory),
            "expected {expected_directory} in {argv:?}"
        );
        let expected_prefix_env = format!(
            "--env=WINEPREFIX={}",
            crate::launch::runtime_helpers::resolve_wine_prefix_path(Path::new(
                &request.prefix_path
            ))
            .display()
        );
        assert!(
            argv.contains(&expected_prefix_env),
            "expected {expected_prefix_env} in {argv:?}"
        );
        assert_eq!(
            &argv[argv.len() - 3..],
            [
                request.proton_path.as_str(),
                "run",
                request.updater_path.as_str()
            ]
        );
        assert!(command.as_std().get_current_dir().is_none());
    }

    #[test]
    fn build_update_command_native_runs_proton_in_updater_directory() {
        let temp_dir = tempdir().expect("temp dir");
        let request = valid_request(temp_dir.path());

        let command = build_update_command_with(&request, false);

        assert_eq!(
            command_argv(&command),
            [
                request.proton_path.as_str(),
                "run",
                request.updater_path.as_str()
            ]
        );
        assert_eq!(command.as_std().get_current_dir(), Some(temp_dir.path()));
        let has_prefix_env = command
            .as_std()
            .get_envs()
            .any(|(key, value)| key == std::ffi::OsStr::new("WINEPREFIX") && value.is_some());
        assert!(has_prefix_env, "native command must carry WINEPREFIX");
    }

    #[test]
    fn update_game_rejects_invalid_request() {
        let temp_dir = tempdir().unwrap();
        let log_path = temp_dir.path().join("update.log");
        std::fs::File::create(&log_path).unwrap();

        let request = UpdateGameRequest::default(); // all empty — will fail validation
        let result = update_game(&request, &log_path);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            UpdateGameError::Validation(UpdateGameValidationError::UpdaterPathRequired)
        ));
    }

    #[test]
    fn validate_update_request_accepts_uppercase_exe_extension() {
        let temp_dir = tempdir().unwrap();
        let mut request = valid_request(temp_dir.path());

        // Create an updater with uppercase .EXE extension
        let exe_path = temp_dir.path().join("Update.EXE");
        write_executable_script(&exe_path, "#!/bin/sh\nexit 0\n");
        request.updater_path = exe_path.to_string_lossy().into_owned();

        let result = validate_update_request(&request);
        assert!(result.is_ok(), "Uppercase .EXE should be accepted");
    }
}
