use std::collections::HashMap;
use std::sync::Mutex;

use crosshook_core::metadata::MetadataStore;
use crosshook_core::protonup::install_root::InstallRootCandidate;
use crosshook_core::protonup::matching::match_community_version;
use crosshook_core::protonup::providers::{describe_providers, ProtonUpProviderDescriptor};
use crosshook_core::protonup::{
    ProtonUpAvailableVersion, ProtonUpCatalogResponse, ProtonUpInstallErrorKind,
    ProtonUpInstallRequest, ProtonUpInstallResult, ProtonUpSuggestion,
};

/// Default provider id used when the frontend omits the field. Unknown
/// provider ids are passed through to the catalog (which returns empty
/// when the registry doesn't recognise them) rather than silently
/// downgrading to GE-Proton.
const DEFAULT_PROVIDER_ID: &str = "ge-proton";
use crosshook_core::steam::{discover_compat_tools, discover_steam_root_candidates};
use tauri::{Emitter as _, State};
use tokio_util::sync::CancellationToken;

// ── ProtonInstallRegistry ─────────────────────────────────────────────────────

/// Tracks in-flight async Proton installs so they can be cancelled by op_id.
///
/// Managed as `Arc<ProtonInstallRegistry>` so the inner value can be cloned
/// into `tokio::spawn` closures without holding a Tauri `State` reference.
#[derive(Default)]
pub struct ProtonInstallRegistry {
    inner: Mutex<HashMap<String, CancellationToken>>,
}

impl ProtonInstallRegistry {
    pub fn register(&self, op_id: &str, token: CancellationToken) {
        self.inner
            .lock()
            .expect("ProtonInstallRegistry mutex poisoned")
            .insert(op_id.to_string(), token);
    }

    pub fn cancel(&self, op_id: &str) -> bool {
        if let Some(token) = self
            .inner
            .lock()
            .expect("ProtonInstallRegistry mutex poisoned")
            .remove(op_id)
        {
            token.cancel();
            true
        } else {
            false
        }
    }

    pub fn remove(&self, op_id: &str) {
        let _ = self
            .inner
            .lock()
            .expect("ProtonInstallRegistry mutex poisoned")
            .remove(op_id);
    }
}

// ── DTOs ──────────────────────────────────────────────────────────────────────

/// Returned from `protonup_install_version_async` so the frontend can track
/// the operation and subscribe to `protonup:install:progress` events.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProtonInstallHandle {
    pub op_id: String,
}

/// Returned from `protonup_uninstall_version`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProtonUninstallResponse {
    pub success: bool,
    pub conflicting_app_ids: Vec<String>,
    pub error_message: Option<String>,
}

// ── existing commands (preserved) ─────────────────────────────────────────────

/// List available Proton versions from provider catalog.
///
/// Dispatch is id-based so the frontend can request any provider the
/// registry knows about (GE-Proton, Proton-CachyOS, Proton-EM, …).
/// Previously this went through the closed `ProtonUpProvider` enum, which
/// silently mapped unknown ids to GE-Proton — hence `Proton-EM` appeared
/// to alias GE-Proton in the UI.
#[tauri::command]
pub async fn protonup_list_available_versions(
    metadata_store: State<'_, MetadataStore>,
    provider: Option<String>,
    force_refresh: Option<bool>,
) -> Result<ProtonUpCatalogResponse, String> {
    let provider_id = provider.as_deref().unwrap_or(DEFAULT_PROVIDER_ID);
    Ok(
        crosshook_core::protonup::catalog::list_available_versions_by_id(
            &metadata_store,
            force_refresh.unwrap_or(false),
            provider_id,
        )
        .await,
    )
}

/// Install a selected Proton version (synchronous/backward-compat variant).
#[tauri::command]
pub async fn protonup_install_version(
    metadata_store: State<'_, MetadataStore>,
    request: ProtonUpInstallRequest,
) -> Result<ProtonUpInstallResult, String> {
    // Look up version info from catalog using the request's exact provider id.
    let catalog = crosshook_core::protonup::catalog::list_available_versions_by_id(
        &metadata_store,
        false,
        request.provider.as_str(),
    )
    .await;

    let version_info = catalog
        .versions
        .iter()
        .find(|v| v.version == request.version && v.provider == request.provider)
        .cloned();

    let Some(version_info) = version_info else {
        return Ok(ProtonUpInstallResult {
            success: false,
            installed_path: None,
            error_kind: Some(ProtonUpInstallErrorKind::DependencyMissing),
            error_message: Some(format!("version {} not found in catalog", request.version)),
        });
    };

    Ok(crosshook_core::protonup::install::install_version(&request, &version_info).await)
}

/// Get runtime suggestion for a community profile.
#[tauri::command]
pub fn protonup_get_suggestion(
    community_version: String,
    steam_client_install_path: Option<String>,
) -> Result<ProtonUpSuggestion, String> {
    let configured_path =
        steam_client_install_path.unwrap_or_else(super::steam::default_steam_client_install_path);
    let mut diagnostics = Vec::new();
    let steam_root_candidates = discover_steam_root_candidates(configured_path, &mut diagnostics);
    let installs = discover_compat_tools(&steam_root_candidates, &mut diagnostics);

    for entry in &diagnostics {
        tracing::debug!(entry, "protonup suggestion diagnostic");
    }

    Ok(match_community_version(&community_version, &installs))
}

// ── new commands (Batch 4, Steps 4.1 + 4.2) ──────────────────────────────────

/// List all registered Proton release providers with their static metadata.
#[tauri::command]
pub fn protonup_list_providers() -> Result<Vec<ProtonUpProviderDescriptor>, String> {
    Ok(describe_providers())
}

/// Enumerate candidate `compatibilitytools.d` install roots for the current
/// environment, including writability status.
///
/// `steam_client_install_path` is the configured Steam client path from
/// settings (used to derive an additional candidate directory). Pass `null`
/// to rely on home-relative defaults only.
#[tauri::command]
pub async fn protonup_resolve_install_roots(
    steam_client_install_path: Option<String>,
) -> Result<Vec<InstallRootCandidate>, String> {
    let path_buf = steam_client_install_path
        .as_deref()
        .map(std::path::PathBuf::from);
    Ok(
        crosshook_core::protonup::install_root::resolve_install_root_candidates(
            path_buf.as_deref(),
        ),
    )
}

/// Spawn a Proton install in the background and return immediately with an
/// `op_id` handle. Progress is streamed via `protonup:install:progress` events.
/// The operation can be cancelled by passing the `op_id` to
/// `protonup_cancel_install`.
#[tauri::command]
pub async fn protonup_install_version_async(
    app: tauri::AppHandle,
    request: ProtonUpInstallRequest,
    version: ProtonUpAvailableVersion,
    registry: State<'_, std::sync::Arc<ProtonInstallRegistry>>,
) -> Result<ProtonInstallHandle, String> {
    use crosshook_core::protonup::install::install_version_with_progress;
    use crosshook_core::protonup::progress::ProgressEmitter;

    let op_id = uuid::Uuid::new_v4().to_string();
    let (emitter, mut rx) = ProgressEmitter::new(&op_id);
    let cancel = CancellationToken::new();

    // Register the token so it can be cancelled via protonup_cancel_install.
    registry.register(&op_id, cancel.clone());

    // Clone the Arc so we can move it into the install task.
    let registry_arc = (*registry).clone();
    let op_id_task = op_id.clone();

    // Pump progress events from the broadcast channel into Tauri events.
    // No post-loop sentinel: the install orchestrator already emits a terminal
    // Phase (Done / Failed / Cancelled) before dropping the sender, which is
    // the signal the frontend uses to flip into a terminal state.
    let app_pump = app.clone();
    tokio::spawn(async move {
        while let Ok(progress) = rx.recv().await {
            let _ = app_pump.emit("protonup:install:progress", &progress);
        }
    });

    // Spawn the actual install task.
    tokio::spawn(async move {
        let _result =
            install_version_with_progress(&request, &version, Some(emitter), Some(cancel)).await;
        registry_arc.remove(&op_id_task);
    });

    Ok(ProtonInstallHandle { op_id })
}

/// Cancel a running Proton install by its `op_id`.
///
/// Returns `true` if the operation was found and cancelled, `false` if it was
/// already complete or the `op_id` was not recognised.
#[tauri::command]
pub fn protonup_cancel_install(
    op_id: String,
    registry: State<'_, std::sync::Arc<ProtonInstallRegistry>>,
) -> Result<bool, String> {
    Ok(registry.cancel(&op_id))
}

/// Uninstall a Proton tool from disk.
///
/// Returns a structured response rather than a Tauri error so the UI can
/// render plan warnings (e.g. conflicting App IDs) before or after removal.
#[tauri::command]
pub async fn protonup_uninstall_version(
    tool_path: String,
    steam_client_install_path: Option<String>,
) -> Result<ProtonUninstallResponse, String> {
    use crosshook_core::protonup::uninstall::{execute_uninstall, plan_uninstall};

    let tool = std::path::Path::new(&tool_path);
    let steam_buf = steam_client_install_path
        .as_deref()
        .map(std::path::PathBuf::from);
    let steam = steam_buf.as_deref();

    match plan_uninstall(tool, steam) {
        Ok(plan) => {
            let conflicts = plan.conflicting_app_ids.clone();
            match execute_uninstall(plan) {
                Ok(()) => Ok(ProtonUninstallResponse {
                    success: true,
                    conflicting_app_ids: conflicts,
                    error_message: None,
                }),
                Err(e) => Ok(ProtonUninstallResponse {
                    success: false,
                    conflicting_app_ids: conflicts,
                    error_message: Some(e.to_string()),
                }),
            }
        }
        Err(e) => Ok(ProtonUninstallResponse {
            success: false,
            conflicting_app_ids: vec![],
            error_message: Some(e.to_string()),
        }),
    }
}
