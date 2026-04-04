use crosshook_core::metadata::MetadataStore;
use crosshook_core::prefix_deps::detection::{detect_binary, resolve_winetricks_path};
use crosshook_core::prefix_deps::lock::PrefixDepsInstallLock;
use crosshook_core::prefix_deps::runner;
use crosshook_core::prefix_deps::{
    BinaryDetectionResult, DependencyState, PrefixDependencyStatus, PrefixDepsTool,
};
use crosshook_core::profile::ProfileStore;
use crosshook_core::settings::SettingsStore;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio::io::AsyncBufReadExt;

/// Managed state wrapping the global install lock.
pub struct PrefixDepsInstallState {
    pub lock: PrefixDepsInstallLock,
}

impl PrefixDepsInstallState {
    pub fn new() -> Self {
        Self {
            lock: PrefixDepsInstallLock::new(),
        }
    }
}

/// Payload emitted via `prefix-dep-log` events.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DepLogPayload {
    profile_name: String,
    prefix_path: String,
    line: String,
}

/// Payload emitted via `prefix-dep-complete` events.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DepCompletePayload {
    profile_name: String,
    prefix_path: String,
    succeeded: bool,
    exit_code: Option<i32>,
}

fn parse_nonzero_steam_app_id(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if !trimmed.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let parsed = trimmed.parse::<u32>().ok()?;
    if parsed == 0 {
        return None;
    }
    Some(parsed.to_string())
}

fn resolve_profile_steam_app_id(profile_store: &ProfileStore, profile_name: &str) -> Option<String> {
    let profile = profile_store.load(profile_name).ok()?;
    let effective = profile.effective_profile();
    parse_nonzero_steam_app_id(&effective.steam.app_id)
        .or_else(|| parse_nonzero_steam_app_id(&effective.runtime.steam_app_id))
}

fn resolve_tool_type(detection: &BinaryDetectionResult) -> PrefixDepsTool {
    detection.tool_type.unwrap_or_else(|| {
        if detection.binary_name.contains("protontricks") {
            PrefixDepsTool::Protontricks
        } else {
            PrefixDepsTool::Winetricks
        }
    })
}

/// Detect the protontricks/winetricks binary.
#[tauri::command]
pub fn detect_protontricks_binary(
    store: State<'_, SettingsStore>,
) -> Result<BinaryDetectionResult, String> {
    let settings = store.load().map_err(|e| e.to_string())?;
    Ok(detect_binary(&settings.protontricks_binary_path))
}

/// Check which prefix dependencies are installed for a profile.
#[tauri::command]
pub async fn check_prefix_dependencies(
    profile_name: String,
    prefix_path: String,
    packages: Vec<String>,
    store: State<'_, SettingsStore>,
    metadata_store: State<'_, MetadataStore>,
    profile_store: State<'_, ProfileStore>,
) -> Result<Vec<PrefixDependencyStatus>, String> {
    let settings = store.load().map_err(|e| e.to_string())?;
    let detection = detect_binary(&settings.protontricks_binary_path);
    if !detection.found {
        return Err("No winetricks or protontricks binary found. Install winetricks or configure the path in Settings.".to_string());
    }
    let mut tool_type = resolve_tool_type(&detection);
    let mut binary_path = detection.binary_path.unwrap();
    let steam_app_id = resolve_profile_steam_app_id(&profile_store, &profile_name);
    if matches!(tool_type, PrefixDepsTool::Protontricks) && steam_app_id.is_none() {
        if let Some(winetricks_path) = resolve_winetricks_path() {
            binary_path = winetricks_path;
            tool_type = PrefixDepsTool::Winetricks;
        } else {
            return Err("protontricks requires a valid nonzero Steam App ID; configure winetricks or set steam.app_id".to_string());
        }
    }

    // Run check
    let installed = runner::check_installed(
        &binary_path,
        &prefix_path,
        tool_type,
        steam_app_id.as_deref(),
    )
        .await
        .map_err(|e| e.to_string())?;

    // Build status for each requested package, upsert to SQLite
    let profile_id = metadata_store
        .lookup_profile_id(&profile_name)
        .ok()
        .flatten()
        .unwrap_or_else(|| profile_name.clone());

    let mut statuses = Vec::new();
    for pkg in &packages {
        let state = if installed.contains(pkg) {
            DependencyState::Installed
        } else {
            DependencyState::Missing
        };
        let state_str = match state {
            DependencyState::Installed => "installed",
            DependencyState::Missing => "missing",
            _ => "unknown",
        };

        // Upsert to SQLite (fail-soft)
        if let Err(e) = metadata_store.upsert_prefix_dep_state(
            &profile_id,
            pkg,
            &prefix_path,
            state_str,
            None,
        ) {
            tracing::warn!(%e, pkg, "failed to persist prefix dep state");
        }

        statuses.push(PrefixDependencyStatus {
            package_name: pkg.clone(),
            state,
            checked_at: Some(chrono::Utc::now().to_rfc3339()),
            installed_at: if state_str == "installed" {
                Some(chrono::Utc::now().to_rfc3339())
            } else {
                None
            },
            last_error: None,
        });
    }

    Ok(statuses)
}

/// Install prefix dependencies for a profile. Streams progress via events.
#[tauri::command]
pub async fn install_prefix_dependency(
    profile_name: String,
    prefix_path: String,
    packages: Vec<String>,
    app: AppHandle,
    store: State<'_, SettingsStore>,
    metadata_store: State<'_, MetadataStore>,
    install_state: State<'_, PrefixDepsInstallState>,
    profile_store: State<'_, ProfileStore>,
) -> Result<(), String> {
    let settings = store.load().map_err(|e| e.to_string())?;
    let detection = detect_binary(&settings.protontricks_binary_path);
    if !detection.found {
        return Err("No winetricks or protontricks binary found.".to_string());
    }
    let mut tool_type = resolve_tool_type(&detection);
    let mut binary_path = detection.binary_path.unwrap();
    let steam_app_id = resolve_profile_steam_app_id(&profile_store, &profile_name);
    if matches!(tool_type, PrefixDepsTool::Protontricks) && steam_app_id.is_none() {
        if let Some(winetricks_path) = resolve_winetricks_path() {
            binary_path = winetricks_path;
            tool_type = PrefixDepsTool::Winetricks;
        } else {
            return Err("protontricks requires a valid nonzero Steam App ID; configure winetricks or set steam.app_id".to_string());
        }
    }

    // Acquire global install lock
    let guard = install_state
        .lock
        .try_acquire(prefix_path.clone())
        .await
        .map_err(|e| e.to_string())?;

    // Spawn install process
    let mut child = runner::install_packages(
        &binary_path,
        &prefix_path,
        &packages,
        tool_type,
        steam_app_id.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())?;

    let profile_id = metadata_store
        .lookup_profile_id(&profile_name)
        .ok()
        .flatten()
        .unwrap_or_else(|| profile_name.clone());

    // Clone what we need for the background task
    let ms_clone = (*metadata_store).clone();
    let pfx_clone = prefix_path.clone();
    let profile_name_clone = profile_name.clone();
    let pkgs_clone = packages.clone();

    // Stream stdout/stderr to frontend
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let app_clone = app.clone();
    let profile_for_stdout = profile_name.clone();
    let prefix_for_stdout = prefix_path.clone();

    tauri::async_runtime::spawn(async move {
        // Stream stdout
        if let Some(stdout) = stdout {
            let reader = tokio::io::BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let clean = runner::sanitize_output_for_ui(&line);
                let _ = app_clone.emit(
                    "prefix-dep-log",
                    DepLogPayload {
                        profile_name: profile_for_stdout.clone(),
                        prefix_path: prefix_for_stdout.clone(),
                        line: clean,
                    },
                );
            }
        }
    });

    let app_clone2 = app.clone();
    let profile_for_stderr = profile_name.clone();
    let prefix_for_stderr = prefix_path.clone();
    tauri::async_runtime::spawn(async move {
        // Stream stderr
        if let Some(stderr) = stderr {
            let reader = tokio::io::BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let clean = runner::sanitize_output_for_ui(&line);
                let _ = app_clone2.emit(
                    "prefix-dep-log",
                    DepLogPayload {
                        profile_name: profile_for_stderr.clone(),
                        prefix_path: prefix_for_stderr.clone(),
                        line: clean,
                    },
                );
            }
        }
    });

    // Wait for process completion in background
    let app_final = app.clone();
    tauri::async_runtime::spawn(async move {
        // Keep lock guard in scope until install process fully exits.
        let _install_guard = guard;
        let status = child.wait().await;
        let (succeeded, exit_code) = match status {
            Ok(s) => (s.success(), s.code()),
            Err(e) => {
                tracing::error!(%e, "prefix dep install wait failed");
                (false, None)
            }
        };

        // Update dep states in SQLite
        let state_str = if succeeded { "installed" } else { "install_failed" };
        for pkg in &pkgs_clone {
            if let Err(e) = ms_clone.upsert_prefix_dep_state(
                &profile_id,
                pkg,
                &pfx_clone,
                state_str,
                if succeeded { None } else { Some("install process failed") },
            ) {
                tracing::warn!(%e, pkg, "failed to persist prefix dep state after install");
            }
        }

        let _ = app_final.emit(
            "prefix-dep-complete",
            DepCompletePayload {
                profile_name: profile_name_clone,
                prefix_path: pfx_clone.clone(),
                succeeded,
                exit_code,
            },
        );
    });

    Ok(())
}

/// Get cached dependency status for a profile.
#[tauri::command]
pub fn get_dependency_status(
    profile_name: String,
    prefix_path: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<PrefixDependencyStatus>, String> {
    let profile_id = metadata_store
        .lookup_profile_id(&profile_name)
        .ok()
        .flatten()
        .unwrap_or_else(|| profile_name.clone());

    let rows = metadata_store
        .load_prefix_dep_states(&profile_id)
        .map_err(|e| e.to_string())?;

    Ok(rows
        .into_iter()
        .filter(|row| row.prefix_path == prefix_path)
        .map(|row| PrefixDependencyStatus {
            package_name: row.package_name,
            state: match row.state.as_str() {
                "installed" => DependencyState::Installed,
                "missing" => DependencyState::Missing,
                "install_failed" => DependencyState::InstallFailed,
                "check_failed" => DependencyState::CheckFailed,
                "user_skipped" => DependencyState::UserSkipped,
                _ => DependencyState::Unknown,
            },
            checked_at: row.checked_at,
            installed_at: row.installed_at,
            last_error: row.last_error,
        })
        .collect())
}
