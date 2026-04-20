//! Tests for capability derivation logic.

use super::super::derive::derive_capabilities_with_map;
use super::super::types::CapabilityState;
use super::test_helpers::*;
use crate::onboarding::{HostDistroFamily, HostToolCheckResult, ReadinessCheckResult};
use crate::profile::health::HealthIssueSeverity;

#[test]
fn derive_capabilities_all_available_fixture() {
    // Pin a path scope containing an umu-run stub so the live-probe
    // fallback in `synthesize_umu_run_check` deterministically returns Some,
    // independent of host state and concurrent tests that swap the scope.
    let umu_dir = tempfile::tempdir().expect("tempdir");
    let umu_stub = umu_dir.path().join("umu-run");
    std::fs::write(&umu_stub, "#!/bin/sh\nexit 0\n").expect("write umu stub");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o755))
        .expect("chmod umu stub");
    let _path_guard = crate::launch::test_support::ScopedCommandSearchPath::new(umu_dir.path());

    let result = readiness_result(vec![
        tool_check("gamescope", "Gamescope", "performance", true),
        tool_check("mangohud", "MangoHud", "overlay", true),
        tool_check("gamemode", "GameMode", "performance", true),
        tool_check("winetricks", "Winetricks", "prefix_tools", true),
        tool_check("protontricks", "Protontricks", "prefix_tools", true),
    ]);

    let capabilities = derive_capabilities_with_map(
        &result,
        &sample_capability_map(),
        &sample_readiness_catalog(),
    );

    assert_eq!(capabilities.len(), 5);
    assert!(capabilities
        .iter()
        .all(|capability| capability.state == CapabilityState::Available));
    assert!(capabilities
        .iter()
        .all(|capability| capability.rationale.is_none()));
}

#[test]
fn derive_capabilities_missing_required_fixture() {
    let mut result = readiness_result(vec![
        tool_check("gamescope", "Gamescope", "performance", true),
        tool_check("mangohud", "MangoHud", "overlay", true),
        tool_check("gamemode", "GameMode", "performance", true),
    ]);
    result.checks = vec![issue("umu_run_available", HealthIssueSeverity::Warning)];
    result.all_passed = false;
    result.warnings = 1;

    let capabilities = derive_capabilities_with_map(
        &result,
        &sample_capability_map(),
        &sample_readiness_catalog(),
    );
    let capability = capabilities
        .iter()
        .find(|capability| capability.id == "non_steam_launch")
        .expect("non_steam_launch");

    assert_eq!(capability.state, CapabilityState::Unavailable);
    assert_eq!(capability.missing_required.len(), 1);
    assert_eq!(capability.missing_required[0].tool_id, "umu_run");
    assert_eq!(
        capability.rationale.as_deref(),
        Some("Non-Steam launch is unavailable because umu-launcher is not available on the host.")
    );
    assert_eq!(capability.install_hints.len(), 1);
    assert_eq!(capability.install_hints[0].command, "install umu");
}

#[test]
fn derive_capabilities_missing_optional_fixture() {
    // Pin a path scope containing umu-run so concurrent tests that swap the
    // scope to an empty dir cannot make `synthesize_umu_run_check` flip the
    // `non_steam_launch` capability state under us. This test only asserts
    // on `prefix_tools`, but holding the lock keeps the fixture isolated.
    let umu_dir = tempfile::tempdir().expect("tempdir");
    let umu_stub = umu_dir.path().join("umu-run");
    std::fs::write(&umu_stub, "#!/bin/sh\nexit 0\n").expect("write umu stub");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o755))
        .expect("chmod umu stub");
    let _path_guard = crate::launch::test_support::ScopedCommandSearchPath::new(umu_dir.path());

    let result = readiness_result(vec![
        tool_check("gamescope", "Gamescope", "performance", true),
        tool_check("mangohud", "MangoHud", "overlay", true),
        tool_check("gamemode", "GameMode", "performance", true),
        tool_check("winetricks", "Winetricks", "prefix_tools", true),
        HostToolCheckResult {
            tool_id: "protontricks".to_string(),
            display_name: "Protontricks".to_string(),
            is_available: false,
            is_required: false,
            category: "prefix_tools".to_string(),
            docs_url: "https://example.invalid/protontricks".to_string(),
            tool_version: None,
            resolved_path: None,
            install_guidance: None,
        },
    ]);

    let capabilities = derive_capabilities_with_map(
        &result,
        &sample_capability_map(),
        &sample_readiness_catalog(),
    );
    let capability = capabilities
        .iter()
        .find(|capability| capability.id == "prefix_tools")
        .expect("prefix_tools");

    assert_eq!(capability.state, CapabilityState::Degraded);
    assert_eq!(capability.missing_optional.len(), 1);
    assert_eq!(capability.missing_optional[0].tool_id, "protontricks");
    assert_eq!(
        capability.rationale.as_deref(),
        Some("Prefix tools is degraded because optional tool Protontricks is not available on the host.")
    );
    assert_eq!(capability.install_hints.len(), 1);
    assert_eq!(capability.install_hints[0].command, "install protontricks");
}

#[test]
fn derive_capabilities_multiple_missing_optional_degraded_rationale_uses_plural_tool_word() {
    let umu_dir = tempfile::tempdir().expect("tempdir");
    let umu_stub = umu_dir.path().join("umu-run");
    std::fs::write(&umu_stub, "#!/bin/sh\nexit 0\n").expect("write umu stub");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o755))
        .expect("chmod umu stub");
    let _path_guard = crate::launch::test_support::ScopedCommandSearchPath::new(umu_dir.path());

    let result = readiness_result(vec![
        tool_check("gamescope", "Gamescope", "performance", true),
        tool_check("mangohud", "MangoHud", "overlay", true),
        tool_check("gamemode", "GameMode", "performance", true),
        HostToolCheckResult {
            tool_id: "winetricks".to_string(),
            display_name: "Winetricks".to_string(),
            is_available: false,
            is_required: false,
            category: "prefix_tools".to_string(),
            docs_url: "https://example.invalid/winetricks".to_string(),
            tool_version: None,
            resolved_path: None,
            install_guidance: None,
        },
        HostToolCheckResult {
            tool_id: "protontricks".to_string(),
            display_name: "Protontricks".to_string(),
            is_available: false,
            is_required: false,
            category: "prefix_tools".to_string(),
            docs_url: "https://example.invalid/protontricks".to_string(),
            tool_version: None,
            resolved_path: None,
            install_guidance: None,
        },
    ]);

    let capabilities = derive_capabilities_with_map(
        &result,
        &sample_capability_map(),
        &sample_readiness_catalog(),
    );
    let capability = capabilities
        .iter()
        .find(|capability| capability.id == "prefix_tools")
        .expect("prefix_tools");

    assert_eq!(capability.state, CapabilityState::Degraded);
    assert_eq!(capability.missing_optional.len(), 2);
    assert_eq!(
        capability.rationale.as_deref(),
        Some(
            "Prefix tools is degraded because optional tools Winetricks and Protontricks are not available on the host."
        )
    );
}

#[test]
fn derive_capabilities_empty_tool_checks_fixture() {
    // Pin the search PATH to an empty directory so the umu-run live-probe
    // fallback in `synthesize_umu_run_check` deterministically returns None,
    // independent of whether the dev/CI host has umu-run installed.
    let empty_dir = tempfile::tempdir().expect("tempdir");
    let _path_guard = crate::launch::test_support::ScopedCommandSearchPath::new(empty_dir.path());

    let result = ReadinessCheckResult {
        checks: Vec::new(),
        all_passed: false,
        critical_failures: 0,
        warnings: 0,
        umu_install_guidance: None,
        steam_deck_caveats: None,
        tool_checks: Vec::new(),
        detected_distro_family: HostDistroFamily::Unknown.as_str().to_string(),
    };

    let capabilities = derive_capabilities_with_map(
        &result,
        &sample_capability_map(),
        &sample_readiness_catalog(),
    );

    assert_eq!(
        capabilities
            .iter()
            .find(|capability| capability.id == "gamescope")
            .expect("gamescope")
            .state,
        CapabilityState::Unavailable
    );
    assert_eq!(
        capabilities
            .iter()
            .find(|capability| capability.id == "prefix_tools")
            .expect("prefix_tools")
            .state,
        CapabilityState::Degraded
    );
    assert_eq!(
        capabilities
            .iter()
            .find(|capability| capability.id == "non_steam_launch")
            .expect("non_steam_launch")
            .state,
        CapabilityState::Unavailable
    );
}

/// Regression: capabilities derived from a cached SQLite snapshot have an empty
/// `checks` Vec (HealthIssues are not persisted). `synthesize_umu_run_check`
/// must fall back to a live host probe in that case so umu-launcher is not
/// reported as missing when it is actually installed on the host PATH.
#[test]
fn derive_capabilities_from_cached_snapshot_detects_umu_via_live_probe() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let umu_stub = tmp.path().join("umu-run");
    std::fs::write(&umu_stub, "#!/bin/sh\nexit 0\n").expect("write umu stub");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o755))
        .expect("chmod umu stub");
    let _path_guard = crate::launch::test_support::ScopedCommandSearchPath::new(tmp.path());

    // Mirror what `readiness_result_from_snapshot` produces for the
    // get_capabilities cached path: tool_checks present, but checks empty
    // (HealthIssues are not persisted in `host_readiness_snapshots`).
    let snapshot_result = ReadinessCheckResult {
        checks: Vec::new(),
        all_passed: false,
        critical_failures: 0,
        warnings: 0,
        umu_install_guidance: None,
        steam_deck_caveats: None,
        tool_checks: vec![
            tool_check("gamescope", "Gamescope", "performance", true),
            tool_check("mangohud", "MangoHud", "overlay", true),
            tool_check("gamemode", "GameMode", "performance", true),
        ],
        detected_distro_family: HostDistroFamily::Unknown.as_str().to_string(),
    };

    let capabilities = derive_capabilities_with_map(
        &snapshot_result,
        &sample_capability_map(),
        &sample_readiness_catalog(),
    );
    let non_steam = capabilities
        .iter()
        .find(|capability| capability.id == "non_steam_launch")
        .expect("non_steam_launch");

    assert_eq!(
        non_steam.state,
        CapabilityState::Available,
        "umu-run on PATH must be detected via live probe when cached snapshot omits HealthIssues"
    );
    assert!(
        non_steam.missing_required.is_empty(),
        "non_steam_launch should have no missing required tools when umu-run is on PATH"
    );
}
