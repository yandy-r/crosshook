use std::process::Stdio;

use tokio::process::Command;

use super::command::build_prefix_dep_mutation_command;
use super::snapshot::{is_windows_version_verb, snapshot_prefix_windows_version};
use crate::platform::normalize_flatpak_host_path;
use crate::prefix_deps::validation::validate_protontricks_verbs;
use crate::prefix_deps::{PrefixDepsError, PrefixDepsTool, PrefixVersionRestorePlan};

/// Validated dependency installation that has not spawned a process yet.
///
/// Callers persist `restore_plan()` before consuming this value with
/// `into_parts()`, ensuring crash recovery is durable before prefix mutation
/// begins.
#[derive(Debug)]
pub struct PreparedPrefixDependencyInstall {
    pub(super) command: Command,
    restore_plan: Option<PrefixVersionRestorePlan>,
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
    })
}
