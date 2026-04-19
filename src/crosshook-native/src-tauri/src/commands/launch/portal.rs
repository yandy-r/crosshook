use crosshook_core::launch::{should_register_gamemode_portal, LaunchRequest};
use crosshook_core::platform::portals::gamemode::{self as gamemode_portal, GameModeRegistration};

/// Attempts to register CrossHook's own sandbox-side PID with the GameMode
/// portal, if the request and environment warrant it.
///
/// `effective_method` is the method under which the child process will
/// actually run. For direct Proton game launches this equals
/// `request.resolved_method()`; for Flatpak Steam trainer launches it is
/// rewritten to `METHOD_PROTON_RUN` because the helper spawns the trainer
/// through Proton directly.
pub(super) async fn try_register_gamemode_portal_for_launch(
    request: &LaunchRequest,
    effective_method: &str,
) -> Option<GameModeRegistration> {
    if !should_register_gamemode_portal(request, effective_method) {
        return None;
    }

    match gamemode_portal::probe_and_register_via_portal().await {
        Ok(Some(guard)) => {
            tracing::info!(
                registered_pid = guard.registered_pid(),
                "gamemode portal registration: backend=Portal"
            );
            Some(guard)
        }
        Ok(None) => {
            tracing::info!(
                "gamemode portal registration skipped: org.freedesktop.portal.GameMode not reachable"
            );
            None
        }
        Err(error) => {
            tracing::warn!(%error, "gamemode portal: RegisterGame failed; falling back to host gamemoderun wrapper only");
            None
        }
    }
}
