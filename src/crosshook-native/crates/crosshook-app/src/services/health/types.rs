use crosshook_core::metadata::{DriftState, HealthSnapshotRow, OfflineReadinessRow};
use crosshook_core::offline::OfflineReadinessReport;
use crosshook_core::profile::health::{HealthIssue, ProfileHealthReport};
use serde::{Deserialize, Serialize};

/// Persisted launch and version metadata enriching one health report.
///
/// Present only when the metadata store is available; otherwise the report's
/// `metadata` is `None`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileHealthMetadata {
    /// Stable metadata-store id for the profile, if it has been registered.
    pub profile_id: Option<String>,
    /// Timestamp of the profile's most recent successful launch, if any.
    pub last_success: Option<String>,
    /// Number of failed launches recorded in the last 30 days.
    pub failure_count_30d: i64,
    /// Total launches recorded for the profile.
    pub total_launches: i64,
    /// Whether the exported launcher still matches the profile, if one exists.
    pub launcher_drift_state: Option<DriftState>,
    /// True when the profile originated from a community import.
    pub is_community_import: bool,
    /// True when the user marked the profile as a favorite.
    pub is_favorite: bool,
    /// Correlation between the stored version snapshot and the current game and
    /// trainer (for example `game_updated`), if a snapshot exists.
    pub version_status: Option<String>,
    /// Steam build id captured in the last version snapshot.
    pub snapshot_build_id: Option<String>,
    /// Steam build id read from the installed app manifest right now.
    pub current_build_id: Option<String>,
    /// Trainer version captured in the last version snapshot.
    pub trainer_version: Option<String>,
}

/// Compact offline readiness details accompanying a profile health report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineReadinessBrief {
    /// Name of the profile that was scored.
    pub profile_name: String,
    /// Readiness score, 0 (not ready) to 100 (fully ready offline).
    pub score: u8,
    /// Readiness state label computed for the profile (for example `ready`).
    pub readiness_state: String,
    /// Kind of trainer the score was computed for.
    pub trainer_type: String,
    /// Human-readable reasons that cap or block offline readiness.
    pub blocking_reasons: Vec<String>,
    /// Timestamp of the check that produced this brief.
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

/// Health validation with optional metadata and offline readiness enrichment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedProfileHealthReport {
    /// Path and field validation result from `crosshook-core`, flattened into
    /// the serialized object.
    #[serde(flatten)]
    pub core: ProfileHealthReport,
    /// Launch history and version data; `None` when the metadata store is
    /// unavailable.
    pub metadata: Option<ProfileHealthMetadata>,
    /// Cached offline readiness for the profile; omitted when none is cached.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offline_readiness: Option<OfflineReadinessBrief>,
}

/// Batch health counts and enriched per-profile reports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedHealthSummary {
    /// One enriched report per validated profile.
    pub profiles: Vec<EnrichedProfileHealthReport>,
    /// Number of profiles whose status is healthy.
    pub healthy_count: usize,
    /// Number of profiles whose status is stale.
    pub stale_count: usize,
    /// Number of profiles whose status is broken.
    pub broken_count: usize,
    /// Number of profiles validated.
    pub total_count: usize,
    /// Timestamp of when the batch validation ran.
    pub validated_at: String,
}

/// IPC-facing struct for a cached health snapshot row.
///
/// Holds the result of the last completed batch validation, so the UI can show
/// health before a fresh validation finishes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedHealthSnapshot {
    /// Metadata-store id of the profile.
    pub profile_id: String,
    /// Display name of the profile.
    pub profile_name: String,
    /// Health status recorded for the profile in that validation.
    pub status: String,
    /// Number of issues found in that validation.
    pub issue_count: i64,
    /// Timestamp of the validation that produced this row.
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

/// Cached offline readiness factors from the last completed scan.
///
/// Integer factor fields are stored as SQLite integers: `0` means false and
/// non-zero means true.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedOfflineReadinessSnapshot {
    /// Metadata-store id of the profile.
    pub profile_id: String,
    /// Display name of the profile.
    pub profile_name: String,
    /// Readiness state label computed for the profile (for example `ready`).
    pub readiness_state: String,
    /// Readiness score, 0 (not ready) to 100 (fully ready offline).
    pub readiness_score: i64,
    /// Kind of trainer the score was computed for.
    pub trainer_type: String,
    /// Whether the trainer executable exists on disk.
    pub trainer_present: i64,
    /// Whether the trainer file hash matches the recorded hash.
    pub trainer_hash_valid: i64,
    /// Whether the trainer has been activated for offline use.
    pub trainer_activated: i64,
    /// Whether the selected Proton runtime is installed.
    pub proton_available: i64,
    /// Whether the community tap providing the profile has a local clone.
    pub community_tap_cached: i64,
    /// Whether launching requires a network connection.
    pub network_required: i64,
    /// Blocking reasons stored with the snapshot, if any.
    pub blocking_reasons: Option<String>,
    /// Timestamp of the scan that produced this row.
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
            issue.path = crate::presenter::display_path_tilde(&issue.path);
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
