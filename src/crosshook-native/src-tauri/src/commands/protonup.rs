use crosshook_core::metadata::MetadataStore;
use crosshook_core::protonup::matching::match_community_version;
use crosshook_core::protonup::{
    parse_protonup_provider, ProtonUpCatalogResponse, ProtonUpInstallErrorKind,
    ProtonUpInstallRequest, ProtonUpInstallResult, ProtonUpSuggestion,
};
use crosshook_core::steam::{discover_compat_tools, discover_steam_root_candidates};
use tauri::State;

/// List available Proton versions from provider catalog.
#[tauri::command]
pub async fn protonup_list_available_versions(
    metadata_store: State<'_, MetadataStore>,
    provider: Option<String>,
    force_refresh: Option<bool>,
) -> Result<ProtonUpCatalogResponse, String> {
    let provider = parse_protonup_provider(provider.as_deref());
    Ok(
        crosshook_core::protonup::catalog::list_available_versions(
            &metadata_store,
            force_refresh.unwrap_or(false),
            provider,
        )
        .await,
    )
}

/// Install a selected Proton version.
#[tauri::command]
pub async fn protonup_install_version(
    metadata_store: State<'_, MetadataStore>,
    request: ProtonUpInstallRequest,
) -> Result<ProtonUpInstallResult, String> {
    // Look up version info from catalog first (same provider as the install request).
    let provider = parse_protonup_provider(Some(request.provider.as_str()));
    let catalog = crosshook_core::protonup::catalog::list_available_versions(
        &metadata_store,
        false,
        provider,
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
            error_message: Some(format!(
                "version {} not found in catalog",
                request.version
            )),
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
