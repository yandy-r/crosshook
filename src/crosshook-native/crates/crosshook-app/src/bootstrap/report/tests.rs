use serde_json::{json, to_string, to_value};

use super::{
    DegradedStores, FlatpakMigrationReport, FlatpakStartup, JobSnapshotKind, JobState,
    OfflineReadinessScanSummary, OnboardingStartup, StartupJobs, StartupReport, UmuRefreshSummary,
    VersionScanSummary,
};
use crate::bootstrap::FlatpakMode;
use crate::services::background::BackgroundProtectionState;
use crosshook_core::metadata::MetadataStatus;

fn all_pending_report() -> StartupReport {
    StartupReport {
        auto_load_profile: None,
        onboarding: OnboardingStartup {
            show: false,
            has_profiles: false,
        },
        flatpak: FlatpakStartup {
            mode: FlatpakMode::Native,
            migration: None,
        },
        degraded: DegradedStores {
            metadata: MetadataStatus::Ok,
            metadata_error: None,
            settings_unreadable: false,
            logging_unavailable: false,
        },
        jobs: StartupJobs {
            health: JobState::Pending,
            offline_readiness: JobState::Pending,
            version_scan: JobState::Pending,
            umu_refresh: JobState::Pending,
            background_portal: JobState::Pending,
        },
    }
}

/// The exact JSON the frontend contract (`src/types/startup.ts`) promises for
/// a fresh report: snake_case keys, `JobState` tagged `"state"`, and
/// `degraded.metadata` mirroring `MetadataStatus`.
#[test]
fn all_pending_report_serializes_the_exact_contract_shape() {
    let report = all_pending_report();
    assert_eq!(
        to_value(&report).unwrap(),
        json!({
            "auto_load_profile": null,
            "onboarding": { "show": false, "has_profiles": false },
            "flatpak": { "mode": "native", "migration": null },
            "degraded": {
                "metadata": { "state": "ok" },
                "metadata_error": null,
                "settings_unreadable": false,
                "logging_unavailable": false
            },
            "jobs": {
                "health": { "state": "pending" },
                "offline_readiness": { "state": "pending" },
                "version_scan": { "state": "pending" },
                "umu_refresh": { "state": "pending" },
                "background_portal": { "state": "pending" }
            }
        })
    );
}

/// The stable serialized sample shared with the shell bridge (S2) and the
/// frontend tests: one canonical string, byte-for-byte.
#[test]
fn all_pending_report_serializes_to_the_canonical_string() {
    assert_eq!(
        to_string(&all_pending_report()).unwrap(),
        "{\"auto_load_profile\":null,\
         \"onboarding\":{\"show\":false,\"has_profiles\":false},\
         \"flatpak\":{\"mode\":\"native\",\"migration\":null},\
         \"degraded\":{\"metadata\":{\"state\":\"ok\"},\"metadata_error\":null,\
         \"settings_unreadable\":false,\"logging_unavailable\":false},\
         \"jobs\":{\"health\":{\"state\":\"pending\"},\
         \"offline_readiness\":{\"state\":\"pending\"},\
         \"version_scan\":{\"state\":\"pending\"},\
         \"umu_refresh\":{\"state\":\"pending\"},\
         \"background_portal\":{\"state\":\"pending\"}}}"
    );
}

#[test]
fn job_state_lifecycle_variants_serialize_tagged_state() {
    assert_eq!(
        to_value(JobState::<u8>::Pending).unwrap(),
        json!({"state": "pending"})
    );
    assert_eq!(
        to_value(JobState::<u8>::Failed {
            message: "metadata unavailable".into()
        })
        .unwrap(),
        json!({"state": "failed", "message": "metadata unavailable"})
    );
    assert_eq!(
        to_value(JobState::<u8>::Skipped {
            reason: "CROSSHOOK_BENCH=1".into()
        })
        .unwrap(),
        json!({"state": "skipped", "reason": "CROSSHOOK_BENCH=1"})
    );
}

#[test]
fn job_state_complete_carries_data_payload() {
    assert_eq!(
        to_value(JobState::Complete {
            data: OfflineReadinessScanSummary { total_profiles: 3 }
        })
        .unwrap(),
        json!({"state": "complete", "data": {"total_profiles": 3}})
    );
    assert_eq!(
        to_value(JobState::Complete {
            data: BackgroundProtectionState::NotApplicable
        })
        .unwrap(),
        json!({"state": "complete", "data": "not_applicable"})
    );
}

#[test]
fn job_snapshot_kind_serializes_snake_case() {
    assert_eq!(
        to_value(JobSnapshotKind::Pending).unwrap(),
        json!("pending")
    );
    assert_eq!(
        to_value(JobSnapshotKind::Running).unwrap(),
        json!("running")
    );
    assert_eq!(
        to_value(JobSnapshotKind::Complete).unwrap(),
        json!("complete")
    );
    assert_eq!(to_value(JobSnapshotKind::Failed).unwrap(), json!("failed"));
    assert_eq!(
        to_value(JobSnapshotKind::Skipped).unwrap(),
        json!("skipped")
    );
}

#[test]
fn flatpak_mode_serializes_snake_case() {
    assert_eq!(to_value(FlatpakMode::Native).unwrap(), json!("native"));
    assert_eq!(to_value(FlatpakMode::Isolated).unwrap(), json!("isolated"));
    assert_eq!(
        to_value(FlatpakMode::HostXdgShared).unwrap(),
        json!("host_xdg_shared")
    );
}

#[test]
fn flatpak_migration_report_serializes_every_key() {
    let report = FlatpakMigrationReport {
        imported_config: true,
        imported_subtrees: vec!["crosshook/community".into()],
        skipped_subtrees: vec!["crosshook/prefixes".into()],
        error: Some("migration failed".into()),
    };
    assert_eq!(
        to_value(&report).unwrap(),
        json!({
            "imported_config": true,
            "imported_subtrees": ["crosshook/community"],
            "skipped_subtrees": ["crosshook/prefixes"],
            "error": "migration failed"
        })
    );
    // Success carries an explicit null, keeping the key set unconditional.
    assert_eq!(
        to_value(FlatpakMigrationReport {
            imported_config: false,
            imported_subtrees: Vec::new(),
            skipped_subtrees: Vec::new(),
            error: None,
        })
        .unwrap(),
        json!({
            "imported_config": false,
            "imported_subtrees": [],
            "skipped_subtrees": [],
            "error": null
        })
    );
}

#[test]
fn flatpak_startup_nests_mode_and_migration() {
    let startup = FlatpakStartup {
        mode: FlatpakMode::HostXdgShared,
        migration: Some(FlatpakMigrationReport {
            imported_config: false,
            imported_subtrees: Vec::new(),
            skipped_subtrees: Vec::new(),
            error: None,
        }),
    };
    assert_eq!(
        to_value(&startup).unwrap(),
        json!({
            "mode": "host_xdg_shared",
            "migration": {
                "imported_config": false,
                "imported_subtrees": [],
                "skipped_subtrees": [],
                "error": null
            }
        })
    );
}

#[test]
fn degraded_stores_mirror_the_metadata_status_ipc_shape() {
    let degraded = DegradedStores {
        metadata: MetadataStatus::NewerSchema {
            found: 22,
            supported: 21,
        },
        metadata_error: Some("metadata schema newer than supported".into()),
        settings_unreadable: true,
        logging_unavailable: false,
    };
    assert_eq!(
        to_value(&degraded).unwrap(),
        json!({
            "metadata": { "state": "newer_schema", "found": 22, "supported": 21 },
            "metadata_error": "metadata schema newer than supported",
            "settings_unreadable": true,
            "logging_unavailable": false
        })
    );
}

#[test]
fn summary_payloads_serialize_the_legacy_event_shapes() {
    assert_eq!(
        to_value(OfflineReadinessScanSummary { total_profiles: 6 }).unwrap(),
        json!({ "total_profiles": 6 })
    );
    assert_eq!(
        to_value(VersionScanSummary {
            scanned: 4,
            mismatches: 1
        })
        .unwrap(),
        json!({ "scanned": 4, "mismatches": 1 })
    );
    assert_eq!(
        to_value(UmuRefreshSummary {
            refreshed: false,
            cached_at: None,
            reason: "CROSSHOOK_BENCH=1".into()
        })
        .unwrap(),
        json!({ "refreshed": false, "cached_at": null, "reason": "CROSSHOOK_BENCH=1" })
    );
}

/// Completed job data accumulates into the snapshot, so a late subscriber
/// reading a fresh report still sees every finished result.
#[test]
fn completed_jobs_accumulate_into_the_snapshot_json() {
    let mut report = all_pending_report();
    report.auto_load_profile = Some("cyberpunk".into());
    report.onboarding = OnboardingStartup {
        show: true,
        has_profiles: false,
    };
    report.jobs.offline_readiness = JobState::Complete {
        data: OfflineReadinessScanSummary { total_profiles: 2 },
    };
    report.jobs.version_scan = JobState::Complete {
        data: VersionScanSummary {
            scanned: 2,
            mismatches: 0,
        },
    };
    report.jobs.umu_refresh = JobState::Skipped {
        reason: "CROSSHOOK_BENCH=1".into(),
    };
    let value = to_value(&report).unwrap();
    assert_eq!(value["auto_load_profile"], json!("cyberpunk"));
    assert_eq!(
        value["onboarding"],
        json!({ "show": true, "has_profiles": false })
    );
    assert_eq!(
        value["jobs"]["offline_readiness"],
        json!({ "state": "complete", "data": { "total_profiles": 2 } })
    );
    assert_eq!(
        value["jobs"]["version_scan"],
        json!({ "state": "complete", "data": { "scanned": 2, "mismatches": 0 } })
    );
    assert_eq!(
        value["jobs"]["umu_refresh"],
        json!({ "state": "skipped", "reason": "CROSSHOOK_BENCH=1" })
    );
    // Untouched slots keep their pending state.
    assert_eq!(value["jobs"]["health"], json!({ "state": "pending" }));
    assert_eq!(
        value["jobs"]["background_portal"],
        json!({ "state": "pending" })
    );
}
