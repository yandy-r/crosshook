//! Thin IPC adapters over the Proton runtime management service.

use crosshook_app::ipc::cancel_accepted;
use crosshook_app::services::protonup::{
    InstallOwner, ProtonInstallHandle, ProtonUninstallPlanResponse, ProtonUninstallResponse,
};
use crosshook_core::protonup::install_root::InstallRootCandidate;
use crosshook_core::protonup::providers::ProtonUpProviderDescriptor;
use crosshook_core::protonup::{
    ProtonUpAvailableVersion, ProtonUpCatalogResponse, ProtonUpInstallRequest,
    ProtonUpInstallResult, ProtonUpSuggestion,
};
use tauri::AppHandle;

use super::{AppState, IpcResult};
use crate::error_map::ipc;
use crate::event_bridge::relay_protonup_progress;

// Only pass the label of the injected invoking webview, never a JS argument.
fn install_owner(label: &str) -> InstallOwner {
    InstallOwner::new(label)
}

#[tauri::command]
pub async fn protonup_list_available_versions(
    provider: Option<String>,
    force_refresh: Option<bool>,
    app: AppState<'_>,
) -> IpcResult<ProtonUpCatalogResponse> {
    ipc(app
        .services()
        .protonup
        .list_available_versions(provider, force_refresh.unwrap_or(false))
        .await)
}

#[tauri::command]
pub async fn protonup_install_version(
    request: ProtonUpInstallRequest,
    app: AppState<'_>,
) -> IpcResult<ProtonUpInstallResult> {
    ipc(app.services().protonup.install_version(request).await)
}

#[tauri::command]
pub async fn protonup_get_suggestion(
    community_version: String,
    steam_client_install_path: Option<String>,
    app: AppState<'_>,
) -> IpcResult<ProtonUpSuggestion> {
    ipc(app
        .services()
        .protonup
        .get_suggestion(community_version, steam_client_install_path)
        .await)
}

#[tauri::command]
pub async fn protonup_list_providers(
    app: AppState<'_>,
) -> IpcResult<Vec<ProtonUpProviderDescriptor>> {
    Ok(app.services().protonup.list_providers())
}

/// Enumerate candidate install roots, including writability status.
#[tauri::command]
pub async fn protonup_resolve_install_roots(
    steam_client_install_path: Option<String>,
    app: AppState<'_>,
) -> IpcResult<Vec<InstallRootCandidate>> {
    ipc(app
        .services()
        .protonup
        .resolve_install_roots(steam_client_install_path)
        .await)
}

/// Start a supervised install and relay its progress to the existing IPC event.
#[tauri::command]
pub async fn protonup_install_version_async(
    handle: AppHandle,
    webview: tauri::Webview,
    request: ProtonUpInstallRequest,
    version: ProtonUpAvailableVersion,
    app: AppState<'_>,
) -> IpcResult<ProtonInstallHandle> {
    let start = ipc(app
        .services()
        .protonup
        .start_install(install_owner(webview.label()), request, version)
        .await)?;
    relay_protonup_progress(handle, start.progress);
    Ok(ProtonInstallHandle { op_id: start.op_id })
}

/// Cancel this webview's active install; unknown, foreign or completed ids return false.
#[tauri::command]
pub async fn protonup_cancel_install(
    op_id: String,
    webview: tauri::Webview,
    app: AppState<'_>,
) -> IpcResult<bool> {
    ipc(cancel_accepted(
        app.services()
            .protonup
            .cancel_install(&install_owner(webview.label()), &op_id),
    ))
}

#[tauri::command]
pub async fn protonup_uninstall_version(
    tool_path: String,
    steam_client_install_path: Option<String>,
    app: AppState<'_>,
) -> IpcResult<ProtonUninstallResponse> {
    ipc(app
        .services()
        .protonup
        .uninstall(tool_path, steam_client_install_path)
        .await)
}

#[tauri::command]
pub async fn protonup_plan_uninstall_version(
    tool_path: String,
    steam_client_install_path: Option<String>,
    app: AppState<'_>,
) -> IpcResult<ProtonUninstallPlanResponse> {
    ipc(app
        .services()
        .protonup
        .plan_uninstall(tool_path, steam_client_install_path)
        .await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_owner_is_derived_from_the_webview_label() {
        assert_eq!(install_owner("main"), InstallOwner::new("main"));
        assert_ne!(install_owner("main"), install_owner("other"));
    }

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = protonup_list_available_versions;
        let _ = protonup_install_version;
        let _ = protonup_get_suggestion;
        let _ = protonup_list_providers;
        let _ = protonup_resolve_install_roots;
        let _ = protonup_install_version_async;
        let _ = protonup_cancel_install;
        let _ = protonup_uninstall_version;
        let _ = protonup_plan_uninstall_version;
    }
}
