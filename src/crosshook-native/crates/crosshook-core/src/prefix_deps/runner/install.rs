use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;
use tokio::process::{Child, ChildStderr, ChildStdout};

use super::command::build_prefix_dep_mutation_command;
use super::restore::RESTORE_TIMEOUT_SECS;
use super::restore_prefix_windows_version_with_timeout;
use super::sanitize::sanitize_stderr;
use super::snapshot::{is_windows_version_verb, snapshot_prefix_windows_version};
use crate::platform::normalize_flatpak_host_path;
use crate::prefix_deps::validation::validate_protontricks_verbs;
use crate::prefix_deps::{
    PrefixDependencyInstallOutcome, PrefixDepsError, PrefixDepsTool, PrefixVersionRestoreOutcome,
    PrefixVersionRestorePlan, PrefixVersionRestoreState,
};

/// Validated dependency installation that has not spawned a process yet.
///
/// Callers can persist `restore_plan()` before consuming this value with
/// `into_parts()` (or `spawn()`), ensuring crash recovery is durable before
/// prefix mutation begins.
#[derive(Debug)]
pub struct PreparedPrefixDependencyInstall {
    pub(super) command: Command,
    restore_plan: Option<PrefixVersionRestorePlan>,
    restore_timeout: Duration,
}

impl PreparedPrefixDependencyInstall {
    pub fn restore_plan(&self) -> Option<&PrefixVersionRestorePlan> {
        self.restore_plan.as_ref()
    }

    /// Split into the unspawned install command and its restore plan.
    ///
    /// The command leads its own process group (`process_group(0)`), is
    /// `kill_on_drop`, has piped stdout/stderr and a null stdin.
    pub fn into_parts(self) -> (Command, Option<PrefixVersionRestorePlan>) {
        (self.command, self.restore_plan)
    }

    pub fn spawn(mut self) -> Result<PrefixDependencyInstall, PrefixDepsError> {
        let child = self
            .command
            .spawn()
            .map_err(|error| PrefixDepsError::ProcessFailed {
                exit_code: None,
                stderr: format!("failed to spawn install process: {error}"),
            })?;

        Ok(PrefixDependencyInstall {
            child,
            restore_plan: self.restore_plan,
            restore_timeout: self.restore_timeout,
        })
    }

    #[cfg(test)]
    pub(super) fn spawn_with_restore_timeout(
        mut self,
        restore_timeout: Duration,
    ) -> Result<PrefixDependencyInstall, PrefixDepsError> {
        self.restore_timeout = restore_timeout;
        self.spawn()
    }
}

/// Running prefix-dependency installation with an optional Windows-version restore.
#[derive(Debug)]
pub struct PrefixDependencyInstall {
    child: Child,
    restore_plan: Option<PrefixVersionRestorePlan>,
    restore_timeout: Duration,
}

impl PrefixDependencyInstall {
    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }

    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }

    /// Wait for installation and restore the prefix's original Windows version.
    ///
    /// Restoration runs even when dependency installation exits unsuccessfully,
    /// and the two results remain separate for persistence and UI reporting.
    pub async fn wait_and_restore(mut self) -> PrefixDependencyInstallOutcome {
        let install_result = self.child.wait().await;
        let (install_succeeded, install_exit_code, install_error) = match install_result {
            Ok(status) => (status.success(), status.code(), None),
            Err(error) => (
                false,
                None,
                Some(sanitize_stderr(&format!(
                    "failed to wait for dependency install: {error}"
                ))),
            ),
        };

        let restore = match self.restore_plan.as_ref() {
            Some(plan) => {
                restore_prefix_windows_version_with_timeout(plan, self.restore_timeout).await
            }
            None => PrefixVersionRestoreOutcome {
                state: PrefixVersionRestoreState::NotRequired,
                error: None,
            },
        };

        PrefixDependencyInstallOutcome {
            install_succeeded,
            install_exit_code,
            install_error,
            restore_state: restore.state,
            restore_error: restore.error,
        }
    }
}

/// Prepare an install process without spawning it.
///
/// The caller must persist `restore_plan()` before spawning the command so an
/// interrupted process remains recoverable after restart.
///
/// Security checklist:
/// - validate_protontricks_verbs() called first
/// - Per-verb .arg() calls (never joined)
/// - Flag-like and malformed verbs rejected before argv construction
/// - apply_host_environment() used (NOT env_clear())
/// - .kill_on_drop(true)
/// - Prefix path normalized via resolve_wine_prefix_path()
/// - pfx/ existence check
pub fn prepare_install_packages(
    binary_path: &str,
    prefix_path: &str,
    verbs: &[String],
    tool_type: PrefixDepsTool,
    steam_app_id: Option<&str>,
) -> Result<PreparedPrefixDependencyInstall, PrefixDepsError> {
    // Validate verbs first (security gate).
    validate_protontricks_verbs(verbs)?;

    let (mut cmd, resolved_prefix) = build_prefix_dep_mutation_command(binary_path, prefix_path)?;

    // Protontricks takes app_id first, then -q.
    if matches!(tool_type, PrefixDepsTool::Protontricks) {
        let app_id = steam_app_id.ok_or_else(|| {
            PrefixDepsError::ValidationError(
                "steam app id is required when using protontricks".to_string(),
            )
        })?;
        cmd.arg(app_id);
    }

    // Quiet mode.
    cmd.arg("-q");

    // Each validated verb is passed as an individual argument. Winetricks does
    // not support a `--` option separator, so structural validation above is
    // the flag-injection boundary.
    for verb in verbs {
        cmd.arg(verb);
    }

    let restore_plan = if verbs.iter().any(|verb| is_windows_version_verb(verb)) {
        None
    } else {
        Some(PrefixVersionRestorePlan {
            binary_path: normalize_flatpak_host_path(binary_path),
            prefix_path: resolved_prefix.to_string_lossy().into_owned(),
            tool_type,
            steam_app_id: steam_app_id.map(str::to_string),
            restore_verb: snapshot_prefix_windows_version(&resolved_prefix)?,
        })
    };

    // Own process group so supervision can signal the whole install tree.
    cmd.process_group(0);
    cmd.kill_on_drop(true);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    Ok(PreparedPrefixDependencyInstall {
        command: cmd,
        restore_plan,
        restore_timeout: Duration::from_secs(RESTORE_TIMEOUT_SECS),
    })
}

/// Compatibility wrapper for callers that do not need to persist a repair plan.
///
/// New application integrations must use `prepare_install_packages`, persist
/// the exposed restore plan, and only then spawn the command.
pub async fn install_packages(
    binary_path: &str,
    prefix_path: &str,
    verbs: &[String],
    tool_type: PrefixDepsTool,
    steam_app_id: Option<&str>,
) -> Result<PrefixDependencyInstall, PrefixDepsError> {
    prepare_install_packages(binary_path, prefix_path, verbs, tool_type, steam_app_id)?.spawn()
}
