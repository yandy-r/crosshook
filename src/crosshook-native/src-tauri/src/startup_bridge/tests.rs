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

#[test]
fn ui_settings_persist_failed_payload_is_an_object_with_message() {
    let payload = UiSettingsPersistFailedPayload {
        message: "could not save settings".to_owned(),
    };
    assert_eq!(
        serde_json::to_value(payload).unwrap(),
        json!({"message": "could not save settings"})
    );
}

#[test]
fn ui_settings_persist_failed_uses_classic_event_name() {
    assert_eq!(
        UI_SETTINGS_PERSIST_FAILED_EVENT,
        "ui-settings-persist-failed"
    );
}

#[test]
fn named_scan_completion_events_are_not_bridged() {
    for event in [
        AppEvent::HealthScanCompleted {
            total: 3,
            healthy: 1,
            stale: 1,
            broken: 1,
        },
        AppEvent::OfflineReadinessScanCompleted { total_profiles: 3 },
        AppEvent::VersionScanCompleted {
            scanned: 3,
            mismatches: 1,
        },
    ] {
        assert!(!is_bridged(&event), "{event:?} would double-emit");
    }
    for event in [
        AppEvent::StartupJobFinished {
            job: StartupJobKind::HealthBatch,
        },
        AppEvent::Resync { missed: 1 },
        AppEvent::ProfilesChanged {
            reason: ProfileChangeReason::Saved,
            names: vec![],
        },
        AppEvent::UiSettingsPersistFailed {
            message: "x".into(),
        },
        AppEvent::SessionStarted {
            session_id: serde_json::from_value(json!("550e8400-e29b-41d4-a716-446655440000"))
                .unwrap(),
            profile_name: None,
            kind: crosshook_core::launch::SessionKind::Game,
            seq: 1,
        },
    ] {
        assert!(is_bridged(&event), "{event:?} must stay bridged");
    }
}

/// The bridge loop emits `launch-session-changed` for every bridged event
/// `session_payload` maps: starts and exits carry their wire payload and a
/// bus lag becomes the resync marker; unrelated events emit nothing.
#[test]
fn session_events_and_resync_map_to_launch_session_changed_payloads() {
    let id = "550e8400-e29b-41d4-a716-446655440000";
    let session_id: crosshook_core::launch::SessionId = serde_json::from_value(json!(id)).unwrap();
    let mapped = |event: AppEvent| -> Option<Value> {
        assert!(is_bridged(&event), "{event:?} must reach the bridge loop");
        session_payload(&event).map(|payload| serde_json::to_value(payload).unwrap())
    };

    assert_eq!(
        mapped(AppEvent::SessionStarted {
            session_id,
            profile_name: Some("Alpha".into()),
            kind: crosshook_core::launch::SessionKind::Game,
            seq: 1,
        }),
        Some(json!({
            "kind": "started",
            "session_id": id,
            "profile_name": "Alpha",
            "session_kind": "game",
            "seq": 1,
        }))
    );
    assert_eq!(
        mapped(AppEvent::SessionExited {
            session_id,
            profile_name: None,
            kind: crosshook_core::launch::SessionKind::Trainer,
            code: Some(1),
            signal: None,
            seq: 2,
        }),
        Some(json!({
            "kind": "exited",
            "session_id": id,
            "profile_name": null,
            "session_kind": "trainer",
            "code": 1,
            "signal": null,
            "seq": 2,
        }))
    );
    assert_eq!(
        mapped(AppEvent::Resync { missed: 4 }),
        Some(json!({ "kind": "resync" }))
    );
    assert_eq!(
        mapped(AppEvent::StartupJobFinished {
            job: StartupJobKind::HealthBatch,
        }),
        None
    );
    assert_eq!(
        mapped(AppEvent::ProfilesChanged {
            reason: ProfileChangeReason::Saved,
            names: vec![],
        }),
        None
    );
}
