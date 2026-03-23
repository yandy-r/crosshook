use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crosshook_core::launch::{
    script_runner::{build_helper_command, build_trainer_command},
    validate, SteamLaunchRequest,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Serialize)]
pub struct LaunchResult {
    pub succeeded: bool,
    pub message: String,
    pub helper_log_path: String,
}

#[tauri::command]
pub fn validate_launch(request: SteamLaunchRequest) -> Result<(), String> {
    validate(&request).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn launch_game(
    app: AppHandle,
    request: SteamLaunchRequest,
) -> Result<LaunchResult, String> {
    let mut request = request;
    request.launch_game_only = true;
    request.launch_trainer_only = false;
    validate_launch(request.clone())?;

    let log_path = create_log_path("game", &request.steam_app_id)?;
    let script_path = resolve_script_path(&app, "steam-launch-helper.sh")?;
    let mut command = build_helper_command(&request, &script_path, &log_path);
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
    request: SteamLaunchRequest,
) -> Result<LaunchResult, String> {
    let mut request = request;
    request.launch_trainer_only = true;
    request.launch_game_only = false;
    validate_launch(request.clone())?;

    let log_path = create_log_path("trainer", &request.steam_app_id)?;
    let script_path = resolve_script_path(&app, "steam-launch-trainer.sh")?;
    let mut command = build_trainer_command(&request, &script_path, &log_path);
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
    tauri::async_runtime::spawn(async move {
        stream_log_lines(app, log_path, child).await;
    });
}

async fn stream_log_lines(app: AppHandle, log_path: PathBuf, mut child: tokio::process::Child) {
    let mut last_len = 0usize;

    loop {
        if let Ok(content) = tokio::fs::read_to_string(&log_path).await {
            if content.len() < last_len {
                last_len = 0;
            }

            if content.len() > last_len {
                let chunk = &content[last_len..];
                for line in chunk.lines() {
                    if !line.is_empty() {
                        let _ = app.emit("launch-log", line.to_string());
                    }
                }
                last_len = content.len();
            }
        }

        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(_) => break,
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

fn create_log_path(prefix: &str, app_id: &str) -> Result<PathBuf, String> {
    let log_dir = PathBuf::from("/tmp/crosshook-logs");
    fs::create_dir_all(&log_dir).map_err(|error| error.to_string())?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();

    let file_name = format!("{prefix}-{app_id}-{timestamp}.log");
    let log_path = log_dir.join(file_name);
    fs::File::create(&log_path).map_err(|error| error.to_string())?;
    Ok(log_path)
}

fn resolve_script_path(app: &AppHandle, script_name: &str) -> Result<PathBuf, String> {
    let resource_path = app
        .path()
        .resolve(script_name, tauri::path::BaseDirectory::Resource)
        .ok();

    if let Some(path) = resource_path {
        if path.exists() {
            return Ok(path);
        }
    }

    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("src")
        .join("CrossHookEngine.App")
        .join("runtime-helpers")
        .join(script_name);

    if dev_path.exists() {
        Ok(dev_path)
    } else {
        Err(format!("unable to resolve launch script: {script_name}"))
    }
}
