#![cfg(test)]

use super::super::*;
use super::fixtures::*;
use crate::profile::TrainerLoadingMode;
use serde_json::json;

#[test]
fn preview_shows_resolved_method_for_steam_applaunch() {
    let (_td, request) = steam_request();
    let preview = build_launch_preview(&request).expect("preview");
    assert_eq!(
        preview.resolved_method,
        ResolvedLaunchMethod::SteamApplaunch
    );
}

#[test]
fn preview_shows_resolved_method_for_proton_run() {
    let (_td, request) = proton_request();
    let preview = build_launch_preview(&request).expect("preview");
    assert_eq!(preview.resolved_method, ResolvedLaunchMethod::ProtonRun);
}

#[test]
fn preview_shows_resolved_method_for_native() {
    let (_td, request) = native_request();
    let preview = build_launch_preview(&request).expect("preview");
    assert_eq!(preview.resolved_method, ResolvedLaunchMethod::Native);
}

#[test]
fn preview_validation_passes_for_valid_request() {
    let (_td, request) = steam_request();
    let preview = build_launch_preview(&request).expect("preview");
    assert!(
        preview.validation.issues.is_empty(),
        "expected validation to pass, issues: {:?}",
        preview.validation.issues
    );
    assert!(
        preview.validation.issues.is_empty(),
        "expected no issues, got: {:?}",
        preview.validation.issues
    );
}

#[test]
fn preview_validation_collects_multiple_issues() {
    let (_td, mut request) = steam_request();
    request.game_path.clear();
    request.steam.app_id.clear();
    request.steam.compatdata_path.clear();
    request.steam.proton_path.clear();

    let preview = build_launch_preview(&request).expect("preview");
    assert!(
        !preview.validation.issues.is_empty(),
        "expected validation to fail"
    );
    assert!(
        preview.validation.issues.len() >= 4,
        "expected at least 4 issues, got {}: {:?}",
        preview.validation.issues.len(),
        preview.validation.issues
    );
}

#[test]
fn preview_returns_partial_results_on_directive_failure() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let _scoped_path = crate::launch::test_support::ScopedCommandSearchPath::new(temp_dir.path());

    let (_td, mut request) = proton_request();
    // Enable an optimization that requires a missing wrapper binary.
    request.optimizations.enabled_option_ids = vec!["show_mangohud_overlay".to_string()];

    let preview = build_launch_preview(&request).expect("preview");

    // Directives failed — error is captured, env/command are None.
    assert!(
        preview.directives_error.is_some(),
        "expected directives_error to be Some"
    );
    assert!(
        preview.environment.is_none(),
        "expected environment to be None when directives fail"
    );
    assert!(
        preview.effective_command.is_none(),
        "expected effective_command to be None when directives fail"
    );

    // Validation and game info should still be populated.
    assert!(!preview.game_executable.is_empty());
    // The validation should have collected the directive error as an issue too.
    assert!(
        !preview.validation.issues.is_empty(),
        "expected validation issues for missing wrapper"
    );
}

#[test]
fn preview_trainer_info_with_copy_to_prefix() {
    let (_td, mut request) = proton_request();
    request.trainer_loading_mode = TrainerLoadingMode::CopyToPrefix;
    request.trainer_host_path = "/home/user/trainers/MyTrainer.exe".to_string();

    let preview = build_launch_preview(&request).expect("preview");
    let trainer = preview.trainer.expect("trainer info should be present");

    assert_eq!(trainer.loading_mode, TrainerLoadingMode::CopyToPrefix);
    assert_eq!(
        trainer.staged_path,
        Some("C:\\CrossHook\\StagedTrainers\\MyTrainer\\MyTrainer.exe".to_string())
    );
}

#[test]
fn preview_hides_proton_for_native() {
    let (_td, request) = native_request();
    let preview = build_launch_preview(&request).expect("preview");
    assert!(
        preview.proton_setup.is_none(),
        "expected proton_setup to be None for native, got: {:?}",
        preview.proton_setup
    );
}

#[test]
fn preview_hides_working_directory_for_steam_applaunch() {
    let (_td, request) = steam_request();
    let preview = build_launch_preview(&request).expect("preview");

    assert!(
        preview.working_directory.is_empty(),
        "expected no working directory for steam_applaunch preview, got: {:?}",
        preview.working_directory
    );
}

#[test]
fn preview_generated_at_is_recent() {
    let (_td, request) = steam_request();
    let preview = build_launch_preview(&request).expect("preview");

    let parsed = chrono::DateTime::parse_from_rfc3339(&preview.generated_at);
    assert!(
        parsed.is_ok(),
        "generated_at '{}' should parse as ISO 8601 / RFC 3339",
        preview.generated_at
    );

    let generated = parsed.unwrap();
    let now = chrono::Utc::now();
    let age = now.signed_duration_since(generated);
    assert!(
        age.num_seconds() < 10,
        "generated_at should be within 10 seconds of now, got age: {}s",
        age.num_seconds()
    );
}

#[test]
fn preview_serializes_typed_fields_as_snake_case_strings() {
    let (_td, mut request) = proton_request();
    request.trainer_loading_mode = TrainerLoadingMode::CopyToPrefix;

    let preview = build_launch_preview(&request).expect("preview");
    let value = serde_json::to_value(&preview).expect("serialize preview");

    assert_eq!(value["resolved_method"], json!("proton_run"));
    assert_eq!(value["trainer"]["loading_mode"], json!("copy_to_prefix"));
}
