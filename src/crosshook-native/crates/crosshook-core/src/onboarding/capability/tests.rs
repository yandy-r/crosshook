//! Tests for capability derivation.

use super::derive::derive_capabilities_with_map;
use super::tool_check::synthesize_umu_run_check;
use super::types::{CapabilityDefinition, CapabilityMap, CapabilityState};
use crate::onboarding::{
    HostDistroFamily, HostToolCheckResult, HostToolEntry, HostToolInstallCommand,
    ReadinessCatalog, ReadinessCheckResult, UmuInstallGuidance,
};
use crate::profile::health::{HealthIssue, HealthIssueSeverity};

fn sample_catalog_entry(
    tool_id: &str,
    display_name: &str,
    category: &str,
    command: &str,
) -> HostToolEntry {
    HostToolEntry {
        tool_id: tool_id.to_string(),
        binary_name: tool_id.to_string(),
        display_name: display_name.to_string(),
        description: format!("{display_name} description"),
        docs_url: format!("https://example.invalid/{tool_id}"),
        required: false,
        category: category.to_string(),
        install_commands: vec![HostToolInstallCommand {
            distro_family: HostDistroFamily::Unknown.as_str().to_string(),
            command: command.to_string(),
            alternatives: format!("Install {display_name} another way"),
        }],
    }
}

fn sample_capability_map() -> CapabilityMap {
    CapabilityMap::from_entries(
        1,
        vec![
            CapabilityDefinition {
                id: "gamescope".to_string(),
                label: "Gamescope".to_string(),
                category: "performance".to_string(),
                required_tools: vec!["gamescope".to_string()],
                optional_tools: vec![],
            },
            CapabilityDefinition {
                id: "mangohud".to_string(),
                label: "MangoHud".to_string(),
                category: "overlay".to_string(),
                required_tools: vec!["mangohud".to_string()],
                optional_tools: vec![],
            },
            CapabilityDefinition {
                id: "gamemode".to_string(),
                label: "GameMode".to_string(),
                category: "performance".to_string(),
                required_tools: vec!["gamemode".to_string()],
                optional_tools: vec![],
            },
            CapabilityDefinition {
                id: "prefix_tools".to_string(),
                label: "Prefix tools".to_string(),
                category: "prefix_tools".to_string(),
                required_tools: vec![],
                optional_tools: vec!["winetricks".to_string(), "protontricks".to_string()],
            },
            CapabilityDefinition {
                id: "non_steam_launch".to_string(),
                label: "Non-Steam launch".to_string(),
                category: "runtime".to_string(),
                required_tools: vec!["umu_run".to_string()],
                optional_tools: vec![],
            },
        ],
    )
}

fn sample_readiness_catalog() -> ReadinessCatalog {
    ReadinessCatalog::from_entries(
        1,
        vec![
            sample_catalog_entry("gamescope", "Gamescope", "performance", "install gamescope"),
            sample_catalog_entry("mangohud", "MangoHud", "overlay", "install mangohud"),
            sample_catalog_entry("gamemode", "GameMode", "performance", "install gamemode"),
            sample_catalog_entry(
                "winetricks",
                "Winetricks",
                "prefix_tools",
                "install winetricks",
            ),
            sample_catalog_entry(
                "protontricks",
                "Protontricks",
                "prefix_tools",
                "install protontricks",
            ),
            sample_catalog_entry("umu_run", "umu-launcher", "runtime", "install umu"),
        ],
    )
}

fn issue(field: &str, severity: HealthIssueSeverity) -> HealthIssue {
    HealthIssue {
        field: field.to_string(),
        path: String::new(),
        message: field.to_string(),
        remediation: String::new(),
        severity,
    }
}

fn tool_check(
    tool_id: &str,
    display_name: &str,
    category: &str,
    is_available: bool,
) -> HostToolCheckResult {
    HostToolCheckResult {
        tool_id: tool_id.to_string(),
        display_name: display_name.to_string(),
        is_available,
        is_required: false,
        category: category.to_string(),
        docs_url: format!("https://example.invalid/{tool_id}"),
        tool_version: None,
        resolved_path: None,
        install_guidance: None,
    }
}

fn readiness_result(tool_checks: Vec<HostToolCheckResult>) -> ReadinessCheckResult {
    ReadinessCheckResult {
        checks: vec![issue("umu_run_available", HealthIssueSeverity::Info)],
        all_passed: true,
        critical_failures: 0,
        warnings: 0,
        umu_install_guidance: None,
        steam_deck_caveats: None,
        tool_checks,
        detected_distro_family: HostDistroFamily::Unknown.as_str().to_string(),
    }
}

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
    let _path_guard =
        crate::launch::test_support::ScopedCommandSearchPath::new(empty_dir.path());

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

/// Regression (F007): `synthesize_umu_run_check` must not put the
/// human-readable `description` string into the `alternatives` field.
/// The `alternatives` slot is for alternative install methods; when
/// `UmuInstallGuidance` supplies none, the field must be empty.
#[test]
fn synthesize_umu_run_check_alternatives_empty_when_guidance_has_no_alternatives() {
    let empty_dir = tempfile::tempdir().expect("tempdir");
    let _path_guard =
        crate::launch::test_support::ScopedCommandSearchPath::new(empty_dir.path());

    let result = ReadinessCheckResult {
        checks: vec![issue("umu_run_available", HealthIssueSeverity::Warning)],
        all_passed: false,
        critical_failures: 0,
        warnings: 1,
        umu_install_guidance: Some(UmuInstallGuidance {
            install_command: "sudo pacman -S umu-launcher".to_string(),
            docs_url: "https://example.invalid/umu".to_string(),
            description:
                "Install umu-launcher on your Arch-based host to enable Non-Steam launch."
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
    let _path_guard =
        crate::launch::test_support::ScopedCommandSearchPath::new(empty_dir.path());

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
