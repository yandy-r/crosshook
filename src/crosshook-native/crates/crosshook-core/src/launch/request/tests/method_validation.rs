use std::fs;

use crate::launch::request::{validate, validate_all, ValidationError, METHOD_NATIVE};

use super::support::{native_request, proton_request, steam_request};

#[test]
fn validates_steam_applaunch_request() {
    let (_temp_dir, request) = steam_request();
    assert_eq!(validate(&request), Ok(()));
}

#[test]
fn validates_steam_applaunch_request_with_flatpak_host_mounted_paths() {
    let (_temp_dir, mut request) = steam_request();
    request.trainer_host_path = format!("/run/host{}", request.trainer_host_path);
    request.steam.compatdata_path = format!("/run/host{}", request.steam.compatdata_path);
    request.steam.proton_path = format!("/run/host{}", request.steam.proton_path);

    assert_eq!(validate(&request), Ok(()));
}

#[test]
fn steam_applaunch_rejects_unknown_launch_optimization() {
    let (_temp_dir, mut request) = steam_request();
    request.optimizations.enabled_option_ids = vec!["unknown_toggle".to_string()];

    assert_eq!(
        validate(&request),
        Err(ValidationError::UnknownLaunchOptimization(
            "unknown_toggle".to_string()
        ))
    );
}

#[test]
fn steam_applaunch_validate_all_collects_launch_optimization_issue() {
    let (_temp_dir, mut request) = steam_request();
    request.optimizations.enabled_option_ids = vec!["unknown_toggle".to_string()];

    let issues = validate_all(&request);
    assert!(
        issues
            .iter()
            .any(|issue| issue.message.contains("unknown_toggle")),
        "expected optimization issue in: {issues:?}"
    );
}

#[test]
fn allows_game_only_steam_launch_without_trainer_paths() {
    let (_temp_dir, mut request) = steam_request();
    request.launch_game_only = true;
    request.trainer_path.clear();
    request.trainer_host_path.clear();

    assert_eq!(validate(&request), Ok(()));
}

#[test]
fn validates_proton_run_request() {
    let (_temp_dir, request) = proton_request();
    assert_eq!(validate(&request), Ok(()));
}

#[test]
fn validates_proton_run_request_with_flatpak_host_mounted_paths() {
    let (_temp_dir, mut request) = proton_request();
    request.game_path = format!("/run/host{}", request.game_path);
    request.trainer_host_path = format!("/run/host{}", request.trainer_host_path);
    request.runtime.prefix_path = format!("/run/host{}", request.runtime.prefix_path);
    request.runtime.proton_path = format!("/run/host{}", request.runtime.proton_path);

    assert_eq!(validate(&request), Ok(()));
}

#[test]
fn proton_run_rejects_unknown_launch_optimization() {
    let (_temp_dir, mut request) = proton_request();
    request.optimizations.enabled_option_ids = vec!["unknown_toggle".to_string()];

    assert_eq!(
        validate(&request),
        Err(ValidationError::UnknownLaunchOptimization(
            "unknown_toggle".to_string()
        ))
    );
}

#[test]
fn proton_run_rejects_duplicate_launch_optimizations() {
    let (_temp_dir, mut request) = proton_request();
    request.optimizations.enabled_option_ids = vec![
        "disable_steam_input".to_string(),
        "disable_steam_input".to_string(),
    ];

    assert_eq!(
        validate(&request),
        Err(ValidationError::DuplicateLaunchOptimization(
            "disable_steam_input".to_string()
        ))
    );
}

#[test]
fn proton_run_rejects_conflicting_launch_optimizations() {
    let (_temp_dir, mut request) = proton_request();
    request.optimizations.enabled_option_ids = vec![
        "use_gamemode".to_string(),
        "use_game_performance".to_string(),
    ];

    assert_eq!(
        validate(&request),
        Err(ValidationError::IncompatibleLaunchOptimizations {
            first: "use_gamemode".to_string(),
            second: "use_game_performance".to_string(),
        })
    );
}

#[test]
fn proton_run_requires_runtime_prefix_path() {
    let (_temp_dir, mut request) = proton_request();
    request.runtime.prefix_path.clear();

    assert_eq!(
        validate(&request),
        Err(ValidationError::RuntimePrefixPathRequired)
    );
}

#[test]
fn native_requires_linux_native_executable() {
    let (_temp_dir, mut request) = native_request();
    request.game_path = request.game_path.replace("game.sh", "game.exe");
    fs::write(&request.game_path, b"game").expect("game exe");

    assert_eq!(
        validate(&request),
        Err(ValidationError::NativeWindowsExecutableNotSupported)
    );
}

#[test]
fn native_rejects_trainer_only_launches() {
    let (_temp_dir, mut request) = native_request();
    request.launch_trainer_only = true;

    assert_eq!(
        validate(&request),
        Err(ValidationError::NativeTrainerLaunchUnsupported)
    );
}

#[test]
fn native_rejects_launch_optimizations() {
    let (_temp_dir, mut request) = native_request();
    request.optimizations.enabled_option_ids = vec!["disable_steam_input".to_string()];

    assert_eq!(
        validate(&request),
        Err(ValidationError::LaunchOptimizationsUnsupportedForMethod(
            METHOD_NATIVE.to_string()
        ))
    );
}

#[test]
fn rejects_unsupported_method() {
    let (_temp_dir, mut request) = steam_request();
    request.method = "direct".to_string();

    assert_eq!(
        validate(&request),
        Err(ValidationError::UnsupportedMethod("direct".to_string()))
    );
}

#[test]
fn validate_all_returns_empty_for_valid_steam_request() {
    let (_temp_dir, request) = steam_request();
    let issues = validate_all(&request);
    assert!(issues.is_empty(), "expected no issues, got: {issues:?}");
}

#[test]
fn validate_all_collects_multiple_issues() {
    let (_temp_dir, mut request) = steam_request();
    request.game_path.clear();
    request.steam.app_id.clear();
    request.steam.compatdata_path.clear();
    request.steam.proton_path.clear();
    request.steam.steam_client_install_path.clear();

    let issues = validate_all(&request);
    assert!(
        issues.len() >= 4,
        "expected at least 4 issues, got {}: {issues:?}",
        issues.len()
    );

    let messages: Vec<&str> = issues.iter().map(|i| i.message.as_str()).collect();
    assert!(messages.iter().any(|m| m.contains("game executable path")));
    assert!(messages.iter().any(|m| m.contains("Steam App ID")));
    assert!(messages.iter().any(|m| m.contains("compatdata path")));
    assert!(messages.iter().any(|m| m.contains("Proton path")));
}

#[test]
fn validate_all_proton_collects_directive_error_alongside_path_issues() {
    let (_temp_dir, mut request) = proton_request();
    request.runtime.prefix_path.clear();
    request.optimizations.enabled_option_ids = vec!["unknown_toggle".to_string()];

    let issues = validate_all(&request);
    assert!(
        issues.len() >= 2,
        "expected at least 2 issues, got {}: {issues:?}",
        issues.len()
    );

    let messages: Vec<&str> = issues.iter().map(|i| i.message.as_str()).collect();
    assert!(
        messages.iter().any(|m| m.contains("prefix path")),
        "expected prefix path issue in: {messages:?}"
    );
    assert!(
        messages.iter().any(|m| m.contains("unknown_toggle")),
        "expected directive error issue in: {messages:?}"
    );
}
