use std::path::{Path, PathBuf};

use tokio::process::{Child, Command};

use crate::launch::runtime_helpers::attach_log_stdio;
use crate::platform::normalize_flatpak_host_path;
use crate::run_executable::{RunExecutableError, RunExecutableRequest, RunExecutableResult};

use super::adhoc_prefix::{
    provision_prefix, resolve_adhoc_prefix_path, resolve_default_adhoc_prefix_path,
};
use super::command_builder::build_run_executable_command;
use super::validation::validate_run_executable_request;

/// A validated run with its prefix provisioned and its command built.
///
/// The command has no stdio, process group or `kill_on_drop` configured; the
/// caller spawns and supervises it.
#[derive(Debug)]
pub struct PreparedRunExecutable {
    pub command: Command,
    pub prefix_path: PathBuf,
    /// `true` when the prefix was auto-resolved under `adhoc_root` and should
    /// be removed once the run finishes.
    pub is_throwaway: bool,
}

/// Validates `request`, resolves the prefix, provisions it and builds the
/// lifecycle-bound command.
///
/// An empty `request.prefix_path` resolves a unique throwaway prefix
/// `<adhoc_root>/<slug>-<run_suffix>` (see
/// [`super::adhoc_prefix::resolve_adhoc_prefix_path`]).
pub fn prepare_run_executable(
    request: &RunExecutableRequest,
    adhoc_root: &Path,
    run_suffix: &str,
) -> Result<PreparedRunExecutable, RunExecutableError> {
    validate_run_executable_request(request)?;

    let is_throwaway = request.prefix_path.trim().is_empty();
    let prefix_path = if is_throwaway {
        let normalized_executable_path = normalize_flatpak_host_path(&request.executable_path);
        resolve_adhoc_prefix_path(
            adhoc_root,
            Path::new(normalized_executable_path.trim()),
            run_suffix,
        )
    } else {
        PathBuf::from(normalize_flatpak_host_path(&request.prefix_path))
    };

    provision_prefix(&prefix_path)?;
    let command = build_run_executable_command(request, &prefix_path)?;

    Ok(PreparedRunExecutable {
        command,
        prefix_path,
        is_throwaway,
    })
}

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

    let mut command = build_run_executable_command(request, &prefix_path)?;
    attach_log_stdio(&mut command, log_path).map_err(|error| {
        RunExecutableError::LogAttachmentFailed {
            path: log_path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
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
