use crate::platform;

use super::super::request::{LaunchRequest, METHOD_PROTON_RUN};

/// Optimization ID for the `gamemoderun` wrapper / GameMode integration.
///
/// Exposed as a constant so the launch orchestrator can decide whether to
/// register CrossHook's own sandbox PID with
/// `org.freedesktop.portal.GameMode` before spawning the host command.
/// See `docs/architecture/adr-0002-flatpak-portal-contracts.md`.
pub const USE_GAMEMODE_OPTIMIZATION_ID: &str = "use_gamemode";

/// Returns true when the request opts into `use_gamemode`, the child will
/// actually be launched through Proton (the effective execution method is
/// `proton_run`), and the process is running under Flatpak — in which case
/// the launch orchestrator should call
/// `crate::platform::portals::gamemode::register_self_pid_with_portal()` to
/// register CrossHook's own sandbox PID with the host GameMode daemon.
///
/// `effective_method` is the method the child will actually run under, not
/// the method stored in the request. The two diverge for Flatpak Steam
/// trainer launches, where the parent method is `steam_applaunch` but the
/// helper rewrites the trainer subprocess to go through Proton directly
/// (see `script_runner::build_flatpak_steam_trainer_command`). Per the
/// repository's trainer-execution-parity rule, the portal decision must
/// follow the actual runtime path, not the parent request method.
///
/// This helper does **not** touch D-Bus. It only encodes the "should we try"
/// decision; the async `portal_available` + `register_self_pid_with_portal`
/// calls happen in the IPC layer (`src-tauri/src/commands/launch.rs`).
///
/// Host games continue to use the `gamemoderun` wrapper unconditionally when
/// `use_gamemode` is enabled — the portal is for CrossHook's own PID only.
pub fn should_register_gamemode_portal(request: &LaunchRequest, effective_method: &str) -> bool {
    should_register_gamemode_portal_with(request, platform::is_flatpak(), effective_method)
}

/// Testable helper for [`should_register_gamemode_portal`] that takes the
/// `is_flatpak` signal and effective execution method as injected parameters.
pub(crate) fn should_register_gamemode_portal_with(
    request: &LaunchRequest,
    is_flatpak: bool,
    effective_method: &str,
) -> bool {
    if !is_flatpak {
        return false;
    }
    if effective_method != METHOD_PROTON_RUN {
        return false;
    }
    request
        .optimizations
        .enabled_option_ids
        .iter()
        .any(|id| id == USE_GAMEMODE_OPTIMIZATION_ID)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::request::METHOD_STEAM_APPLAUNCH;

    fn gamemode_proton_request() -> LaunchRequest {
        LaunchRequest {
            method: METHOD_PROTON_RUN.to_string(),
            optimizations: crate::launch::request::LaunchOptimizationsRequest {
                enabled_option_ids: vec![USE_GAMEMODE_OPTIMIZATION_ID.to_string()],
            },
            ..Default::default()
        }
    }

    #[test]
    fn should_register_gamemode_portal_native_is_false() {
        let request = gamemode_proton_request();
        assert!(!should_register_gamemode_portal_with(
            &request,
            false,
            METHOD_PROTON_RUN
        ));
    }

    #[test]
    fn should_register_gamemode_portal_flatpak_with_use_gamemode_is_true() {
        let request = gamemode_proton_request();
        assert!(should_register_gamemode_portal_with(
            &request,
            true,
            METHOD_PROTON_RUN
        ));
    }

    #[test]
    fn should_register_gamemode_portal_flatpak_without_use_gamemode_is_false() {
        let request = LaunchRequest {
            method: METHOD_PROTON_RUN.to_string(),
            ..Default::default()
        };
        assert!(!should_register_gamemode_portal_with(
            &request,
            true,
            METHOD_PROTON_RUN
        ));
    }

    #[test]
    fn should_register_gamemode_portal_non_proton_effective_method_is_false() {
        // Even if the request carries `method = proton_run`, if the caller
        // tells us the child actually runs under another method, skip.
        let request = gamemode_proton_request();
        assert!(!should_register_gamemode_portal_with(
            &request,
            true,
            METHOD_STEAM_APPLAUNCH
        ));
    }

    #[test]
    fn should_register_gamemode_portal_follows_effective_method_not_request_method() {
        // Regression: Flatpak Steam trainer launches carry
        // `method = steam_applaunch` on the request but the helper rewrites
        // the child to run under `proton_run` and applies `gamemoderun`.
        // The portal decision must follow the actual execution method.
        let mut request = gamemode_proton_request();
        request.method = METHOD_STEAM_APPLAUNCH.to_string();
        assert!(should_register_gamemode_portal_with(
            &request,
            true,
            METHOD_PROTON_RUN
        ));
    }
}
