//! AC4: only the owner of an install can cancel it, and only installs.

use std::sync::Arc;

use tokio::sync::Notify;

use super::super::super::messages::MSG_OP_NOT_FOUND;
use super::super::super::progress::next_progress;
use super::super::helpers::{finished, owner, request, Harness, Mode};
use crate::jobs::{JobEventKind, JobEvents, JobId, JobSpec, Outcome};
use crate::AppErrorKind;
use crosshook_core::protonup::progress::Phase;

fn catalog_entry() -> crosshook_core::protonup::ProtonUpAvailableVersion {
    super::super::helpers::ge_catalog().versions.remove(0)
}

fn gated() -> (Harness, Arc<Notify>) {
    let gate = Arc::new(Notify::new());
    (Harness::new(Mode::Gated(Arc::clone(&gate))), gate)
}

async fn start(harness: &Harness, label: &str) -> super::super::super::ProtonInstallStart {
    match harness
        .service
        .start_install(owner(label), request("GE-Proton9-21"), catalog_entry())
        .await
    {
        Ok(started) => started,
        Err(error) => panic!("start_install failed: {}", error.message()),
    }
}

/// Drains `events` to the job's `Finished` outcome.
async fn outcome_of(events: &mut JobEvents) -> Outcome {
    while let Some(event) = events.recv().await {
        if let JobEventKind::Finished(outcome) = event.kind {
            return outcome;
        }
    }
    panic!("job ended without a Finished event");
}

#[tokio::test]
async fn foreign_owner_cannot_cancel_and_the_install_completes() {
    let (harness, gate) = gated();
    let started = start(&harness, "main").await;
    let id = JobId::from_string(started.op_id.clone());
    let mut events = harness.jobs.subscribe(&id).unwrap();

    let error = harness
        .service
        .cancel_install(&owner("other"), &started.op_id)
        .unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::NotFound);
    assert_eq!(error.message(), MSG_OP_NOT_FOUND);

    // The install token is untouched: releasing the gate completes it.
    gate.notify_one();
    assert_eq!(outcome_of(&mut events).await, Outcome::Succeeded);
    finished(&harness.jobs).await;
}

#[tokio::test]
async fn unrelated_job_ids_are_rejected_and_the_job_is_untouched() {
    let (harness, _gate) = gated();
    let release = Arc::new(Notify::new());
    let job_gate = Arc::clone(&release);
    let handle = harness
        .jobs
        .start(JobSpec::new("unrelated"), move |_ctx| async move {
            job_gate.notified().await;
            Outcome::Succeeded
        })
        .unwrap();

    let error = harness
        .service
        .cancel_install(&owner("main"), handle.id().as_str())
        .unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::NotFound);
    assert_eq!(error.message(), MSG_OP_NOT_FOUND);
    assert!(!handle.cancel_token().is_cancelled());

    let id = handle.id().clone();
    let mut events = handle.into_events();
    release.notify_one();
    assert_eq!(outcome_of(&mut events).await, Outcome::Succeeded);
    harness.jobs.reap(&id).unwrap();
}

#[tokio::test]
async fn unknown_and_empty_ids_are_rejected() {
    let harness = Harness::new(Mode::Succeed);
    for op_id in ["", "not-a-job", "00000000-0000-0000-0000-000000000000"] {
        let error = harness
            .service
            .cancel_install(&owner("main"), op_id)
            .unwrap_err();
        assert_eq!(error.kind(), AppErrorKind::NotFound);
        assert_eq!(error.message(), MSG_OP_NOT_FOUND);
    }
}

#[tokio::test]
async fn owner_cancel_stops_the_install_and_reports_cancelled() {
    let (harness, _gate) = gated();
    let mut started = start(&harness, "main").await;
    let id = JobId::from_string(started.op_id.clone());
    let mut events = harness.jobs.subscribe(&id).unwrap();

    harness
        .service
        .cancel_install(&owner("main"), &started.op_id)
        .unwrap();

    assert_eq!(outcome_of(&mut events).await, Outcome::Cancelled);

    let mut last = None;
    while let Some(progress) = next_progress(&mut started.progress).await {
        last = Some(progress);
    }
    let last = last.expect("progress was relayed");
    assert!(matches!(last.phase, Phase::Cancelled));
    finished(&harness.jobs).await;
    assert_eq!(harness.service.owner_count(), 0);
}

#[tokio::test]
async fn a_completed_install_id_is_not_found() {
    let harness = Harness::new(Mode::Succeed);
    let started = start(&harness, "main").await;
    finished(&harness.jobs).await;
    let error = harness
        .service
        .cancel_install(&owner("main"), &started.op_id)
        .unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::NotFound);
    assert_eq!(error.message(), MSG_OP_NOT_FOUND);
}

#[tokio::test]
async fn a_second_cancel_by_the_owner_is_not_found() {
    let (harness, _gate) = gated();
    let started = start(&harness, "main").await;
    let id = JobId::from_string(started.op_id.clone());
    let mut events = harness.jobs.subscribe(&id).unwrap();

    harness
        .service
        .cancel_install(&owner("main"), &started.op_id)
        .unwrap();
    let error = harness
        .service
        .cancel_install(&owner("main"), &started.op_id)
        .unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::NotFound);
    assert_eq!(error.message(), MSG_OP_NOT_FOUND);

    assert_eq!(outcome_of(&mut events).await, Outcome::Cancelled);
    finished(&harness.jobs).await;
    assert_eq!(harness.service.owner_count(), 0);
}
