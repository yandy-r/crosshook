use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use super::command::build_prefix_dep_command_with;
use super::sanitize::sanitize_stderr;
use crate::platform::is_flatpak;
use crate::prefix_deps::{PrefixDepsError, PrefixDepsTool};

/// Default timeout for check operations (seconds).
const CHECK_TIMEOUT_SECS: u64 = 30;

/// Build the lifecycle-bound `list-installed` probe.
///
/// Inside Flatpak the probe runs through `flatpak-spawn --host --watch-bus`, so
/// the host process dies with the proxy when the check times out or is dropped.
pub(crate) fn build_check_command_with(
    binary_path: &str,
    prefix_path: &str,
    tool_type: PrefixDepsTool,
    steam_app_id: Option<&str>,
    flatpak: bool,
) -> Result<Command, PrefixDepsError> {
    let (mut cmd, _resolved_prefix) =
        build_prefix_dep_command_with(binary_path, prefix_path, flatpak)?;
    if matches!(tool_type, PrefixDepsTool::Protontricks) {
        let app_id = steam_app_id.ok_or_else(|| {
            PrefixDepsError::ValidationError(
                "steam app id is required when using protontricks".to_string(),
            )
        })?;
        cmd.arg(app_id);
    }
    cmd.arg("list-installed");
    cmd.kill_on_drop(true);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    Ok(cmd)
}

/// Check which packages are already installed in the given prefix.
///
/// Runs `binary_path list-installed` with WINEPREFIX set.
pub async fn check_installed(
    binary_path: &str,
    prefix_path: &str,
    tool_type: PrefixDepsTool,
    steam_app_id: Option<&str>,
) -> Result<Vec<String>, PrefixDepsError> {
    let mut cmd = build_check_command_with(
        binary_path,
        prefix_path,
        tool_type,
        steam_app_id,
        is_flatpak(),
    )?;

    let child = cmd.spawn().map_err(|e| PrefixDepsError::ProcessFailed {
        exit_code: None,
        stderr: format!("failed to spawn: {e}"),
    })?;

    let output = tokio::time::timeout(
        Duration::from_secs(CHECK_TIMEOUT_SECS),
        child.wait_with_output(),
    )
    .await
    .map_err(|_| PrefixDepsError::Timeout {
        seconds: CHECK_TIMEOUT_SECS,
    })?
    .map_err(|e| PrefixDepsError::ProcessFailed {
        exit_code: None,
        stderr: format!("failed to wait for process: {e}"),
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PrefixDepsError::ProcessFailed {
            exit_code: output.status.code(),
            stderr: sanitize_stderr(&stderr),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let packages: Vec<String> = stdout
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .map(std::string::ToString::to_string)
        .collect();

    Ok(packages)
}
