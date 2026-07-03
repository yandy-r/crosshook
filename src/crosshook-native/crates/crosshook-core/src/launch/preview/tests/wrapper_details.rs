#![cfg(test)]

use super::super::details::{build_gamescope_decision, build_wrapper_details, WrapperDetailInputs};
use super::super::*;
use super::fixtures::*;
use crate::launch::optimizations::WrapperOrigin;
use crate::launch::test_support::ScopedCommandSearchPath;
use crate::profile::GamescopeConfig;

fn mangohud_origin() -> WrapperOrigin {
    WrapperOrigin {
        wrapper: "mangohud".to_string(),
        option_id: "show_mangohud_overlay".to_string(),
        option_label: "Show MangoHud overlay".to_string(),
    }
}

#[test]
fn optimization_wrapper_detail_carries_id_label_and_reason() {
    let (_td, mut request) = proton_request();
    let tool_dir = tempfile::tempdir().expect("tool dir");
    write_executable_file(&tool_dir.path().join("gamemoderun"));
    let _guard = ScopedCommandSearchPath::new(tool_dir.path());
    request.optimizations.enabled_option_ids = vec!["use_gamemode".to_string()];

    let preview = build_launch_preview(&request).expect("preview");
    let details = preview.wrapper_details.expect("wrapper details");

    assert_eq!(details.len(), 1);
    let detail = &details[0];
    assert_eq!(detail.command, vec!["gamemoderun".to_string()]);
    assert_eq!(detail.source, PreviewWrapperSource::Optimization);
    assert!(detail.active);
    assert_eq!(
        detail.reason,
        "Enabled by launch optimization 'Use GameMode'"
    );
    assert_eq!(detail.optimization_id.as_deref(), Some("use_gamemode"));
    assert_eq!(detail.optimization_label.as_deref(), Some("Use GameMode"));
    assert_eq!(detail.folded_into, None);
}

#[test]
fn gamescope_decision_reports_nested_session_skip() {
    let enabled = GamescopeConfig {
        enabled: true,
        ..GamescopeConfig::default()
    };

    let skipped = build_gamescope_decision(&enabled, false, true, false);
    assert!(skipped.enabled);
    assert!(!skipped.allow_nested);
    assert!(skipped.inside_gamescope_session);
    assert!(!skipped.active);
    assert_eq!(
        skipped.reason,
        "skipped: already inside a gamescope session and nested gamescope is not allowed"
    );

    let disabled = build_gamescope_decision(&GamescopeConfig::default(), false, false, false);
    assert!(!disabled.enabled);
    assert_eq!(disabled.reason, "disabled in profile");

    let active = build_gamescope_decision(&enabled, true, false, false);
    assert!(active.active);
    assert_eq!(active.reason, "active");
}

#[test]
fn unshare_detail_inactive_when_unavailable_and_active_when_available() {
    let gamescope_config = GamescopeConfig::default();
    let decision = build_gamescope_decision(&gamescope_config, false, false, false);

    let unavailable = build_wrapper_details(&WrapperDetailInputs {
        launch_trainer_only: true,
        network_isolation: true,
        unshare_available: false,
        wrapper_origins: &[],
        gamescope_config: &gamescope_config,
        gamescope_decision: &decision,
    });
    assert_eq!(unavailable.len(), 1);
    assert_eq!(
        unavailable[0].source,
        PreviewWrapperSource::NetworkIsolation
    );
    assert_eq!(
        unavailable[0].command,
        vec!["unshare".to_string(), "--net".to_string()]
    );
    assert!(!unavailable[0].active);
    assert_eq!(
        unavailable[0].reason,
        "unshare --net is not available on the host"
    );
    assert_eq!(unavailable[0].optimization_id, None);

    let available = build_wrapper_details(&WrapperDetailInputs {
        launch_trainer_only: true,
        network_isolation: true,
        unshare_available: true,
        wrapper_origins: &[],
        gamescope_config: &gamescope_config,
        gamescope_decision: &decision,
    });
    assert!(available[0].active);
    assert_eq!(
        available[0].reason,
        "Trainer network isolation enabled in profile"
    );

    let game_launch = build_wrapper_details(&WrapperDetailInputs {
        launch_trainer_only: false,
        network_isolation: true,
        unshare_available: true,
        wrapper_origins: &[],
        gamescope_config: &gamescope_config,
        gamescope_decision: &decision,
    });
    assert!(game_launch.is_empty());
}

#[test]
fn mangohud_fold_sets_folded_into_and_keeps_base_reason() {
    let gamescope_config = GamescopeConfig {
        enabled: true,
        allow_nested: true,
        ..GamescopeConfig::default()
    };
    let origins = vec![mangohud_origin()];

    let folded_decision = build_gamescope_decision(&gamescope_config, true, false, true);
    let folded = build_wrapper_details(&WrapperDetailInputs {
        launch_trainer_only: false,
        network_isolation: false,
        unshare_available: true,
        wrapper_origins: &origins,
        gamescope_config: &gamescope_config,
        gamescope_decision: &folded_decision,
    });
    assert_eq!(folded[0].source, PreviewWrapperSource::Gamescope);
    assert_eq!(folded[1].source, PreviewWrapperSource::Optimization);
    assert!(folded[1].active);
    assert_eq!(
        folded[1].reason,
        "Enabled by launch optimization 'Show MangoHud overlay'"
    );
    assert_eq!(
        folded[1].folded_into.as_deref(),
        Some("gamescope --mangoapp")
    );

    let plain_decision = build_gamescope_decision(&gamescope_config, true, false, false);
    let plain = build_wrapper_details(&WrapperDetailInputs {
        launch_trainer_only: false,
        network_isolation: false,
        unshare_available: true,
        wrapper_origins: &origins,
        gamescope_config: &gamescope_config,
        gamescope_decision: &plain_decision,
    });
    assert_eq!(plain[1].source, PreviewWrapperSource::Optimization);
    assert_eq!(
        plain[1].reason,
        "Enabled by launch optimization 'Show MangoHud overlay'"
    );
    assert_eq!(plain[1].folded_into, None);
}

#[test]
fn active_gamescope_leads_chain_and_inactive_gamescope_trails() {
    let gamescope_config = GamescopeConfig {
        enabled: true,
        ..GamescopeConfig::default()
    };
    let origins = vec![mangohud_origin()];

    let active_decision = build_gamescope_decision(&gamescope_config, true, false, false);
    let active = build_wrapper_details(&WrapperDetailInputs {
        launch_trainer_only: true,
        network_isolation: true,
        unshare_available: true,
        wrapper_origins: &origins,
        gamescope_config: &gamescope_config,
        gamescope_decision: &active_decision,
    });
    assert_eq!(
        active
            .iter()
            .map(|detail| detail.source)
            .collect::<Vec<_>>(),
        vec![
            PreviewWrapperSource::Gamescope,
            PreviewWrapperSource::NetworkIsolation,
            PreviewWrapperSource::Optimization,
        ]
    );

    let skipped_decision = build_gamescope_decision(&gamescope_config, false, true, false);
    let skipped = build_wrapper_details(&WrapperDetailInputs {
        launch_trainer_only: true,
        network_isolation: true,
        unshare_available: true,
        wrapper_origins: &origins,
        gamescope_config: &gamescope_config,
        gamescope_decision: &skipped_decision,
    });
    assert_eq!(
        skipped
            .iter()
            .map(|detail| detail.source)
            .collect::<Vec<_>>(),
        vec![
            PreviewWrapperSource::NetworkIsolation,
            PreviewWrapperSource::Optimization,
            PreviewWrapperSource::Gamescope,
        ]
    );
    assert!(!skipped[2].active);
}

#[test]
fn preview_gamescope_fold_marks_decision_and_leads_with_gamescope_detail() {
    let (_td, mut request) = proton_request();
    let tool_dir = tempfile::tempdir().expect("tool dir");
    write_executable_file(&tool_dir.path().join("mangohud"));
    let _guard = ScopedCommandSearchPath::new(tool_dir.path());
    request.optimizations.enabled_option_ids = vec!["show_mangohud_overlay".to_string()];
    request.gamescope.enabled = true;
    request.gamescope.allow_nested = true;

    let preview = build_launch_preview(&request).expect("preview");

    assert!(preview.gamescope_active);
    assert!(preview.gamescope_decision.mangohud_folded_into_gamescope);
    let details = preview.wrapper_details.expect("wrapper details");

    let mangohud_detail = details
        .iter()
        .find(|detail| detail.source == PreviewWrapperSource::Optimization)
        .expect("mangohud detail");
    assert_eq!(
        mangohud_detail.reason,
        "Enabled by launch optimization 'Show MangoHud overlay'"
    );
    assert_eq!(
        mangohud_detail.folded_into.as_deref(),
        Some("gamescope --mangoapp")
    );

    let gamescope_detail = details
        .first()
        .filter(|detail| detail.source == PreviewWrapperSource::Gamescope)
        .expect("active gamescope detail leads the chain");
    assert_eq!(
        gamescope_detail.command.first().map(String::as_str),
        Some("gamescope")
    );
    assert_eq!(
        gamescope_detail.command.last().map(String::as_str),
        Some("--")
    );
    assert!(gamescope_detail.active);
    assert_eq!(gamescope_detail.reason, "active");
}

#[test]
fn directive_failure_yields_none_details_but_gamescope_decision_present() {
    let (_td, mut request) = proton_request();
    let tool_dir = tempfile::tempdir().expect("tool dir");
    let _guard = ScopedCommandSearchPath::new(tool_dir.path());
    request.optimizations.enabled_option_ids = vec!["show_mangohud_overlay".to_string()];

    let preview = build_launch_preview(&request).expect("preview");

    assert!(preview.wrapper_details.is_none());
    assert!(preview.wrappers.is_none());
    assert!(preview.directives_error.is_some());
    assert!(!preview.gamescope_decision.enabled);
    assert_eq!(preview.gamescope_decision.reason, "disabled in profile");
}

#[test]
fn native_preview_has_empty_wrapper_details() {
    let (_td, request) = native_request();

    let preview = build_launch_preview(&request).expect("preview");

    assert_eq!(preview.wrapper_details, Some(Vec::new()));
    assert!(!preview.gamescope_decision.enabled);
    assert_eq!(preview.gamescope_decision.reason, "disabled in profile");
}
