use super::enrich::enrich_profile;
use super::prefetch::{prefetch_batch_metadata, BatchMetadataPrefetch};
use super::types::{EnrichedHealthSummary, EnrichedProfileHealthReport, OfflineReadinessBrief};
use crosshook_core::metadata::MetadataStore;
use crosshook_core::offline::OfflineReadinessReport;
use crosshook_core::profile::health::{
    batch_check_health, batch_check_health_with_enrich, HealthCheckSummary,
};
use crosshook_core::profile::ProfileStore;
use std::collections::HashMap;
use tauri::State;

pub fn build_enriched_health_summary(
    store: &ProfileStore,
    metadata_store: &MetadataStore,
) -> EnrichedHealthSummary {
    let mut offline_map: HashMap<String, OfflineReadinessReport> = HashMap::new();
    let mut cached_prefetch: Option<BatchMetadataPrefetch> = None;

    let summary = if metadata_store.is_available() {
        match store.list() {
            Ok(names) => {
                let prefetch_offline = prefetch_batch_metadata(metadata_store, store, &names);
                let result =
                    metadata_store.with_sqlite_conn("batch profile health with offline", |conn| {
                        Ok(batch_check_health_with_enrich(
                            store,
                            |name, profile, report| {
                                if let Some(pid) = prefetch_offline.profile_id_map.get(name) {
                                    if let Ok(Some(off)) =
                                        crosshook_core::offline::enrich_health_report_with_offline(
                                            conn,
                                            name,
                                            pid.as_str(),
                                            profile,
                                            report,
                                        )
                                    {
                                        offline_map.insert(name.to_string(), off.clone());
                                    }
                                }
                            },
                        ))
                    });
                match result {
                    Ok(s) => {
                        cached_prefetch = Some(prefetch_offline);
                        s
                    }
                    Err(e) => {
                        tracing::warn!(
                            %e,
                            "batch profile health with offline failed; falling back"
                        );
                        offline_map.clear();
                        cached_prefetch = Some(prefetch_offline);
                        batch_check_health(store)
                    }
                }
            }
            Err(_) => batch_check_health(store),
        }
    } else {
        batch_check_health(store)
    };

    let prefetch = cached_prefetch.unwrap_or_else(|| {
        let profile_names: Vec<String> = summary
            .profiles
            .iter()
            .map(|report| report.name.clone())
            .collect();
        prefetch_batch_metadata(metadata_store, store, &profile_names)
    });

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
            let mut row = enrich_profile(report, &prefetch);
            if let Some(off) = offline_map.get(&row.core.name) {
                row.offline_readiness = Some(OfflineReadinessBrief::from(off));
            }
            row
        })
        .collect();

    for enriched in &enriched_profiles {
        if let Some(ref metadata) = enriched.metadata {
            if let Some(ref profile_id) = metadata.profile_id {
                let status_str = match enriched.core.status {
                    crosshook_core::profile::health::HealthStatus::Healthy => "healthy",
                    crosshook_core::profile::health::HealthStatus::Stale => "stale",
                    crosshook_core::profile::health::HealthStatus::Broken => "broken",
                };
                if let Err(error) = metadata_store.upsert_health_snapshot(
                    profile_id,
                    status_str,
                    enriched.core.issues.len(),
                    &enriched.core.checked_at,
                ) {
                    tracing::warn!(
                        %error,
                        profile_id,
                        "failed to persist health snapshot"
                    );
                }
            }
        }
    }

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
