use super::steam::live_steam_build_ids_for_profiles;
use super::FAILURE_TREND_WINDOW_DAYS;
use crosshook_core::metadata::{
    DriftState, MetadataStore, PrefixDependencyStateRow, VersionSnapshotRow,
};
use crosshook_core::profile::ProfileStore;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(super) struct BatchMetadataPrefetch {
    pub(super) metadata_available: bool,
    pub(super) failure_trends: HashMap<String, (i64, i64)>,
    pub(super) last_success_map: HashMap<String, String>,
    pub(super) total_launches_map: HashMap<String, i64>,
    pub(super) favorite_profiles: HashSet<String>,
    pub(super) profile_id_map: HashMap<String, String>,
    pub(super) launcher_drift_map: HashMap<String, DriftState>,
    pub(super) profile_source_map: HashMap<String, String>,
    pub(super) version_snapshot_map: HashMap<String, VersionSnapshotRow>,
    pub(super) live_build_id_by_profile: HashMap<String, Option<String>>,
    pub(super) dep_states_by_profile: HashMap<String, Vec<PrefixDependencyStateRow>>,
    pub(super) required_verbs_by_profile: HashMap<String, Vec<String>>,
    pub(super) active_prefix_by_profile: HashMap<String, String>,
}

pub(super) fn prefetch_batch_metadata(
    metadata_store: &MetadataStore,
    profile_store: &ProfileStore,
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

    let version_snapshot_map: HashMap<String, VersionSnapshotRow> = metadata_store
        .load_version_snapshots_for_profiles()
        .unwrap_or_default()
        .into_iter()
        .map(|row| (row.profile_id.clone(), row))
        .collect();

    let live_build_id_by_profile = live_steam_build_ids_for_profiles(profile_store, profile_names);

    // Prefix dependency states for health enrichment
    let mut dep_states_by_profile: HashMap<String, Vec<PrefixDependencyStateRow>> = HashMap::new();
    let mut required_verbs_by_profile: HashMap<String, Vec<String>> = HashMap::new();
    let mut active_prefix_by_profile: HashMap<String, String> = HashMap::new();
    for name in profile_names {
        // Load required verbs from profile
        if let Ok(profile) = profile_store.load(name) {
            let effective = profile.effective_profile();
            let verbs = effective.trainer.required_protontricks.clone();
            if !verbs.is_empty() {
                required_verbs_by_profile.insert(name.clone(), verbs);
                let active_prefix = if !effective.runtime.prefix_path.trim().is_empty() {
                    effective.runtime.prefix_path.clone()
                } else {
                    effective.steam.compatdata_path.clone()
                };
                active_prefix_by_profile.insert(name.clone(), active_prefix);
                // Load cached dep states from SQLite
                if let Some(pid) = profile_id_map.get(name) {
                    if let Ok(states) = metadata_store.load_prefix_dep_states(pid) {
                        dep_states_by_profile.insert(name.clone(), states);
                    }
                }
            }
        }
    }

    BatchMetadataPrefetch {
        metadata_available,
        failure_trends,
        last_success_map,
        total_launches_map,
        favorite_profiles,
        profile_id_map,
        launcher_drift_map,
        profile_source_map,
        version_snapshot_map,
        live_build_id_by_profile,
        dep_states_by_profile,
        required_verbs_by_profile,
        active_prefix_by_profile,
    }
}
