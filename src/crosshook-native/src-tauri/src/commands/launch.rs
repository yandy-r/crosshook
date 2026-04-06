use std::collections::BTreeMap;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use std::fs;

use crosshook_core::launch::{
    analyze, build_launch_preview,
    build_steam_launch_options_command as build_steam_launch_options_command_core,
    collect_trainer_hash_launch_warnings,
    diagnostics::FailureMode,
    script_runner::{
        build_helper_command, build_native_game_command, build_proton_game_command,
        build_proton_trainer_command, build_trainer_command,
    },
    should_surface_report, validate, DiagnosticReport, LaunchPreview, LaunchRequest,
    LaunchValidationIssue, ValidationError, METHOD_NATIVE, METHOD_PROTON_RUN,
    METHOD_STEAM_APPLAUNCH,
};
use crosshook_core::metadata::{compute_correlation_status, hash_trainer_file, MetadataStore};
use crosshook_core::offline::readiness::MIN_OFFLINE_READINESS_SCORE;
use crosshook_core::profile::GamescopeConfig;
use crosshook_core::profile::ProfileStore;
use crosshook_core::steam::discover_steam_root_candidates;
use crosshook_core::steam::libraries::discover_steam_libraries;
use crosshook_core::steam::manifest::parse_manifest_full;
use crosshook_core::storage::{check_low_disk_warning, DEFAULT_LOW_DISK_WARNING_MB};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use super::shared::{create_log_path, sanitize_display_path};

#[derive(Debug, Clone, Serialize)]
pub struct LaunchResult {
    pub succeeded: bool,
    pub message: String,
    pub helper_log_path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<LaunchValidationIssue>,
}

#[tauri::command]
pub fn validate_launch(request: LaunchRequest) -> Result<(), LaunchValidationIssue> {
    validate(&request).map_err(|error| error.issue())
}

#[tauri::command]
pub fn preview_launch(request: LaunchRequest) -> Result<LaunchPreview, String> {
    build_launch_preview(&request).map_err(|error| error.to_string())
}

/// Builds a Steam per-game “Launch Options” line from the same optimization IDs as `proton_run`,
/// plus profile custom env vars (custom wins on duplicate keys in the prefix).
///
/// When `gamescope` is provided and enabled, the gamescope compositor is inserted as a wrapper
/// (e.g. `gamescope -w 2560 -h 1440 -f -- %command%`).
#[tauri::command]
pub fn build_steam_launch_options_command(
    enabled_option_ids: Vec<String>,
    custom_env_vars: BTreeMap<String, String>,
    gamescope: Option<GamescopeConfig>,
) -> Result<String, String> {
    build_steam_launch_options_command_core(
        &enabled_option_ids,
        &custom_env_vars,
        gamescope.as_ref(),
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn check_gamescope_session() -> bool {
    crosshook_core::launch::is_inside_gamescope_session()
}

/// Checks whether a process whose name matches `exe_name` is currently running.
///
/// Scans `/proc/<pid>/comm` for exact matches, handling both the original name
/// (e.g. `game.exe`) and the name without the `.exe` suffix (`game`).
/// When `comm` is exactly 15 characters (the Linux `TASK_COMM_LEN` truncation
/// boundary), falls back to `/proc/<pid>/cmdline` for the full argv\[0\] basename.
fn is_process_running(exe_name: &str) -> bool {
    let name = exe_name.trim();
    if name.is_empty() {
        return false;
    }

    let without_exe = name
        .strip_suffix(".exe")
        .or_else(|| name.strip_suffix(".EXE"));
    let candidates: Vec<&str> = match without_exe {
        Some(stripped) => vec![name, stripped],
        None => vec![name],
    };

    let Ok(proc_dir) = fs::read_dir("/proc") else {
        return false;
    };

    for entry in proc_dir.flatten() {
        let dir_name = entry.file_name();
        let dir_name_str = dir_name.to_string_lossy();

        if !dir_name_str.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        let pid_path = entry.path();

        if let Ok(comm) = fs::read_to_string(pid_path.join("comm")) {
            let comm = comm.trim_end_matches('\n');
            if candidates.iter().any(|c| *c == comm) {
                return true;
            }

            // comm is truncated at 15 chars; check cmdline for the full name.
            if comm.len() == 15 {
                if let Ok(cmdline) = fs::read_to_string(pid_path.join("cmdline")) {
                    let argv0 = cmdline.split('\0').next().unwrap_or("");
                    let basename = argv0.rsplit('/').next().unwrap_or(argv0);
                    let basename = basename.rsplit('\\').next().unwrap_or(basename);
                    if candidates.iter().any(|c| *c == basename) {
                        return true;
                    }
                }
            }
        }
    }

    false
}

#[tauri::command]
pub fn check_game_running(exe_name: String) -> bool {
    let name = exe_name.trim();
    if name.is_empty() {
        return false;
    }
    is_process_running(name)
}

/// Non-blocking offline readiness advisory when the profile has a trainer configured.
async fn collect_offline_launch_warnings(
    request: &LaunchRequest,
    profile_name: Option<String>,
    profile_store: ProfileStore,
    metadata_store: MetadataStore,
) -> Vec<LaunchValidationIssue> {
    let mut warnings = collect_low_disk_warning(request).await;
    let Some(name) = profile_name.filter(|n| !n.trim().is_empty()) else {
        return warnings;
    };
    if !metadata_store.is_available() {
        return warnings;
    }
    let ps = profile_store;
    let ms = metadata_store;
    let mut offline_warnings = tauri::async_runtime::spawn_blocking(move || {
        let profile = match ps.load(&name) {
            Ok(p) => p,
            Err(_) => return Vec::new(),
        };
        if profile.effective_profile().trainer.path.trim().is_empty() {
            return Vec::new();
        }
        let profile_id = match ms.lookup_profile_id(&name) {
            Ok(Some(id)) => id,
            Ok(None) | Err(_) => return Vec::new(),
        };
        let report = match ms.check_offline_readiness_for_profile(&name, &profile_id, &profile) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        if report.score >= MIN_OFFLINE_READINESS_SCORE {
            return Vec::new();
        }
        vec![ValidationError::OfflineReadinessInsufficient {
            score: report.score,
            reasons: report.blocking_reasons.clone(),
        }
        .issue()]
    })
    .await
    .unwrap_or_default();
    warnings.append(&mut offline_warnings);
    warnings
}

/// SHA-256 baseline / community digest advisory (non-blocking).
async fn collect_trainer_hash_launch_warnings_ipc(
    profile_name: Option<String>,
    profile_store: ProfileStore,
    metadata_store: MetadataStore,
) -> Vec<LaunchValidationIssue> {
    let Some(name) = profile_name.filter(|n| !n.trim().is_empty()) else {
        return Vec::new();
    };
    if !metadata_store.is_available() {
        return Vec::new();
    }
    let ps = profile_store;
    let ms = metadata_store;
    tauri::async_runtime::spawn_blocking(move || {
        let profile = match ps.load(&name) {
            Ok(p) => p,
            Err(_) => return Vec::new(),
        };
        let profile_id = match ms.lookup_profile_id(&name) {
            Ok(Some(id)) => id,
            Ok(None) | Err(_) => return Vec::new(),
        };
        collect_trainer_hash_launch_warnings(&ms, &profile_id, &profile)
    })
    .await
    .unwrap_or_default()
}

async fn collect_low_disk_warning(request: &LaunchRequest) -> Vec<LaunchValidationIssue> {
    let raw_prefix_path = if request.resolved_method() == METHOD_STEAM_APPLAUNCH {
        request.steam.compatdata_path.trim()
    } else {
        request.runtime.prefix_path.trim()
    };
    if raw_prefix_path.is_empty() {
        return Vec::new();
    }

    let prefix_path = PathBuf::from(raw_prefix_path);
    let check_result = tauri::async_runtime::spawn_blocking(move || {
        check_low_disk_warning(&prefix_path, DEFAULT_LOW_DISK_WARNING_MB)
    })
    .await;

    let warning = match check_result {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => {
            tracing::warn!(path = raw_prefix_path, %error, "low-disk check failed");
            None
        }
        Err(error) => {
            tracing::warn!(path = raw_prefix_path, %error, "low-disk check task failed");
            None
        }
    };

    match warning {
        Some(value) => vec![ValidationError::LowDiskSpaceAdvisory {
            available_mb: value.available_bytes / (1024 * 1024),
            threshold_mb: value.threshold_bytes / (1024 * 1024),
            mount_path: value.mount_path,
        }
        .issue()],
        None => Vec::new(),
    }
}

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

    let sanitized_log_path = sanitize_display_path(&log_path.to_string_lossy());
    let operation_id = record_launch_start(
        &metadata_store,
        request.profile_name.as_deref(),
        method,
        &sanitized_log_path,
    )
    .await;

    // Extract before spawn_log_stream — request is moved into the closure
    let snap_steam_app_id = request.steam.app_id.clone();
    let snap_trainer_host_path = {
        let p = request.trainer_host_path.trim().to_string();
        if p.is_empty() {
            None
        } else {
            Some(p)
        }
    };
    let snap_profile_name = request.profile_name.clone();
    let snap_steam_client_path = request.steam.steam_client_install_path.clone();

    spawn_log_stream(
        app,
        log_path.clone(),
        child,
        method,
        metadata_store,
        operation_id,
        snap_steam_app_id,
        snap_trainer_host_path,
        snap_profile_name,
        snap_steam_client_path,
    );

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

    let sanitized_log_path = sanitize_display_path(&log_path.to_string_lossy());
    let operation_id = record_launch_start(
        &metadata_store,
        request.profile_name.as_deref(),
        method,
        &sanitized_log_path,
    )
    .await;

    // Extract before spawn_log_stream — request is moved into the closure
    let snap_steam_app_id = request.steam.app_id.clone();
    let snap_trainer_host_path = {
        let p = request.trainer_host_path.trim().to_string();
        if p.is_empty() {
            None
        } else {
            Some(p)
        }
    };
    let snap_profile_name = request.profile_name.clone();
    let snap_steam_client_path = request.steam.steam_client_install_path.clone();

    spawn_log_stream(
        app,
        log_path.clone(),
        child,
        method,
        metadata_store,
        operation_id,
        snap_steam_app_id,
        snap_trainer_host_path,
        snap_profile_name,
        snap_steam_client_path,
    );

    Ok(LaunchResult {
        succeeded: true,
        message: "Trainer launch started.".to_string(),
        helper_log_path: log_path.to_string_lossy().into_owned(),
        warnings,
    })
}

fn spawn_log_stream(
    app: AppHandle,
    log_path: PathBuf,
    child: tokio::process::Child,
    method: &'static str,
    metadata_store: MetadataStore,
    operation_id: Option<String>,
    steam_app_id: String,
    trainer_host_path: Option<String>,
    profile_name: Option<String>,
    steam_client_path: String,
) {
    let handle = tauri::async_runtime::spawn(async move {
        stream_log_lines(
            app,
            log_path,
            child,
            method,
            metadata_store,
            operation_id,
            steam_app_id,
            trainer_host_path,
            profile_name,
            steam_client_path,
        )
        .await;
    });

    tauri::async_runtime::spawn(async move {
        if let Err(error) = handle.await {
            tracing::error!(%error, "launch log stream task failed");
        }
    });
}

async fn record_launch_start(
    metadata_store: &MetadataStore,
    profile_name: Option<&str>,
    method: &'static str,
    log_path: &str,
) -> Option<String> {
    let ms_clone = metadata_store.clone();
    let pn = profile_name.map(str::to_owned);
    let lp = log_path.to_string();
    let operation_id: String = tauri::async_runtime::spawn_blocking(move || {
        ms_clone.record_launch_started(pn.as_deref(), method, Some(&lp))
    })
    .await
    .unwrap_or_else(|e| {
        tracing::warn!("metadata spawn_blocking join failed: {e}");
        Ok(String::new())
    })
    .unwrap_or_else(|e| {
        tracing::warn!(%e, "record_launch_started failed");
        String::new()
    });

    if operation_id.is_empty() {
        None
    } else {
        Some(operation_id)
    }
}

async fn stream_log_lines(
    app: AppHandle,
    log_path: PathBuf,
    mut child: tokio::process::Child,
    method: &'static str,
    metadata_store: MetadataStore,
    operation_id: Option<String>,
    steam_app_id: String,
    trainer_host_path: Option<String>,
    profile_name: Option<String>,
    steam_client_path: String,
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

    if let Some(ref op_id) = operation_id {
        let ms = metadata_store.clone();
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

    // Version snapshot — record on clean exit or indeterminate (steam_applaunch helper
    // exits before the game, so its exit code 0 is Indeterminate, not CleanExit).
    if matches!(
        report.exit_info.failure_mode,
        FailureMode::CleanExit | FailureMode::Indeterminate
    ) {
        if let Some(ref pname) = profile_name {
            let ms = metadata_store.clone();
            let pname_c = pname.clone();
            let app_id_c = steam_app_id.clone();
            let trainer_path_c = trainer_host_path.clone();
            let steam_install_c = steam_client_path.clone();
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
                        let p = lib.steamapps_path.join(format!("appmanifest_{}.acf", app_id_c.trim()));
                        if p.is_file() { parse_manifest_full(&p).ok() } else { None }
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

                let snapshot_build_id = prior_snapshot.as_ref().and_then(|s| s.steam_build_id.as_deref());
                let snapshot_trainer_hash = prior_snapshot.as_ref().and_then(|s| s.trainer_file_hash.as_deref());

                let status = compute_correlation_status(
                    current_build_id,
                    snapshot_build_id,
                    trainer_file_hash.as_deref(),
                    snapshot_trainer_hash,
                    state_flags,
                );

                if matches!(status, crosshook_core::metadata::VersionCorrelationStatus::UpdateInProgress) {
                    tracing::debug!(profile_name = %pname_c, "version snapshot skipped: game update in progress");
                    return;
                }

                if let Err(e) = ms.upsert_version_snapshot(
                    &profile_id,
                    app_id_c.trim(),
                    if current_build_id.is_empty() { None } else { Some(current_build_id) },
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

        // Known-good tagging — mark the most recent config revision as known-good.
        // Uses the same success heuristic: CleanExit or Indeterminate (steam_applaunch
        // helper exits before the game, so its code 0 is Indeterminate, not CleanExit).
        // Reuses the profile_id already resolved by the version snapshot block above.
        // If metadata is unavailable or no revisions exist yet, log and continue.
        if let Some(ref pname) = profile_name {
            // Reuse the profile_id from the version snapshot block if available.
            // Re-lookup only when version snapshot was skipped or lookup failed.
            let ms = metadata_store.clone();
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
