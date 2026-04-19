use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crosshook_core::launch::{
    gamescope_watchdog as gamescope_watchdog_core,
    script_runner::{
        build_flatpak_steam_trainer_command, build_helper_command, build_native_game_command,
        build_proton_game_command, build_proton_trainer_command, build_trainer_command,
        gamescope_pid_capture_path,
    },
    validate, LaunchRequest, METHOD_NATIVE, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
};
use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::ProfileStore;
use tauri::{AppHandle, Manager, State};

use super::portal::try_register_gamemode_portal_for_launch;
use super::shared::{LaunchResult, LaunchStreamContext};
use super::streaming::spawn_log_stream;
use super::warnings::{collect_offline_launch_warnings, collect_trainer_hash_launch_warnings_ipc};
use crate::commands::shared::{create_log_path, sanitize_display_path};

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
        && (request.gamescope.allow_nested
            || !crosshook_core::launch::is_inside_gamescope_session());
    let game_exe_name = Path::new(&request.game_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_string();

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
/// CrossHook but does **not** exit when the game inside it exits.
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
