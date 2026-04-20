use super::enrich::build_profile_metadata;
use super::steam::live_steam_build_id_for_profile;
use super::types::{sanitize_report, EnrichedProfileHealthReport, OfflineReadinessBrief};
use super::FAILURE_TREND_WINDOW_DAYS;
use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::health::{build_dependency_health_issues, check_profile_health};
use crosshook_core::profile::ProfileStore;
use std::collections::HashSet;
use tauri::State;

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
    let mut report = check_profile_health(&name, &profile);
    if !metadata_store.is_available() {
        return Ok(EnrichedProfileHealthReport {
            core: sanitize_report(report),
            metadata: None,
            offline_readiness: None,
        });
    }

    let mut offline_readiness: Option<OfflineReadinessBrief> = None;
    if let Some(pid) = metadata_store.lookup_profile_id(&name).ok().flatten() {
        if let Ok(Some(off)) =
            metadata_store.with_sqlite_conn("get_profile_health offline", |conn| {
                crosshook_core::offline::enrich_health_report_with_offline(
                    conn,
                    &name,
                    &pid,
                    &profile,
                    &mut report,
                )
            })
        {
            offline_readiness = Some(OfflineReadinessBrief::from(&off));
        }
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

    let version_snapshot = profile_id.as_deref().and_then(|pid| {
        metadata_store
            .lookup_latest_version_snapshot(pid)
            .ok()
            .flatten()
    });
    let version_status = version_snapshot.as_ref().map(|s| s.status.clone());
    let snapshot_build_id = version_snapshot
        .as_ref()
        .and_then(|s| s.steam_build_id.clone());
    let trainer_version = version_snapshot
        .as_ref()
        .and_then(|s| s.trainer_version.clone());
    let current_build_id = live_steam_build_id_for_profile(&profile);

    let effective_profile = profile.effective_profile();
    let required_verbs = &effective_profile.trainer.required_protontricks;
    if !required_verbs.is_empty() {
        if let Some(ref pid) = profile_id {
            let dep_states = metadata_store
                .load_prefix_dep_states(pid)
                .unwrap_or_default();
            let active_prefix = if !effective_profile.runtime.prefix_path.trim().is_empty() {
                effective_profile.runtime.prefix_path.as_str()
            } else {
                effective_profile.steam.compatdata_path.as_str()
            };
            let dep_issues =
                build_dependency_health_issues(&dep_states, required_verbs, active_prefix);
            report.issues.extend(dep_issues);
        }
    }

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
            version_status,
            snapshot_build_id,
            current_build_id,
            trainer_version,
        )),
        offline_readiness,
    })
}
