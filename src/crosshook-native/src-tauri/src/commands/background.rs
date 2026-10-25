use crosshook_app::services::{BackgroundGrantHolder, BackgroundProtectionState};
use std::sync::Arc;
use tauri::State;

/// Tauri command: report the current background-protection state so the
/// host-tool dashboard can surface it as a capability row.
///
/// Returns [`BackgroundProtectionState::Pending`] transiently at startup
/// under Flatpak while the one-time `RequestBackground` call is in flight;
/// the frontend should treat Pending as an indeterminate state and refresh.
/// No TypeScript consumer exists yet (ADR-0002 § Capability integration
/// marks the UI integration as deferred).
#[tauri::command]
pub fn get_background_protection_state(
    holder: State<'_, Arc<BackgroundGrantHolder>>,
) -> BackgroundProtectionState {
    holder.protection_state()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = get_background_protection_state
            as fn(State<'_, Arc<BackgroundGrantHolder>>) -> BackgroundProtectionState;
    }
}
