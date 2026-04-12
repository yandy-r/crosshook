use std::sync::Mutex;

use crosshook_core::update::{
    update_game as update_game_core, validate_update_request as validate_update_request_core,
    UpdateGameRequest, UpdateGameResult,
};
use tauri::{AppHandle, Manager};

use super::log_stream::spawn_log_stream;
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

    let app_handle_for_clear = app.clone();
    spawn_log_stream(
        app,
        log_path,
        child,
        "update-log",
        "update-complete",
        Box::new(move || {
            if let Some(state) = app_handle_for_clear.try_state::<UpdateProcessState>() {
                *state.pid.lock().unwrap() = None;
            }
        }),
    );

    Ok(result)
}

#[tauri::command]
pub async fn cancel_update(state: tauri::State<'_, UpdateProcessState>) -> Result<(), String> {
    let pid = *state.pid.lock().unwrap();

    if let Some(pid) = pid {
        let status = crosshook_core::platform::host_std_command("kill")
            .arg(pid.to_string())
            .status()
            .map_err(|error| format!("failed to signal updater process {pid}: {error}"))?;
        if !status.success() {
            return Err(format!(
                "failed to signal updater process {pid}: kill exited with {status}"
            ));
        }
    }

    Ok(())
}
