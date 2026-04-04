use crosshook_core::metadata::{
    DriftState, HealthSnapshotRow, MetadataStore, OfflineReadinessRow, PrefixDependencyStateRow,
    VersionSnapshotRow,
};
use crosshook_core::offline::OfflineReadinessReport;
use crosshook_core::profile::health::{
    batch_check_health, batch_check_health_with_enrich, build_dependency_health_issues,
    check_profile_health, HealthCheckSummary, HealthIssue, HealthIssueSeverity, HealthStatus,
    ProfileHealthReport,
};
use crosshook_core::profile::{GameProfile, ProfileStore};
use crosshook_core::steam::libraries::discover_steam_libraries;
use crosshook_core::steam::manifest::parse_manifest_full;
use crosshook_core::steam::{discover_steam_root_candidates, SteamLibrary};
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
    pub version_status: Option<String>,
    pub snapshot_build_id: Option<String>,
    pub current_build_id: Option<String>,
    pub trainer_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineReadinessBrief {
    pub profile_name: String,
    pub score: u8,
    pub readiness_state: String,
    pub trainer_type: String,
    pub blocking_reasons: Vec<String>,
    pub checked_at: String,
}

impl From<&OfflineReadinessReport> for OfflineReadinessBrief {
    fn from(r: &OfflineReadinessReport) -> Self {
        Self {
            profile_name: r.profile_name.clone(),
            score: r.score,
            readiness_state: r.readiness_state.clone(),
            trainer_type: r.trainer_type.clone(),
            blocking_reasons: r.blocking_reasons.clone(),
            checked_at: r.checked_at.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedProfileHealthReport {
    #[serde(flatten)]
    pub core: ProfileHealthReport,
    pub metadata: Option<ProfileHealthMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offline_readiness: Option<OfflineReadinessBrief>,
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

/// Same derivation as `commands/profile.rs::derive_steam_client_install_path` — profiles
/// store Proton compatdata, not the Steam client install path directly.
fn steam_client_install_path_from_profile(profile: &GameProfile) -> String {
    const STEAM_COMPATDATA_MARKER: &str = "/steamapps/compatdata/";
    let compatdata_path = profile.steam.compatdata_path.trim().replace('\\', "/");
    compatdata_path
        .split_once(STEAM_COMPATDATA_MARKER)
        .map(|(steam_root, _)| steam_root.to_string())
        .unwrap_or_default()
}

fn manifest_build_id_from_libraries(libraries: &[SteamLibrary], app_id: &str) -> Option<String> {
    for library in libraries {
        let candidate = library
            .steamapps_path
            .join(format!("appmanifest_{app_id}.acf"));
        if candidate.is_file() {
            if let Ok(data) = parse_manifest_full(&candidate) {
                if !data.build_id.is_empty() {
                    return Some(data.build_id);
                }
            }
        }
    }
    None
}

/// Live Steam `buildid` from the installed app manifest for this profile's App ID.
fn live_steam_build_id_for_profile(profile: &GameProfile) -> Option<String> {
    let app_id = profile.steam.app_id.trim();
    if app_id.is_empty() {
        return None;
    }
    let mut diagnostics = Vec::new();
    let configured = steam_client_install_path_from_profile(profile);
    let steam_roots = discover_steam_root_candidates(
        if configured.is_empty() {
            ""
        } else {
            configured.as_str()
        },
        &mut diagnostics,
    );
    let libraries = discover_steam_libraries(&steam_roots, &mut diagnostics);
    for entry in &diagnostics {
        tracing::debug!(entry, "health steam discovery diagnostic");
    }
    manifest_build_id_from_libraries(&libraries, app_id)
}

/// Resolve live build IDs for many profiles, running Steam discovery once per distinct
/// configured Steam client install path.
fn live_steam_build_ids_for_profiles(
    profile_store: &ProfileStore,
    profile_names: &[String],
) -> HashMap<String, Option<String>> {
    let mut by_steam_path: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for name in profile_names {
        let Ok(profile) = profile_store.load(name) else {
            continue;
        };
        let app_id = profile.steam.app_id.trim().to_string();
        if app_id.is_empty() {
            continue;
        }
        let path_key = steam_client_install_path_from_profile(&profile);
        by_steam_path
            .entry(path_key)
            .or_default()
            .push((name.clone(), app_id));
    }

    let mut out: HashMap<String, Option<String>> = HashMap::new();
    for (steam_path, entries) in by_steam_path {
        let mut diagnostics = Vec::new();
        let steam_roots = discover_steam_root_candidates(
            if steam_path.is_empty() {
                ""
            } else {
                steam_path.as_str()
            },
            &mut diagnostics,
        );
        let libraries = discover_steam_libraries(&steam_roots, &mut diagnostics);
        for entry in &diagnostics {
            tracing::debug!(entry, "health batch steam discovery diagnostic");
        }
        for (profile_name, app_id) in entries {
            out.insert(
                profile_name,
                manifest_build_id_from_libraries(&libraries, &app_id),
            );
        }
    }
    out
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
    version_snapshot_map: HashMap<String, VersionSnapshotRow>,
    live_build_id_by_profile: HashMap<String, Option<String>>,
    dep_states_by_profile: HashMap<String, Vec<PrefixDependencyStateRow>>,
    required_verbs_by_profile: HashMap<String, Vec<String>>,
    active_prefix_by_profile: HashMap<String, String>,
}

fn prefetch_batch_metadata(
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

    // Inject version mismatch as a Warning health issue (BR-6: Warning, not Error)
    let mut report = report;
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

    // Inject prefix dependency health issues from cached state
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

pub(crate) fn build_enriched_health_summary(
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

    // Persist health snapshots (fail-soft)
    for enriched in &enriched_profiles {
        if let Some(ref metadata) = enriched.metadata {
            if let Some(ref profile_id) = metadata.profile_id {
                let status_str = match enriched.core.status {
                    HealthStatus::Healthy => "healthy",
                    HealthStatus::Stale => "stale",
                    HealthStatus::Broken => "broken",
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

/// IPC-facing struct for a cached health snapshot row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedHealthSnapshot {
    pub profile_id: String,
    pub profile_name: String,
    pub status: String,
    pub issue_count: i64,
    pub checked_at: String,
}

impl From<HealthSnapshotRow> for CachedHealthSnapshot {
    fn from(row: HealthSnapshotRow) -> Self {
        CachedHealthSnapshot {
            profile_id: row.profile_id,
            profile_name: row.profile_name,
            status: row.status,
            issue_count: row.issue_count,
            checked_at: row.checked_at,
        }
    }
}

/// Returns the cached health snapshots from the last batch validation run.
///
/// Called on frontend mount to display instant badge status before the live scan
/// completes. Only returns rows for non-deleted profiles (enforced by the JOIN in
/// `load_health_snapshots`). Returns an empty list when MetadataStore is unavailable.
#[tauri::command]
pub fn get_cached_health_snapshots(
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<CachedHealthSnapshot>, String> {
    let snapshots = metadata_store
        .load_health_snapshots()
        .map_err(|e| e.to_string())?;

    Ok(snapshots
        .into_iter()
        .map(CachedHealthSnapshot::from)
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedOfflineReadinessSnapshot {
    pub profile_id: String,
    pub profile_name: String,
    pub readiness_state: String,
    pub readiness_score: i64,
    pub trainer_type: String,
    pub trainer_present: i64,
    pub trainer_hash_valid: i64,
    pub trainer_activated: i64,
    pub proton_available: i64,
    pub community_tap_cached: i64,
    pub network_required: i64,
    pub blocking_reasons: Option<String>,
    pub checked_at: String,
}

impl From<OfflineReadinessRow> for CachedOfflineReadinessSnapshot {
    fn from(row: OfflineReadinessRow) -> Self {
        CachedOfflineReadinessSnapshot {
            profile_id: row.profile_id,
            profile_name: row.profile_name,
            readiness_state: row.readiness_state,
            readiness_score: row.readiness_score,
            trainer_type: row.trainer_type,
            trainer_present: row.trainer_present,
            trainer_hash_valid: row.trainer_hash_valid,
            trainer_activated: row.trainer_activated,
            proton_available: row.proton_available,
            community_tap_cached: row.community_tap_cached,
            network_required: row.network_required,
            blocking_reasons: row.blocking_reasons,
            checked_at: row.checked_at,
        }
    }
}

#[tauri::command]
pub fn get_cached_offline_readiness_snapshots(
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<CachedOfflineReadinessSnapshot>, String> {
    let rows = metadata_store
        .load_offline_readiness_snapshot_rows()
        .map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(CachedOfflineReadinessSnapshot::from)
        .collect())
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

    // Inject prefix dependency health issues
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
            let dep_issues = build_dependency_health_issues(&dep_states, required_verbs, active_prefix);
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
