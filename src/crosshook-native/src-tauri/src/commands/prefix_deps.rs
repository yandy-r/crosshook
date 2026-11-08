//! Thin IPC adapters over the prefix-dependency service.
//!
//! The service owns tool selection, the per-prefix job key, the supervised
//! install child, the Windows-version restore journal and its recovery. An
//! install's events are relayed by `crate::event_bridge` with the raw request
//! `profile_name`/`prefix_path` (listeners match on the exact strings they sent).

use crosshook_app::services::prefix_deps::PrefixVersionRepairStatus;
use crosshook_core::prefix_deps::{BinaryDetectionResult, PrefixDependencyStatus};
use tauri::AppHandle;

use super::{AppState, IpcResult};
use crate::error_map::ipc;
use crate::event_bridge::relay_prefix_deps;

#[tauri::command]
pub async fn detect_protontricks_binary(app: AppState<'_>) -> IpcResult<BinaryDetectionResult> {
    ipc(app.services().prefix_deps.detect_binary().await)
}

/// Check which prefix dependencies are installed for a profile.
#[tauri::command]
pub async fn check_prefix_dependencies(
    profile_name: String,
    prefix_path: String,
    packages: Vec<String>,
    app: AppState<'_>,
) -> IpcResult<Vec<PrefixDependencyStatus>> {
    ipc(app
        .services()
        .prefix_deps
        .check(profile_name, prefix_path, packages)
        .await)
}

/// Start installing prefix dependencies; progress streams on `prefix-dep-log`,
/// then `prefix-dep-complete`.
#[tauri::command]
pub async fn install_prefix_dependency(
    profile_name: String,
    prefix_path: String,
    packages: Vec<String>,
    handle: AppHandle,
    app: AppState<'_>,
) -> IpcResult<()> {
    let run = ipc(app
        .services()
        .prefix_deps
        .start_install(profile_name.clone(), prefix_path.clone(), packages)
        .await)?;
    relay_prefix_deps(handle, profile_name, prefix_path, run);
    Ok(())
}

/// Return whether a prefix has an interrupted or failed Windows-version restore.
/// `profile_name` is part of the frozen IPC arguments; the status is per prefix.
#[tauri::command]
pub async fn get_prefix_version_repair_status(
    profile_name: String,
    prefix_path: String,
    app: AppState<'_>,
) -> IpcResult<PrefixVersionRepairStatus> {
    drop(profile_name);
    ipc(app.services().prefix_deps.repair_status(prefix_path).await)
}

/// Retry and verify a persisted Windows-version restore plan.
/// `profile_name` is part of the frozen IPC arguments; the repair is per prefix.
#[tauri::command]
pub async fn repair_prefix_windows_version(
    profile_name: String,
    prefix_path: String,
    app: AppState<'_>,
) -> IpcResult<PrefixVersionRepairStatus> {
    drop(profile_name);
    ipc(app.services().prefix_deps.repair(prefix_path).await)
}

#[tauri::command]
pub async fn get_dependency_status(
    profile_name: String,
    prefix_path: String,
    app: AppState<'_>,
) -> IpcResult<Vec<PrefixDependencyStatus>> {
    ipc(app
        .services()
        .prefix_deps
        .dependency_status(profile_name, prefix_path)
        .await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = detect_protontricks_binary;
        let _ = check_prefix_dependencies;
        let _ = install_prefix_dependency;
        let _ = get_prefix_version_repair_status;
        let _ = repair_prefix_windows_version;
        let _ = get_dependency_status;
    }
}
