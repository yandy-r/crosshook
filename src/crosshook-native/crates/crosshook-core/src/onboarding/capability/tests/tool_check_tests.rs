//! Tests for tool checking and synthesis functions.

use super::super::tool_check::synthesize_umu_run_check;
use super::test_helpers::*;
use crate::onboarding::{HostDistroFamily, HostToolCheckResult, ReadinessCheckResult, UmuInstallGuidance};
use crate::profile::health::HealthIssueSeverity;

/// Regression (F007): `synthesize_umu_run_check` must not put the
/// human-readable `description` string into the `alternatives` field.
/// The `alternatives` slot is for alternative install methods; when
/// `UmuInstallGuidance` supplies none, the field must be empty.
#[test]
fn synthesize_umu_run_check_alternatives_empty_when_guidance_has_no_alternatives() {
    let empty_dir = tempfile::tempdir().expect("tempdir");
    let _path_guard = crate::launch::test_support::ScopedCommandSearchPath::new(empty_dir.path());

    let result = ReadinessCheckResult {
        checks: vec![issue("umu_run_available", HealthIssueSeverity::Warning)],
        all_passed: false,
        critical_failures: 0,
        warnings: 1,
        umu_install_guidance: Some(UmuInstallGuidance {
            install_command: "sudo pacman -S umu-launcher".to_string(),
            docs_url: "https://example.invalid/umu".to_string(),
            description: "Install umu-launcher on your Arch-based host to enable Non-Steam launch."
                .to_string(),
        }),
        steam_deck_caveats: None,
        tool_checks: Vec::new(),
        detected_distro_family: HostDistroFamily::Arch.as_str().to_string(),
    };

    let catalog = sample_readiness_catalog();
    let check = synthesize_umu_run_check(&result, &catalog, true);
    assert!(!check.is_available);
    let guidance = check
        .install_guidance
        .expect("install_guidance must be present");
    assert_eq!(guidance.command, "sudo pacman -S umu-launcher");
    assert!(
        guidance.alternatives.is_empty(),
        "alternatives must not contain the description; got: {:?}",
        guidance.alternatives
    );
}

/// Regression (F006): when the `checks` Vec is empty (cached SQLite snapshot)
/// and `tool_checks` already has an `umu_run` entry with a non-empty
/// `resolved_path`, the live host probe (`resolve_umu_run_path`) must be
/// skipped.  We verify this by pointing PATH at an empty directory — if the
/// live probe ran it would return `None` and `is_available` would be `false`.
#[test]
fn synthesize_umu_run_check_uses_cached_resolved_path_skips_live_probe() {
    let empty_dir = tempfile::tempdir().expect("tempdir");
    let _path_guard = crate::launch::test_support::ScopedCommandSearchPath::new(empty_dir.path());

    let cached_path = "/cached/host/bin/umu-run".to_string();
    let result = ReadinessCheckResult {
        checks: Vec::new(),
        all_passed: true,
        critical_failures: 0,
        warnings: 0,
        umu_install_guidance: None,
        steam_deck_caveats: None,
        tool_checks: vec![HostToolCheckResult {
            tool_id: "umu_run".to_string(),
            display_name: "umu-launcher".to_string(),
            is_available: true,
            is_required: false,
            category: "runtime".to_string(),
            docs_url: String::new(),
            tool_version: None,
            resolved_path: Some(cached_path.clone()),
            install_guidance: None,
        }],
        detected_distro_family: HostDistroFamily::Unknown.as_str().to_string(),
    };

    let catalog = sample_readiness_catalog();
    let check = synthesize_umu_run_check(&result, &catalog, false);
    assert!(
        check.is_available,
        "cached resolved_path must signal available without a live probe"
    );
    assert_eq!(
        check.resolved_path.as_deref(),
        Some("/cached/host/bin/umu-run")
    );
}
