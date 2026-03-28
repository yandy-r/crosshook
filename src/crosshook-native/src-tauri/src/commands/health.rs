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
    failure_count_30d: i64,
    total_launches: i64,
    last_success: Option<String>,
    profile_id: Option<String>,
    launcher_drift_state: Option<DriftState>,
    profile_source: Option<&str>,
    is_favorite: bool,
) -> ProfileHealthMetadata {
    ProfileHealthMetadata {
        profile_id,
        last_success,
        failure_count_30d,
        total_launches,
        launcher_drift_state,
        is_community_import: profile_source == Some("import"),
        is_favorite,
    }
}

#[derive(Default)]
struct BatchMetadataPrefetch {
    metadata_available: bool,
    failure_trends: HashMap<String, (i64, i64)>,
    last_success_map: HashMap<String, String>,
    total_launches_map: HashMap<String, i64>,
    favorite_profiles: HashSet<String>,
    profile_id_map: HashMap<String, String>,
    launcher_drift_map: HashMap<String, DriftState>,
    profile_source_map: HashMap<String, String>,
}

fn prefetch_batch_metadata(
    metadata_store: &MetadataStore,
    profile_names: &[String],
) -> BatchMetadataPrefetch {
    let metadata_available = metadata_store.is_available();
    if !metadata_available {
        return BatchMetadataPrefetch::default();
    }

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

    let total_launches_map: HashMap<String, i64> = metadata_store
        .query_total_launches_for_profiles(profile_names)
        .unwrap_or_default()
        .into_iter()
        .collect();

    let favorite_profiles: HashSet<String> = metadata_store
        .list_favorite_profiles()
        .unwrap_or_default()
        .into_iter()
        .collect();

    let profile_id_map: HashMap<String, String> = metadata_store
        .query_profile_ids_for_names(profile_names)
        .unwrap_or_default()
        .into_iter()
        .collect();

    let profile_ids: Vec<String> = profile_id_map.values().cloned().collect();
    let launcher_drift_map: HashMap<String, DriftState> = metadata_store
        .query_launcher_drift_for_profile_ids(&profile_ids)
        .unwrap_or_default()
        .into_iter()
        .collect();

    let profile_source_map: HashMap<String, String> = metadata_store
        .query_profile_sources_for_names(profile_names)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(name, source)| source.map(|value| (name, value)))
        .collect();

    BatchMetadataPrefetch {
        metadata_available,
        failure_trends,
        last_success_map,
        total_launches_map,
        favorite_profiles,
        profile_id_map,
        launcher_drift_map,
        profile_source_map,
    }
}

fn enrich_profile(
    report: ProfileHealthReport,
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
    let profile_source = prefetch.profile_source_map.get(&report.name).map(String::as_str);
    let is_favorite = prefetch.favorite_profiles.contains(&report.name);
    let metadata = if prefetch.metadata_available {
        Some(build_profile_metadata(
            failure_count_30d,
            total_launches,
            last_success,
            profile_id,
            launcher_drift_state,
            profile_source,
            is_favorite,
        ))
    } else {
        None
    };

    EnrichedProfileHealthReport {
        core: sanitize_report(report),
        metadata,
    }
}

pub(crate) fn build_enriched_health_summary(
    store: &ProfileStore,
    metadata_store: &MetadataStore,
) -> EnrichedHealthSummary {
    let summary = batch_check_health(store);
    let profile_names: Vec<String> = summary.profiles.iter().map(|report| report.name.clone()).collect();
    let prefetch = prefetch_batch_metadata(metadata_store, &profile_names);

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
        .map(|report| enrich_profile(report, &prefetch))
        .collect();

    EnrichedHealthSummary {
        healthy_count,
        stale_count,
        broken_count,
        total_count,
        validated_at,
        profiles: enriched_profiles,
    }
}

/// Returns health check results for all profiles in the store, enriched with
/// MetadataStore failure trends, last-success timestamps, and launcher drift state.
///
/// Path strings in every `HealthIssue` are sanitized (home directory replaced with `~`)
/// before being sent over IPC. Metadata enrichment is fail-soft — if MetadataStore is
/// unavailable the `metadata` field is omitted.
#[tauri::command]
pub fn batch_validate_profiles(
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<EnrichedHealthSummary, String> {
    Ok(build_enriched_health_summary(&store, &metadata_store))
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
    if !metadata_store.is_available() {
        return Ok(EnrichedProfileHealthReport {
            core: sanitize_report(report),
            metadata: None,
        });
    }

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

    let profile_id = metadata_store.lookup_profile_id(&name).ok().flatten();
    let launcher_drift_state = profile_id.as_deref().and_then(|pid| {
        metadata_store
            .query_launcher_drift_for_profile(pid)
            .ok()
            .flatten()
    });
    let profile_source = metadata_store.query_profile_source(&name).ok().flatten();
    let is_favorite = favorite_profiles.contains(&name);

    Ok(EnrichedProfileHealthReport {
        core: sanitize_report(report),
        metadata: Some(build_profile_metadata(
            failure_count_30d,
            total_launches,
            last_success,
            profile_id,
            launcher_drift_state,
            profile_source.as_deref(),
            is_favorite,
        )),
    })
}
