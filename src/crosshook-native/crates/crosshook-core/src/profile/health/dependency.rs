use std::collections::HashSet;

use crate::metadata::PrefixDependencyStateRow;

use super::types::{HealthIssue, HealthIssueSeverity};

/// Build health issues for prefix dependency states.
///
/// CRITICAL: This function is pure -- takes data in, returns issues out.
/// NO subprocess spawning, NO async, NO Command. The health scan runs at
/// startup and must be fast and synchronous.
pub fn build_dependency_health_issues(
    dep_states: &[PrefixDependencyStateRow],
    required_verbs: &[String],
    active_prefix: &str,
) -> Vec<HealthIssue> {
    if required_verbs.is_empty() {
        return Vec::new();
    }

    let mut issues = Vec::new();

    // If profile has required deps but no state rows at all, deps haven't been checked
    if dep_states.is_empty() {
        issues.push(HealthIssue {
            field: "prefix_dependencies".to_string(),
            path: String::new(),
            message: "Prefix dependencies have not been checked".to_string(),
            remediation: "Open the profile and run a dependency check.".to_string(),
            severity: HealthIssueSeverity::Warning,
        });
        return issues;
    }

    let mut seen = HashSet::new();

    for verb in required_verbs.iter().filter(|verb| seen.insert(verb.as_str())) {
        let state = dep_states
            .iter()
            .find(|row| row.package_name == *verb && row.prefix_path == active_prefix)
            .map(|row| row.state.as_str());

        match state {
            Some("installed") => {
                // Healthy -- no issue
            }
            Some("missing") | Some("install_failed") => {
                issues.push(HealthIssue {
                    field: format!("prefix_dep:{verb}"),
                    path: String::new(),
                    message: format!("Required WINE dependency '{verb}' is not installed"),
                    remediation: format!(
                        "Install '{verb}' via the prefix dependencies panel or run winetricks manually."
                    ),
                    severity: HealthIssueSeverity::Warning,
                });
            }
            Some("check_failed") | Some("unknown") | None => {
                issues.push(HealthIssue {
                    field: format!("prefix_dep:{verb}"),
                    path: String::new(),
                    message: format!("Dependency status unknown for '{verb}'"),
                    remediation: "Run a dependency check to determine current status.".to_string(),
                    severity: HealthIssueSeverity::Warning,
                });
            }
            Some("user_skipped") => {
                issues.push(HealthIssue {
                    field: format!("prefix_dep:{verb}"),
                    path: String::new(),
                    message: format!("User skipped '{verb}' installation"),
                    remediation: format!(
                        "Install '{verb}' if the game requires it, or skip this warning."
                    ),
                    severity: HealthIssueSeverity::Info,
                });
            }
            Some(other) => {
                // Unknown state string -- treat as stale
                issues.push(HealthIssue {
                    field: format!("prefix_dep:{verb}"),
                    path: String::new(),
                    message: format!("Unexpected dependency state '{other}' for '{verb}'"),
                    remediation: "Run a dependency check to refresh status.".to_string(),
                    severity: HealthIssueSeverity::Warning,
                });
            }
        }
    }

    issues
}
