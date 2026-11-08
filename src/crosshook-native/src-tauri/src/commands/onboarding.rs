use crosshook_app::services::onboarding::{CachedHostReadinessSnapshot, LegacyNag};
use crosshook_core::onboarding::{
    Capability, HostToolDetails, ReadinessCheckResult, TrainerGuidanceContent,
};

use super::{AppState, IpcResult};
use crate::error_map::ipc;

#[tauri::command]
pub async fn check_readiness(app: AppState<'_>) -> IpcResult<ReadinessCheckResult> {
    ipc(app.services().onboarding.check_readiness().await)
}

#[tauri::command]
pub async fn check_generalized_readiness(app: AppState<'_>) -> IpcResult<ReadinessCheckResult> {
    ipc(app
        .services()
        .onboarding
        .check_generalized_readiness()
        .await)
}

#[tauri::command]
pub async fn probe_host_tool_details(
    tool_id: String,
    app: AppState<'_>,
) -> IpcResult<HostToolDetails> {
    ipc(app.services().host_tools.tool_details(tool_id).await)
}

#[tauri::command]
pub async fn get_cached_host_readiness_snapshot(
    app: AppState<'_>,
) -> IpcResult<Option<CachedHostReadinessSnapshot>> {
    ipc(app
        .services()
        .onboarding
        .cached_host_readiness_snapshot()
        .await)
}

#[tauri::command]
pub async fn get_capabilities(app: AppState<'_>) -> IpcResult<Vec<Capability>> {
    ipc(app.services().onboarding.capabilities().await)
}

#[tauri::command]
pub async fn dismiss_onboarding(app: AppState<'_>) -> IpcResult<()> {
    ipc(app.services().onboarding.dismiss_onboarding().await)
}

#[tauri::command]
pub async fn dismiss_umu_install_nag(app: AppState<'_>) -> IpcResult<()> {
    ipc(app
        .services()
        .onboarding
        .dismiss_legacy_nag(LegacyNag::UmuInstall)
        .await)
}

#[tauri::command]
pub async fn dismiss_steam_deck_caveats(app: AppState<'_>) -> IpcResult<()> {
    ipc(app
        .services()
        .onboarding
        .dismiss_legacy_nag(LegacyNag::SteamDeckCaveats)
        .await)
}

#[tauri::command]
pub async fn dismiss_readiness_nag(
    app: AppState<'_>,
    tool_id: String,
    ttl_days: Option<u32>,
) -> IpcResult<()> {
    ipc(app
        .services()
        .onboarding
        .dismiss_readiness_nag(tool_id, ttl_days)
        .await)
}

#[tauri::command]
pub async fn get_trainer_guidance(app: AppState<'_>) -> IpcResult<TrainerGuidanceContent> {
    Ok(app.services().onboarding.trainer_guidance())
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
