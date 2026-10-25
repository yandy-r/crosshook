use super::*;

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
fn updates_determine_their_own_job_and_watch_only_terminal_kinds() {
    let mut jobs = jobs();
    let all = [
        StartupJobKind::HealthBatch,
        StartupJobKind::OfflineReadiness,
        StartupJobKind::VersionScan,
        StartupJobKind::UmuRefresh,
        StartupJobKind::BackgroundPortal,
    ];
    for job in all {
        assert_eq!(apply(&mut jobs, JobUpdate::running(job)), job);
        assert_eq!(terminal_kind(job, &JobKinds::from(&jobs)), None);
        assert_eq!(apply(&mut jobs, JobUpdate::cancelled(job)), job);
        assert_eq!(
            terminal_kind(job, &JobKinds::from(&jobs)),
            Some(JobSnapshotKind::Skipped)
        );
        assert_eq!(
            apply(
                &mut jobs,
                JobUpdate::start_failed(job, "unavailable".into())
            ),
            job
        );
        assert_eq!(
            terminal_kind(job, &JobKinds::from(&jobs)),
            Some(JobSnapshotKind::Failed)
        );
    }
    assert!(std::mem::size_of::<JobKinds>() <= 5 * std::mem::size_of::<JobSnapshotKind>());
}

#[test]
fn snapshots_share_health_result_instead_of_copying_profiles() {
    let mut jobs = jobs();
    apply(
        &mut jobs,
        JobUpdate::health_complete(EnrichedHealthSummary {
            profiles: vec![],
            healthy_count: 0,
            stale_count: 0,
            broken_count: 0,
            total_count: 0,
            validated_at: "now".into(),
        }),
    );
    let cloned = jobs.clone();
    match (&jobs.health, &cloned.health) {
        (JobState::Complete { data: original }, JobState::Complete { data: cloned }) => {
            assert!(Arc::ptr_eq(original, cloned));
        }
        _ => panic!("health must be complete"),
    }
    assert_eq!(
        terminal_kind(StartupJobKind::HealthBatch, &JobKinds::from(&jobs)),
        Some(JobSnapshotKind::Complete)
    );
}
