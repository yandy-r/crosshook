use std::process::Stdio;
use std::time::Duration;

use super::bounded::{run_bounded, BoundedError, Limits, DRAIN_GRACE};

use super::command::build_prefix_dep_mutation_command;
use super::sanitize::sanitize_stderr;
use super::snapshot::{is_windows_version_verb, snapshot_prefix_windows_version};
use crate::prefix_deps::{
    PrefixDepsTool, PrefixVersionRestoreOutcome, PrefixVersionRestorePlan,
    PrefixVersionRestoreState,
};

/// Maximum time allowed for compatibility restoration in production.
pub(super) const RESTORE_TIMEOUT_SECS: u64 = 90;

/// Restore and verify a prefix's captured Windows compatibility tier.
pub async fn restore_prefix_windows_version(
    plan: &PrefixVersionRestorePlan,
) -> PrefixVersionRestoreOutcome {
    restore_prefix_windows_version_with_timeout(plan, Duration::from_secs(RESTORE_TIMEOUT_SECS))
        .await
}

/// [`restore_prefix_windows_version`] with an explicit timeout.
pub(crate) async fn restore_prefix_windows_version_with_timeout(
    plan: &PrefixVersionRestorePlan,
    timeout: Duration,
) -> PrefixVersionRestoreOutcome {
    restore_prefix_windows_version_with_limits(
        plan,
        Limits {
            timeout,
            drain_grace: DRAIN_GRACE,
        },
    )
    .await
}

/// The restore child leads its own process group: on timeout the whole group
/// (winetricks and its Wine children) is killed, and after a natural exit the
/// stderr drain is bounded by `limits.drain_grace`.
pub(super) async fn restore_prefix_windows_version_with_limits(
    plan: &PrefixVersionRestorePlan,
    limits: Limits,
) -> PrefixVersionRestoreOutcome {
    if !is_windows_version_verb(&plan.restore_verb) {
        return failed_restore("invalid Windows version restore verb".to_string());
    }

    let (mut cmd, resolved_prefix) =
        match build_prefix_dep_mutation_command(&plan.binary_path, &plan.prefix_path) {
            Ok(command) => command,
            Err(error) => return failed_restore(error.to_string()),
        };
    if matches!(plan.tool_type, PrefixDepsTool::Protontricks) {
        let Some(app_id) = plan.steam_app_id.as_deref() else {
            return failed_restore(
                "steam app id is required to restore the prefix Windows version".to_string(),
            );
        };
        cmd.arg(app_id);
    }
    cmd.arg("-q").arg(&plan.restore_verb);
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::piped());

    let output = match run_bounded(cmd, limits).await {
        Ok(output) => output,
        Err(BoundedError::Spawn(error)) => {
            return failed_restore(sanitize_stderr(&format!(
                "failed to spawn Windows version restore: {error}"
            )));
        }
        Err(BoundedError::Wait(error)) => {
            return failed_restore(sanitize_stderr(&format!(
                "failed to wait for Windows version restore: {error}"
            )));
        }
        Err(BoundedError::TimedOut) => {
            return failed_restore(format!(
                "restoring Windows version {} timed out after {} seconds",
                plan.restore_verb,
                limits.timeout.as_secs_f64()
            ));
        }
    };
    let status = output.status;
    let stderr = output.stderr;

    if !status.success() {
        let stderr = sanitize_stderr(&String::from_utf8_lossy(&stderr));
        return failed_restore(format!(
            "restoring Windows version {} failed (exit code {:?}): {}",
            plan.restore_verb,
            status.code(),
            stderr
        ));
    }

    match snapshot_prefix_windows_version(&resolved_prefix) {
        Ok(actual) if actual == plan.restore_verb => PrefixVersionRestoreOutcome {
            state: PrefixVersionRestoreState::Succeeded,
            error: None,
        },
        Ok(actual) => failed_restore(format!(
            "Windows version restore verification failed: expected {}, found {actual}",
            plan.restore_verb
        )),
        Err(error) => failed_restore(format!(
            "Windows version restore verification failed: {error}"
        )),
    }
}

fn failed_restore(error: String) -> PrefixVersionRestoreOutcome {
    PrefixVersionRestoreOutcome {
        state: PrefixVersionRestoreState::Failed,
        error: Some(sanitize_stderr(&error)),
    }
}
