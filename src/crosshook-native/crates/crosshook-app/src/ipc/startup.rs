//! Bus-event to wire-event mapping for the startup bridge: retained terminal
//! job outcomes, Classic's legacy success events, `profiles-changed`,
//! `ui-settings-persist-failed` and `launch-session-changed`.
//!
//! [`wire_events`] is pure: a shell calls it for every [`AppEvent`] of its bus
//! subscription and forwards the returned events in order.

use serde::Serialize;

use super::sessions::session_changed_event;
use super::WireEvent;
use crate::bootstrap::{JobSnapshotKind, JobState, StartupJobKind, StartupJobs};
use crate::{AppEvent, ProfileChangeReason};

/// Terminal notification for every startup job.
pub const STARTUP_JOB_FINISHED_EVENT: &str = "startup-job-finished";
/// `profiles-changed` event name.
pub const PROFILES_CHANGED_EVENT: &str = "profiles-changed";
/// Event Classic listens for one `[ui]` persist failure per session.
pub const UI_SETTINGS_PERSIST_FAILED_EVENT: &str = "ui-settings-persist-failed";

const BRIDGED_JOBS: [StartupJobKind; 5] = [
    StartupJobKind::HealthBatch,
    StartupJobKind::OfflineReadiness,
    StartupJobKind::VersionScan,
    StartupJobKind::UmuRefresh,
    StartupJobKind::BackgroundPortal,
];

/// `startup-job-finished` payload: `{ job, state }`.
#[derive(Debug, Clone, Copy, Serialize)]
struct FinishedPayload {
    job: StartupJobKind,
    state: JobSnapshotKind,
}

/// `profiles-changed` wire payload: `{ reason: { kind, ... }, names }` (D1).
#[derive(Debug, Clone, Serialize)]
struct ProfilesChangedPayload {
    reason: ProfileChangeReason,
    names: Vec<String>,
}

/// `ui-settings-persist-failed` wire payload: `{ message }`, where `message`
/// is the already-sanitized `AppError` text.
#[derive(Debug, Clone, Serialize)]
struct UiSettingsPersistFailedPayload {
    message: String,
}

/// Conservative profile resync emitted after a bus lag (D14): views refresh
/// everything because the dropped changes are unknown.
fn profiles_resync_payload() -> ProfilesChangedPayload {
    ProfilesChangedPayload {
        reason: ProfileChangeReason::Saved,
        names: Vec::new(),
    }
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

/// Classic's legacy success event with the snapshot payload, serialized once
/// straight from the job data.
fn legacy_completion(kind: StartupJobKind, jobs: &StartupJobs) -> Option<WireEvent> {
    let name = legacy_event_name(kind)?;
    match kind {
        StartupJobKind::HealthBatch => {
            complete_data(&jobs.health).and_then(|data| WireEvent::new(name, &**data))
        }
        StartupJobKind::OfflineReadiness => {
            complete_data(&jobs.offline_readiness).and_then(|data| WireEvent::new(name, data))
        }
        StartupJobKind::VersionScan => {
            complete_data(&jobs.version_scan).and_then(|data| WireEvent::new(name, data))
        }
        StartupJobKind::UmuRefresh | StartupJobKind::BackgroundPortal => None,
    }
}

/// The legacy success event (when the job completed) followed by the generic
/// terminal notification. Serialization of one never suppresses the other.
fn finished_events(kind: StartupJobKind, jobs: &StartupJobs) -> Vec<WireEvent> {
    let mut events = Vec::with_capacity(2);
    events.extend(legacy_completion(kind, jobs));
    if let Some(payload) = finished_payload(kind, jobs) {
        events.extend(WireEvent::new(STARTUP_JOB_FINISHED_EVENT, &payload));
    }
    log_completion(kind, jobs);
    events
}

fn log_completion(kind: StartupJobKind, jobs: &StartupJobs) {
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

/// Whether `event` produces any wire emission. The named scan-completion
/// events are typed bus notifications only: the legacy events already come
/// from `StartupJobFinished`, so bridging them too would emit every payload
/// twice.
pub fn is_bridged(event: &AppEvent) -> bool {
    !matches!(
        event,
        AppEvent::HealthScanCompleted { .. }
            | AppEvent::OfflineReadinessScanCompleted { .. }
            | AppEvent::VersionScanCompleted { .. }
    )
}

/// The wire events `event` produces, in emission order. `jobs` supplies the
/// retained startup job results and is only called for events that need them.
///
/// Session changes come first (a bus lag becomes `{"kind":"resync"}` because
/// the dropped session changes are unknown), then the event's own emissions.
pub fn wire_events(event: AppEvent, jobs: impl FnOnce() -> StartupJobs) -> Vec<WireEvent> {
    if !is_bridged(&event) {
        return Vec::new();
    }
    let mut events: Vec<WireEvent> = session_changed_event(&event).into_iter().collect();
    match event {
        AppEvent::StartupJobFinished { job } => events.extend(finished_events(job, &jobs())),
        AppEvent::Resync { .. } => {
            let jobs = jobs();
            for kind in BRIDGED_JOBS {
                events.extend(finished_events(kind, &jobs));
            }
            events.extend(WireEvent::new(
                PROFILES_CHANGED_EVENT,
                &profiles_resync_payload(),
            ));
        }
        AppEvent::ProfilesChanged { reason, names } => {
            events.extend(WireEvent::new(
                PROFILES_CHANGED_EVENT,
                &ProfilesChangedPayload { reason, names },
            ));
        }
        AppEvent::UiSettingsPersistFailed { message } => {
            events.extend(WireEvent::new(
                UI_SETTINGS_PERSIST_FAILED_EVENT,
                &UiSettingsPersistFailedPayload { message },
            ));
        }
        // Session changes were mapped above. Named scan-completion events
        // never reach here: see `is_bridged`. `AppEvent` is non-exhaustive;
        // future unrelated events need no bridge.
        _ => {}
    }
    events
}

#[cfg(test)]
mod tests;
