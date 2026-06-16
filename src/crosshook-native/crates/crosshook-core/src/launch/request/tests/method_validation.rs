use std::fs;

use crate::launch::request::models::LaunchCommandArgumentsRequest;
use crate::launch::request::{validate, validate_all, ValidationError, METHOD_NATIVE};

use super::support::{native_request, proton_request, steam_request};

fn with_command_arguments(
    mut request: crate::launch::request::LaunchRequest,
    enabled_argument_ids: Vec<String>,
    custom_args: Vec<String>,
) -> crate::launch::request::LaunchRequest {
    request.command_arguments = LaunchCommandArgumentsRequest {
        enabled_argument_ids,
        custom_args,
    };
    request
}

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
            .any(|issue| issue.code.as_deref() == Some("unknown_launch_optimization")),
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
    assert!(issues
        .iter()
        .any(|issue| issue.code.as_deref() == Some("game_path_required")));
    assert!(issues
        .iter()
        .any(|issue| issue.code.as_deref() == Some("steam_app_id_required")));
    assert!(issues
        .iter()
        .any(|issue| { issue.code.as_deref() == Some("steam_compat_data_path_required") }));
    assert!(issues
        .iter()
        .any(|issue| { issue.code.as_deref() == Some("steam_proton_path_required") }));
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
    assert!(
        issues
            .iter()
            .any(|issue| issue.code.as_deref() == Some("runtime_prefix_path_required")),
        "expected prefix path issue in: {issues:?}"
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.code.as_deref() == Some("unknown_launch_optimization")),
        "expected directive error issue in: {issues:?}"
    );
}

#[test]
fn proton_run_accepts_valid_command_arguments() {
    let (_temp_dir, request) = proton_request();
    let request = with_command_arguments(
        request,
        vec!["force_vulkan".to_string()],
        vec![
            "--flag=value".to_string(),
            "+set".to_string(),
            "-dx11".to_string(),
            "/path/with spaces/game.exe".to_string(),
        ],
    );

    assert_eq!(validate(&request), Ok(()));
    assert!(validate_all(&request).is_empty());
}

#[test]
fn steam_applaunch_accepts_valid_command_arguments() {
    let (_temp_dir, request) = steam_request();
    let request = with_command_arguments(
        request,
        vec!["skip_launcher".to_string()],
        vec!["-windowed".to_string()],
    );

    assert_eq!(validate(&request), Ok(()));
}

#[test]
fn proton_run_rejects_unknown_command_argument() {
    let (_temp_dir, request) = proton_request();
    let request = with_command_arguments(request, vec!["not_a_real_argument".to_string()], vec![]);

    assert_eq!(
        validate(&request),
        Err(ValidationError::UnknownCommandArgument(
            "not_a_real_argument".to_string()
        ))
    );
}

#[test]
fn proton_run_rejects_duplicate_command_arguments() {
    let (_temp_dir, request) = proton_request();
    let request = with_command_arguments(
        request,
        vec!["force_vulkan".to_string(), "force_vulkan".to_string()],
        vec![],
    );

    assert_eq!(
        validate(&request),
        Err(ValidationError::DuplicateCommandArgument(
            "force_vulkan".to_string()
        ))
    );
}

#[test]
fn proton_run_rejects_incompatible_command_arguments() {
    let (_temp_dir, request) = proton_request();
    let request = with_command_arguments(
        request,
        vec!["force_vulkan".to_string(), "force_dx11".to_string()],
        vec![],
    );

    assert_eq!(
        validate(&request),
        Err(ValidationError::IncompatibleCommandArguments {
            first: "force_vulkan".to_string(),
            second: "force_dx11".to_string(),
        })
    );
}

#[test]
fn native_rejects_nonempty_command_arguments() {
    let (_temp_dir, request) = native_request();
    let request = with_command_arguments(request, vec![], vec!["-windowed".to_string()]);

    assert_eq!(
        validate(&request),
        Err(ValidationError::CommandArgumentsUnsupportedForMethod(
            METHOD_NATIVE.to_string()
        ))
    );
}

#[test]
fn native_rejects_curated_command_arguments() {
    let (_temp_dir, request) = native_request();
    let request = with_command_arguments(request, vec!["force_vulkan".to_string()], vec![]);

    assert_eq!(
        validate(&request),
        Err(ValidationError::CommandArgumentsUnsupportedForMethod(
            METHOD_NATIVE.to_string()
        ))
    );
}

#[test]
fn proton_run_rejects_empty_custom_command_argument_token() {
    let (_temp_dir, request) = proton_request();
    let request = with_command_arguments(request, vec![], vec!["   ".to_string()]);

    assert_eq!(
        validate(&request),
        Err(ValidationError::CommandArgumentCustomTokenEmpty)
    );
}

#[test]
fn proton_run_rejects_control_characters_in_custom_command_argument() {
    let (_temp_dir, request) = proton_request();
    let request = with_command_arguments(request, vec![], vec!["bad\x07arg".to_string()]);

    assert_eq!(
        validate(&request),
        Err(ValidationError::CommandArgumentCustomTokenContainsControlCharacter)
    );
}

#[test]
fn proton_run_rejects_nul_in_custom_command_argument() {
    let (_temp_dir, request) = proton_request();
    let request = with_command_arguments(request, vec![], vec!["bad\x00arg".to_string()]);

    assert_eq!(
        validate(&request),
        Err(ValidationError::CommandArgumentCustomTokenContainsControlCharacter)
    );
}

#[test]
fn proton_run_rejects_excessive_custom_command_argument_length() {
    let (_temp_dir, request) = proton_request();
    let request = with_command_arguments(request, vec![], vec!["a".repeat(513)]);

    assert_eq!(
        validate(&request),
        Err(ValidationError::CommandArgumentTokenTooLong { max_len: 512 })
    );
}

#[test]
fn proton_run_rejects_excessive_command_argument_token_count() {
    let (_temp_dir, request) = proton_request();
    let custom_args = (0..65).map(|index| format!("-arg{index}")).collect();

    let request = with_command_arguments(request, vec![], custom_args);

    assert_eq!(
        validate(&request),
        Err(ValidationError::CommandArgumentTokenCountExceeded { max_count: 64 })
    );
}

#[test]
fn validate_all_collects_command_argument_issues() {
    let (_temp_dir, request) = proton_request();
    let request = with_command_arguments(
        request,
        vec!["unknown_argument".to_string()],
        vec!["   ".to_string(), "bad\x00arg".to_string()],
    );

    let issues = validate_all(&request);
    assert!(
        issues
            .iter()
            .any(|issue| issue.code.as_deref() == Some("unknown_command_argument")),
        "expected unknown command argument issue in: {issues:?}"
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.code.as_deref() == Some("command_argument_custom_token_empty")),
        "expected empty custom token issue in: {issues:?}"
    );
    assert!(
        issues.iter().any(|issue| {
            issue.code.as_deref()
                == Some("command_argument_custom_token_contains_control_character")
        }),
        "expected control character issue in: {issues:?}"
    );
}
