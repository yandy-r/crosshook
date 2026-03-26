use std::path::PathBuf;
use std::time::Duration;

use crosshook_core::launch::{
    build_steam_launch_options_command as build_steam_launch_options_command_core,
    script_runner::{
        build_helper_command, build_native_game_command, build_proton_game_command,
        build_proton_trainer_command, build_trainer_command,
    },
    validate, LaunchRequest, LaunchValidationIssue, METHOD_NATIVE, METHOD_PROTON_RUN,
    METHOD_STEAM_APPLAUNCH,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::shared::create_log_path;

#[derive(Debug, Clone, Serialize)]
pub struct LaunchResult {
    pub succeeded: bool,
    pub message: String,
    pub helper_log_path: String,
}

#[tauri::command]
pub fn validate_launch(request: LaunchRequest) -> Result<(), LaunchValidationIssue> {
    validate(&request).map_err(|error| error.issue())
}

/// Builds a Steam per-game “Launch Options” line from the same optimization IDs as `proton_run`.
#[tauri::command]
pub fn build_steam_launch_options_command(
    enabled_option_ids: Vec<String>,
) -> Result<String, String> {
    build_steam_launch_options_command_core(&enabled_option_ids).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn launch_game(app: AppHandle, request: LaunchRequest) -> Result<LaunchResult, String> {
    let mut request = request;
    request.launch_game_only = true;
    request.launch_trainer_only = false;
    validate(&request).map_err(|error| error.to_string())?;

    let log_path = create_log_path("game", &request.log_target_slug())?;
    let mut command = match request.resolved_method() {
        METHOD_STEAM_APPLAUNCH => {
            let script_path = resolve_script_path(&app, "steam-launch-helper.sh")?;
            build_helper_command(&request, &script_path, &log_path)
        }
        METHOD_PROTON_RUN => build_proton_game_command(&request, &log_path)
            .map_err(|error| format!("failed to build Proton game launch: {error}"))?,
        METHOD_NATIVE => build_native_game_command(&request, &log_path)
            .map_err(|error| format!("failed to build native game launch: {error}"))?,
        other => return Err(format!("unsupported launch method: {other}")),
    };
    let child = command
        .spawn()
        .map_err(|error| format!("failed to launch helper: {error}"))?;

    spawn_log_stream(app, log_path.clone(), child);

    Ok(LaunchResult {
        succeeded: true,
        message: "Game launch started.".to_string(),
        helper_log_path: log_path.to_string_lossy().into_owned(),
    })
}

#[tauri::command]
pub async fn launch_trainer(
    app: AppHandle,
    request: LaunchRequest,
) -> Result<LaunchResult, String> {
    let mut request = request;
    request.launch_trainer_only = true;
    request.launch_game_only = false;
    validate(&request).map_err(|error| error.to_string())?;

    let log_path = create_log_path("trainer", &request.log_target_slug())?;
    let mut command = match request.resolved_method() {
        METHOD_STEAM_APPLAUNCH => {
            let script_path = resolve_script_path(&app, "steam-launch-trainer.sh")?;
            build_trainer_command(&request, &script_path, &log_path)
        }
        METHOD_PROTON_RUN => build_proton_trainer_command(&request, &log_path)
            .map_err(|error| format!("failed to build Proton trainer launch: {error}"))?,
        METHOD_NATIVE => return Err("native launch does not support trainer launch.".to_string()),
        other => return Err(format!("unsupported launch method: {other}")),
    };
    let child = command
        .spawn()
        .map_err(|error| format!("failed to launch trainer helper: {error}"))?;

    spawn_log_stream(app, log_path.clone(), child);

    Ok(LaunchResult {
        succeeded: true,
        message: "Trainer launch started.".to_string(),
        helper_log_path: log_path.to_string_lossy().into_owned(),
    })
}

fn spawn_log_stream(app: AppHandle, log_path: PathBuf, child: tokio::process::Child) {
    let handle = tauri::async_runtime::spawn(async move {
        stream_log_lines(app, log_path, child).await;
    });

    tauri::async_runtime::spawn(async move {
        if let Err(error) = handle.await {
            tracing::error!(%error, "launch log stream task failed");
        }
    });
}

async fn stream_log_lines(app: AppHandle, log_path: PathBuf, mut child: tokio::process::Child) {
    let mut last_len = 0usize;

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
                            if let Err(error) = app.emit("launch-log", line.to_string()) {
                                tracing::warn!(%error, "failed to emit launch log line; stopping stream");
                                return;
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
            Ok(Some(_)) => break,
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
                if let Err(error) = app.emit("launch-log", line.to_string()) {
                    tracing::warn!(%error, "failed to emit final launch log line");
                    break;
                }
            }
        }
    }
}

fn resolve_script_path(app: &AppHandle, script_name: &str) -> Result<PathBuf, String> {
    for resource_name in [
        script_name.to_string(),
        format!("runtime-helpers/{script_name}"),
        format!("_up_/runtime-helpers/{script_name}"),
    ] {
        if let Some(path) = app
            .path()
            .resolve(&resource_name, tauri::path::BaseDirectory::Resource)
            .ok()
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
