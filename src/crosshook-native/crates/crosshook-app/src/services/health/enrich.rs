use super::prefetch::BatchMetadataPrefetch;
use super::types::{sanitize_report, EnrichedProfileHealthReport, ProfileHealthMetadata};
use crosshook_core::metadata::DriftState;
use crosshook_core::profile::health::{
    build_dependency_health_issues, HealthIssue, HealthIssueSeverity, ProfileHealthReport,
};

pub(super) fn build_profile_metadata(
    failure_count_30d: i64,
    total_launches: i64,
    last_success: Option<String>,
    profile_id: Option<String>,
    launcher_drift_state: Option<DriftState>,
    profile_source: Option<&str>,
    is_favorite: bool,
    version_status: Option<String>,
    snapshot_build_id: Option<String>,
    current_build_id: Option<String>,
    trainer_version: Option<String>,
) -> ProfileHealthMetadata {
    ProfileHealthMetadata {
        profile_id,
        last_success,
        failure_count_30d,
        total_launches,
        launcher_drift_state,
        is_community_import: profile_source == Some("import"),
        is_favorite,
        version_status,
        snapshot_build_id,
        current_build_id,
        trainer_version,
    }
}

pub(super) fn enrich_profile(
    mut report: ProfileHealthReport,
    prefetch: &BatchMetadataPrefetch,
) -> EnrichedProfileHealthReport {
    let (failure_count_30d, _successes) = prefetch
        .failure_trends
        .get(&report.name)
        .copied()
        .unwrap_or((0, 0));

    let total_launches = prefetch
        .total_launches_map
        .get(&report.name)
        .copied()
        .unwrap_or(0);

    let last_success = prefetch.last_success_map.get(&report.name).cloned();
    let profile_id = prefetch.profile_id_map.get(&report.name).cloned();
    let launcher_drift_state = profile_id
        .as_deref()
        .and_then(|pid| prefetch.launcher_drift_map.get(pid).copied());
    let profile_source = prefetch
        .profile_source_map
        .get(&report.name)
        .map(String::as_str);
    let is_favorite = prefetch.favorite_profiles.contains(&report.name);

    let version_snapshot = profile_id
        .as_deref()
        .and_then(|pid| prefetch.version_snapshot_map.get(pid));
    let version_status = version_snapshot.map(|s| s.status.clone());
    let snapshot_build_id = version_snapshot.and_then(|s| s.steam_build_id.clone());
    let trainer_version = version_snapshot.and_then(|s| s.trainer_version.clone());
    let current_build_id = prefetch
        .live_build_id_by_profile
        .get(&report.name)
        .cloned()
        .flatten();

    if let Some(ref status) = version_status {
        if matches!(
            status.as_str(),
            "game_updated" | "trainer_changed" | "both_changed"
        ) {
            let (message, remediation) = match status.as_str() {
                "game_updated" => (
                    "Game version has changed since last successful launch".to_string(),
                    "Verify the trainer still works and acknowledge the version change".to_string(),
                ),
                "trainer_changed" => (
                    "Trainer binary has changed since last successful launch".to_string(),
                    "Verify the trainer still works and acknowledge the version change".to_string(),
                ),
                _ => (
                    "Both game and trainer versions have changed since last successful launch"
                        .to_string(),
                    "Verify the trainer still works with the new game version and acknowledge the change"
                        .to_string(),
                ),
            };
            report.issues.push(HealthIssue {
                field: "version".to_string(),
                path: String::new(),
                message,
                remediation,
                severity: HealthIssueSeverity::Warning,
            });
        }
    }

    if let Some(required_verbs) = prefetch.required_verbs_by_profile.get(&report.name) {
        let dep_states = prefetch
            .dep_states_by_profile
            .get(&report.name)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let active_prefix = prefetch
            .active_prefix_by_profile
            .get(&report.name)
            .map(String::as_str)
            .unwrap_or("");
        let dep_issues = build_dependency_health_issues(dep_states, required_verbs, active_prefix);
        report.issues.extend(dep_issues);
    }

    let metadata = if prefetch.metadata_available {
        Some(build_profile_metadata(
            failure_count_30d,
            total_launches,
            last_success,
            profile_id,
            launcher_drift_state,
            profile_source,
            is_favorite,
            version_status,
            snapshot_build_id,
            current_build_id,
            trainer_version,
        ))
    } else {
        None
    };

    EnrichedProfileHealthReport {
        core: sanitize_report(report),
        metadata,
        offline_readiness: None,
    }
}
