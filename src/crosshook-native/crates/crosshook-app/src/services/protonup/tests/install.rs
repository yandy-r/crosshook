use std::sync::atomic::Ordering;
use std::sync::Arc;

use crosshook_core::protonup::progress::{Phase, ProtonInstallProgress};
use crosshook_core::protonup::ProtonUpInstallErrorKind;
use tokio::sync::Notify;

use super::super::messages::{
    MSG_INSTALL_CANCELLED, MSG_INSTALL_FAILED, MSG_INSTALL_IN_PROGRESS, MSG_OFFLINE,
    MSG_SETTINGS_UNREADABLE,
};
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

fn terminal_count(progress: &[ProtonInstallProgress]) -> usize {
    progress
        .iter()
        .filter(|p| matches!(p.phase, Phase::Done | Phase::Failed | Phase::Cancelled))
        .count()
}

#[tokio::test]
async fn cancelled_error_without_a_terminal_phase_emits_one_synthetic_cancelled() {
    let harness = Harness::new(Mode::CancelNoTerminal);
    let mut started = start(&harness).await;
    let progress = drain(&mut started).await;
    assert_eq!(terminal_count(&progress), 1);
    let last = progress.last().expect("a terminal phase was synthesized");
    assert!(matches!(last.phase, Phase::Cancelled));
    assert_eq!(last.message.as_deref(), Some(MSG_INSTALL_CANCELLED));
    finished(&harness.jobs).await;
    assert_eq!(harness.service.owner_count(), 0);
}

#[tokio::test]
async fn unsuccessful_result_without_a_terminal_phase_emits_one_synthetic_failed() {
    let harness = Harness::new(Mode::UnsuccessfulNoTerminal);
    let mut started = start(&harness).await;
    let progress = drain(&mut started).await;
    assert_eq!(terminal_count(&progress), 1);
    let last = progress.last().expect("a terminal phase was synthesized");
    assert!(matches!(last.phase, Phase::Failed));
    assert_eq!(last.message.as_deref(), Some(MSG_INSTALL_FAILED));
    finished(&harness.jobs).await;
}

#[tokio::test]
async fn failure_without_a_terminal_phase_emits_exactly_one_terminal_event() {
    let harness = Harness::new(Mode::FailNoTerminal);
    let mut started = start(&harness).await;
    let progress = drain(&mut started).await;
    assert_eq!(terminal_count(&progress), 1);
    finished(&harness.jobs).await;
}

#[tokio::test]
async fn legacy_install_of_a_version_being_installed_is_refused_without_installing() {
    let gate = Arc::new(Notify::new());
    let harness = Harness::new(Mode::Gated(Arc::clone(&gate)));
    let mut first = start(&harness).await;

    let result = harness
        .service
        .install_version(request("GE-Proton9-21"))
        .await
        .unwrap();
    assert!(!result.success);
    assert_eq!(result.installed_path, None);
    assert_eq!(
        result.error_kind,
        Some(ProtonUpInstallErrorKind::AlreadyInstalled)
    );
    assert_eq!(
        result.error_message.as_deref(),
        Some(MSG_INSTALL_IN_PROGRESS)
    );
    assert_eq!(harness.installer.calls.load(Ordering::SeqCst), 1);

    gate.notify_one();
    drain(&mut first).await;
    finished(&harness.jobs).await;
}

#[tokio::test]
async fn async_install_conflicts_with_a_running_legacy_install() {
    let gate = Arc::new(Notify::new());
    let harness = Harness::new(Mode::Gated(Arc::clone(&gate)));
    let legacy = {
        let service = harness.service.clone();
        tokio::spawn(async move { service.install_version(request("GE-Proton9-21")).await })
    };
    // The legacy install holds the job key once it is registered.
    while harness.jobs.job_counts().0 == 0 {
        tokio::task::yield_now().await;
    }

    let error = match harness
        .service
        .start_install(owner("main"), request("GE-Proton9-21"), catalog_entry())
        .await
    {
        Ok(_) => panic!("async install overlapped a legacy install"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), AppErrorKind::Conflict);

    gate.notify_one();
    let result = legacy.await.unwrap().unwrap();
    assert!(result.success, "{result:?}");
    finished(&harness.jobs).await;
}

#[tokio::test]
async fn legacy_install_success_is_returned_and_the_job_is_reaped() {
    let harness = Harness::new(Mode::Succeed);
    let result = harness
        .service
        .install_version(request("GE-Proton9-21"))
        .await
        .unwrap();
    assert!(result.success, "{result:?}");
    assert_eq!(harness.jobs.job_counts(), (0, 0));
}

fn corrupt_settings(harness: &Harness) {
    let path = harness.settings.settings_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b"offline_mode = [not valid toml").unwrap();
}

#[tokio::test]
async fn unreadable_settings_fail_closed_for_async_install() {
    let harness = Harness::new(Mode::Succeed);
    corrupt_settings(&harness);
    let error = match harness
        .service
        .start_install(owner("main"), request("GE-Proton9-21"), catalog_entry())
        .await
    {
        Ok(_) => panic!("install started with unreadable settings"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), AppErrorKind::Unavailable);
    assert_eq!(error.message(), MSG_SETTINGS_UNREADABLE);
    assert_eq!(harness.catalog.calls.load(Ordering::SeqCst), 0);
    harness.assert_rejected();
}

#[tokio::test]
async fn unreadable_settings_fail_closed_for_legacy_install() {
    let harness = Harness::new(Mode::Succeed);
    corrupt_settings(&harness);
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
    assert_eq!(
        result.error_message.as_deref(),
        Some(MSG_SETTINGS_UNREADABLE)
    );
    assert_eq!(harness.catalog.calls.load(Ordering::SeqCst), 0);
    harness.assert_rejected();
}
