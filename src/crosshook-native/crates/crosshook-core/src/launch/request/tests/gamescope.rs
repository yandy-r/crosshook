use crate::launch::request::validate_all;

use super::support::{native_request, proton_request, steam_request};

#[test]
fn gamescope_validation_passes_for_steam_applaunch() {
    let (_td, mut request) = steam_request();
    request.gamescope = crate::profile::GamescopeConfig {
        enabled: true,
        internal_width: Some(1920),
        internal_height: Some(1080),
        ..Default::default()
    };
    let issues = validate_all(&request);
    let gamescope_method_issue = issues
        .iter()
        .any(|issue| issue.code.as_deref() == Some("gamescope_not_supported_for_method"));
    assert!(
        !gamescope_method_issue,
        "steam_applaunch should not emit GamescopeNotSupportedForMethod"
    );
}

#[test]
fn gamescope_validation_rejected_for_native() {
    let (_td, mut request) = native_request();
    request.gamescope = crate::profile::GamescopeConfig {
        enabled: true,
        ..Default::default()
    };
    let issues = validate_all(&request);
    let gamescope_method_issue = issues
        .iter()
        .any(|issue| issue.code.as_deref() == Some("gamescope_not_supported_for_method"));
    assert!(
        gamescope_method_issue,
        "native method should emit GamescopeNotSupportedForMethod"
    );
}

#[test]
fn trainer_only_validation_uses_trainer_gamescope_before_main_gamescope() {
    let (_td, mut request) = proton_request();
    request.launch_trainer_only = true;
    request.launch_game_only = false;
    request.gamescope = crate::profile::GamescopeConfig::default();
    request.trainer_gamescope = Some(crate::profile::GamescopeConfig {
        enabled: true,
        internal_width: Some(1920),
        internal_height: None,
        ..Default::default()
    });

    let issues = validate_all(&request);
    assert!(
        issues
            .iter()
            .any(|issue| { issue.code.as_deref() == Some("gamescope_resolution_pair_incomplete") }),
        "expected trainer gamescope validation issue in: {issues:?}"
    );
}
