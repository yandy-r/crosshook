use crosshook_core::profile::health::{
    batch_check_health, check_profile_health, HealthCheckSummary, HealthIssue, ProfileHealthReport,
};
use crosshook_core::profile::ProfileStore;
use tauri::State;

use super::shared::sanitize_display_path;

fn sanitize_issues(issues: Vec<HealthIssue>) -> Vec<HealthIssue> {
    issues
        .into_iter()
        .map(|mut issue| {
            issue.path = sanitize_display_path(&issue.path);
            issue
        })
        .collect()
}

fn sanitize_report(report: ProfileHealthReport) -> ProfileHealthReport {
    ProfileHealthReport {
        issues: sanitize_issues(report.issues),
        ..report
    }
}

/// Returns health check results for all profiles in the store.
///
/// Path strings in every `HealthIssue` are sanitized (home directory replaced with `~`)
/// before being sent over IPC.
#[tauri::command]
pub fn batch_validate_profiles(
    store: State<'_, ProfileStore>,
) -> Result<HealthCheckSummary, String> {
    let mut summary = batch_check_health(&store);
    summary.profiles = summary.profiles.into_iter().map(sanitize_report).collect();
    Ok(summary)
}

/// Returns the health check result for a single named profile.
///
/// Path strings in every `HealthIssue` are sanitized before being sent over IPC.
/// Returns an error string when the profile is not found or cannot be loaded.
#[tauri::command]
pub fn get_profile_health(
    name: String,
    store: State<'_, ProfileStore>,
) -> Result<ProfileHealthReport, String> {
    let profile = store.load(&name).map_err(|e| e.to_string())?;
    let report = check_profile_health(&name, &profile);
    Ok(sanitize_report(report))
}
