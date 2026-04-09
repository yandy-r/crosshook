use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use crosshook_core::run_executable::{
    is_throwaway_prefix_path, run_executable as run_executable_core,
    validate_run_executable_request as validate_run_executable_request_core, RunExecutableError,
    RunExecutableRequest, RunExecutableResult, RunExecutableValidationError,
};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use super::log_stream::spawn_log_stream;
use super::shared::{create_log_path, slugify_target};

/// Pinned absolute path for the `rm -rf` final-fallback in
/// [`cleanup_throwaway_prefix`]. Pinning prevents a hostile or misconfigured
/// `PATH` from resolving `rm` to an attacker-controlled binary in the rare
/// edge cases where `std::fs::remove_dir_all` cannot finish the job.
const RM_BINARY_PATH: &str = "/usr/bin/rm";

#[derive(Debug, Clone)]
struct RunningProcessInfo {
    pid: u32,
    /// Resolved prefix path. Used by [`stop_run_executable`] to drive the
    /// `/proc` env-walk that kills lingering Wine processes attached to the
    /// same prefix. The throwaway/keep-around decision is owned by the
    /// [`spawn_log_stream`] callback below, which captures its own copy of
    /// the prefix path so this struct does not need a separate flag.
    prefix_path: PathBuf,
}

pub struct RunExecutableProcessState {
    info: Mutex<Option<RunningProcessInfo>>,
}

impl RunExecutableProcessState {
    pub fn new() -> Self {
        Self {
            info: Mutex::new(None),
        }
    }
}

/// Locks `info` and silently recovers from poisoning. A panicking previous
/// handler must not permanently disable the cancel/stop pathway for the
/// rest of the session.
fn lock_info(state: &RunExecutableProcessState) -> MutexGuard<'_, Option<RunningProcessInfo>> {
    state
        .info
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Structured error envelope returned to the frontend by every `run_executable`
/// command. The frontend matches on `kind` and uses `field` for input-level
/// error placement, removing the need for any message-text parsing.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunCommandError {
    Validation {
        variant: RunExecutableValidationError,
        field: &'static str,
        message: String,
    },
    Runtime {
        message: String,
    },
}

impl RunCommandError {
    fn runtime(message: impl Into<String>) -> Self {
        Self::Runtime {
            message: message.into(),
        }
    }
}

impl From<RunExecutableValidationError> for RunCommandError {
    fn from(value: RunExecutableValidationError) -> Self {
        Self::Validation {
            field: value.field(),
            message: value.message(),
            variant: value,
        }
    }
}

impl From<RunExecutableError> for RunCommandError {
    fn from(value: RunExecutableError) -> Self {
        match value {
            RunExecutableError::Validation(variant) => RunCommandError::from(variant),
            other => RunCommandError::Runtime {
                message: other.to_string(),
            },
        }
    }
}

#[tauri::command]
pub async fn validate_run_executable_request(
    request: RunExecutableRequest,
) -> Result<(), RunCommandError> {
    tauri::async_runtime::spawn_blocking(move || validate_run_executable_request_core(&request))
        .await
        .map_err(|join_error| RunCommandError::runtime(join_error.to_string()))?
        .map_err(RunCommandError::from)
}

#[tauri::command]
pub async fn run_executable(
    app: AppHandle,
    state: tauri::State<'_, RunExecutableProcessState>,
    request: RunExecutableRequest,
) -> Result<RunExecutableResult, RunCommandError> {
    let executable_stem = Path::new(&request.executable_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_string();
    let slug = slugify_target(&executable_stem, "run-executable");
    let log_path = create_log_path("run-executable", &slug).map_err(RunCommandError::runtime)?;

    // Capture before `request` is moved into spawn_blocking.
    let is_throwaway = request.prefix_path.trim().is_empty();

    let log_path_clone = log_path.clone();
    let (result, child) = tauri::async_runtime::spawn_blocking(move || {
        run_executable_core(&request, &log_path_clone)
    })
    .await
    .map_err(|join_error| RunCommandError::runtime(join_error.to_string()))?
    .map_err(RunCommandError::from)?;

    if let Some(pid) = child.id() {
        *lock_info(&state) = Some(RunningProcessInfo {
            pid,
            prefix_path: PathBuf::from(&result.resolved_prefix_path),
        });
    }

    // Cleanup info captured by value into the post-exit callback. The
    // callback fires when the wrapper child exits via ANY path — natural
    // completion, Cancel-induced exit, or Stop-induced kill — and is the
    // single source of truth for throwaway prefix removal.
    let cleanup_prefix = if is_throwaway {
        Some(PathBuf::from(&result.resolved_prefix_path))
    } else {
        None
    };

    let app_handle_for_clear = app.clone();
    spawn_log_stream(
        app,
        log_path,
        child,
        "run-executable-log",
        "run-executable-complete",
        Box::new(move || {
            if let Some(state) = app_handle_for_clear.try_state::<RunExecutableProcessState>() {
                *lock_info(&state) = None;
            }

            if let Some(prefix) = cleanup_prefix {
                // Fire-and-forget cleanup on a blocking thread so we don't
                // stall the log_stream task on filesystem and process
                // teardown work.
                tauri::async_runtime::spawn_blocking(move || {
                    cleanup_throwaway_prefix(&prefix);
                });
            }
        }),
    );

    Ok(result)
}

/// Polite cancellation. Sends `SIGTERM` to the tracked Proton wrapper PID and
/// lets the executable shut itself down. Use [`stop_run_executable`] when the
/// process is wedged and ignoring `SIGTERM`.
///
/// Prefix cleanup is NOT triggered here directly; once the wrapper exits in
/// response to `SIGTERM`, the [`spawn_log_stream`] callback handles it.
#[tauri::command]
pub async fn cancel_run_executable(
    state: tauri::State<'_, RunExecutableProcessState>,
) -> Result<(), RunCommandError> {
    let info = lock_info(&state).take();

    if let Some(info) = info {
        let _ = std::process::Command::new("kill")
            .arg(info.pid.to_string())
            .status();
    }

    Ok(())
}

/// Forcefully terminate the run-executable process tree.
///
/// 1. `SIGKILL` the tracked Proton wrapper PID.
/// 2. Walk `/proc/[pid]/environ` and `SIGKILL` every process whose env
///    references our prefix path string. Catches `wineserver`, the actual
///    game executable, and any orphaned Wine helpers regardless of process
///    group membership.
///
/// Prefix removal is handled by the [`spawn_log_stream`] callback once the
/// wrapper actually exits — not here — so there is exactly one cleanup path
/// regardless of how a run ended.
#[tauri::command]
pub async fn stop_run_executable(
    state: tauri::State<'_, RunExecutableProcessState>,
) -> Result<(), RunCommandError> {
    let info = lock_info(&state).take();

    let Some(info) = info else {
        return Ok(());
    };

    tauri::async_runtime::spawn_blocking(move || {
        let _ = std::process::Command::new("kill")
            .arg("-KILL")
            .arg(info.pid.to_string())
            .status();

        kill_processes_using_prefix(&info.prefix_path);
    })
    .await
    .map_err(|join_error| RunCommandError::runtime(join_error.to_string()))?;

    Ok(())
}

/// Removes a throwaway prefix directory after the wrapper child exits.
///
/// Strategy:
/// 1. Defense-in-depth: refuse anything that isn't a direct child of the
///    canonical `_run-adhoc/` namespace under the platform data-local dir.
///    The check delegates to [`is_throwaway_prefix_path`] in core so the
///    Tauri layer and the resolver can never disagree.
/// 2. Fast attempt — if `wineserver` already exited cleanly along with the
///    main wine process, the directory is unlocked and `remove_dir_all`
///    succeeds immediately.
/// 3. If the first attempt fails, force-kill every process whose env still
///    references the prefix path, settle briefly, then retry.
/// 4. Final fallback shells out to a pinned `/usr/bin/rm -rf`, which
///    handles a few edge cases that `std::fs::remove_dir_all` does not
///    (read-only bits, etc.).
fn cleanup_throwaway_prefix(prefix_path: &Path) {
    if !is_throwaway_prefix_path(prefix_path) {
        tracing::warn!(
            prefix = %prefix_path.display(),
            "cleanup_throwaway_prefix: refusing to delete prefix outside the canonical _run-adhoc namespace root"
        );
        return;
    }

    if !prefix_path.exists() {
        return;
    }

    if try_remove_dir(prefix_path) {
        return;
    }

    // Wineserver may still be lingering. Force-kill survivors and retry.
    kill_processes_using_prefix(prefix_path);
    std::thread::sleep(Duration::from_millis(300));

    if try_remove_dir(prefix_path) {
        return;
    }

    // Last-ditch fallback: shell out to a pinned `rm -rf` which handles
    // edge cases (read-only bits, etc.) better than
    // `std::fs::remove_dir_all`. The path is hard-coded so a hostile
    // `PATH` cannot redirect us to an attacker-controlled binary.
    let rm_status = std::process::Command::new(RM_BINARY_PATH)
        .arg("-rf")
        .arg(prefix_path)
        .status();

    match rm_status {
        Ok(status) if status.success() => tracing::info!(
            prefix = %prefix_path.display(),
            "cleanup_throwaway_prefix: removed via rm -rf fallback"
        ),
        Ok(status) => tracing::warn!(
            ?status,
            prefix = %prefix_path.display(),
            "cleanup_throwaway_prefix: rm -rf fallback returned non-zero status"
        ),
        Err(error) => tracing::warn!(
            %error,
            prefix = %prefix_path.display(),
            "cleanup_throwaway_prefix: rm -rf fallback failed to spawn"
        ),
    }
}

fn try_remove_dir(prefix_path: &Path) -> bool {
    match std::fs::remove_dir_all(prefix_path) {
        Ok(()) => {
            tracing::info!(
                prefix = %prefix_path.display(),
                "cleanup_throwaway_prefix: removed throwaway prefix"
            );
            true
        }
        Err(error) => {
            tracing::warn!(
                %error,
                prefix = %prefix_path.display(),
                "cleanup_throwaway_prefix: remove_dir_all failed; will retry"
            );
            false
        }
    }
}

/// `SIGKILL`s every process whose `/proc/[pid]/environ` blob contains the
/// prefix path substring.
///
/// Substring matching is intentional: Proton overrides `WINEPREFIX` for
/// child processes (sets it to `<prefix>/pfx` once it bootstraps the
/// structure), but the parent wrapper, intermediate scripts, and the game
/// executable can each carry the prefix path under different keys
/// (`WINEPREFIX`, `STEAM_COMPAT_DATA_PATH`, `WINEDLLPATH`, …) with or
/// without the `pfx` suffix. Searching for the path bytes anywhere in the
/// env blob catches all of them in one pass. The `_run-adhoc/<slug>`
/// namespace is unique enough that false positives are not a real concern.
///
/// Errors reading individual `/proc` entries are silently skipped — exited
/// processes, kernel threads, and processes owned by other users are
/// expected to fail and are not actionable.
fn kill_processes_using_prefix(prefix_path: &Path) {
    // Slugified ad-hoc prefixes are guaranteed ASCII by `slugify`. Assert
    // in debug builds so any future caller passing a non-UTF-8 path is
    // caught immediately rather than silently performing lossy substring
    // matching against `/proc/[pid]/environ` blobs.
    debug_assert!(
        prefix_path.to_str().is_some(),
        "kill_processes_using_prefix expects a UTF-8 path; got non-UTF-8 bytes which will be lossily converted",
    );
    let target_str = prefix_path.to_string_lossy().to_string();
    let target_bytes = target_str.as_bytes();
    if target_bytes.is_empty() {
        return;
    }

    let proc_dir = match std::fs::read_dir("/proc") {
        Ok(dir) => dir,
        Err(error) => {
            tracing::warn!(%error, "kill_processes_using_prefix: unable to read /proc");
            return;
        }
    };

    let mut killed = 0u32;
    for entry in proc_dir.flatten() {
        let name_os = entry.file_name();
        let name = match name_os.to_str() {
            Some(s) => s,
            None => continue,
        };
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }

        let environ_path = entry.path().join("environ");
        let environ_bytes = match std::fs::read(&environ_path) {
            Ok(b) => b,
            Err(_) => continue,
        };

        let target_present = environ_bytes
            .windows(target_bytes.len())
            .any(|window| window == target_bytes);
        if !target_present {
            continue;
        }

        tracing::info!(
            pid = %name,
            prefix = %target_str,
            "kill_processes_using_prefix: SIGKILL"
        );
        let _ = std::process::Command::new("kill")
            .arg("-KILL")
            .arg(name)
            .status();
        killed += 1;
    }

    if killed > 0 {
        tracing::info!(
            killed,
            prefix = %target_str,
            "kill_processes_using_prefix: kill sweep complete"
        );
    }
}
