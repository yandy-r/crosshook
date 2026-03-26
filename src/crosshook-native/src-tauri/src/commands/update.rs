use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crosshook_core::update::{
    update_game as update_game_core,
    validate_update_request as validate_update_request_core, UpdateGameRequest, UpdateGameResult,
};
use tauri::{AppHandle, Emitter};

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
    request: UpdateGameRequest,
) -> Result<UpdateGameResult, String> {
    let slug = update_target_slug(&request.profile_name);
    let log_path = create_log_path("update", &slug)?;

    let log_path_clone = log_path.clone();
    let (result, child) = tauri::async_runtime::spawn_blocking(move || {
        update_game_core(&request, &log_path_clone).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())??;

    spawn_log_stream(app, log_path, child, "update-log");

    Ok(result)
}

fn create_log_path(prefix: &str, target_slug: &str) -> Result<PathBuf, String> {
    let log_dir = PathBuf::from("/tmp/crosshook-logs");
    std::fs::create_dir_all(&log_dir).map_err(|error| error.to_string())?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();

    let file_name = format!("{prefix}-{target_slug}-{timestamp}.log");
    let log_path = log_dir.join(file_name);
    std::fs::File::create(&log_path).map_err(|error| error.to_string())?;
    Ok(log_path)
}

fn spawn_log_stream(
    app: AppHandle,
    log_path: PathBuf,
    child: tokio::process::Child,
    event_name: &'static str,
) {
    let handle = tauri::async_runtime::spawn(async move {
        stream_log_lines(app, log_path, child, event_name).await;
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
) {
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
                tracing::warn!(%error, path = %log_path.display(), "failed to read update log file");
            }
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                if let Err(error) = app.emit("update-complete", status.code()) {
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

fn update_target_slug(profile_name: &str) -> String {
    let slug = profile_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();

    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        "update".to_string()
    } else {
        trimmed.to_string()
    }
}
