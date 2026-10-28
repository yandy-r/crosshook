use crosshook_app::services::onboarding::{CachedHostReadinessSnapshot, LegacyNag};
use crosshook_app::services::{HostToolsService, OnboardingService};
use crosshook_core::onboarding::{
    Capability, HostToolDetails, ReadinessCheckResult, TrainerGuidanceContent,
};
use tauri::State;

#[tauri::command]
pub async fn check_readiness(
    service: State<'_, OnboardingService>,
) -> Result<ReadinessCheckResult, String> {
    service
        .check_readiness()
        .await
        .map_err(|e| e.message().to_string())
}

#[tauri::command]
pub async fn check_generalized_readiness(
    service: State<'_, OnboardingService>,
) -> Result<ReadinessCheckResult, String> {
    service
        .check_generalized_readiness()
        .await
        .map_err(|e| e.message().to_string())
}

#[tauri::command]
pub async fn probe_host_tool_details(
    tool_id: String,
    service: State<'_, HostToolsService>,
) -> Result<HostToolDetails, String> {
    service
        .tool_details(tool_id)
        .await
        .map_err(|e| e.message().to_string())
}

#[tauri::command]
pub async fn get_cached_host_readiness_snapshot(
    service: State<'_, OnboardingService>,
) -> Result<Option<CachedHostReadinessSnapshot>, String> {
    service
        .cached_host_readiness_snapshot()
        .await
        .map_err(|e| e.message().to_string())
}

#[tauri::command]
pub async fn get_capabilities(
    service: State<'_, OnboardingService>,
) -> Result<Vec<Capability>, String> {
    service
        .capabilities()
        .await
        .map_err(|e| e.message().to_string())
}

#[tauri::command]
pub async fn dismiss_onboarding(service: State<'_, OnboardingService>) -> Result<(), String> {
    service
        .dismiss_onboarding()
        .await
        .map_err(|e| e.message().to_string())
}

#[tauri::command]
pub async fn dismiss_umu_install_nag(service: State<'_, OnboardingService>) -> Result<(), String> {
    service
        .dismiss_legacy_nag(LegacyNag::UmuInstall)
        .await
        .map_err(|e| e.message().to_string())
}

#[tauri::command]
pub async fn dismiss_steam_deck_caveats(
    service: State<'_, OnboardingService>,
) -> Result<(), String> {
    service
        .dismiss_legacy_nag(LegacyNag::SteamDeckCaveats)
        .await
        .map_err(|e| e.message().to_string())
}

#[tauri::command]
pub async fn dismiss_readiness_nag(
    service: State<'_, OnboardingService>,
    tool_id: String,
    ttl_days: Option<u32>,
) -> Result<(), String> {
    service
        .dismiss_readiness_nag(tool_id, ttl_days)
        .await
        .map_err(|e| e.message().to_string())
}

#[tauri::command]
pub async fn get_trainer_guidance(
    service: State<'_, OnboardingService>,
) -> Result<TrainerGuidanceContent, String> {
    Ok(service.trainer_guidance())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = check_readiness;
        let _ = check_generalized_readiness;
        let _ = probe_host_tool_details;
        let _ = get_cached_host_readiness_snapshot;
        let _ = get_capabilities;
        let _ = dismiss_onboarding;
        let _ = dismiss_umu_install_nag;
        let _ = dismiss_steam_deck_caveats;
        let _ = dismiss_readiness_nag;
        let _ = get_trainer_guidance;
    }
}
