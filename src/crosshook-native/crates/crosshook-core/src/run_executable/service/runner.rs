use std::path::{Path, PathBuf};

use tokio::process::Child;

use crate::platform::normalize_flatpak_host_path;
use crate::run_executable::{RunExecutableError, RunExecutableRequest, RunExecutableResult};

use super::adhoc_prefix::{provision_prefix, resolve_default_adhoc_prefix_path};
use super::command_builder::build_run_executable_command;
use super::validation::validate_run_executable_request;

pub fn run_executable(
    request: &RunExecutableRequest,
    log_path: &Path,
) -> Result<(RunExecutableResult, Child), RunExecutableError> {
    validate_run_executable_request(request)?;

    let prefix_path = if request.prefix_path.trim().is_empty() {
        let normalized_executable_path = normalize_flatpak_host_path(&request.executable_path);
        resolve_default_adhoc_prefix_path(Path::new(normalized_executable_path.trim()))?
    } else {
        PathBuf::from(normalize_flatpak_host_path(&request.prefix_path))
    };

    provision_prefix(&prefix_path)?;

    let mut command = build_run_executable_command(request, &prefix_path, log_path)?;
    let child = command
        .spawn()
        .map_err(|error| RunExecutableError::RunnerSpawnFailed {
            message: error.to_string(),
        })?;

    let result = RunExecutableResult {
        succeeded: true,
        message: "Executable launched.".to_string(),
        helper_log_path: log_path.display().to_string(),
        resolved_prefix_path: prefix_path.display().to_string(),
    };

    Ok((result, child))
}
