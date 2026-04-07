//! Shared async log poll-and-emit helper used by `update` and `run_executable`.
//!
//! Background-spawns a task that polls a log file every 500ms, emits each new
//! non-empty line on the configured event channel, and emits the final exit
//! code on the completion event when the child exits. The caller supplies a
//! `clear_pid` callback so the helper stays state-agnostic.

use std::path::PathBuf;
use std::time::Duration;

use tauri::{AppHandle, Emitter};

/// Type-erased "clear stored process PID" callback. Invoked exactly once when
/// the streamed child exits, before any final log lines are flushed.
pub type ClearPidCallback = Box<dyn FnOnce() + Send + Sync + 'static>;

pub fn spawn_log_stream(
    app: AppHandle,
    log_path: PathBuf,
    child: tokio::process::Child,
    event_name: &'static str,
    complete_event_name: &'static str,
    clear_pid: ClearPidCallback,
) {
    let handle = tauri::async_runtime::spawn(async move {
        stream_log_lines(
            app,
            log_path,
            child,
            event_name,
            complete_event_name,
            clear_pid,
        )
        .await;
    });

    tauri::async_runtime::spawn(async move {
        if let Err(error) = handle.await {
            tracing::error!(%error, "log stream task failed");
        }
    });
}

async fn stream_log_lines(
    app: AppHandle,
    log_path: PathBuf,
    mut child: tokio::process::Child,
    event_name: &'static str,
    complete_event_name: &'static str,
    clear_pid: ClearPidCallback,
) {
    let mut last_len = 0usize;
    let mut consecutive_read_failures = 0u32;
    let mut interrupted_emitted = false;

    loop {
        match tokio::fs::read_to_string(&log_path).await {
            Ok(content) => {
                if interrupted_emitted {
                    // Surface recovery so the user can correlate "interrupted"
                    // with the lines that follow once reads succeed again.
                    let _ =
                        app.emit(event_name, "Log stream resumed.".to_string());
                    interrupted_emitted = false;
                }
                consecutive_read_failures = 0;

                if content.len() < last_len {
                    last_len = 0;
                }

                if content.len() > last_len {
                    let chunk = &content[last_len..];
                    for line in chunk.lines() {
                        if !line.is_empty() {
                            if let Err(error) = app.emit(event_name, line.to_string()) {
                                tracing::warn!(
                                    %error,
                                    event = event_name,
                                    "failed to emit log line; stopping stream"
                                );
                                clear_pid();
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
                    tracing::warn!(%error, path = %log_path.display(), "failed to read log file");
                }
                if consecutive_read_failures == 5 {
                    let _ = app.emit(
                        event_name,
                        "Log stream interrupted: unable to read log file.".to_string(),
                    );
                    interrupted_emitted = true;
                }
            }
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                if let Err(error) = app.emit(complete_event_name, status.code()) {
                    tracing::warn!(
                        %error,
                        event = complete_event_name,
                        "failed to emit completion event"
                    );
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

    // Clear the stored PID now that the process has exited.
    clear_pid();

    // Final read to capture lines written between last poll and process exit.
    if let Ok(content) = tokio::fs::read_to_string(&log_path).await {
        if content.len() > last_len {
            for line in content[last_len..].lines().filter(|l| !l.is_empty()) {
                if let Err(error) = app.emit(event_name, line.to_string()) {
                    tracing::warn!(
                        %error,
                        event = event_name,
                        "failed to emit final log line"
                    );
                    break;
                }
            }
        }
    }
}
