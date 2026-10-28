//! Thin IPC adapters over the Proton runtime management service.

use crosshook_app::services::protonup::{
    next_progress, InstallOwner, ProtonInstallHandle, ProtonUninstallPlanResponse,
    ProtonUninstallResponse,
};
use crosshook_app::services::ProtonUpService;
use crosshook_app::{AppError, AppErrorKind};
use crosshook_core::protonup::install_root::InstallRootCandidate;
use crosshook_core::protonup::providers::ProtonUpProviderDescriptor;
use crosshook_core::protonup::{
    ProtonUpAvailableVersion, ProtonUpCatalogResponse, ProtonUpInstallRequest,
    ProtonUpInstallResult, ProtonUpSuggestion,
};
use tauri::{Emitter as _, State};

fn map_app_error(error: AppError) -> String {
    error.message().to_string()
}

// Only pass the label of the injected invoking webview, never a JS argument.
fn install_owner(label: &str) -> InstallOwner {
    InstallOwner::new(label)
}

/// List available Proton versions from the selected provider catalog.
#[tauri::command]
pub async fn protonup_list_available_versions(
    provider: Option<String>,
    force_refresh: Option<bool>,
    service: State<'_, ProtonUpService>,
) -> Result<ProtonUpCatalogResponse, String> {
    service
        .list_available_versions(provider, force_refresh.unwrap_or(false))
        .await
        .map_err(map_app_error)
}

/// Install a selected Proton version and wait for it to finish (legacy variant).
#[tauri::command]
pub async fn protonup_install_version(
    request: ProtonUpInstallRequest,
    service: State<'_, ProtonUpService>,
) -> Result<ProtonUpInstallResult, String> {
    service
        .install_version(request)
        .await
        .map_err(map_app_error)
}

/// Get a runtime suggestion for a community profile.
#[tauri::command]
pub async fn protonup_get_suggestion(
    community_version: String,
    steam_client_install_path: Option<String>,
    service: State<'_, ProtonUpService>,
) -> Result<ProtonUpSuggestion, String> {
    service
        .get_suggestion(community_version, steam_client_install_path)
        .await
        .map_err(map_app_error)
}

/// List all registered Proton release providers with their static metadata.
#[tauri::command]
pub fn protonup_list_providers(
    service: State<'_, ProtonUpService>,
) -> Result<Vec<ProtonUpProviderDescriptor>, String> {
    Ok(service.list_providers())
}

/// Enumerate candidate install roots, including writability status.
#[tauri::command]
pub async fn protonup_resolve_install_roots(
    steam_client_install_path: Option<String>,
    service: State<'_, ProtonUpService>,
) -> Result<Vec<InstallRootCandidate>, String> {
    service
        .resolve_install_roots(steam_client_install_path)
        .await
        .map_err(map_app_error)
}

/// Start a supervised install and relay its progress to the existing IPC event.
#[tauri::command]
pub async fn protonup_install_version_async(
    app: tauri::AppHandle,
    webview: tauri::Webview,
    request: ProtonUpInstallRequest,
    version: ProtonUpAvailableVersion,
    service: State<'_, ProtonUpService>,
) -> Result<ProtonInstallHandle, String> {
    let start = service
        .start_install(install_owner(webview.label()), request, version)
        .await
        .map_err(map_app_error)?;
    let handle = ProtonInstallHandle { op_id: start.op_id };
    let mut progress = start.progress;
    // next_progress skips lagged snapshots and ends only when the sender closes.
    drop(tauri::async_runtime::spawn(async move {
        while let Some(progress) = next_progress(&mut progress).await {
            let _ = app.emit("protonup:install:progress", &progress);
        }
    }));
    Ok(handle)
}

/// Cancel this webview's active install; unknown, foreign or completed ids return false.
#[tauri::command]
pub fn protonup_cancel_install(
    op_id: String,
    webview: tauri::Webview,
    service: State<'_, ProtonUpService>,
) -> Result<bool, String> {
    match service.cancel_install(&install_owner(webview.label()), &op_id) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == AppErrorKind::NotFound => Ok(false),
        Err(error) => Err(map_app_error(error)),
    }
}

/// Uninstall a Proton tool, returning a structured refusal or failure if needed.
#[tauri::command]
pub async fn protonup_uninstall_version(
    tool_path: String,
    steam_client_install_path: Option<String>,
    service: State<'_, ProtonUpService>,
) -> Result<ProtonUninstallResponse, String> {
    service
        .uninstall(tool_path, steam_client_install_path)
        .await
        .map_err(map_app_error)
}

/// Plan a Proton uninstall without mutating disk.
#[tauri::command]
pub async fn protonup_plan_uninstall_version(
    tool_path: String,
    steam_client_install_path: Option<String>,
    service: State<'_, ProtonUpService>,
) -> Result<ProtonUninstallPlanResponse, String> {
    service
        .plan_uninstall(tool_path, steam_client_install_path)
        .await
        .map_err(map_app_error)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn app_error_maps_to_its_plain_message() {
        let error = AppError::validation(
            "version",
            "install metadata did not match server-resolved catalog entry",
        );
        assert_eq!(
            map_app_error(error),
            "install metadata did not match server-resolved catalog entry"
        );
    }

    #[test]
    fn install_owner_is_derived_from_the_webview_label() {
        assert_eq!(install_owner("main"), InstallOwner::new("main"));
        assert_ne!(install_owner("main"), install_owner("proton-manager"));
    }
}
