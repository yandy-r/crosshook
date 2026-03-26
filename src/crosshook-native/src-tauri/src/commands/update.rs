use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use crosshook_core::update::{
    update_game as update_game_core,
    validate_update_request as validate_update_request_core, UpdateGameRequest, UpdateGameResult,
};
use tauri::{AppHandle, Emitter, Manager};

use super::shared::{create_log_path, slugify_target};

pub struct UpdateProcessState {
    pid: Mutex<Option<u32>>,
}

impl UpdateProcessState {
    pub fn new() -> Self {
        Self {
            pid: Mutex::new(None),
        }
    }
}

#[tauri::command]
pub async fn validate_update_request(request: UpdateGameRequest) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        validate_update_request_core(&request).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn update_game(
    app: AppHandle,
    state: tauri::State<'_, UpdateProcessState>,
    request: UpdateGameRequest,
) -> Result<UpdateGameResult, String> {
    let slug = slugify_target(&request.profile_name, "update");
    let log_path = create_log_path("update", &slug)?;

    let log_path_clone = log_path.clone();
    let (result, child) = tauri::async_runtime::spawn_blocking(move || {
        update_game_core(&request, &log_path_clone).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())??;

    // Store the child PID so it can be cancelled later
    if let Some(pid) = child.id() {
        *state.pid.lock().unwrap() = Some(pid);
    }

    spawn_log_stream(app, log_path, child, "update-log", "update-complete");

    Ok(result)
}

#[tauri::command]
pub async fn cancel_update(
    state: tauri::State<'_, UpdateProcessState>,
) -> Result<(), String> {
    let pid = state.pid.lock().unwrap().take();

    if let Some(pid) = pid {
        let _ = std::process::Command::new("kill")
            .arg(pid.to_string())
            .status();
    }

    Ok(())
}

fn spawn_log_stream(
    app: AppHandle,
    log_path: PathBuf,
    child: tokio::process::Child,
    event_name: &'static str,
    complete_event_name: &'static str,
) {
    let handle = tauri::async_runtime::spawn(async move {
        stream_log_lines(app, log_path, child, event_name, complete_event_name).await;
    });

    tauri::async_runtime::spawn(async move {
        if let Err(error) = handle.await {
            tracing::error!(%error, "update log stream task failed");
        }
    });
}

async fn stream_log_lines(
    app: AppHandle,
    log_path: PathBuf,
    mut child: tokio::process::Child,
    event_name: &'static str,
    complete_event_name: &'static str,
) {
    let mut last_len = 0usize;
    let mut consecutive_read_failures = 0u32;

    loop {
        match tokio::fs::read_to_string(&log_path).await {
            Ok(content) => {
                consecutive_read_failures = 0;

                if content.len() < last_len {
                    last_len = 0;
                }

                if content.len() > last_len {
                    let chunk = &content[last_len..];
                    for line in chunk.lines() {
                        if !line.is_empty() {
                            if let Err(error) = app.emit(event_name, line.to_string()) {
                                tracing::warn!(%error, "failed to emit update log line; stopping stream");
                                return;
                            }
                        }
                    }
                    last_len = content.len();
                }
            }
            Err(error) => {
                consecutive_read_failures += 1;
                if consecutive_read_failures <= 5 {
                    tracing::warn!(%error, path = %log_path.display(), "failed to read update log file");
                }
                if consecutive_read_failures == 5 {
                    let _ = app.emit(
                        event_name,
                        "Log stream interrupted: unable to read log file.".to_string(),
                    );
                }
            }
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                if let Err(error) = app.emit(complete_event_name, status.code()) {
                    tracing::warn!(%error, "failed to emit update-complete event");
                }
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

    // Clear the stored PID now that the process has exited
    if let Some(state) = app.try_state::<UpdateProcessState>() {
        *state.pid.lock().unwrap() = None;
    }

    // Final read to capture lines written between last poll and process exit
    if let Ok(content) = tokio::fs::read_to_string(&log_path).await {
        if content.len() > last_len {
            for line in content[last_len..].lines().filter(|l| !l.is_empty()) {
                if let Err(error) = app.emit(event_name, line.to_string()) {
                    tracing::warn!(%error, "failed to emit final update log line");
                    break;
                }
            }
        }
    }
}
