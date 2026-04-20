use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use crosshook_core::launch::{
    gamescope_watchdog as gamescope_watchdog_core, is_inside_gamescope_session,
    script_runner::{
        build_flatpak_steam_trainer_command, build_helper_command, build_native_game_command,
        build_proton_game_command, build_proton_trainer_command, build_trainer_command,
        gamescope_pid_capture_path,
    },
    validate, LaunchRequest, LaunchSessionRegistry, SessionId, SessionKind, TeardownReason,
    WatchdogOutcome, METHOD_NATIVE, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
};
use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::ProfileStore;
use tauri::{AppHandle, Manager, State};
use tokio::sync::broadcast;

use super::portal::try_register_gamemode_portal_for_launch;
use super::shared::{LaunchResult, LaunchStreamContext};
use super::streaming::spawn_log_stream;
use super::warnings::{collect_offline_launch_warnings, collect_trainer_hash_launch_warnings_ipc};
use crate::commands::shared::{create_log_path, sanitize_display_path};

/// Session registry key for launches that lack a user-facing profile name.
/// Unlikely in practice (the validator rejects most such requests) but keeps
/// the registry lookup robust when it does slip through.
const ANONYMOUS_PROFILE_KEY: &str = "__crosshook_anonymous_profile__";

fn session_profile_key(request: &LaunchRequest) -> String {
    request
        .profile_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(ANONYMOUS_PROFILE_KEY)
        .to_string()
}

#[tauri::command]
pub async fn launch_game(
    app: AppHandle,
    request: LaunchRequest,
    profile_store: State<'_, ProfileStore>,
    session_registry: State<'_, Arc<LaunchSessionRegistry>>,
) -> Result<LaunchResult, String> {
    let mut request = request;
    request.launch_game_only = true;
    request.launch_trainer_only = false;
    validate(&request).map_err(|error| error.to_string())?;
    let profile_store = profile_store.inner().clone();
    let metadata_store = app.state::<MetadataStore>().inner().clone();
    let session_registry = session_registry.inner().clone();
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
        && (request.gamescope.allow_nested || !is_inside_gamescope_session())
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

    let snap_steam_app_id = request.steam.app_id.clone();
    let snap_trainer_host_path = {
        let path = request.trainer_host_path.trim().to_string();
        if path.is_empty() {
            None
        } else {
            Some(path)
        }
    };
    let snap_profile_name = request.profile_name.clone();
    let snap_steam_client_path = request.steam.steam_client_install_path.clone();

    let gamescope_active = method == METHOD_PROTON_RUN
        && request.gamescope.enabled
        && (request.gamescope.allow_nested || !is_inside_gamescope_session());
    let game_exe_name = Path::new(&request.game_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_string();

    let watchdog_outcome = WatchdogOutcome::new();
    let (session_id, cancel_rx) =
        session_registry.register(SessionKind::Game, session_profile_key(&request));
    let stream_context = LaunchStreamContext {
        metadata_store,
        operation_id,
        steam_app_id: snap_steam_app_id,
        trainer_host_path: snap_trainer_host_path,
        profile_name: snap_profile_name,
        steam_client_path: snap_steam_client_path,
        watchdog_outcome: watchdog_outcome.clone(),
        session_id: Some(session_id),
        session_kind: Some(SessionKind::Game),
        session_registry: Some(session_registry.clone()),
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
                watchdog_outcome,
                flatpak_gamescope_pid_capture,
                cancel_rx,
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
    session_registry: State<'_, Arc<LaunchSessionRegistry>>,
) -> Result<LaunchResult, String> {
    let mut request = request;
    request.launch_trainer_only = true;
    request.launch_game_only = false;
    validate(&request).map_err(|error| error.to_string())?;
    let profile_store = profile_store.inner().clone();
    let metadata_store = app.state::<MetadataStore>().inner().clone();
    let session_registry = session_registry.inner().clone();
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

    // A Flatpak Proton-run trainer uses the same gamescope handoff as the
    // game path, so the pre-spawn PID capture file must exist for the
    // watchdog to read. Gating predicate mirrors the game launch path exactly
    // to preserve trainer execution parity (CLAUDE.md).
    let trainer_gamescope = request.resolved_trainer_gamescope();
    let trainer_gamescope_active = execution_method == METHOD_PROTON_RUN
        && trainer_gamescope.enabled
        && (trainer_gamescope.allow_nested || !is_inside_gamescope_session());
    let flatpak_trainer_pid_capture =
        if trainer_gamescope_active && crosshook_core::platform::is_flatpak() {
            Some(gamescope_pid_capture_path(&log_path))
        } else {
            None
        };

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
    let gamemode_portal_guard =
        try_register_gamemode_portal_for_launch(&request, execution_method).await;

    let child = command.spawn().map_err(|error| {
        format!("failed to launch trainer (method={execution_method}): {error}")
    })?;
    let child_pid = child.id();

    let sanitized_log_path = sanitize_display_path(&log_path.to_string_lossy());
    let operation_id = record_launch_start(
        &metadata_store,
        request.profile_name.as_deref(),
        execution_method,
        &sanitized_log_path,
    )
    .await;

    let snap_steam_app_id = request.steam.app_id.clone();
    let snap_trainer_host_path = {
        let path = request.trainer_host_path.trim().to_string();
        if path.is_empty() {
            None
        } else {
            Some(path)
        }
    };
    let snap_profile_name = request.profile_name.clone();
    let snap_steam_client_path = request.steam.steam_client_install_path.clone();

    let profile_key = session_profile_key(&request);
    let watchdog_outcome = WatchdogOutcome::new();
    let (session_id, cancel_rx) = session_registry.register(SessionKind::Trainer, &profile_key);

    // Link this trainer session to the active game session for the same
    // profile (if any). When the game finalizes, the trainer's cancel channel
    // receives LinkedSessionExit and its watchdog tears the trainer tree down.
    if let Some(parent_id) = session_registry
        .sessions_for_profile(&profile_key, Some(SessionKind::Game))
        .into_iter()
        .next()
    {
        if let Err(error) = session_registry.link_to_parent(session_id, parent_id) {
            tracing::warn!(
                %error,
                trainer_session = %session_id,
                parent_session = %parent_id,
                "launch session: trainer → game link skipped"
            );
        } else {
            tracing::info!(
                trainer_session = %session_id,
                parent_session = %parent_id,
                profile_key = %profile_key,
                "launch session: trainer linked to game parent"
            );
        }
    }

    let stream_context = LaunchStreamContext {
        metadata_store,
        operation_id,
        steam_app_id: snap_steam_app_id,
        trainer_host_path: snap_trainer_host_path,
        profile_name: snap_profile_name,
        steam_client_path: snap_steam_client_path,
        watchdog_outcome: watchdog_outcome.clone(),
        session_id: Some(session_id),
        session_kind: Some(SessionKind::Trainer),
        session_registry: Some(session_registry.clone()),
    };

    let trainer_exe_name = Path::new(&request.trainer_host_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_string();

    let watchdog_app_handle = app.clone();
    spawn_log_stream(
        app,
        log_path.clone(),
        child,
        execution_method,
        stream_context,
        gamemode_portal_guard,
    );

    // Spawn a trainer-side gamescope watchdog with the same gating predicate
    // as the game path. The watchdog receives this trainer session's cancel
    // channel, so a parent game finalize broadcasts straight into its poll
    // loop.
    if trainer_gamescope_active {
        if let Some(pid) = child_pid {
            spawn_gamescope_watchdog(
                &watchdog_app_handle,
                pid,
                trainer_exe_name,
                watchdog_outcome,
                flatpak_trainer_pid_capture,
                cancel_rx,
            );
        }
    } else {
        // Even without gamescope the session still needs its receiver drained
        // so the broadcast channel stays healthy and any incoming cancel is
        // recorded in the launch log. The trainer process will exit on its
        // own lifecycle; stream finalization handles deregister.
        tauri::async_runtime::spawn(drain_cancel_on_trainer_no_watchdog(session_id, cancel_rx));
    }

    Ok(LaunchResult {
        succeeded: true,
        message: "Trainer launch started.".to_string(),
        helper_log_path: log_path.to_string_lossy().into_owned(),
        warnings,
    })
}

async fn drain_cancel_on_trainer_no_watchdog(
    session_id: SessionId,
    mut cancel_rx: broadcast::Receiver<TeardownReason>,
) {
    match cancel_rx.recv().await {
        Ok(reason) => {
            tracing::info!(
                session_id = %session_id,
                teardown_reason = %reason,
                "trainer session without watchdog received cancel; process exit will finalize the session"
            );
        }
        Err(broadcast::error::RecvError::Closed) => {
            tracing::debug!(
                session_id = %session_id,
                "trainer cancel channel closed before any signal"
            );
        }
        Err(broadcast::error::RecvError::Lagged(_)) => {
            tracing::debug!(
                session_id = %session_id,
                "trainer cancel channel lagged before any signal"
            );
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
    let profile_name = profile_name.map(str::to_owned);
    let log_path = log_path.to_string();
    let operation_id: String = tauri::async_runtime::spawn_blocking(move || {
        ms_clone.record_launch_started(profile_name.as_deref(), method, Some(&log_path))
    })
    .await
    .unwrap_or_else(|error| {
        tracing::warn!("metadata spawn_blocking join failed: {error}");
        Ok(String::new())
    })
    .unwrap_or_else(|error| {
        tracing::warn!(%error, "record_launch_started failed");
        String::new()
    });

    if operation_id.is_empty() {
        None
    } else {
        Some(operation_id)
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

/// When gamescope wraps a Proton launch it becomes the direct child of
/// CrossHook but does **not** exit when the game inside it exits. This helper
/// is shared by both game and trainer launch paths — the trainer-side spawn
/// passes the registry-backed cancel channel so a parent game session can
/// cascade teardown.
fn spawn_gamescope_watchdog(
    app: &AppHandle,
    gamescope_pid: u32,
    exe_name: String,
    outcome: WatchdogOutcome,
    host_pid_capture_path: Option<PathBuf>,
    cancel_rx: broadcast::Receiver<TeardownReason>,
) {
    if exe_name.is_empty() {
        tracing::warn!(
            gamescope_pid,
            "gamescope watchdog disabled: launch path did not yield an executable basename"
        );
        return;
    }

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
        gamescope_watchdog_core(
            gamescope_pid,
            &exe_name,
            outcome,
            host_pid_capture_path,
            cancel_rx,
        )
        .await;
    });
}
