use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crosshook_core::launch::{
    analyze, build_launch_preview,
    build_steam_launch_options_command as build_steam_launch_options_command_core,
    collect_trainer_hash_launch_warnings,
    diagnostics::FailureMode,
    gamescope_watchdog as gamescope_watchdog_core,
    script_runner::{
        build_flatpak_steam_trainer_command, build_helper_command, build_native_game_command,
        build_proton_game_command, build_proton_trainer_command, build_trainer_command,
        gamescope_pid_capture_path,
    },
    should_register_gamemode_portal, should_surface_report, validate, DiagnosticReport,
    LaunchPreview, LaunchRequest, LaunchValidationIssue, ValidationError, ValidationSeverity,
    METHOD_NATIVE, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
};
use crosshook_core::metadata::{compute_correlation_status, hash_trainer_file, MetadataStore};
use crosshook_core::offline::readiness::MIN_OFFLINE_READINESS_SCORE;
use crosshook_core::platform::portals::gamemode::{self as gamemode_portal, GameModeRegistration};
use crosshook_core::profile::GamescopeConfig;
use crosshook_core::profile::ProfileStore;
use crosshook_core::steam::discover_steam_root_candidates;
use crosshook_core::steam::libraries::discover_steam_libraries;
use crosshook_core::steam::manifest::parse_manifest_full;
use crosshook_core::storage::{check_low_disk_warning, DEFAULT_LOW_DISK_WARNING_MB};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use super::shared::{create_log_path, sanitize_display_path};

const GAMESCOPE_XDG_BACKEND_SOURCE_MARKER: &str = "xdg_backend:";
const GAMESCOPE_XDG_BACKEND_MESSAGE_MARKER: &str =
    "Compositor released us but we were not acquired";
const GAMESCOPE_XDG_BACKEND_SUPPRESSION_NOTICE: &str =
    "[crosshook] Suppressing repeated gamescope xdg_backend console noise. The raw launch log still contains every line.";

#[derive(Debug, Default)]
struct LaunchLogRelayState {
    gamescope_xdg_backend_seen: bool,
    gamescope_xdg_backend_suppressed: usize,
    suppression_notice_emitted: bool,
}

fn is_gamescope_xdg_backend_line(line: &str) -> bool {
    line.contains(GAMESCOPE_XDG_BACKEND_SOURCE_MARKER)
        && line.contains(GAMESCOPE_XDG_BACKEND_MESSAGE_MARKER)
}

fn transform_launch_log_line_for_ui(state: &mut LaunchLogRelayState, line: &str) -> Vec<String> {
    if !is_gamescope_xdg_backend_line(line) {
        return vec![line.to_string()];
    }

    if !state.gamescope_xdg_backend_seen {
        state.gamescope_xdg_backend_seen = true;
        return vec![line.to_string()];
    }

    state.gamescope_xdg_backend_suppressed += 1;
    if !state.suppression_notice_emitted {
        state.suppression_notice_emitted = true;
        return vec![GAMESCOPE_XDG_BACKEND_SUPPRESSION_NOTICE.to_string()];
    }

    Vec::new()
}

fn suppression_summary_line(state: &LaunchLogRelayState) -> Option<String> {
    (state.gamescope_xdg_backend_suppressed > 0).then(|| {
        format!(
            "[crosshook] Suppressed {} repeated gamescope xdg_backend lines from the live console. See the raw launch log for full output.",
            state.gamescope_xdg_backend_suppressed
        )
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct LaunchResult {
    pub succeeded: bool,
    pub message: String,
    pub helper_log_path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<LaunchValidationIssue>,
}

#[derive(Clone)]
struct LaunchStreamContext {
    metadata_store: MetadataStore,
    operation_id: Option<String>,
    steam_app_id: String,
    trainer_host_path: Option<String>,
    profile_name: Option<String>,
    steam_client_path: String,
    watchdog_killed: Arc<AtomicBool>,
}

#[tauri::command]
pub fn validate_launch(request: LaunchRequest) -> Result<(), LaunchValidationIssue> {
    validate(&request).map_err(|error| error.issue())
}

#[tauri::command]
pub fn preview_launch(request: LaunchRequest) -> Result<LaunchPreview, String> {
    build_launch_preview(&request).map_err(|error| error.to_string())
}

/// Builds a Steam per-game “Launch Options” line from the same optimization IDs as `proton_run`,
/// plus profile custom env vars (custom wins on duplicate keys in the prefix).
///
/// When `gamescope` is provided and enabled, the gamescope compositor is inserted as a wrapper
/// (e.g. `gamescope -w 2560 -h 1440 -f -- %command%`).
#[tauri::command]
pub fn build_steam_launch_options_command(
    enabled_option_ids: Vec<String>,
    custom_env_vars: BTreeMap<String, String>,
    gamescope: Option<GamescopeConfig>,
) -> Result<String, String> {
    build_steam_launch_options_command_core(
        &enabled_option_ids,
        &custom_env_vars,
        gamescope.as_ref(),
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn check_gamescope_session() -> bool {
    crosshook_core::launch::is_inside_gamescope_session()
}

/// Returns Flatpak sandbox and host capability flags for launch UI (not persisted).
#[tauri::command]
pub fn launch_platform_status() -> crosshook_core::launch::LaunchPlatformCapabilities {
    crosshook_core::launch::launch_platform_capabilities()
}

#[tauri::command]
pub fn check_game_running(exe_name: String) -> bool {
    let name = exe_name.trim();
    if name.is_empty() {
        return false;
    }
    crosshook_core::launch::is_process_running(name)
}

/// Non-blocking offline readiness advisory when the profile has a trainer configured.
async fn collect_offline_launch_warnings(
    request: &LaunchRequest,
    profile_name: Option<String>,
    profile_store: ProfileStore,
    metadata_store: MetadataStore,
) -> Vec<LaunchValidationIssue> {
    let mut warnings = collect_low_disk_warning(request).await;
    let Some(name) = profile_name.filter(|n| !n.trim().is_empty()) else {
        return warnings;
    };
    if !metadata_store.is_available() {
        return warnings;
    }
    let ps = profile_store;
    let ms = metadata_store;
    let mut offline_warnings = tauri::async_runtime::spawn_blocking(move || {
        let profile = match ps.load(&name) {
            Ok(p) => p,
            Err(_) => return Vec::new(),
        };
        if profile.effective_profile().trainer.path.trim().is_empty() {
            return Vec::new();
        }
        let profile_id = match ms.lookup_profile_id(&name) {
            Ok(Some(id)) => id,
            Ok(None) | Err(_) => return Vec::new(),
        };
        let report = match ms.check_offline_readiness_for_profile(&name, &profile_id, &profile) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        if report.score >= MIN_OFFLINE_READINESS_SCORE {
            return Vec::new();
        }
        vec![ValidationError::OfflineReadinessInsufficient {
            score: report.score,
            reasons: report.blocking_reasons.clone(),
        }
        .issue()]
    })
    .await
    .unwrap_or_default();
    warnings.append(&mut offline_warnings);
    warnings
}

/// SHA-256 baseline / community digest advisory (non-blocking).
async fn collect_trainer_hash_launch_warnings_ipc(
    profile_name: Option<String>,
    profile_store: ProfileStore,
    metadata_store: MetadataStore,
) -> Vec<LaunchValidationIssue> {
    let Some(name) = profile_name.filter(|n| !n.trim().is_empty()) else {
        return Vec::new();
    };
    if !metadata_store.is_available() {
        return Vec::new();
    }
    let ps = profile_store;
    let ms = metadata_store;
    tauri::async_runtime::spawn_blocking(move || {
        let profile = match ps.load(&name) {
            Ok(p) => p,
            Err(_) => return Vec::new(),
        };
        let profile_id = match ms.lookup_profile_id(&name) {
            Ok(Some(id)) => id,
            Ok(None) | Err(_) => return Vec::new(),
        };
        collect_trainer_hash_launch_warnings(&ms, &profile_id, &profile)
    })
    .await
    .unwrap_or_default()
}

async fn collect_low_disk_warning(request: &LaunchRequest) -> Vec<LaunchValidationIssue> {
    let raw_prefix_path = if request.resolved_method() == METHOD_STEAM_APPLAUNCH {
        request.steam.compatdata_path.trim()
    } else {
        request.runtime.prefix_path.trim()
    };
    if raw_prefix_path.is_empty() {
        return Vec::new();
    }

    let prefix_path = PathBuf::from(raw_prefix_path);
    let check_result = tauri::async_runtime::spawn_blocking(move || {
        check_low_disk_warning(&prefix_path, DEFAULT_LOW_DISK_WARNING_MB)
    })
    .await;

    let warning = match check_result {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => {
            tracing::warn!(path = raw_prefix_path, %error, "low-disk check failed");
            None
        }
        Err(error) => {
            tracing::warn!(path = raw_prefix_path, %error, "low-disk check task failed");
            None
        }
    };

    match warning {
        Some(value) => vec![ValidationError::LowDiskSpaceAdvisory {
            available_mb: value.available_bytes / (1024 * 1024),
            threshold_mb: value.threshold_bytes / (1024 * 1024),
            mount_path: value.mount_path,
        }
        .issue()],
        None => Vec::new(),
    }
}

#[tauri::command]
pub async fn launch_game(
    app: AppHandle,
    request: LaunchRequest,
    profile_store: State<'_, ProfileStore>,
) -> Result<LaunchResult, String> {
    let mut request = request;
    request.launch_game_only = true;
    request.launch_trainer_only = false;
    validate(&request).map_err(|error| error.to_string())?;
    let profile_store = profile_store.inner().clone();
    let metadata_store = app.state::<MetadataStore>().inner().clone();
    let mut warnings = collect_offline_launch_warnings(
        &request,
        request.profile_name.clone(),
        profile_store.clone(),
        metadata_store.clone(),
    )
    .await;
    warnings.append(
        &mut collect_trainer_hash_launch_warnings_ipc(
            request.profile_name.clone(),
            profile_store,
            metadata_store.clone(),
        )
        .await,
    );
    let method: &'static str = match request.resolved_method() {
        METHOD_STEAM_APPLAUNCH => METHOD_STEAM_APPLAUNCH,
        METHOD_PROTON_RUN => METHOD_PROTON_RUN,
        METHOD_NATIVE => METHOD_NATIVE,
        _ => METHOD_NATIVE,
    };

    let log_path = create_log_path("game", &request.log_target_slug())?;
    let flatpak_gamescope_pid_capture = if request.resolved_method() == METHOD_PROTON_RUN
        && crosshook_core::platform::is_flatpak()
        && request.gamescope.enabled
        && (request.gamescope.allow_nested
            || !crosshook_core::launch::is_inside_gamescope_session())
    {
        Some(gamescope_pid_capture_path(&log_path))
    } else {
        None
    };
    let mut command = match method {
        METHOD_STEAM_APPLAUNCH => {
            let script_path = resolve_script_path(&app, "steam-launch-helper.sh")?;
            build_helper_command(&request, &script_path, &log_path)
                .map_err(|error| format!("failed to build Steam helper launch: {error}"))?
        }
        METHOD_PROTON_RUN => {
            let mut command = build_proton_game_command(&request, &log_path)
                .map_err(|error| format!("failed to build Proton game launch: {error}"))?;
            command.stdout(Stdio::piped());
            command.stderr(Stdio::piped());
            command
        }
        METHOD_NATIVE => build_native_game_command(&request, &log_path)
            .map_err(|error| format!("failed to build native game launch: {error}"))?,
        other => return Err(format!("unsupported launch method: {other}")),
    };

    // Register CrossHook's own PID with the GameMode portal before spawning
    // the host command, if the user enabled `use_gamemode` under Flatpak.
    // Host games still receive the `gamemoderun` wrapper via the optimization
    // catalog (ADR-0002 § GameMode portal contract). Games use the resolved
    // method as-is — Flatpak Steam game launches go through the helper script
    // and never apply CrossHook-side `gamemoderun` wrapping, so the portal
    // decision follows the actual execution path.
    let gamemode_portal_guard = try_register_gamemode_portal_for_launch(&request, method).await;

    let child = command
        .spawn()
        .map_err(|error| format!("failed to launch helper: {error}"))?;
    let child_pid = child.id();

    let sanitized_log_path = sanitize_display_path(&log_path.to_string_lossy());
    let operation_id = record_launch_start(
        &metadata_store,
        request.profile_name.as_deref(),
        method,
        &sanitized_log_path,
    )
    .await;

    // Extract before spawn_log_stream — request is moved into the closure
    let snap_steam_app_id = request.steam.app_id.clone();
    let snap_trainer_host_path = {
        let p = request.trainer_host_path.trim().to_string();
        if p.is_empty() {
            None
        } else {
            Some(p)
        }
    };
    let snap_profile_name = request.profile_name.clone();
    let snap_steam_client_path = request.steam.steam_client_install_path.clone();

    // Determine if gamescope is the direct child (it won't exit when the game
    // exits, so a watchdog is needed to terminate it).
    let gamescope_active = method == METHOD_PROTON_RUN
        && request.gamescope.enabled
        && (request.gamescope.allow_nested
            || !crosshook_core::launch::is_inside_gamescope_session());
    let game_exe_name = Path::new(&request.game_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    // Shared flag: the gamescope watchdog sets this before killing the
    // compositor so finalize_launch_stream can suppress the false SIGKILL
    // diagnostic that would otherwise surface as a warning.
    let watchdog_killed = Arc::new(AtomicBool::new(false));
    let stream_context = LaunchStreamContext {
        metadata_store,
        operation_id,
        steam_app_id: snap_steam_app_id,
        trainer_host_path: snap_trainer_host_path,
        profile_name: snap_profile_name,
        steam_client_path: snap_steam_client_path,
        watchdog_killed: Arc::clone(&watchdog_killed),
    };

    let watchdog_app_handle = app.clone();
    spawn_log_stream(
        app,
        log_path.clone(),
        child,
        method,
        stream_context,
        gamemode_portal_guard,
    );

    if gamescope_active {
        if let Some(pid) = child_pid {
            spawn_gamescope_watchdog(
                &watchdog_app_handle,
                pid,
                game_exe_name,
                watchdog_killed,
                flatpak_gamescope_pid_capture,
            );
        }
    }

    Ok(LaunchResult {
        succeeded: true,
        message: "Game launch started.".to_string(),
        helper_log_path: log_path.to_string_lossy().into_owned(),
        warnings,
    })
}

#[tauri::command]
pub async fn launch_trainer(
    app: AppHandle,
    request: LaunchRequest,
    profile_store: State<'_, ProfileStore>,
) -> Result<LaunchResult, String> {
    let mut request = request;
    request.launch_trainer_only = true;
    request.launch_game_only = false;
    validate(&request).map_err(|error| error.to_string())?;
    let profile_store = profile_store.inner().clone();
    let metadata_store = app.state::<MetadataStore>().inner().clone();
    let mut warnings = collect_offline_launch_warnings(
        &request,
        request.profile_name.clone(),
        profile_store.clone(),
        metadata_store.clone(),
    )
    .await;
    warnings.append(
        &mut collect_trainer_hash_launch_warnings_ipc(
            request.profile_name.clone(),
            profile_store,
            metadata_store.clone(),
        )
        .await,
    );
    let resolved_method: &'static str = match request.resolved_method() {
        METHOD_STEAM_APPLAUNCH => METHOD_STEAM_APPLAUNCH,
        METHOD_PROTON_RUN => METHOD_PROTON_RUN,
        METHOD_NATIVE => METHOD_NATIVE,
        _ => METHOD_NATIVE,
    };
    let is_flatpak = crosshook_core::platform::is_flatpak();
    let execution_method: &'static str = if resolved_method == METHOD_STEAM_APPLAUNCH && is_flatpak
    {
        METHOD_PROTON_RUN
    } else {
        resolved_method
    };

    let log_path = create_log_path("trainer", &request.log_target_slug())?;
    let mut command = match resolved_method {
        METHOD_STEAM_APPLAUNCH => {
            if crosshook_core::platform::is_flatpak() {
                let mut command = build_flatpak_steam_trainer_command(&request, &log_path)
                    .map_err(|error| {
                        format!("failed to build Flatpak Steam trainer launch: {error}")
                    })?;
                command.stdout(Stdio::piped());
                command.stderr(Stdio::piped());
                command
            } else {
                let script_path = resolve_script_path(&app, "steam-launch-trainer.sh")?;
                build_trainer_command(&request, &script_path, &log_path)
                    .map_err(|error| format!("failed to build Steam trainer launch: {error}"))?
            }
        }
        METHOD_PROTON_RUN => {
            let mut command = build_proton_trainer_command(&request, &log_path)
                .map_err(|error| format!("failed to build Proton trainer launch: {error}"))?;
            command.stdout(Stdio::piped());
            command.stderr(Stdio::piped());
            command
        }
        METHOD_NATIVE => return Err("native launch does not support trainer launch.".to_string()),
        other => return Err(format!("unsupported launch method: {other}")),
    };
    // Register CrossHook's own PID with the GameMode portal before spawning
    // the trainer. We key off `execution_method` (not the request's parent
    // method) because Flatpak Steam trainer launches rewrite the trainer
    // subprocess to run through Proton directly — `gamemoderun` is applied
    // and the portal self-registration must fire in lockstep. This matches
    // the trainer-execution-parity rule in CLAUDE.md.
    let gamemode_portal_guard =
        try_register_gamemode_portal_for_launch(&request, execution_method).await;

    let child = command.spawn().map_err(|error| {
        format!("failed to launch trainer (method={execution_method}): {error}")
    })?;

    let sanitized_log_path = sanitize_display_path(&log_path.to_string_lossy());
    let operation_id = record_launch_start(
        &metadata_store,
        request.profile_name.as_deref(),
        execution_method,
        &sanitized_log_path,
    )
    .await;

    // Extract before spawn_log_stream — request is moved into the closure
    let snap_steam_app_id = request.steam.app_id.clone();
    let snap_trainer_host_path = {
        let p = request.trainer_host_path.trim().to_string();
        if p.is_empty() {
            None
        } else {
            Some(p)
        }
    };
    let snap_profile_name = request.profile_name.clone();
    let snap_steam_client_path = request.steam.steam_client_install_path.clone();

    // No gamescope watchdog for trainer launches. Trainers often exit quickly
    // after injection while their effects persist inside the game process —
    // the exe-based watchdog would tear down gamescope prematurely. Trainer
    // gamescope cleanup is handled by the game's own watchdog when the game
    // exits, since they share the same Wine prefix / process neighbourhood.
    let watchdog_killed = Arc::new(AtomicBool::new(false));
    let stream_context = LaunchStreamContext {
        metadata_store,
        operation_id,
        steam_app_id: snap_steam_app_id,
        trainer_host_path: snap_trainer_host_path,
        profile_name: snap_profile_name,
        steam_client_path: snap_steam_client_path,
        watchdog_killed: Arc::clone(&watchdog_killed),
    };

    spawn_log_stream(
        app,
        log_path.clone(),
        child,
        execution_method,
        stream_context,
        gamemode_portal_guard,
    );

    Ok(LaunchResult {
        succeeded: true,
        message: "Trainer launch started.".to_string(),
        helper_log_path: log_path.to_string_lossy().into_owned(),
        warnings,
    })
}

fn spawn_log_stream(
    app: AppHandle,
    log_path: PathBuf,
    child: tokio::process::Child,
    method: &'static str,
    context: LaunchStreamContext,
    gamemode_portal_guard: Option<GameModeRegistration>,
) {
    let child_uses_pipe_capture = child.stdout.is_some() || child.stderr.is_some();
    let handle = tauri::async_runtime::spawn(async move {
        stream_log_lines(
            app,
            log_path,
            child,
            child_uses_pipe_capture,
            method,
            context,
        )
        .await;
        // The GameMode portal registration is released here, when the launch
        // stream ends (child exited). This matches ADR-0002 lifetime: register
        // around spawn, unregister when the orchestrated process exits.
        if let Some(guard) = gamemode_portal_guard {
            if let Err(error) = guard.unregister().await {
                tracing::warn!(%error, "gamemode portal: UnregisterGame failed on launch end");
            }
        }
    });

    tauri::async_runtime::spawn(async move {
        if let Err(error) = handle.await {
            tracing::error!(%error, "launch log stream task failed");
        }
    });
}

/// Attempts to register CrossHook's own sandbox-side PID with the GameMode
/// portal, if the request and environment warrant it.
///
/// `effective_method` is the method under which the child process will
/// actually run. For direct Proton game launches this equals
/// `request.resolved_method()`; for Flatpak Steam trainer launches it is
/// rewritten to `METHOD_PROTON_RUN` because the helper spawns the trainer
/// through Proton directly (see
/// `crosshook_core::launch::script_runner::build_flatpak_steam_trainer_command`).
///
/// Returns `None` when:
/// - the request does not enable `use_gamemode`, or the effective method is
///   not `proton_run`, or we are not running under Flatpak
///   (`should_register_gamemode_portal` short-circuits these).
/// - the portal is not reachable on the session bus.
/// - the portal's `RegisterGame` call fails.
///
/// In the failure cases the launch proceeds normally; host games continue
/// to use the `gamemoderun` wrapper through the optimization catalog. This
/// function never blocks the launch — it returns `None` on any error after
/// logging a single `tracing::warn!`.
async fn try_register_gamemode_portal_for_launch(
    request: &LaunchRequest,
    effective_method: &str,
) -> Option<GameModeRegistration> {
    if !should_register_gamemode_portal(request, effective_method) {
        return None;
    }
    if !gamemode_portal::portal_available().await {
        tracing::info!(
            "gamemode portal registration skipped: org.freedesktop.portal.GameMode not reachable"
        );
        return None;
    }
    match gamemode_portal::register_self_pid_with_portal().await {
        Ok(guard) => {
            tracing::info!(
                registered_pid = guard.registered_pid(),
                "gamemode portal registration: backend=Portal"
            );
            Some(guard)
        }
        Err(error) => {
            tracing::warn!(%error, "gamemode portal: RegisterGame failed; falling back to host gamemoderun wrapper only");
            None
        }
    }
}

async fn record_launch_start(
    metadata_store: &MetadataStore,
    profile_name: Option<&str>,
    method: &'static str,
    log_path: &str,
) -> Option<String> {
    let ms_clone = metadata_store.clone();
    let pn = profile_name.map(str::to_owned);
    let lp = log_path.to_string();
    let operation_id: String = tauri::async_runtime::spawn_blocking(move || {
        ms_clone.record_launch_started(pn.as_deref(), method, Some(&lp))
    })
    .await
    .unwrap_or_else(|e| {
        tracing::warn!("metadata spawn_blocking join failed: {e}");
        Ok(String::new())
    })
    .unwrap_or_else(|e| {
        tracing::warn!(%e, "record_launch_started failed");
        String::new()
    });

    if operation_id.is_empty() {
        None
    } else {
        Some(operation_id)
    }
}

async fn stream_log_lines(
    app: AppHandle,
    log_path: PathBuf,
    child: tokio::process::Child,
    child_uses_pipe_capture: bool,
    method: &'static str,
    context: LaunchStreamContext,
) {
    if child_uses_pipe_capture {
        let exit_status = stream_log_pipes(&app, &log_path, child).await;
        finalize_launch_stream(app, log_path, exit_status, method, context).await;
        return;
    }

    let mut child = child;
    let mut last_len = 0usize;
    let mut exit_status: Option<std::process::ExitStatus> = None;
    let mut relay_state = LaunchLogRelayState::default();

    loop {
        match tokio::fs::read_to_string(&log_path).await {
            Ok(content) => {
                if content.len() < last_len {
                    last_len = 0;
                }

                if content.len() > last_len {
                    let chunk = &content[last_len..];
                    for line in chunk.lines() {
                        if !line.is_empty() {
                            for ui_line in transform_launch_log_line_for_ui(&mut relay_state, line)
                            {
                                if let Err(error) = app.emit("launch-log", ui_line) {
                                    tracing::warn!(%error, "failed to emit launch log line; continuing stream");
                                }
                            }
                        }
                    }
                    last_len = content.len();
                }
            }
            Err(error) => {
                tracing::warn!(%error, path = %log_path.display(), "failed to read launch log file");
            }
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                exit_status = Some(status);
                break;
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(%error, "failed to check child process status");
                break;
            }
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // Final read to capture lines written between last poll and process exit
    if let Ok(content) = tokio::fs::read_to_string(&log_path).await {
        if content.len() > last_len {
            for line in content[last_len..].lines().filter(|l| !l.is_empty()) {
                for ui_line in transform_launch_log_line_for_ui(&mut relay_state, line) {
                    if let Err(error) = app.emit("launch-log", ui_line) {
                        tracing::warn!(%error, "failed to emit final launch log line");
                    }
                }
            }
        }
    }
    if let Some(summary_line) = suppression_summary_line(&relay_state) {
        if let Err(error) = app.emit("launch-log", summary_line) {
            tracing::warn!(%error, "failed to emit launch log suppression summary");
        }
    }

    finalize_launch_stream(app, log_path, exit_status, method, context).await;
}

async fn stream_log_pipes(
    app: &AppHandle,
    log_path: &Path,
    mut child: tokio::process::Child,
) -> Option<std::process::ExitStatus> {
    let Some(stdout) = child.stdout.take() else {
        return child.wait().await.ok();
    };
    let stderr = child.stderr.take();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    tauri::async_runtime::spawn(pipe_reader_task(stdout, tx.clone()));
    if let Some(stderr) = stderr {
        tauri::async_runtime::spawn(pipe_reader_task(stderr, tx.clone()));
    }
    drop(tx);

    let wait_handle = tauri::async_runtime::spawn(async move { child.wait().await.ok() });

    let mut log_file = match tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .await
    {
        Ok(file) => Some(file),
        Err(error) => {
            tracing::warn!(%error, path = %log_path.display(), "failed to open pipe-backed launch log");
            None
        }
    };
    let mut relay_state = LaunchLogRelayState::default();
    while let Some(line) = rx.recv().await {
        if let Some(file) = log_file.as_mut() {
            if let Err(error) = file.write_all(format!("{line}\n").as_bytes()).await {
                tracing::warn!(%error, path = %log_path.display(), "failed to append pipe-backed launch log line");
                log_file = None;
            }
        }
        if !line.is_empty() {
            for ui_line in transform_launch_log_line_for_ui(&mut relay_state, &line) {
                if let Err(error) = app.emit("launch-log", ui_line) {
                    tracing::warn!(%error, "failed to emit launch log line");
                }
            }
        }
    }

    if let Some(file) = log_file.as_mut() {
        if let Err(error) = file.flush().await {
            tracing::warn!(%error, path = %log_path.display(), "failed to flush pipe-backed launch log");
        }
    }

    let exit_status = match wait_handle.await {
        Ok(status) => status,
        Err(error) => {
            tracing::warn!(%error, "pipe-backed child wait task failed");
            None
        }
    };
    if let Some(summary_line) = suppression_summary_line(&relay_state) {
        if let Err(error) = app.emit("launch-log", summary_line) {
            tracing::warn!(%error, "failed to emit launch log suppression summary");
        }
    }
    exit_status
}

async fn pipe_reader_task<R>(reader: R, tx: mpsc::UnboundedSender<String>)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    let mut lines = BufReader::new(reader).lines();
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                if tx.send(line).is_err() {
                    break;
                }
            }
            Ok(None) => break,
            Err(error) => {
                let _ = tx.send(format!("crosshook log capture error: {error}"));
                break;
            }
        }
    }
}

async fn finalize_launch_stream(
    app: AppHandle,
    log_path: PathBuf,
    exit_status: Option<std::process::ExitStatus>,
    method: &'static str,
    context: LaunchStreamContext,
) {
    let exit_code = exit_status
        .as_ref()
        .and_then(std::process::ExitStatus::code);
    let signal = exit_status
        .as_ref()
        .and_then(std::os::unix::process::ExitStatusExt::signal);

    let log_tail = safe_read_tail(
        &log_path,
        crosshook_core::launch::diagnostics::MAX_LOG_TAIL_BYTES,
    )
    .await;
    let diagnostic_method = diagnostic_method_for_log(method, &log_tail);
    let mut report = analyze(exit_status, &log_tail, diagnostic_method);
    report.log_tail_path = Some(sanitize_display_path(&log_path.to_string_lossy()));
    let mut report = sanitize_diagnostic_report(report);

    // When the gamescope watchdog killed the compositor the exit signal is
    // SIGTERM or SIGKILL — a false positive that would surface as a warning.
    // Override to CleanExit so the UI treats it as a normal shutdown.
    if context.watchdog_killed.load(Ordering::Acquire) {
        report.exit_info.failure_mode = FailureMode::CleanExit;
        report.summary = "Game exited; gamescope compositor cleaned up.".to_string();
        report.exit_info.description = "Game exited; gamescope compositor cleaned up.".to_string();
        report.exit_info.severity = ValidationSeverity::Info;
        report.suggestions.clear();
    }

    if let Some(ref op_id) = context.operation_id {
        let ms = context.metadata_store.clone();
        let op = op_id.clone();
        let ec = exit_code;
        let sig = signal;
        let rpt = report.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            if let Err(e) = ms.record_launch_finished(&op, ec, sig, &rpt) {
                tracing::warn!(%e, operation_id = %op, "record_launch_finished failed");
            }
        })
        .await;
        if let Err(e) = result {
            tracing::warn!(%e, operation_id = %op_id, "record_launch_finished join failed");
        }
    }

    if matches!(
        report.exit_info.failure_mode,
        FailureMode::CleanExit | FailureMode::Indeterminate
    ) {
        if let Some(ref pname) = context.profile_name {
            let ms = context.metadata_store.clone();
            let pname_c = pname.clone();
            let app_id_c = context.steam_app_id.clone();
            let trainer_path_c = context.trainer_host_path.clone();
            let steam_install_c = context.steam_client_path.clone();
            let result = tauri::async_runtime::spawn_blocking(move || {
                let profile_id = match ms.lookup_profile_id(&pname_c) {
                    Ok(Some(id)) => id,
                    Ok(None) => {
                        tracing::warn!(profile_name = %pname_c, "version snapshot skipped: profile not in metadata");
                        return;
                    }
                    Err(e) => {
                        tracing::warn!(%e, "version snapshot skipped: lookup_profile_id failed");
                        return;
                    }
                };

                let manifest_data = if !app_id_c.trim().is_empty() {
                    let mut diag = Vec::new();
                    let roots = discover_steam_root_candidates(&steam_install_c, &mut diag);
                    let libraries = discover_steam_libraries(&roots, &mut diag);
                    libraries.iter().find_map(|lib| {
                        let p = lib.steamapps_path.join(format!("appmanifest_{}.acf", app_id_c.trim()));
                        if p.is_file() { parse_manifest_full(&p).ok() } else { None }
                    })
                } else {
                    None
                };

                let build_id = manifest_data.as_ref().map(|d| d.build_id.clone());
                let current_build_id = build_id.as_deref().unwrap_or("");
                let state_flags = manifest_data.as_ref().and_then(|d| d.state_flags);

                let trainer_file_hash = trainer_path_c
                    .as_deref()
                    .and_then(|p| hash_trainer_file(std::path::Path::new(p)));

                let prior_snapshot = match ms.lookup_latest_version_snapshot(&profile_id) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!(%e, "version snapshot: prior lookup failed, continuing");
                        None
                    }
                };

                let snapshot_build_id = prior_snapshot.as_ref().and_then(|s| s.steam_build_id.as_deref());
                let snapshot_trainer_hash = prior_snapshot.as_ref().and_then(|s| s.trainer_file_hash.as_deref());

                let status = compute_correlation_status(
                    current_build_id,
                    snapshot_build_id,
                    trainer_file_hash.as_deref(),
                    snapshot_trainer_hash,
                    state_flags,
                );

                if matches!(status, crosshook_core::metadata::VersionCorrelationStatus::UpdateInProgress) {
                    tracing::debug!(profile_name = %pname_c, "version snapshot skipped: game update in progress");
                    return;
                }

                if let Err(e) = ms.upsert_version_snapshot(
                    &profile_id,
                    app_id_c.trim(),
                    if current_build_id.is_empty() { None } else { Some(current_build_id) },
                    None,
                    trainer_file_hash.as_deref(),
                    None,
                    status.as_str(),
                ) {
                    tracing::warn!(%e, "version snapshot upsert failed");
                }
            })
            .await;
            if let Err(e) = result {
                tracing::warn!(%e, "version snapshot spawn_blocking join failed");
            }
        }

        if let Some(ref pname) = context.profile_name {
            let ms = context.metadata_store.clone();
            let pname_c = pname.clone();
            let result = tauri::async_runtime::spawn_blocking(move || {
                let profile_id = match ms.lookup_profile_id(&pname_c) {
                    Ok(Some(id)) => id,
                    Ok(None) => {
                        tracing::debug!(profile_name = %pname_c, "known-good tagging skipped: profile not in metadata");
                        return;
                    }
                    Err(e) => {
                        tracing::warn!(%e, "known-good tagging skipped: lookup_profile_id failed");
                        return;
                    }
                };

                let revisions = match ms.list_config_revisions(&profile_id, Some(1)) {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!(%e, "known-good tagging skipped: list_config_revisions failed");
                        return;
                    }
                };

                let latest = match revisions.into_iter().next() {
                    Some(r) => r,
                    None => {
                        tracing::debug!(profile_name = %pname_c, "known-good tagging skipped: no config revisions exist");
                        return;
                    }
                };

                if let Err(e) = ms.set_known_good_revision(&profile_id, latest.id) {
                    tracing::warn!(%e, "known-good tagging failed: set_known_good_revision failed");
                }
            })
            .await;
            if let Err(e) = result {
                tracing::warn!(%e, "known-good tagging spawn_blocking join failed");
            }
        }
    }

    if should_surface_report(&report) {
        if let Err(error) = app.emit("launch-diagnostic", &report) {
            tracing::warn!(%error, "failed to emit launch-diagnostic event");
        }
    }

    if let Err(error) = app.emit(
        "launch-complete",
        serde_json::json!({
            "code": exit_code,
            "signal": signal,
        }),
    ) {
        tracing::warn!(%error, "failed to emit launch-complete event");
    }
}

fn resolve_script_path(app: &AppHandle, script_name: &str) -> Result<PathBuf, String> {
    for resource_name in [
        script_name.to_string(),
        format!("runtime-helpers/{script_name}"),
        format!("_up_/runtime-helpers/{script_name}"),
    ] {
        if let Ok(path) = app
            .path()
            .resolve(&resource_name, tauri::path::BaseDirectory::Resource)
        {
            if path.exists() {
                tracing::debug!(path = %path.display(), script_name, resource_name, "resolved bundled launch script");
                return Ok(path);
            }
        }
    }

    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("runtime-helpers")
        .join(script_name);

    if dev_path.exists() {
        tracing::debug!(path = %dev_path.display(), script_name, "falling back to development launch script");
        Ok(dev_path)
    } else {
        Err(format!(
            "unable to resolve launch script '{script_name}': neither bundled nor development path exists"
        ))
    }
}

async fn safe_read_tail(path: &Path, max_bytes: u64) -> String {
    let file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(error) => {
            tracing::warn!(%error, path = %path.display(), "failed to open launch log tail");
            return String::new();
        }
    };

    let metadata = match file.metadata().await {
        Ok(metadata) => metadata,
        Err(error) => {
            tracing::warn!(%error, path = %path.display(), "failed to read launch log metadata");
            return String::new();
        }
    };

    let mut file = file;
    if metadata.len() > max_bytes {
        let offset = -(max_bytes as i64);
        if let Err(error) = file.seek(std::io::SeekFrom::End(offset)).await {
            tracing::warn!(%error, path = %path.display(), "failed to seek launch log tail");
            return String::new();
        }
    }

    let mut buffer = Vec::new();
    if let Err(error) = file.read_to_end(&mut buffer).await {
        tracing::warn!(%error, path = %path.display(), "failed to read launch log tail");
        return String::new();
    }

    String::from_utf8_lossy(&buffer).into_owned()
}

fn sanitize_diagnostic_report(mut report: DiagnosticReport) -> DiagnosticReport {
    report.summary = sanitize_display_path(&report.summary);
    report.exit_info.description = sanitize_display_path(&report.exit_info.description);
    report.launch_method = sanitize_display_path(&report.launch_method);
    report.log_tail_path = report.log_tail_path.as_deref().map(sanitize_display_path);

    for pattern_match in &mut report.pattern_matches {
        pattern_match.summary = sanitize_display_path(&pattern_match.summary);
        pattern_match.suggestion = sanitize_display_path(&pattern_match.suggestion);
        pattern_match.matched_line = pattern_match
            .matched_line
            .as_deref()
            .map(sanitize_display_path);
    }

    for suggestion in &mut report.suggestions {
        suggestion.title = sanitize_display_path(&suggestion.title);
        suggestion.description = sanitize_display_path(&suggestion.description);
    }

    report
}

fn diagnostic_method_for_log(method: &'static str, log_tail: &str) -> &'static str {
    if method == METHOD_STEAM_APPLAUNCH
        && log_tail.contains("[steam-trainer-runner]")
        && log_tail.contains("trainer_launch_mode=")
    {
        METHOD_PROTON_RUN
    } else {
        method
    }
}

// ---------------------------------------------------------------------------
// Gamescope watchdog
// ---------------------------------------------------------------------------

/// When gamescope wraps a Proton launch it becomes the direct child of
/// CrossHook but does **not** exit when the game inside it exits — lingering
/// clients (`mangoapp`, `winedevice.exe`, `gamescopereaper`) keep the
/// compositor alive indefinitely. This watchdog polls for the game executable
/// and, once it disappears, terminates gamescope so the normal
/// stream-log / finalize cleanup path can proceed.
///
/// Under Flatpak the watchdog is a sandbox-side Tokio task and is subject to
/// sandbox reclaim when the Tauri window is minimized. The Background portal
/// grant requested at app startup (ADR-0002 § Background portal contract)
/// tells xdg-desktop-portal to keep CrossHook alive; here we simply log the
/// grant state at spawn time so a failed watchdog run can be correlated with
/// a missing/denied grant.
fn spawn_gamescope_watchdog(
    app: &AppHandle,
    gamescope_pid: u32,
    exe_name: String,
    killed_flag: Arc<AtomicBool>,
    host_pid_capture_path: Option<PathBuf>,
) {
    if exe_name.is_empty() {
        tracing::warn!(
            gamescope_pid,
            "gamescope watchdog disabled: launch path did not yield an executable basename"
        );
        return;
    }
    // Under Flatpak, synchronize with the one-time `request_background` call
    // kicked off in `.setup(...)` (ADR-0002 § Background portal contract).
    // Awaiting here (with a short timeout) inside the spawned task avoids
    // racing the log/decision on the holder's initial `Pending` state;
    // functionally the portal's session-scoped grant protects CrossHook
    // regardless of when the watchdog spawned, but the synchronized log
    // makes diagnostics accurate.
    let grant_check_handle =
        if crosshook_core::platform::portals::background::background_supported() {
            let holder_handle = app.clone();
            Some(tauri::async_runtime::spawn(async move {
                let holder = holder_handle.state::<crate::BackgroundGrantHolder>();
                let state = holder
                    .wait_for_initialization(std::time::Duration::from_millis(500))
                    .await;
                tracing::info!(
                    gamescope_pid,
                    protection_state = ?state,
                    active_grant = holder.has_active_grant(),
                    initialized = holder.is_initialized(),
                    "background portal grant state at watchdog spawn"
                );
            }))
        } else {
            None
        };

    tauri::async_runtime::spawn(async move {
        if let Some(handle) = grant_check_handle {
            let _ = handle.await;
        }
        gamescope_watchdog_core(gamescope_pid, &exe_name, killed_flag, host_pid_capture_path).await;
    });
}

#[cfg(test)]
mod tests {
    use super::{
        diagnostic_method_for_log, suppression_summary_line, transform_launch_log_line_for_ui,
        LaunchLogRelayState,
    };
    use crosshook_core::launch::{METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH};

    #[test]
    fn diagnostic_method_uses_proton_run_for_trainer_runner_logs() {
        let log_tail = "[steam-helper] Delegating trainer leg to steam-host-trainer-runner.sh\n[steam-trainer-runner] trainer_launch_mode=direct_proton\n";

        assert_eq!(
            diagnostic_method_for_log(METHOD_STEAM_APPLAUNCH, log_tail),
            METHOD_PROTON_RUN
        );
    }

    #[test]
    fn diagnostic_method_keeps_steam_for_plain_helper_logs() {
        let log_tail = "[steam-helper] Launching Steam AppID 12345\n";

        assert_eq!(
            diagnostic_method_for_log(METHOD_STEAM_APPLAUNCH, log_tail),
            METHOD_STEAM_APPLAUNCH
        );
    }

    #[test]
    fn launch_log_ui_shows_first_gamescope_xdg_backend_line_then_suppresses_repeats() {
        let mut state = LaunchLogRelayState::default();
        let line = "[gamescope] [\u{1b}[0;31mError\u{1b}[0m] \u{1b}[0;37mxdg_backend:\u{1b}[0m Compositor released us but we were not acquired. Oh no.";

        let first = transform_launch_log_line_for_ui(&mut state, line);
        let second = transform_launch_log_line_for_ui(&mut state, line);
        let third = transform_launch_log_line_for_ui(&mut state, line);

        assert_eq!(first, vec![line.to_string()]);
        assert_eq!(
            second,
            vec![String::from(
                "[crosshook] Suppressing repeated gamescope xdg_backend console noise. The raw launch log still contains every line."
            )]
        );
        assert!(third.is_empty());
        assert_eq!(state.gamescope_xdg_backend_suppressed, 2);
    }

    #[test]
    fn launch_log_ui_suppression_summary_reports_suppressed_count() {
        let mut state = LaunchLogRelayState::default();
        state.gamescope_xdg_backend_suppressed = 329;

        let summary = suppression_summary_line(&state).expect("summary line");
        assert!(summary.contains("329 repeated gamescope xdg_backend lines"));
    }
}
