use chrono::Utc;

use crate::launch::request::{METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH};
use crate::profile::models::{resolve_launch_method, GameProfile};
use crate::profile::toml_store::ProfileStore;

use super::checks::{
    check_file_path, check_optional_path, check_required_directory, check_required_executable,
    check_required_file,
};
use super::types::{
    HealthCheckSummary, HealthIssue, HealthIssueSeverity, HealthStatus, ProfileHealthReport,
};

/// Validates path fields of a `GameProfile` and returns a `ProfileHealthReport`.
///
/// Method-aware: only fields required by the resolved launch method are checked as required.
/// All populated optional fields (icon_path, working_directory) are validated at Info severity.
pub fn check_profile_health(name: &str, profile: &GameProfile) -> ProfileHealthReport {
    let effective_profile = profile.effective_profile();
    let launch_method = resolve_launch_method(&effective_profile).to_string();
    let mut issues: Vec<HealthIssue> = Vec::new();
    let mut has_stale = false;
    let mut has_broken = false;

    // Collect required-field results — each returns (issue, is_stale)
    let mut required_results: Vec<Option<(HealthIssue, bool)>> = Vec::new();

    // game.executable_path — required for all methods
    required_results.push(check_required_file(
        "game.executable_path",
        &effective_profile.game.executable_path,
    ));

    // trainer.path — required only if non-empty
    if !effective_profile.trainer.path.trim().is_empty() {
        required_results.push(check_file_path(
            "trainer.path",
            &effective_profile.trainer.path,
            HealthIssueSeverity::Error,
        ));
    }

    // injection.dll_paths — each non-empty entry must exist as a file
    for (i, dll_path) in effective_profile.injection.dll_paths.iter().enumerate() {
        if !dll_path.trim().is_empty() {
            required_results.push(check_file_path(
                &format!("injection.dll_paths[{i}]"),
                dll_path,
                HealthIssueSeverity::Error,
            ));
        }
    }

    // Method-specific required fields
    match launch_method.as_str() {
        METHOD_STEAM_APPLAUNCH => {
            required_results.push(check_required_directory(
                "steam.compatdata_path",
                &effective_profile.steam.compatdata_path,
            ));
            required_results.push(check_required_executable(
                "steam.proton_path",
                &effective_profile.steam.proton_path,
            ));
        }
        METHOD_PROTON_RUN => {
            required_results.push(check_required_directory(
                "runtime.prefix_path",
                &effective_profile.runtime.prefix_path,
            ));
            required_results.push(check_required_executable(
                "runtime.proton_path",
                &effective_profile.runtime.proton_path,
            ));
        }
        _ => {
            // native — no additional required path fields
        }
    }

    // Process required-field results
    for (issue, stale) in required_results.into_iter().flatten() {
        if stale {
            has_stale = true;
        } else {
            has_broken = true;
        }
        issues.push(issue);
    }

    // Optional fields — checked at Info severity regardless of method
    if let Some(issue) = check_optional_path(
        "steam.launcher.icon_path",
        &effective_profile.steam.launcher.icon_path,
    ) {
        issues.push(issue);
    }
    if let Some(issue) = check_optional_path(
        "runtime.working_directory",
        &effective_profile.runtime.working_directory,
    ) {
        issues.push(issue);
    }

    // Determine overall status.
    // Unconfigured profiles (all empty required fields) also classify as Broken per business rules;
    // the UI presents them with badge-only (no banner) based on all issues having empty path fields.
    let status = if has_broken {
        HealthStatus::Broken
    } else if has_stale {
        HealthStatus::Stale
    } else {
        HealthStatus::Healthy
    };

    ProfileHealthReport {
        name: name.to_string(),
        status,
        launch_method,
        issues,
        checked_at: Utc::now().to_rfc3339(),
    }
}

/// Like [`batch_check_health`], but invokes `enrich` after each successful `check_profile_health`
/// so callers can attach SQLite-backed checks (e.g. offline readiness) using one `Connection`.
pub fn batch_check_health_with_enrich<F>(store: &ProfileStore, mut enrich: F) -> HealthCheckSummary
where
    F: FnMut(&str, &GameProfile, &mut ProfileHealthReport),
{
    let now = Utc::now().to_rfc3339();

    let names = match store.list() {
        Ok(names) => names,
        Err(err) => {
            return HealthCheckSummary {
                profiles: vec![ProfileHealthReport {
                    name: "<unknown>".to_string(),
                    status: HealthStatus::Broken,
                    launch_method: String::new(),
                    issues: vec![HealthIssue {
                        field: String::new(),
                        path: String::new(),
                        message: format!("Could not list profiles: {err}"),
                        remediation: "Check filesystem permissions for the profiles directory."
                            .to_string(),
                        severity: HealthIssueSeverity::Error,
                    }],
                    checked_at: now.clone(),
                }],
                healthy_count: 0,
                stale_count: 0,
                broken_count: 1,
                total_count: 1,
                validated_at: now,
            };
        }
    };

    let mut profiles: Vec<ProfileHealthReport> = Vec::with_capacity(names.len());

    for name in &names {
        let report = match store.load(name) {
            Ok(profile) => {
                let mut report = check_profile_health(name, &profile);
                enrich(name, &profile, &mut report);
                report
            }
            Err(err) => ProfileHealthReport {
                name: name.clone(),
                status: HealthStatus::Broken,
                launch_method: String::new(),
                issues: vec![HealthIssue {
                    field: String::new(),
                    path: String::new(),
                    message: format!("Profile could not be loaded: {err}"),
                    remediation: "The profile TOML may be malformed. Delete and re-create the profile, or edit the file manually.".to_string(),
                    severity: HealthIssueSeverity::Error,
                }],
                checked_at: Utc::now().to_rfc3339(),
            },
        };
        profiles.push(report);
    }

    let healthy_count = profiles
        .iter()
        .filter(|r| matches!(r.status, HealthStatus::Healthy))
        .count();
    let stale_count = profiles
        .iter()
        .filter(|r| matches!(r.status, HealthStatus::Stale))
        .count();
    let broken_count = profiles
        .iter()
        .filter(|r| matches!(r.status, HealthStatus::Broken))
        .count();
    let total_count = profiles.len();

    HealthCheckSummary {
        profiles,
        healthy_count,
        stale_count,
        broken_count,
        total_count,
        validated_at: now,
    }
}

/// Runs `check_profile_health` for every profile in the store and returns a summary.
///
/// Errors loading individual profiles are captured as `Broken` entries and do not abort
/// the batch — this function never propagates `ProfileStoreError` from the per-profile loop.
pub fn batch_check_health(store: &ProfileStore) -> HealthCheckSummary {
    batch_check_health_with_enrich(store, |_, _, _| {})
}
