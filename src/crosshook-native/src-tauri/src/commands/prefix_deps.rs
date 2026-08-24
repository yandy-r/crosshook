use crosshook_core::metadata::{MetadataStore, PrefixVersionRestoreJournalRow};
use crosshook_core::prefix_deps::detection::{detect_binary, resolve_winetricks_path};
use crosshook_core::prefix_deps::lock::PrefixDepsInstallLock;
use crosshook_core::prefix_deps::runner;
use crosshook_core::prefix_deps::{
    BinaryDetectionResult, DependencyState, PrefixDependencyInstallOutcome, PrefixDependencyStatus,
    PrefixDepsTool, PrefixVersionRestorePlan, PrefixVersionRestoreState,
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
    install_succeeded: bool,
    install_exit_code: Option<i32>,
    restore_state: PrefixVersionRestoreState,
    restore_error: Option<String>,
}

/// Restart-safe Windows compatibility repair status for a Wine prefix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefixVersionRepairStatus {
    pub required: bool,
    pub state: Option<PrefixVersionRestoreState>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum JournalDecision {
    Delete,
    MarkFailed(String),
    NoMutation,
}

fn parse_journal_tool(raw: &str) -> Result<PrefixDepsTool, String> {
    match raw {
        "winetricks" => Ok(PrefixDepsTool::Winetricks),
        "protontricks" => Ok(PrefixDepsTool::Protontricks),
        other => Err(format!(
            "invalid dependency tool in repair journal: {other}"
        )),
    }
}

fn repair_status_from_row(
    row: Option<PrefixVersionRestoreJournalRow>,
) -> Result<PrefixVersionRepairStatus, String> {
    let Some(row) = row else {
        return Ok(PrefixVersionRepairStatus {
            required: false,
            state: None,
            last_error: None,
        });
    };

    parse_journal_tool(&row.tool_type)?;
    let state = match row.state.as_str() {
        "pending" => PrefixVersionRestoreState::Pending,
        "failed" => PrefixVersionRestoreState::Failed,
        other => return Err(format!("invalid repair state in journal: {other}")),
    };
    Ok(PrefixVersionRepairStatus {
        required: true,
        state: Some(state),
        last_error: row.last_error,
    })
}

fn restore_plan_from_row(
    row: &PrefixVersionRestoreJournalRow,
) -> Result<PrefixVersionRestorePlan, String> {
    let tool_type = parse_journal_tool(&row.tool_type)?;
    crosshook_core::prefix_deps::validation::validate_protontricks_verbs(std::slice::from_ref(
        &row.restore_verb,
    ))
    .map_err(|error| error.to_string())?;
    let steam_app_id = match tool_type {
        PrefixDepsTool::Protontricks => Some(
            row.steam_app_id
                .as_deref()
                .and_then(parse_nonzero_steam_app_id)
                .ok_or_else(|| {
                    "repair journal has an invalid Steam App ID for protontricks; expected a positive decimal value"
                        .to_string()
                })?,
        ),
        PrefixDepsTool::Winetricks => None,
    };
    Ok(PrefixVersionRestorePlan {
        binary_path: row.binary_path.clone(),
        prefix_path: row.prefix_path.clone(),
        tool_type,
        steam_app_id,
        restore_verb: row.restore_verb.clone(),
    })
}

fn journal_decision(outcome: &PrefixDependencyInstallOutcome) -> JournalDecision {
    match outcome.restore_state {
        PrefixVersionRestoreState::Succeeded => JournalDecision::Delete,
        PrefixVersionRestoreState::Failed => JournalDecision::MarkFailed(
            outcome
                .restore_error
                .clone()
                .unwrap_or_else(|| "Windows version restoration failed".to_string()),
        ),
        PrefixVersionRestoreState::NotRequired | PrefixVersionRestoreState::Pending => {
            JournalDecision::NoMutation
        }
    }
}

fn completion_payload(
    profile_name: impl Into<String>,
    prefix_path: impl Into<String>,
    outcome: PrefixDependencyInstallOutcome,
) -> DepCompletePayload {
    let restore_safe = matches!(
        outcome.restore_state,
        PrefixVersionRestoreState::NotRequired | PrefixVersionRestoreState::Succeeded
    );
    DepCompletePayload {
        profile_name: profile_name.into(),
        prefix_path: prefix_path.into(),
        succeeded: outcome.install_succeeded && restore_safe,
        exit_code: outcome.install_exit_code,
        install_succeeded: outcome.install_succeeded,
        install_exit_code: outcome.install_exit_code,
        restore_state: outcome.restore_state,
        restore_error: outcome.restore_error,
    }
}

fn outcome_after_journal_persistence(
    mut outcome: PrefixDependencyInstallOutcome,
    persistence: Result<(), String>,
) -> PrefixDependencyInstallOutcome {
    if let Err(error) = persistence {
        if outcome.restore_state == PrefixVersionRestoreState::Succeeded {
            outcome.restore_state = PrefixVersionRestoreState::Failed;
            outcome.restore_error = Some(format!(
                "Windows version was restored, but repair journal cleanup failed: {error}"
            ));
        }
    }
    outcome
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

fn resolve_profile_steam_app_id(
    profile_store: &ProfileStore,
    profile_name: &str,
) -> Option<String> {
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
        if let Err(e) =
            metadata_store.upsert_prefix_dep_state(&profile_id, pkg, &prefix_path, state_str, None)
        {
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

    let normalized_prefix =
        runner::normalize_prefix_path(&prefix_path).map_err(|error| error.to_string())?;

    // Hold the normalized-prefix lock across journal checks, mutation, restore,
    // persistence, and the final completion event.
    let guard = install_state
        .lock
        .try_acquire(normalized_prefix.clone())
        .await
        .map_err(|e| e.to_string())?;

    if metadata_store
        .load_prefix_version_restore(&normalized_prefix)
        .map_err(|error| error.to_string())?
        .is_some()
    {
        return Err(
            "This prefix requires Windows version repair before installing dependencies."
                .to_string(),
        );
    }

    let profile_id = metadata_store
        .lookup_profile_id(&profile_name)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("profile is not indexed in metadata: {profile_name}"))?;

    let prepared = runner::prepare_install_packages(
        &binary_path,
        &prefix_path,
        &packages,
        tool_type,
        steam_app_id.as_deref(),
    )
    .map_err(|e| e.to_string())?;

    if let Some(plan) = prepared.restore_plan() {
        let now = chrono::Utc::now().to_rfc3339();
        metadata_store
            .upsert_prefix_version_restore(&PrefixVersionRestoreJournalRow {
                prefix_path: plan.prefix_path.clone(),
                profile_id: Some(profile_id.clone()),
                binary_path: plan.binary_path.clone(),
                tool_type: match plan.tool_type {
                    PrefixDepsTool::Winetricks => "winetricks".to_string(),
                    PrefixDepsTool::Protontricks => "protontricks".to_string(),
                },
                steam_app_id: plan.steam_app_id.clone(),
                restore_verb: plan.restore_verb.clone(),
                state: "pending".to_string(),
                last_error: None,
                created_at: now.clone(),
                updated_at: now,
            })
            .map_err(|error| error.to_string())?;
    }

    // A spawn failure intentionally leaves a persisted pending journal row.
    let mut child = prepared.spawn().map_err(|error| error.to_string())?;

    // Clone what we need for the background task
    let ms_clone = (*metadata_store).clone();
    let pfx_clone = prefix_path.clone();
    let normalized_pfx_clone = normalized_prefix;
    let profile_name_clone = profile_name.clone();
    let pkgs_clone = packages.clone();

    // Stream stdout/stderr to frontend
    let stdout = child.take_stdout();
    let stderr = child.take_stderr();
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
        let outcome = child.wait_and_restore().await;

        // Update dep states in SQLite
        let state_str = if outcome.install_succeeded {
            "installed"
        } else {
            "install_failed"
        };
        let install_error = outcome
            .install_error
            .as_deref()
            .unwrap_or("install process failed");
        for pkg in &pkgs_clone {
            if let Err(e) = ms_clone.upsert_prefix_dep_state(
                &profile_id,
                pkg,
                &pfx_clone,
                state_str,
                if outcome.install_succeeded {
                    None
                } else {
                    Some(install_error)
                },
            ) {
                tracing::warn!(%e, pkg, "failed to persist prefix dep state after install");
            }
        }

        let journal_persistence = match journal_decision(&outcome) {
            JournalDecision::Delete => ms_clone
                .delete_prefix_version_restore(&normalized_pfx_clone)
                .map_err(|error| error.to_string()),
            JournalDecision::MarkFailed(ref restore_error) => ms_clone
                .mark_prefix_version_restore_failed(&normalized_pfx_clone, restore_error)
                .map_err(|error| error.to_string()),
            JournalDecision::NoMutation => Ok(()),
        };
        if let Err(error) = &journal_persistence {
            tracing::warn!(%error, "failed to persist prefix repair journal outcome");
        }

        let outcome = outcome_after_journal_persistence(outcome, journal_persistence);
        let payload = completion_payload(profile_name_clone, &pfx_clone, outcome);
        let _ = app_final.emit("prefix-dep-complete", payload);
    });

    Ok(())
}

/// Return whether a prefix has an interrupted or failed Windows-version restore.
#[tauri::command]
pub fn get_prefix_version_repair_status(
    profile_name: String,
    prefix_path: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<PrefixVersionRepairStatus, String> {
    drop(profile_name);
    let normalized_prefix =
        runner::normalize_prefix_path(&prefix_path).map_err(|error| error.to_string())?;
    let row = metadata_store
        .load_prefix_version_restore(&normalized_prefix)
        .map_err(|error| error.to_string())?;
    repair_status_from_row(row)
}

/// Retry and verify a persisted Windows-version restore plan.
#[tauri::command]
pub async fn repair_prefix_windows_version(
    profile_name: String,
    prefix_path: String,
    metadata_store: State<'_, MetadataStore>,
    install_state: State<'_, PrefixDepsInstallState>,
) -> Result<PrefixVersionRepairStatus, String> {
    drop(profile_name);
    let normalized_prefix =
        runner::normalize_prefix_path(&prefix_path).map_err(|error| error.to_string())?;
    let _repair_guard = install_state
        .lock
        .try_acquire(normalized_prefix.clone())
        .await
        .map_err(|error| error.to_string())?;

    // Reload after locking so a concurrent install/repair cannot leave us with
    // a stale plan. A missing row is an idempotent success.
    let Some(row) = metadata_store
        .load_prefix_version_restore(&normalized_prefix)
        .map_err(|error| error.to_string())?
    else {
        return repair_status_from_row(None);
    };
    repair_status_from_row(Some(row.clone()))?;
    let plan = restore_plan_from_row(&row)?;
    let outcome = runner::restore_prefix_windows_version(&plan).await;

    match outcome.state {
        PrefixVersionRestoreState::Succeeded => {
            if metadata_store
                .load_prefix_version_restore(&normalized_prefix)
                .map_err(|error| error.to_string())?
                .is_none()
            {
                return repair_status_from_row(None);
            }
            if let Err(delete_error) =
                metadata_store.delete_prefix_version_restore(&normalized_prefix)
            {
                let cleanup_error = format!(
                    "Windows version was restored, but repair journal cleanup failed: {delete_error}"
                );
                metadata_store
                    .mark_prefix_version_restore_failed(&normalized_prefix, &cleanup_error)
                    .map_err(|persist_error| persist_error.to_string())?;
                let persisted = metadata_store
                    .load_prefix_version_restore(&normalized_prefix)
                    .map_err(|load_error| load_error.to_string())?;
                return repair_status_from_row(persisted);
            }
            repair_status_from_row(None)
        }
        PrefixVersionRestoreState::Failed => {
            let error = outcome
                .error
                .unwrap_or_else(|| "Windows version restoration failed".to_string());
            metadata_store
                .mark_prefix_version_restore_failed(&normalized_prefix, &error)
                .map_err(|persist_error| persist_error.to_string())?;
            let persisted = metadata_store
                .load_prefix_version_restore(&normalized_prefix)
                .map_err(|load_error| load_error.to_string())?;
            repair_status_from_row(persisted)
        }
        PrefixVersionRestoreState::NotRequired | PrefixVersionRestoreState::Pending => {
            let error = "Windows version restoration returned an invalid state";
            metadata_store
                .mark_prefix_version_restore_failed(&normalized_prefix, error)
                .map_err(|persist_error| persist_error.to_string())?;
            let persisted = metadata_store
                .load_prefix_version_restore(&normalized_prefix)
                .map_err(|load_error| load_error.to_string())?;
            repair_status_from_row(persisted)
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crosshook_core::metadata::PrefixVersionRestoreJournalRow;
    use crosshook_core::prefix_deps::PrefixDependencyInstallOutcome;

    fn journal_row(state: &str, tool_type: &str) -> PrefixVersionRestoreJournalRow {
        PrefixVersionRestoreJournalRow {
            prefix_path: "/games/example/pfx".to_string(),
            profile_id: Some("profile-1".to_string()),
            binary_path: "/usr/bin/winetricks".to_string(),
            tool_type: tool_type.to_string(),
            steam_app_id: None,
            restore_verb: "win10".to_string(),
            state: state.to_string(),
            last_error: (state == "failed").then(|| "restore timed out".to_string()),
            created_at: "2026-08-24T00:00:00Z".to_string(),
            updated_at: "2026-08-24T00:00:00Z".to_string(),
        }
    }

    fn install_outcome(
        install_succeeded: bool,
        restore_state: PrefixVersionRestoreState,
    ) -> PrefixDependencyInstallOutcome {
        PrefixDependencyInstallOutcome {
            install_succeeded,
            install_exit_code: install_succeeded.then_some(0),
            install_error: (!install_succeeded).then(|| "install failed".to_string()),
            restore_state,
            restore_error: (restore_state == PrefixVersionRestoreState::Failed)
                .then(|| "restore timed out".to_string()),
        }
    }

    #[test]
    fn row_status_mapping_accepts_pending_and_failed_repairs() {
        let pending = repair_status_from_row(Some(journal_row("pending", "winetricks"))).unwrap();
        assert!(pending.required);
        assert_eq!(pending.state, Some(PrefixVersionRestoreState::Pending));
        assert_eq!(pending.last_error, None);

        let failed = repair_status_from_row(Some(journal_row("failed", "protontricks"))).unwrap();
        assert!(failed.required);
        assert_eq!(failed.state, Some(PrefixVersionRestoreState::Failed));
        assert_eq!(failed.last_error.as_deref(), Some("restore timed out"));

        let missing = repair_status_from_row(None).unwrap();
        assert!(!missing.required);
        assert_eq!(missing.state, None);
    }

    #[test]
    fn row_status_mapping_rejects_invalid_state_and_tool() {
        let invalid_state = repair_status_from_row(Some(journal_row("succeeded", "winetricks")));
        assert!(invalid_state.unwrap_err().contains("invalid repair state"));

        let invalid_tool = repair_status_from_row(Some(journal_row("pending", "wine")));
        assert!(invalid_tool
            .unwrap_err()
            .contains("invalid dependency tool"));
    }

    #[test]
    fn journal_decision_only_deletes_verified_success() {
        assert_eq!(
            journal_decision(&install_outcome(true, PrefixVersionRestoreState::Succeeded)),
            JournalDecision::Delete
        );
        assert_eq!(
            journal_decision(&install_outcome(
                true,
                PrefixVersionRestoreState::NotRequired
            )),
            JournalDecision::NoMutation
        );
        assert!(matches!(
            journal_decision(&install_outcome(true, PrefixVersionRestoreState::Failed)),
            JournalDecision::MarkFailed(ref error) if error == "restore timed out"
        ));
    }

    #[test]
    fn completion_payload_preserves_legacy_success_semantics() {
        let restored = completion_payload(
            "profile",
            "/games/example/pfx",
            install_outcome(true, PrefixVersionRestoreState::Succeeded),
        );
        assert!(restored.succeeded);
        assert!(restored.install_succeeded);
        assert_eq!(restored.exit_code, Some(0));
        assert_eq!(restored.install_exit_code, Some(0));

        let degraded = completion_payload(
            "profile",
            "/games/example/pfx",
            install_outcome(true, PrefixVersionRestoreState::Failed),
        );
        assert!(!degraded.succeeded);
        assert!(degraded.install_succeeded);

        let intentional = completion_payload(
            "profile",
            "/games/example/pfx",
            install_outcome(true, PrefixVersionRestoreState::NotRequired),
        );
        assert!(intentional.succeeded);
    }

    #[test]
    fn persisted_protontricks_app_id_must_be_positive_decimal() {
        for invalid in [None, Some(""), Some("0"), Some("abc"), Some("--help")] {
            let mut row = journal_row("pending", "protontricks");
            row.steam_app_id = invalid.map(str::to_string);
            assert!(restore_plan_from_row(&row).is_err(), "accepted {invalid:?}");
        }

        let mut row = journal_row("pending", "protontricks");
        row.steam_app_id = Some(" 12345 ".to_string());
        let plan = restore_plan_from_row(&row).unwrap();
        assert_eq!(plan.steam_app_id.as_deref(), Some("12345"));
    }

    #[test]
    fn journal_delete_failure_keeps_completion_unsafe() {
        let outcome = install_outcome(true, PrefixVersionRestoreState::Succeeded);
        let outcome = outcome_after_journal_persistence(
            outcome,
            Err("failed to delete repair journal".to_string()),
        );
        let payload = completion_payload("profile", "/games/example/pfx", outcome);

        assert!(!payload.succeeded);
        assert!(payload.install_succeeded);
        assert_eq!(payload.restore_state, PrefixVersionRestoreState::Failed);
        assert!(payload
            .restore_error
            .as_deref()
            .unwrap()
            .contains("delete repair journal"));
    }

    #[test]
    fn repair_commands_keep_snake_case_function_contract() {
        let _get_command = get_prefix_version_repair_status;
        let _repair_command = repair_prefix_windows_version;
        assert_eq!(
            stringify!(get_prefix_version_repair_status),
            "get_prefix_version_repair_status"
        );
        assert_eq!(
            stringify!(repair_prefix_windows_version),
            "repair_prefix_windows_version"
        );
    }
}
