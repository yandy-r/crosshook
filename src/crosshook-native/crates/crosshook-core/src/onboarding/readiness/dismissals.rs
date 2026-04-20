use std::collections::HashSet;

use crate::onboarding::ReadinessCheckResult;

/// Clear structured payloads for tool IDs present in `dismissed` (DB-backed nag dismissals).
pub fn apply_readiness_nag_dismissals(
    result: &mut ReadinessCheckResult,
    dismissed: &HashSet<String>,
) {
    if dismissed.contains("umu_run") {
        result.umu_install_guidance = None;
        for issue in &mut result.checks {
            if issue.field == "umu_run_available" {
                issue.remediation.clear();
            }
        }
    }
    if dismissed.contains("steam_deck_caveats") {
        result.steam_deck_caveats = None;
    }
    for t in &mut result.tool_checks {
        if dismissed.contains(&t.tool_id) {
            t.install_guidance = None;
        }
    }
}

pub fn apply_install_nag_dismissal(
    result: &mut ReadinessCheckResult,
    install_nag_dismissed_at: &Option<String>,
) {
    if install_nag_dismissed_at.is_some() {
        result.umu_install_guidance = None;
    }
}

pub fn apply_steam_deck_caveats_dismissal(
    result: &mut ReadinessCheckResult,
    dismissed_at: &Option<String>,
) {
    if dismissed_at.is_some() {
        result.steam_deck_caveats = None;
    }
}
