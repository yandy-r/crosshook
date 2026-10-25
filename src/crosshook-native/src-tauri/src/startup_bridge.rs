use crosshook_app::bootstrap::{
    JobSnapshotKind, JobState, StartupHandle, StartupJobKind, StartupJobs,
};
use crosshook_app::{AppEvent, EventReceiver};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter};

const BRIDGED_JOBS: [StartupJobKind; 5] = [
    StartupJobKind::HealthBatch,
    StartupJobKind::OfflineReadiness,
    StartupJobKind::VersionScan,
    StartupJobKind::UmuRefresh,
    StartupJobKind::BackgroundPortal,
];

#[derive(Debug, Clone, Copy, Serialize)]
struct FinishedPayload {
    job: StartupJobKind,
    state: JobSnapshotKind,
}

fn finished_payload(job: StartupJobKind, jobs: &StartupJobs) -> Option<FinishedPayload> {
    let state = match job {
        StartupJobKind::HealthBatch => jobs.health.kind(),
        StartupJobKind::OfflineReadiness => jobs.offline_readiness.kind(),
        StartupJobKind::VersionScan => jobs.version_scan.kind(),
        StartupJobKind::UmuRefresh => jobs.umu_refresh.kind(),
        StartupJobKind::BackgroundPortal => jobs.background_portal.kind(),
    };
    match state {
        JobSnapshotKind::Pending | JobSnapshotKind::Running => None,
        JobSnapshotKind::Complete | JobSnapshotKind::Failed | JobSnapshotKind::Skipped => {
            Some(FinishedPayload { job, state })
        }
    }
}

fn complete_payload<T: Serialize>(state: &JobState<T>) -> Option<Value> {
    match state {
        JobState::Complete { data } => serde_json::to_value(data).ok(),
        JobState::Pending
        | JobState::Running
        | JobState::Failed { .. }
        | JobState::Skipped { .. } => None,
    }
}

fn event_for(kind: StartupJobKind, jobs: &StartupJobs) -> Option<(&'static str, Value)> {
    let (name, payload) = match kind {
        StartupJobKind::HealthBatch => (
            "profile-health-batch-complete",
            complete_payload(&jobs.health),
        ),
        StartupJobKind::OfflineReadiness => (
            "offline-readiness-scan-complete",
            complete_payload(&jobs.offline_readiness),
        ),
        StartupJobKind::VersionScan => (
            "version-scan-complete",
            complete_payload(&jobs.version_scan),
        ),
        StartupJobKind::UmuRefresh | StartupJobKind::BackgroundPortal => return None,
    };
    payload.map(|payload| (name, payload))
}

fn emit_finished(handle: &AppHandle, kind: StartupJobKind, jobs: &StartupJobs) {
    // Legacy success payload precedes the generic terminal notification. An
    // emission error must not suppress the fallback signal to job consumers.
    if let Some((name, payload)) = event_for(kind, jobs) {
        if let Err(error) = handle.emit(name, payload) {
            tracing::warn!(%error, event = name, "failed to emit startup completion event");
        }
    }
    if let Some(payload) = finished_payload(kind, jobs) {
        if let Err(error) = handle.emit("startup-job-finished", payload) {
            tracing::warn!(%error, "failed to emit startup terminal event");
        }
    }
    match kind {
        StartupJobKind::HealthBatch => {
            if let JobState::Complete { data } = &jobs.health {
                tracing::info!(
                    total = data.total_count,
                    healthy = data.healthy_count,
                    stale = data.stale_count,
                    broken = data.broken_count,
                    "startup health scan complete"
                );
            }
        }
        StartupJobKind::OfflineReadiness => {
            if let JobState::Complete { .. } = jobs.offline_readiness {
                tracing::info!("startup offline readiness scan complete");
            }
        }
        StartupJobKind::VersionScan
        | StartupJobKind::UmuRefresh
        | StartupJobKind::BackgroundPortal => {}
    }
}

/// Bridges retained terminal outcomes and Classic's legacy success events.
/// Holds only a startup handle and receiver, never an application clone.
pub(crate) fn spawn(handle: AppHandle, startup: StartupHandle, mut rx: EventReceiver) {
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                AppEvent::StartupJobFinished { job } => {
                    emit_finished(&handle, job, &startup.jobs_snapshot());
                }
                AppEvent::Resync { .. } => {
                    let jobs = startup.jobs_snapshot();
                    for kind in BRIDGED_JOBS {
                        emit_finished(&handle, kind, &jobs);
                    }
                }
                // AppEvent is non-exhaustive; future unrelated events need no bridge.
                _ => {}
            }
        }
    });
}

#[cfg(test)]
mod tests;
