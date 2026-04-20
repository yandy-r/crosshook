use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

use crosshook_core::launch::diagnostics::FailureMode;
use crosshook_core::launch::{
    analyze, diagnostics::MAX_LOG_TAIL_BYTES, should_surface_report, SessionKind, TeardownReason,
    ValidationSeverity,
};
use crosshook_core::metadata::VersionCorrelationStatus;
use crosshook_core::steam::discover_steam_root_candidates;
use crosshook_core::steam::libraries::discover_steam_libraries;
use crosshook_core::steam::manifest::parse_manifest_full;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use super::diagnostics::{diagnostic_method_for_log, safe_read_tail, sanitize_diagnostic_report};
use super::shared::{
    suppression_summary_line, transform_launch_log_line_for_ui, LaunchLogRelayState,
    LaunchStreamContext,
};
use crate::commands::shared::sanitize_display_path;
use crosshook_core::metadata::{compute_correlation_status, hash_trainer_file};

pub(super) fn spawn_log_stream(
    app: AppHandle,
    log_path: PathBuf,
    child: tokio::process::Child,
    method: &'static str,
    context: LaunchStreamContext,
    gamemode_portal_guard: Option<
        crosshook_core::platform::portals::gamemode::GameModeRegistration,
    >,
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
    let mut exit_status: Option<ExitStatus> = None;
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

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

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
) -> Option<ExitStatus> {
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
    exit_status: Option<ExitStatus>,
    method: &'static str,
    context: LaunchStreamContext,
) {
    let exit_code = exit_status.as_ref().and_then(ExitStatus::code);
    let signal = exit_status.as_ref().and_then(ExitStatusExt::signal);

    let log_tail = safe_read_tail(&log_path, MAX_LOG_TAIL_BYTES).await;
    let diagnostic_method = diagnostic_method_for_log(method, &log_tail);
    let mut report = analyze(exit_status, &log_tail, diagnostic_method);
    report.log_tail_path = Some(sanitize_display_path(&log_path.to_string_lossy()));
    let mut report = sanitize_diagnostic_report(report);

    if context.watchdog_outcome.was_killed() {
        let message = match context.session_kind {
            Some(SessionKind::Trainer) => {
                "Trainer exited; gamescope compositor cleaned up.".to_string()
            }
            _ => "Game exited; gamescope compositor cleaned up.".to_string(),
        };
        report.exit_info.failure_mode = FailureMode::CleanExit;
        report.summary = message.clone();
        report.exit_info.description = message;
        report.exit_info.severity = ValidationSeverity::Info;
        report.suggestions.clear();
    }

    // Record why this launch was torn down. If the watchdog fired it has the
    // authoritative reason; otherwise the process exited on its own and the
    // launch's session (if present) was tidied up naturally.
    report.teardown_reason = context
        .watchdog_outcome
        .reason()
        .or(Some(TeardownReason::NaturalExit));

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
                        let path = lib
                            .steamapps_path
                            .join(format!("appmanifest_{}.acf", app_id_c.trim()));
                        if path.is_file() {
                            parse_manifest_full(&path).ok()
                        } else {
                            None
                        }
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

                let snapshot_build_id =
                    prior_snapshot.as_ref().and_then(|s| s.steam_build_id.as_deref());
                let snapshot_trainer_hash = prior_snapshot
                    .as_ref()
                    .and_then(|s| s.trainer_file_hash.as_deref());

                let status = compute_correlation_status(
                    current_build_id,
                    snapshot_build_id,
                    trainer_file_hash.as_deref(),
                    snapshot_trainer_hash,
                    state_flags,
                );

                if matches!(status, VersionCorrelationStatus::UpdateInProgress) {
                    tracing::debug!(profile_name = %pname_c, "version snapshot skipped: game update in progress");
                    return;
                }

                if let Err(e) = ms.upsert_version_snapshot(
                    &profile_id,
                    app_id_c.trim(),
                    if current_build_id.is_empty() {
                        None
                    } else {
                        Some(current_build_id)
                    },
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

    finalize_launch_session(&context);
}

/// On game-session teardown, broadcast [`TeardownReason::LinkedSessionExit`]
/// to any linked trainer sessions so their watchdogs cancel out of their poll
/// loops and tear their own trees down. Then deregister this session so the
/// registry doesn't leak entries.
fn finalize_launch_session(context: &LaunchStreamContext) {
    let (Some(registry), Some(session_id), Some(session_kind)) = (
        context.session_registry.as_ref(),
        context.session_id,
        context.session_kind,
    ) else {
        return;
    };

    if session_kind == SessionKind::Game {
        let signalled =
            registry.cancel_linked_children(session_id, TeardownReason::LinkedSessionExit);
        if signalled > 0 {
            tracing::info!(
                game_session_id = %session_id,
                linked_trainers = signalled,
                "launch session: cascading LinkedSessionExit to trainer children"
            );
        }
    }

    registry.deregister(session_id);
}
