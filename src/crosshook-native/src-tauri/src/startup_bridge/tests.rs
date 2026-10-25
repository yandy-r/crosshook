use super::*;
use crosshook_app::bootstrap::{
    OfflineReadinessScanSummary, UmuRefreshSummary, VersionScanSummary,
};
use crosshook_app::services::health::EnrichedHealthSummary;
use crosshook_app::services::BackgroundProtectionState;
use serde_json::{json, Value};
use std::sync::Arc;

fn jobs() -> StartupJobs {
    StartupJobs {
        health: JobState::Pending,
        offline_readiness: JobState::Pending,
        version_scan: JobState::Pending,
        umu_refresh: JobState::Pending,
        background_portal: JobState::Pending,
    }
}

#[test]
fn completed_jobs_keep_legacy_names_and_payloads() {
    let mut jobs = jobs();
    jobs.health = JobState::Complete {
        data: Arc::new(EnrichedHealthSummary {
            profiles: vec![],
            healthy_count: 1,
            stale_count: 2,
            broken_count: 3,
            total_count: 6,
            validated_at: "now".into(),
        }),
    };
    jobs.offline_readiness = JobState::Complete {
        data: OfflineReadinessScanSummary { total_profiles: 6 },
    };
    jobs.version_scan = JobState::Complete {
        data: VersionScanSummary {
            scanned: 6,
            mismatches: 2,
        },
    };
    jobs.umu_refresh = JobState::Complete {
        data: UmuRefreshSummary {
            refreshed: true,
            cached_at: None,
            reason: "refreshed".into(),
        },
    };
    jobs.background_portal = JobState::Complete {
        data: BackgroundProtectionState::NotApplicable,
    };
    assert_eq!(
        legacy_event_name(StartupJobKind::HealthBatch),
        Some("profile-health-batch-complete")
    );
    let health = complete_data(&jobs.health).unwrap();
    assert_eq!(
        serde_json::to_value(&**health).unwrap(),
        json!({"profiles": [], "healthy_count": 1, "stale_count": 2,
        "broken_count": 3, "total_count": 6, "validated_at": "now"})
    );
    assert_eq!(
        legacy_event_name(StartupJobKind::OfflineReadiness),
        Some("offline-readiness-scan-complete")
    );
    assert_eq!(
        complete_data(&jobs.offline_readiness).map(|data| serde_json::to_value(data).unwrap()),
        Some(json!({"total_profiles": 6}))
    );
    assert_eq!(
        legacy_event_name(StartupJobKind::VersionScan),
        Some("version-scan-complete")
    );
    assert_eq!(
        complete_data(&jobs.version_scan).map(|data| serde_json::to_value(data).unwrap()),
        Some(json!({"scanned": 6, "mismatches": 2}))
    );
    assert_eq!(
        BRIDGED_JOBS
            .iter()
            .filter(|kind| legacy_event_name(**kind).is_some())
            .count(),
        3
    );
    let payloads: Vec<Value> = BRIDGED_JOBS
        .iter()
        .map(|kind| serde_json::to_value(finished_payload(*kind, &jobs).unwrap()).unwrap())
        .collect();
    assert_eq!(
        payloads,
        vec![
            json!({"job": "health_batch", "state": "complete"}),
            json!({"job": "offline_readiness", "state": "complete"}),
            json!({"job": "version_scan", "state": "complete"}),
            json!({"job": "umu_refresh", "state": "complete"}),
            json!({"job": "background_portal", "state": "complete"}),
        ]
    );
}

#[test]
fn all_jobs_report_failed_and_skipped_outcomes_on_resync() {
    let mut jobs = jobs();
    jobs.health = JobState::Failed {
        message: "unavailable".into(),
    };
    jobs.offline_readiness = JobState::Skipped {
        reason: "cancelled".into(),
    };
    jobs.version_scan = JobState::Failed {
        message: "unavailable".into(),
    };
    jobs.umu_refresh = JobState::Skipped {
        reason: "bench".into(),
    };
    jobs.background_portal = JobState::Failed {
        message: "unavailable".into(),
    };
    let payloads: Vec<Value> = BRIDGED_JOBS
        .iter()
        .map(|kind| {
            // Failed/skipped jobs carry no legacy success payload.
            let has_payload = match kind {
                StartupJobKind::HealthBatch => complete_data(&jobs.health).is_some(),
                StartupJobKind::OfflineReadiness => {
                    complete_data(&jobs.offline_readiness).is_some()
                }
                StartupJobKind::VersionScan => complete_data(&jobs.version_scan).is_some(),
                StartupJobKind::UmuRefresh | StartupJobKind::BackgroundPortal => false,
            };
            assert!(!has_payload);
            serde_json::to_value(finished_payload(*kind, &jobs).unwrap()).unwrap()
        })
        .collect();
    assert_eq!(
        payloads,
        vec![
            json!({"job": "health_batch", "state": "failed"}),
            json!({"job": "offline_readiness", "state": "skipped"}),
            json!({"job": "version_scan", "state": "failed"}),
            json!({"job": "umu_refresh", "state": "skipped"}),
            json!({"job": "background_portal", "state": "failed"}),
        ]
    );
}

#[test]
fn pending_and_running_jobs_emit_nothing() {
    let mut jobs = jobs();
    for state in [JobState::Pending, JobState::Running] {
        jobs.health = state;
        assert!(finished_payload(StartupJobKind::HealthBatch, &jobs).is_none());
        assert!(complete_data(&jobs.health).is_none());
    }
    assert!(BRIDGED_JOBS
        .iter()
        .all(|kind| finished_payload(*kind, &jobs).is_none()));
}

#[test]
fn profiles_changed_payload_is_an_object_with_tagged_reason() {
    let renamed = ProfilesChangedPayload {
        reason: ProfileChangeReason::Renamed {
            from: "Alpha".into(),
        },
        names: vec!["Beta".into()],
    };
    assert_eq!(
        serde_json::to_value(renamed).unwrap(),
        json!({"reason": {"kind": "renamed", "from": "Alpha"}, "names": ["Beta"]})
    );
    let favorite = ProfilesChangedPayload {
        reason: ProfileChangeReason::FavoriteToggled,
        names: vec!["Alpha".into()],
    };
    assert_eq!(
        serde_json::to_value(favorite).unwrap(),
        json!({"reason": {"kind": "favorite_toggled"}, "names": ["Alpha"]})
    );
}

#[test]
fn resync_emits_conservative_saved_with_no_names() {
    assert_eq!(
        serde_json::to_value(profiles_resync_payload()).unwrap(),
        json!({"reason": {"kind": "saved"}, "names": []})
    );
}
