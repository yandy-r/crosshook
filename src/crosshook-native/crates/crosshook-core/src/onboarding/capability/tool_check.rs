//! Tool checking and synthesis functions.

use crate::launch::runtime_helpers::resolve_umu_run_path;
use crate::onboarding::{
    HostDistroFamily, HostToolCheckResult, HostToolInstallCommand, ReadinessCatalog,
    ReadinessCheckResult,
};
use crate::profile::health::HealthIssueSeverity;

pub(super) fn resolve_tool_check(
    result: &ReadinessCheckResult,
    readiness_catalog: &ReadinessCatalog,
    tool_id: &str,
    capability_requires_tool: bool,
) -> HostToolCheckResult {
    if let Some(existing) = result
        .tool_checks
        .iter()
        .find(|check| check.tool_id == tool_id)
    {
        let mut enriched = existing.clone();
        enriched.is_required = capability_requires_tool;
        if !enriched.is_available && enriched.install_guidance.is_none() {
            enriched.install_guidance = install_hint_for_tool(result, readiness_catalog, tool_id);
        }
        return enriched;
    }

    if tool_id == "umu_run" {
        return synthesize_umu_run_check(result, readiness_catalog, capability_requires_tool);
    }

    let catalog_entry = readiness_catalog.find_by_id(tool_id);
    HostToolCheckResult {
        tool_id: tool_id.to_string(),
        display_name: catalog_entry
            .map(|entry| entry.display_name.clone())
            .unwrap_or_else(|| tool_id.to_string()),
        is_available: false,
        is_required: capability_requires_tool,
        category: catalog_entry
            .map(|entry| entry.category.clone())
            .unwrap_or_default(),
        docs_url: catalog_entry
            .map(|entry| entry.docs_url.clone())
            .unwrap_or_default(),
        tool_version: None,
        resolved_path: None,
        install_guidance: install_hint_for_tool(result, readiness_catalog, tool_id),
    }
}

pub(super) fn synthesize_umu_run_check(
    result: &ReadinessCheckResult,
    readiness_catalog: &ReadinessCatalog,
    capability_requires_tool: bool,
) -> HostToolCheckResult {
    let catalog_entry = readiness_catalog.find_by_id("umu_run");

    // Prefer the HealthIssue emitted by a fresh `check_system_readiness` run.
    // When capabilities are derived from a cached SQLite snapshot the
    // ReadinessCheckResult is rebuilt without HealthIssues, so the lookup
    // returns None — fall back to a live host probe so we never report umu-run
    // as missing when it is actually installed. `resolve_umu_run_path` already
    // handles both native PATH walks and Flatpak host probing.
    let probed_umu_path = match result
        .checks
        .iter()
        .find(|issue| issue.field == "umu_run_available")
    {
        Some(issue) if matches!(issue.severity, HealthIssueSeverity::Info) => {
            // Available per HealthIssue; reuse the path it recorded if any.
            let recorded = issue.path.trim();
            if recorded.is_empty() {
                resolve_umu_run_path()
            } else {
                Some(recorded.to_string())
            }
        }
        Some(_) => None, // Explicit Warning/Error severity → not available.
        None => {
            // When capabilities are derived from a cached SQLite snapshot the
            // `checks` Vec is empty.  Before issuing a live host probe, check
            // whether the cached snapshot already recorded a resolved path for
            // umu_run — if so, trust the cache and skip the I/O-heavy probe.
            result
                .tool_checks
                .iter()
                .find(|c| c.tool_id == "umu_run")
                .and_then(|c| c.resolved_path.as_deref())
                .filter(|p| !p.trim().is_empty())
                .map(std::string::ToString::to_string)
                .or_else(resolve_umu_run_path)
        }
    };

    let available = probed_umu_path.is_some();

    let install_guidance = if available {
        None
    } else if let Some(guidance) = &result.umu_install_guidance {
        Some(HostToolInstallCommand {
            distro_family: if result.detected_distro_family.trim().is_empty() {
                HostDistroFamily::Unknown.as_str().to_string()
            } else {
                result.detected_distro_family.clone()
            },
            command: guidance.install_command.clone(),
            alternatives: String::new(),
        })
    } else {
        install_hint_for_tool(result, readiness_catalog, "umu_run")
    };

    HostToolCheckResult {
        tool_id: "umu_run".to_string(),
        display_name: catalog_entry
            .map(|entry| entry.display_name.clone())
            .unwrap_or_else(|| "umu-launcher".to_string()),
        is_available: available,
        is_required: capability_requires_tool,
        category: catalog_entry
            .map(|entry| entry.category.clone())
            .unwrap_or_else(|| "runtime".to_string()),
        docs_url: catalog_entry
            .map(|entry| entry.docs_url.clone())
            .unwrap_or_default(),
        tool_version: None,
        resolved_path: probed_umu_path,
        install_guidance,
    }
}

fn install_hint_for_tool(
    result: &ReadinessCheckResult,
    readiness_catalog: &ReadinessCatalog,
    tool_id: &str,
) -> Option<HostToolInstallCommand> {
    let distro = HostDistroFamily::from_catalog_key(result.detected_distro_family.trim())
        .unwrap_or(HostDistroFamily::Unknown);
    readiness_catalog
        .find_by_id(tool_id)
        .and_then(|entry| ReadinessCatalog::install_for_distro(entry, distro))
}

pub(super) fn collect_install_hints<'a>(
    tool_checks: impl Iterator<Item = &'a HostToolCheckResult>,
) -> Vec<HostToolInstallCommand> {
    let mut hints = Vec::new();
    for tool_check in tool_checks {
        let Some(hint) = &tool_check.install_guidance else {
            continue;
        };
        if hint.command.trim().is_empty() && hint.alternatives.trim().is_empty() {
            continue;
        }
        if hints.iter().any(|existing| existing == hint) {
            continue;
        }
        hints.push(hint.clone());
    }
    hints
}
