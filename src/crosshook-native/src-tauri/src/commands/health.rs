use crosshook_core::metadata::{DriftState, MetadataStore};
use crosshook_core::profile::health::{
    batch_check_health, check_profile_health, HealthCheckSummary, HealthIssue, ProfileHealthReport,
};
use crosshook_core::profile::ProfileStore;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tauri::State;

use super::shared::sanitize_display_path;

const FAILURE_TREND_WINDOW_DAYS: u32 = 30;
// Bound "most launched" prefetch size to avoid unbounded scans on large datasets.
const MAX_MOST_LAUNCHED_LIMIT: usize = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileHealthMetadata {
    pub profile_id: Option<String>,
    pub last_success: Option<String>,
    pub failure_count_30d: i64,
    pub total_launches: i64,
    pub launcher_drift_state: Option<DriftState>,
    pub is_community_import: bool,
    pub is_favorite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedProfileHealthReport {
    #[serde(flatten)]
    pub core: ProfileHealthReport,
    pub metadata: Option<ProfileHealthMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedHealthSummary {
    pub profiles: Vec<EnrichedProfileHealthReport>,
    pub healthy_count: usize,
    pub stale_count: usize,
    pub broken_count: usize,
    pub total_count: usize,
    pub validated_at: String,
}

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

fn build_profile_metadata(
    profile_name: &str,
    metadata_store: &MetadataStore,
    failure_count_30d: i64,
    total_launches: i64,
    last_success: Option<String>,
    favorite_profiles: &HashSet<String>,
) -> ProfileHealthMetadata {
    let profile_id = metadata_store.lookup_profile_id(profile_name).ok().flatten();

    let launcher_drift_state = profile_id.as_deref().and_then(|pid| {
        metadata_store
            .query_launcher_drift_for_profile(pid)
            .ok()
            .flatten()
    });

    let is_community_import = profile_id
        .as_deref()
        .and_then(|_| metadata_store.query_profile_source(profile_name).ok().flatten())
        .map(|source| source == "import")
        .unwrap_or(false);

    let is_favorite = favorite_profiles.contains(profile_name);

    ProfileHealthMetadata {
        profile_id,
        last_success,
        failure_count_30d,
        total_launches,
        launcher_drift_state,
        is_community_import,
        is_favorite,
    }
}

fn enrich_profile(
    report: ProfileHealthReport,
    metadata_store: &MetadataStore,
    failure_trends: &HashMap<String, (i64, i64)>,
    last_success_map: &HashMap<String, String>,
    total_launches_map: &HashMap<String, i64>,
    favorite_profiles: &HashSet<String>,
) -> EnrichedProfileHealthReport {
    let (failure_count_30d, _successes) = failure_trends
        .get(&report.name)
        .copied()
        .unwrap_or((0, 0));

    let total_launches = total_launches_map
        .get(&report.name)
        .copied()
        .unwrap_or(0);

    let last_success = last_success_map.get(&report.name).cloned();
    let metadata = Some(build_profile_metadata(
        &report.name,
        metadata_store,
        failure_count_30d,
        total_launches,
        last_success,
        favorite_profiles,
    ));

    EnrichedProfileHealthReport {
        core: sanitize_report(report),
        metadata,
    }
}

/// Returns health check results for all profiles in the store, enriched with
/// MetadataStore failure trends, last-success timestamps, and launcher drift state.
///
/// Path strings in every `HealthIssue` are sanitized (home directory replaced with `~`)
/// before being sent over IPC. Metadata enrichment is fail-soft — if MetadataStore is
/// unavailable the `metadata` field is still populated with zero/null defaults.
#[tauri::command]
pub fn batch_validate_profiles(
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<EnrichedHealthSummary, String> {
    let summary = batch_check_health(&store);

    // Batch-fetch failure trends and last-success before the per-profile loop.
    let failure_trends: HashMap<String, (i64, i64)> = metadata_store
        .query_failure_trends(FAILURE_TREND_WINDOW_DAYS)
        .unwrap_or_default()
        .into_iter()
        .map(|row| (row.profile_name, (row.failures, row.successes)))
        .collect();

    let last_success_map: HashMap<String, String> = metadata_store
        .query_last_success_per_profile()
        .unwrap_or_default()
        .into_iter()
        .collect();

    // query_most_launched returns (profile_name, total_count) across all time.
    let total_launches_map: HashMap<String, i64> = metadata_store
        .query_most_launched(MAX_MOST_LAUNCHED_LIMIT)
        .unwrap_or_default()
        .into_iter()
        .collect();

    let favorite_profiles: HashSet<String> = metadata_store
        .list_favorite_profiles()
        .unwrap_or_default()
        .into_iter()
        .collect();

    let HealthCheckSummary {
        profiles: raw_profiles,
        healthy_count,
        stale_count,
        broken_count,
        total_count,
        validated_at,
    } = summary;

    let enriched_profiles: Vec<EnrichedProfileHealthReport> = raw_profiles
        .into_iter()
        .map(|report| {
            enrich_profile(
                report,
                &metadata_store,
                &failure_trends,
                &last_success_map,
                &total_launches_map,
                &favorite_profiles,
            )
        })
        .collect();

    Ok(EnrichedHealthSummary {
        healthy_count,
        stale_count,
        broken_count,
        total_count,
        validated_at,
        profiles: enriched_profiles,
    })
}

/// Returns the health check result for a single named profile, enriched with
/// MetadataStore data where available.
///
/// Path strings in every `HealthIssue` are sanitized before being sent over IPC.
/// Returns an error string when the profile is not found or cannot be loaded.
#[tauri::command]
pub fn get_profile_health(
    name: String,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<EnrichedProfileHealthReport, String> {
    let profile = store.load(&name).map_err(|e| e.to_string())?;
    let report = check_profile_health(&name, &profile);

    let (failure_count_30d, _successes) = metadata_store
        .query_failure_trend_for_profile(&name, FAILURE_TREND_WINDOW_DAYS)
        .unwrap_or((0, 0));

    let last_success = metadata_store
        .query_last_success_for_profile(&name)
        .unwrap_or(None);

    let total_launches = metadata_store
        .query_total_launches_for_profile(&name)
        .unwrap_or(0);

    let favorite_profiles: HashSet<String> = metadata_store
        .list_favorite_profiles()
        .unwrap_or_default()
        .into_iter()
        .collect();

    Ok(EnrichedProfileHealthReport {
        core: sanitize_report(report),
        metadata: Some(build_profile_metadata(
            &name,
            &metadata_store,
            failure_count_30d,
            total_launches,
            last_success,
            &favorite_profiles,
        )),
    })
}
