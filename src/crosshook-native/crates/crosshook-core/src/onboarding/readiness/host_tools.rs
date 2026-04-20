use crate::onboarding::{
    HostDistroFamily, HostToolCheckResult, ReadinessCatalog, ReadinessCheckResult,
};

use super::super::distro::detect_host_distro_family;
use super::system::check_system_readiness;

/// Host tool rows from catalog (skips `umu_run` — covered by core umu check).
pub(super) fn evaluate_host_tool_checks(
    catalog: &ReadinessCatalog,
    distro: HostDistroFamily,
    is_flatpak: bool,
) -> (Vec<HostToolCheckResult>, usize, usize) {
    let mut tool_checks = Vec::new();
    let mut extra_warnings = 0usize;
    let mut extra_critical = 0usize;

    for entry in &catalog.entries {
        if entry.tool_id == "umu_run" {
            continue;
        }
        if entry.binary_name.trim().is_empty() {
            continue;
        }

        let available = crate::platform::host_command_exists(&entry.binary_name);
        if available {
            tool_checks.push(HostToolCheckResult {
                tool_id: entry.tool_id.clone(),
                display_name: entry.display_name.clone(),
                is_available: true,
                is_required: entry.required,
                category: entry.category.clone(),
                docs_url: entry.docs_url.clone(),
                tool_version: None,
                resolved_path: None,
                install_guidance: None,
            });
            continue;
        }

        let guidance = if is_flatpak {
            ReadinessCatalog::install_for_distro(entry, distro)
        } else {
            None
        };

        if entry.required {
            extra_critical += 1;
        } else {
            extra_warnings += 1;
        }

        tool_checks.push(HostToolCheckResult {
            tool_id: entry.tool_id.clone(),
            display_name: entry.display_name.clone(),
            is_available: false,
            is_required: entry.required,
            category: entry.category.clone(),
            docs_url: entry.docs_url.clone(),
            tool_version: None,
            resolved_path: None,
            install_guidance: guidance,
        });
    }

    (tool_checks, extra_warnings, extra_critical)
}

/// Full readiness: core checks plus catalog host tools.
pub fn check_generalized_readiness(catalog: &ReadinessCatalog) -> ReadinessCheckResult {
    let mut result = check_system_readiness();
    let distro = detect_host_distro_family();
    let is_flatpak = crate::platform::is_flatpak();
    result.detected_distro_family = distro.as_str().to_string();

    let (tool_checks, tw, tc) = evaluate_host_tool_checks(catalog, distro, is_flatpak);
    result.tool_checks = tool_checks;

    result.warnings = result.warnings.saturating_add(tw);
    result.critical_failures = result.critical_failures.saturating_add(tc);
    result.all_passed = result.critical_failures == 0 && result.warnings == 0;

    result
}
