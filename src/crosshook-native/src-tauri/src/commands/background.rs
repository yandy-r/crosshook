use super::{AppState, IpcResult};
use crosshook_app::services::BackgroundProtectionState;

/// Reports the current background-protection state so the host-tool dashboard
/// can surface it as a capability row.
///
/// Returns [`BackgroundProtectionState::Pending`] transiently at startup
/// under Flatpak while the one-time `RequestBackground` call is in flight;
/// the frontend should treat Pending as an indeterminate state and refresh.
/// No TypeScript consumer exists yet (ADR-0002 § Capability integration
/// marks the UI integration as deferred).
#[tauri::command]
pub async fn get_background_protection_state(
    app: AppState<'_>,
) -> IpcResult<BackgroundProtectionState> {
    Ok(app.services().background.protection_state())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = get_background_protection_state;
    }
}
