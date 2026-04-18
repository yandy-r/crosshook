use crate::launch::{runtime_helpers::resolve_umu_run_path, LaunchRequest, METHOD_STEAM_APPLAUNCH};

/// Returns the best available identifier for umu-run's `GAMEID`.
///
/// Precedence: `runtime.umu_game_id` (user protonfix override) → `steam.app_id` →
/// `runtime.steam_app_id` → `""`.
pub(crate) fn resolve_steam_app_id_for_umu(request: &LaunchRequest) -> &str {
    let umu_override = request.runtime.umu_game_id.trim();
    if !umu_override.is_empty() {
        return umu_override;
    }
    let steam_id = request.steam.app_id.trim();
    if !steam_id.is_empty() {
        return steam_id;
    }
    let runtime_id = request.runtime.steam_app_id.trim();
    if !runtime_id.is_empty() {
        return runtime_id;
    }
    ""
}

pub(super) fn resolved_umu_game_id_for_env(request: &LaunchRequest) -> String {
    let trimmed = resolve_steam_app_id_for_umu(request).trim();
    if trimmed.is_empty() {
        "umu-0".to_string()
    } else {
        trimmed.to_string()
    }
}

pub(crate) fn force_no_umu_for_launch_request(request: &LaunchRequest) -> bool {
    request.launch_trainer_only
        && crate::platform::is_flatpak()
        && request.resolved_method() == METHOD_STEAM_APPLAUNCH
}

pub(crate) fn should_use_umu(
    request: &LaunchRequest,
    force_no_umu: bool,
) -> (bool, Option<String>) {
    use crate::settings::UmuPreference;
    if force_no_umu {
        tracing::info!(
            preference = ?request.umu_preference,
            "should_use_umu: force_no_umu=true → direct Proton"
        );
        return (false, None);
    }
    match request.umu_preference {
        UmuPreference::Proton => {
            tracing::info!(
                preference = ?request.umu_preference,
                "should_use_umu: preference = Proton → direct Proton"
            );
            return (false, None);
        }
        UmuPreference::Umu | UmuPreference::Auto => {}
    }
    match resolve_umu_run_path() {
        Some(path) => {
            tracing::info!(
                preference = ?request.umu_preference,
                umu_run_path = %path,
                "should_use_umu: using umu-run"
            );
            (true, Some(path))
        }
        None => {
            tracing::info!(
                preference = ?request.umu_preference,
                path_env = ?std::env::var_os("PATH"),
                "should_use_umu: umu-run not found on PATH → direct Proton"
            );
            (false, None)
        }
    }
}

pub(crate) fn proton_path_dirname(proton_path: &str) -> String {
    std::path::Path::new(proton_path.trim())
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub(super) fn warn_on_umu_fallback(request: &LaunchRequest) {
    use crate::settings::UmuPreference;
    if request.umu_preference == UmuPreference::Umu {
        tracing::warn!(
            "umu preference requested but umu-run is not on PATH; falling back to direct Proton for this launch"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::resolved_umu_game_id_for_env;
    use crate::launch::LaunchRequest;

    #[test]
    fn resolved_umu_game_id_prefers_runtime_umu_game_id_over_steam_app_id() {
        let mut request = LaunchRequest::default();
        request.steam.app_id = "70".to_string();
        request.runtime.umu_game_id = "custom-7".to_string();
        assert_eq!(resolved_umu_game_id_for_env(&request), "custom-7");
    }

    #[test]
    fn resolved_umu_game_id_falls_back_to_umu_0_when_all_ids_empty() {
        let request = LaunchRequest::default();
        assert_eq!(resolved_umu_game_id_for_env(&request), "umu-0");
    }

    #[test]
    fn resolved_umu_game_id_uses_runtime_steam_app_id_when_steam_app_id_empty() {
        let mut request = LaunchRequest::default();
        request.runtime.steam_app_id = "999".to_string();
        assert_eq!(resolved_umu_game_id_for_env(&request), "999");
    }
}
