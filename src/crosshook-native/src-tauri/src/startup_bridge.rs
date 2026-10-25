use crosshook_app::bootstrap::{
    JobSnapshotKind, JobState, StartupHandle, StartupJobKind, StartupJobs,
};
use crosshook_app::{AppEvent, EventReceiver};
use serde::Serialize;
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

/// Success payload of a completed job, borrowed from the snapshot.
fn complete_data<T>(state: &JobState<T>) -> Option<&T> {
    match state {
        JobState::Complete { data } => Some(data),
        JobState::Pending
        | JobState::Running
        | JobState::Failed { .. }
        | JobState::Skipped { .. } => None,
    }
}

/// Classic legacy event name carrying the job's success payload, if any.
fn legacy_event_name(kind: StartupJobKind) -> Option<&'static str> {
    match kind {
        StartupJobKind::HealthBatch => Some("profile-health-batch-complete"),
        StartupJobKind::OfflineReadiness => Some("offline-readiness-scan-complete"),
        StartupJobKind::VersionScan => Some("version-scan-complete"),
        StartupJobKind::UmuRefresh | StartupJobKind::BackgroundPortal => None,
    }
}

/// Emits Classic's legacy success event with the snapshot payload, serialized
/// once straight from the job data.
fn emit_legacy_completion(handle: &AppHandle, kind: StartupJobKind, jobs: &StartupJobs) {
    // An emission error must not suppress the fallback signal to job consumers.
    let name = match legacy_event_name(kind) {
        Some(name) => name,
        None => return,
    };
    let result = match kind {
        StartupJobKind::HealthBatch => {
            complete_data(&jobs.health).map(|data| handle.emit(name, &**data))
        }
        StartupJobKind::OfflineReadiness => {
            complete_data(&jobs.offline_readiness).map(|data| handle.emit(name, data))
        }
        StartupJobKind::VersionScan => {
            complete_data(&jobs.version_scan).map(|data| handle.emit(name, data))
        }
        StartupJobKind::UmuRefresh | StartupJobKind::BackgroundPortal => None,
    };
    if let Some(Err(error)) = result {
        tracing::warn!(%error, event = name, "failed to emit startup completion event");
    }
}

fn emit_finished(handle: &AppHandle, kind: StartupJobKind, jobs: &StartupJobs) {
    // Legacy success payload precedes the generic terminal notification. An
    // emission error must not suppress the fallback signal to job consumers.
    emit_legacy_completion(handle, kind, jobs);
    if let Some(payload) = finished_payload(kind, jobs) {
        if let Err(error) = handle.emit("startup-job-finished", payload) {
            tracing::warn!(%error, "failed to emit startup terminal event");
        }
    }
    match kind {
        StartupJobKind::HealthBatch => {
            if let Some(data) = complete_data(&jobs.health) {
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
            if complete_data(&jobs.offline_readiness).is_some() {
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
