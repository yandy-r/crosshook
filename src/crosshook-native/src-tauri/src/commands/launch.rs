use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crosshook_core::launch::{
    analyze, build_launch_preview,
    build_steam_launch_options_command as build_steam_launch_options_command_core,
    should_surface_report,
    script_runner::{
        build_helper_command, build_native_game_command, build_proton_game_command,
        build_proton_trainer_command, build_trainer_command,
    },
    validate, DiagnosticReport, LaunchPreview, LaunchRequest, LaunchValidationIssue, METHOD_NATIVE,
    METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use super::shared::{create_log_path, sanitize_display_path};

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

#[tauri::command]
pub fn preview_launch(request: LaunchRequest) -> Result<LaunchPreview, String> {
    build_launch_preview(&request).map_err(|error| error.to_string())
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
    let method: &'static str = match request.resolved_method() {
        METHOD_STEAM_APPLAUNCH => METHOD_STEAM_APPLAUNCH,
        METHOD_PROTON_RUN => METHOD_PROTON_RUN,
        METHOD_NATIVE => METHOD_NATIVE,
        _ => METHOD_NATIVE,
    };

    let log_path = create_log_path("game", &request.log_target_slug())?;
    let mut command = match method {
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

    spawn_log_stream(app, log_path.clone(), child, method);

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
    let method: &'static str = match request.resolved_method() {
        METHOD_STEAM_APPLAUNCH => METHOD_STEAM_APPLAUNCH,
        METHOD_PROTON_RUN => METHOD_PROTON_RUN,
        METHOD_NATIVE => METHOD_NATIVE,
        _ => METHOD_NATIVE,
    };

    let log_path = create_log_path("trainer", &request.log_target_slug())?;
    let mut command = match method {
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

    spawn_log_stream(app, log_path.clone(), child, method);

    Ok(LaunchResult {
        succeeded: true,
        message: "Trainer launch started.".to_string(),
        helper_log_path: log_path.to_string_lossy().into_owned(),
    })
}

fn spawn_log_stream(
    app: AppHandle,
    log_path: PathBuf,
    child: tokio::process::Child,
    method: &'static str,
) {
    let handle = tauri::async_runtime::spawn(async move {
        stream_log_lines(app, log_path, child, method).await;
    });

    tauri::async_runtime::spawn(async move {
        if let Err(error) = handle.await {
            tracing::error!(%error, "launch log stream task failed");
        }
    });
}

async fn stream_log_lines(
    app: AppHandle,
    log_path: PathBuf,
    mut child: tokio::process::Child,
    method: &'static str,
) {
    let mut last_len = 0usize;
    let mut exit_status: Option<std::process::ExitStatus> = None;

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
                if let Err(error) = app.emit("launch-log", line.to_string()) {
                    tracing::warn!(%error, "failed to emit final launch log line");
                    break;
                }
            }
        }
    }

    let exit_code = exit_status.as_ref().and_then(|status| status.code());
    let signal = exit_status.as_ref().and_then(|status| status.signal());

    let log_tail = safe_read_tail(
        &log_path,
        crosshook_core::launch::diagnostics::MAX_LOG_TAIL_BYTES,
    )
    .await;
    let mut report = analyze(exit_status, &log_tail, method);
    report.log_tail_path = Some(sanitize_display_path(&log_path.to_string_lossy()));
    let report = sanitize_diagnostic_report(report);

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
