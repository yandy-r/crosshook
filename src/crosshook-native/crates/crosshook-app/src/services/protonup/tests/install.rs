use std::sync::atomic::Ordering;
use std::sync::Arc;

use crosshook_core::protonup::progress::{Phase, ProtonInstallProgress};
use crosshook_core::protonup::ProtonUpInstallErrorKind;
use tokio::sync::Notify;

use super::super::messages::{MSG_INSTALL_FAILED, MSG_OFFLINE};
use super::super::progress::next_progress;
use super::super::ProtonInstallStart;
use super::helpers::{finished, ge_catalog, owner, request, Harness, Mode};
use crate::AppErrorKind;

fn catalog_entry() -> crosshook_core::protonup::ProtonUpAvailableVersion {
    ge_catalog().versions.remove(0)
}

async fn start(harness: &Harness) -> ProtonInstallStart {
    match harness
        .service
        .start_install(owner("main"), request("GE-Proton9-21"), catalog_entry())
        .await
    {
        Ok(started) => started,
        Err(error) => panic!("start_install failed: {}", error.message()),
    }
}

async fn drain(started: &mut ProtonInstallStart) -> Vec<ProtonInstallProgress> {
    let mut all = Vec::new();
    while let Some(progress) = next_progress(&mut started.progress).await {
        all.push(progress);
    }
    all
}

#[tokio::test]
async fn op_id_is_usable_before_the_install_completes() {
    let gate = Arc::new(Notify::new());
    let harness = Harness::new(Mode::Gated(Arc::clone(&gate)));
    let mut started = start(&harness).await;

    assert!(!started.op_id.is_empty());
    assert_eq!(started.handle().op_id, started.op_id);
    // The owner is registered and the job is still running.
    assert_eq!(harness.service.owner_count(), 1);
    assert_eq!(harness.jobs.job_counts().0, 1);

    gate.notify_one();
    let progress = drain(&mut started).await;
    assert!(progress.iter().all(|p| p.op_id == started.op_id));
    finished(&harness.jobs).await;
    assert_eq!(harness.service.owner_count(), 0);
}

#[tokio::test]
async fn progress_reaches_done_and_the_job_is_reaped() {
    let harness = Harness::new(Mode::Succeed);
    let mut started = start(&harness).await;
    let progress = drain(&mut started).await;
    let last = progress.last().expect("progress was relayed");
    assert!(matches!(last.phase, Phase::Done));
    assert_eq!(harness.installer.calls.load(Ordering::SeqCst), 1);
    finished(&harness.jobs).await;
    assert_eq!(harness.jobs.job_counts(), (0, 0));
    assert_eq!(harness.service.owner_count(), 0);
}

#[tokio::test]
async fn path_bearing_failure_message_becomes_fixed_text() {
    let harness = Harness::new(Mode::FailWithPathMessage);
    let mut started = start(&harness).await;
    let progress = drain(&mut started).await;
    let failed = progress
        .iter()
        .find(|p| matches!(p.phase, Phase::Failed))
        .expect("a Failed phase was relayed");
    assert_eq!(failed.message.as_deref(), Some(MSG_INSTALL_FAILED));
    let tmp = harness.root.path().to_string_lossy().into_owned();
    assert!(progress
        .iter()
        .all(|p| !p.message.as_deref().unwrap_or("").contains(&tmp)));
    finished(&harness.jobs).await;
}

#[tokio::test]
async fn failure_without_a_terminal_phase_emits_a_synthetic_failed() {
    let harness = Harness::new(Mode::FailNoTerminal);
    let mut started = start(&harness).await;
    let progress = drain(&mut started).await;
    let last = progress.last().expect("a terminal phase was synthesized");
    assert!(matches!(last.phase, Phase::Failed));
    assert_eq!(last.message.as_deref(), Some(MSG_INSTALL_FAILED));
    finished(&harness.jobs).await;
    assert_eq!(harness.service.owner_count(), 0);
}

#[tokio::test]
async fn a_second_install_of_the_same_version_conflicts() {
    let gate = Arc::new(Notify::new());
    let harness = Harness::new(Mode::Gated(Arc::clone(&gate)));
    let mut first = start(&harness).await;

    let error = match harness
        .service
        .start_install(owner("other"), request("GE-Proton9-21"), catalog_entry())
        .await
    {
        Ok(_) => panic!("duplicate install was accepted"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), AppErrorKind::Conflict);
    assert_eq!(harness.installer.calls.load(Ordering::SeqCst), 1);
    assert_eq!(harness.service.owner_count(), 1);

    gate.notify_one();
    drain(&mut first).await;
    finished(&harness.jobs).await;
}

#[tokio::test]
async fn offline_start_is_unavailable_before_any_job() {
    let harness = Harness::new(Mode::Succeed).with_offline();
    let error = match harness
        .service
        .start_install(owner("main"), request("GE-Proton9-21"), catalog_entry())
        .await
    {
        Ok(_) => panic!("offline install was accepted"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), AppErrorKind::Unavailable);
    assert_eq!(error.message(), MSG_OFFLINE);
    assert_eq!(harness.catalog.calls.load(Ordering::SeqCst), 0);
    harness.assert_rejected();
}

#[tokio::test]
async fn offline_legacy_install_returns_a_failed_result() {
    let harness = Harness::new(Mode::Succeed).with_offline();
    let result = harness
        .service
        .install_version(request("GE-Proton9-21"))
        .await
        .unwrap();
    assert!(!result.success);
    assert_eq!(
        result.error_kind,
        Some(ProtonUpInstallErrorKind::NetworkError)
    );
    assert_eq!(result.error_message.as_deref(), Some(MSG_OFFLINE));
    harness.assert_rejected();
}

#[tokio::test]
async fn legacy_install_failure_has_fixed_path_free_text() {
    let harness = Harness::new(Mode::FailNoTerminal);
    let result = harness
        .service
        .install_version(request("GE-Proton9-21"))
        .await
        .unwrap();
    assert!(!result.success);
    assert_eq!(result.error_kind, Some(ProtonUpInstallErrorKind::Unknown));
    let message = result.error_message.unwrap();
    assert_eq!(message, "install failed");
    assert!(!message.contains('/'));
    assert_eq!(harness.jobs.job_counts(), (0, 0));
}

#[tokio::test]
async fn catalog_is_read_without_forcing_a_refresh() {
    let harness = Harness::new(Mode::Succeed);
    let mut started = start(&harness).await;
    drain(&mut started).await;
    let args = harness.catalog.last_args.lock().unwrap().clone();
    assert_eq!(args, Some(("ge-proton".to_string(), false, false)));
    finished(&harness.jobs).await;
}
