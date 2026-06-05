//! Runtime execution for profile launch hooks (`pre_launch_hooks` / `post_exit_hooks`).

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

use crate::launch::runtime_helpers::{
    collect_pressure_vessel_paths, host_environment_map, merge_optimization_and_custom_into_map,
    merge_runtime_proton_into_map, resolve_effective_working_directory,
};
use crate::launch::{
    resolve_launch_directives, LaunchRequest, LaunchValidationIssue, METHOD_PROTON_RUN,
    METHOD_STEAM_APPLAUNCH,
};
use crate::platform::{
    host_std_command_with_env_and_directory, normalize_flatpak_host_path,
    normalized_path_is_executable_file_on_host,
};
use crate::profile::{HookStage, LaunchHook};

/// Default per-hook execution timeout. Fail-open on expiry.
pub const DEFAULT_HOOK_TIMEOUT: Duration = Duration::from_secs(10);

/// Host environment and working directory shared by hook subprocesses for a launch.
#[derive(Debug, Clone, Default)]
pub struct LaunchHookExecutionContext {
    pub env: BTreeMap<String, String>,
    pub working_directory: Option<String>,
    pub custom_env_vars: BTreeMap<String, String>,
}

/// Build the host execution context hooks should inherit for a launch request.
pub fn build_launch_hook_execution_context(
    request: &LaunchRequest,
) -> Result<LaunchHookExecutionContext, crate::launch::request::ValidationError> {
    let method = request.resolved_method();
    let directives = resolve_launch_directives(request)?;

    let mut env = host_environment_map();
    match method {
        METHOD_STEAM_APPLAUNCH => {
            merge_steam_compat_env(&mut env, request);
            merge_optimization_and_custom_into_map(&mut env, &directives.env, &BTreeMap::new());
        }
        METHOD_PROTON_RUN => {
            merge_runtime_proton_into_map(
                &mut env,
                request.runtime.prefix_path.trim(),
                request.steam.steam_client_install_path.trim(),
            );
            merge_optimization_and_custom_into_map(&mut env, &directives.env, &BTreeMap::new());
            let pressure_vessel_paths = collect_pressure_vessel_paths(request).join(":");
            env.insert(
                "STEAM_COMPAT_LIBRARY_PATHS".to_string(),
                pressure_vessel_paths.clone(),
            );
            env.insert(
                "PRESSURE_VESSEL_FILESYSTEMS_RW".to_string(),
                pressure_vessel_paths,
            );
        }
        _ => {
            merge_optimization_and_custom_into_map(
                &mut env,
                &directives.env,
                &request.custom_env_vars,
            );
        }
    }

    for key in request.custom_env_vars.keys() {
        env.remove(key);
    }

    let primary_path = if request.launch_trainer_only {
        normalize_flatpak_host_path(&request.trainer_host_path)
    } else {
        normalize_flatpak_host_path(&request.game_path)
    };
    let normalized_working_directory =
        normalize_flatpak_host_path(&request.runtime.working_directory);
    let working_directory = resolve_effective_working_directory(
        normalized_working_directory.trim(),
        Path::new(primary_path.trim()),
    );

    Ok(LaunchHookExecutionContext {
        env,
        working_directory,
        custom_env_vars: request.custom_env_vars.clone(),
    })
}

fn merge_steam_compat_env(map: &mut BTreeMap<String, String>, request: &LaunchRequest) {
    let normalized_compatdata_path = normalize_flatpak_host_path(&request.steam.compatdata_path);
    let normalized_steam_client_install_path =
        normalize_flatpak_host_path(&request.steam.steam_client_install_path);
    map.insert(
        "STEAM_COMPAT_DATA_PATH".to_string(),
        normalized_compatdata_path.trim().to_string(),
    );
    map.insert(
        "STEAM_COMPAT_CLIENT_INSTALL_PATH".to_string(),
        normalized_steam_client_install_path.trim().to_string(),
    );
    map.insert(
        "WINEPREFIX".to_string(),
        Path::new(normalized_compatdata_path.trim())
            .join("pfx")
            .to_string_lossy()
            .into_owned(),
    );
}

fn enabled_hooks(hooks: &[LaunchHook]) -> impl Iterator<Item = &LaunchHook> {
    hooks.iter().filter(|hook| hook.enabled)
}

/// Run enabled pre-launch hooks sequentially. Fail-open: warnings only.
pub fn run_pre_launch_hooks(
    hooks: &[LaunchHook],
    context: &LaunchHookExecutionContext,
) -> Vec<LaunchValidationIssue> {
    run_hooks(
        enabled_hooks(hooks).filter(|hook| hook.stage == HookStage::PreLaunch),
        context,
    )
}

/// Run enabled post-exit hooks sequentially. Fail-open: warnings only.
pub fn run_post_exit_hooks(
    hooks: &[LaunchHook],
    context: &LaunchHookExecutionContext,
) -> Vec<LaunchValidationIssue> {
    run_hooks(
        enabled_hooks(hooks).filter(|hook| hook.stage == HookStage::PostExit),
        context,
    )
}

fn run_hooks<'a>(
    hooks: impl Iterator<Item = &'a LaunchHook>,
    context: &LaunchHookExecutionContext,
) -> Vec<LaunchValidationIssue> {
    let mut warnings = Vec::new();
    for hook in hooks {
        warnings.extend(run_single_hook(hook, context));
    }
    warnings
}

fn run_single_hook(
    hook: &LaunchHook,
    context: &LaunchHookExecutionContext,
) -> Option<LaunchValidationIssue> {
    let normalized_path = normalize_flatpak_host_path(&hook.path);
    let trimmed_path = normalized_path.trim();
    if trimmed_path.is_empty() {
        return Some(LaunchValidationIssue::launch_hook_skipped(
            hook,
            "Launch hook was skipped because no path is configured.",
            "Set an absolute path to the hook script or disable the hook.",
            Some("launch_hook_missing"),
        ));
    }

    if trimmed_path.contains('\0') {
        return Some(LaunchValidationIssue::launch_hook_skipped(
            hook,
            "Launch hook was skipped because the path is invalid.",
            "Use a plain absolute path without embedded null bytes.",
            Some("launch_hook_invalid_path"),
        ));
    }

    if !Path::new(trimmed_path).is_absolute() {
        return Some(LaunchValidationIssue::launch_hook_skipped(
            hook,
            "Launch hook was skipped because the path is not absolute.",
            "Configure an absolute host path for the hook script.",
            Some("launch_hook_invalid_path"),
        ));
    }

    if !normalized_path_is_executable_file_on_host(trimmed_path) {
        return Some(LaunchValidationIssue::launch_hook_skipped(
            hook,
            "Launch hook was skipped because the file is missing or not executable.",
            "Verify the path exists on the host and run chmod +x if needed.",
            Some("launch_hook_not_executable"),
        ));
    }

    let mut command = host_std_command_with_env_and_directory(
        trimmed_path,
        &context.env,
        context.working_directory.as_deref(),
        &context.custom_env_vars,
    );
    command.stdin(Stdio::null());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            tracing::warn!(
                hook_id = %hook.id,
                hook_name = %hook.name,
                hook_stage = ?hook.stage,
                %error,
                "launch hook spawn failed"
            );
            return Some(LaunchValidationIssue::launch_hook_skipped(
                hook,
                "Launch hook could not be started.",
                "Check permissions and that the hook path is executable on the host.",
                Some("launch_hook_spawn_failed"),
            ));
        }
    };

    match wait_with_timeout(&mut child, DEFAULT_HOOK_TIMEOUT) {
        HookWaitOutcome::Success => None,
        HookWaitOutcome::TimedOut => {
            tracing::warn!(
                hook_id = %hook.id,
                hook_name = %hook.name,
                hook_stage = ?hook.stage,
                "launch hook timed out"
            );
            Some(LaunchValidationIssue::launch_hook_timed_out(hook))
        }
        HookWaitOutcome::NonZeroExit(code) => {
            tracing::warn!(
                hook_id = %hook.id,
                hook_name = %hook.name,
                hook_stage = ?hook.stage,
                exit_code = code,
                "launch hook exited non-zero"
            );
            Some(LaunchValidationIssue::launch_hook_non_zero_exit(hook, code))
        }
        HookWaitOutcome::WaitFailed(error) => {
            tracing::warn!(
                hook_id = %hook.id,
                hook_name = %hook.name,
                hook_stage = ?hook.stage,
                %error,
                "launch hook wait failed"
            );
            Some(LaunchValidationIssue::launch_hook_skipped(
                hook,
                "Launch hook exited unexpectedly.",
                "Check the hook script output in your shell before relying on it.",
                Some("launch_hook_failed"),
            ))
        }
    }
}

enum HookWaitOutcome {
    Success,
    TimedOut,
    NonZeroExit(i32),
    WaitFailed(std::io::Error),
}

fn wait_with_timeout(child: &mut Child, timeout: Duration) -> HookWaitOutcome {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return HookWaitOutcome::Success;
                }
                return HookWaitOutcome::NonZeroExit(status.code().unwrap_or(-1));
            }
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return HookWaitOutcome::TimedOut;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(error) => return HookWaitOutcome::WaitFailed(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::request::ValidationSeverity;
    use std::fs;
    use std::sync::{Mutex, MutexGuard};

    static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn env_test_lock() -> MutexGuard<'static, ()> {
        ENV_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[test]
    fn pre_launch_hooks_run_in_order_and_fail_open() {
        let _guard = env_test_lock();
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let log_path = temp_dir.path().join("hook-order.log");
        let hook_a = write_hook_script(temp_dir.path(), "hook-a.sh", &log_path, "a");
        let hook_b = write_hook_script(temp_dir.path(), "hook-b.sh", &log_path, "b");

        let hooks = vec![
            LaunchHook {
                id: "a".to_string(),
                name: "A".to_string(),
                path: hook_a.to_string_lossy().into_owned(),
                stage: HookStage::PreLaunch,
                enabled: true,
            },
            LaunchHook {
                id: "b".to_string(),
                name: "B".to_string(),
                path: hook_b.to_string_lossy().into_owned(),
                stage: HookStage::PreLaunch,
                enabled: true,
            },
        ];

        let warnings = run_pre_launch_hooks(&hooks, &LaunchHookExecutionContext::default());
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        let log = fs::read_to_string(&log_path).expect("log");
        assert_eq!(log.trim(), "a\nb");
    }

    #[test]
    fn disabled_and_missing_hooks_warn_without_running() {
        let _guard = env_test_lock();
        let hooks = vec![LaunchHook {
            id: "missing".to_string(),
            name: "Missing".to_string(),
            path: "/no/such/hook.sh".to_string(),
            stage: HookStage::PreLaunch,
            enabled: true,
        }];

        let warnings = run_pre_launch_hooks(&hooks, &LaunchHookExecutionContext::default());
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].code.as_deref(),
            Some("launch_hook_not_executable")
        );
        assert_eq!(warnings[0].severity, ValidationSeverity::Warning);
    }

    fn write_hook_script(
        dir: &Path,
        name: &str,
        log_path: &Path,
        marker: &str,
    ) -> std::path::PathBuf {
        let script = dir.join(name);
        fs::write(
            &script,
            format!(
                "#!/bin/sh\nprintf '%s\\n' '{marker}' >> '{}'\n",
                log_path.display()
            ),
        )
        .expect("write script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&script).expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&script, permissions).expect("chmod");
        }
        script
    }
}
