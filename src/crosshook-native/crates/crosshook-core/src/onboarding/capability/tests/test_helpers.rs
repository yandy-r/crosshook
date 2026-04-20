//! Shared test fixtures and helpers.

use crate::onboarding::{
    HostDistroFamily, HostToolCheckResult, HostToolEntry, HostToolInstallCommand, ReadinessCatalog,
    ReadinessCheckResult,
};
use crate::profile::health::{HealthIssue, HealthIssueSeverity};

use super::super::types::{CapabilityDefinition, CapabilityMap};

pub(super) fn sample_catalog_entry(
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

pub(super) fn sample_capability_map() -> CapabilityMap {
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

pub(super) fn sample_readiness_catalog() -> ReadinessCatalog {
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

pub(super) fn issue(field: &str, severity: HealthIssueSeverity) -> HealthIssue {
    HealthIssue {
        field: field.to_string(),
        path: String::new(),
        message: field.to_string(),
        remediation: String::new(),
        severity,
    }
}

pub(super) fn tool_check(
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

pub(super) fn readiness_result(tool_checks: Vec<HostToolCheckResult>) -> ReadinessCheckResult {
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
