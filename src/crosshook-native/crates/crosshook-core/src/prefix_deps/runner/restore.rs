use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncReadExt;

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
    cmd.kill_on_drop(true);
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(error) => {
            return failed_restore(sanitize_stderr(&format!(
                "failed to spawn Windows version restore: {error}"
            )));
        }
    };
    let stderr = child.stderr.take();
    let stderr_task = tokio::spawn(async move {
        let mut bytes = Vec::new();
        if let Some(mut stderr) = stderr {
            let _ = stderr.read_to_end(&mut bytes).await;
        }
        bytes
    });

    let status = match tokio::time::timeout(timeout, child.wait()).await {
        Ok(Ok(status)) => status,
        Ok(Err(error)) => {
            return failed_restore(sanitize_stderr(&format!(
                "failed to wait for Windows version restore: {error}"
            )));
        }
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return failed_restore(format!(
                "restoring Windows version {} timed out after {} seconds",
                plan.restore_verb,
                timeout.as_secs_f64()
            ));
        }
    };
    let stderr = stderr_task.await.unwrap_or_default();

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
