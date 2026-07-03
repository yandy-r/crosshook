use super::types::{GamescopeDecisionPreview, PreviewWrapperDetail, PreviewWrapperSource};
use crate::launch::optimizations::WrapperOrigin;
use crate::launch::runtime_helpers::build_gamescope_args;
use crate::profile::GamescopeConfig;

pub(super) fn build_gamescope_decision(
    config: &GamescopeConfig,
    gamescope_active: bool,
    inside_gamescope_session: bool,
    mangohud_folded_into_gamescope: bool,
) -> GamescopeDecisionPreview {
    let reason = if !config.enabled {
        "disabled in profile".to_string()
    } else if gamescope_active {
        "active".to_string()
    } else {
        "skipped: already inside a gamescope session and nested gamescope is not allowed"
            .to_string()
    };
    GamescopeDecisionPreview {
        enabled: config.enabled,
        allow_nested: config.allow_nested,
        inside_gamescope_session,
        active: gamescope_active,
        reason,
        mangohud_folded_into_gamescope,
    }
}

pub(super) struct WrapperDetailInputs<'a> {
    pub launch_trainer_only: bool,
    pub network_isolation: bool,
    pub unshare_available: bool,
    pub wrapper_origins: &'a [WrapperOrigin],
    pub gamescope_config: &'a GamescopeConfig,
    pub gamescope_decision: &'a GamescopeDecisionPreview,
}

pub(super) fn build_wrapper_details(inputs: &WrapperDetailInputs<'_>) -> Vec<PreviewWrapperDetail> {
    let mut details = Vec::new();

    // Active gamescope is the outermost process (`gamescope <args> -- <wrappers> <runtime>`),
    // so its row leads the chain. An inactive gamescope row trails instead, showing where it
    // would run if enabled.
    let mut gamescope_detail = if inputs.gamescope_config.enabled {
        let mut command = vec!["gamescope".to_string()];
        command.extend(build_gamescope_args(inputs.gamescope_config));
        command.push("--".to_string());
        Some(PreviewWrapperDetail {
            command,
            source: PreviewWrapperSource::Gamescope,
            active: inputs.gamescope_decision.active,
            reason: inputs.gamescope_decision.reason.clone(),
            optimization_id: None,
            optimization_label: None,
            folded_into: None,
        })
    } else {
        None
    };

    if gamescope_detail
        .as_ref()
        .is_some_and(|detail| detail.active)
    {
        details.extend(gamescope_detail.take());
    }

    if inputs.launch_trainer_only && inputs.network_isolation {
        details.push(PreviewWrapperDetail {
            command: vec!["unshare".to_string(), "--net".to_string()],
            source: PreviewWrapperSource::NetworkIsolation,
            active: inputs.unshare_available,
            reason: if inputs.unshare_available {
                "Trainer network isolation enabled in profile".to_string()
            } else {
                "unshare --net is not available on the host".to_string()
            },
            optimization_id: None,
            optimization_label: None,
            folded_into: None,
        });
    }

    for origin in inputs.wrapper_origins {
        let folded = origin.wrapper.trim() == "mangohud"
            && inputs.gamescope_decision.mangohud_folded_into_gamescope;
        details.push(PreviewWrapperDetail {
            command: vec![origin.wrapper.clone()],
            source: PreviewWrapperSource::Optimization,
            active: true,
            reason: format!("Enabled by launch optimization '{}'", origin.option_label),
            optimization_id: Some(origin.option_id.clone()),
            optimization_label: Some(origin.option_label.clone()),
            folded_into: folded.then(|| "gamescope --mangoapp".to_string()),
        });
    }

    details.extend(gamescope_detail);

    details
}
