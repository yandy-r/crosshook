use crosshook_core::metadata::{DriftState, HealthSnapshotRow, OfflineReadinessRow};
use crosshook_core::offline::OfflineReadinessReport;
use crosshook_core::profile::health::{HealthIssue, ProfileHealthReport};
use serde::{Deserialize, Serialize};

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

pub(super) fn sanitize_issues(issues: Vec<HealthIssue>) -> Vec<HealthIssue> {
    issues
        .into_iter()
        .map(|mut issue| {
            issue.path = crate::commands::shared::sanitize_display_path(&issue.path);
            issue
        })
        .collect()
}

pub(super) fn sanitize_report(report: ProfileHealthReport) -> ProfileHealthReport {
    ProfileHealthReport {
        issues: sanitize_issues(report.issues),
        ..report
    }
}
