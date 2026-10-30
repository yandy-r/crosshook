//! Per-prefix exclusivity (feature-spec AC 815-3, L1–L3): one job per
//! canonical prefix across install, repair, startup recovery and any other
//! prefix-keyed job; other prefixes run in parallel.

use std::fs;
use std::os::unix::fs::symlink;

use std::sync::Arc;

use super::super::journal::RecoveryResult;
use super::super::msg_already_installing;
use super::super::seams::{CoreRestorer, MetadataJournalStore};
use super::fakes::{CountingRestorer, FakeInUse, FakeProbe};
use super::helpers::{
    fake_tool, pending_row, read_calls, write_system_reg, Harness, ToolScript, WinVersion, PROFILE,
};
use super::install::{drain, gated_tool, open_gate, packages, raw_prefix, wait_for_line};
use crate::jobs::{JobRegistry, JobSpec, Outcome};
use crate::services::process::prefix_job_key;
use crate::AppErrorKind;

// L1: a second install on the same prefix, also through a symlink alias,
// conflicts; only one installer ever ran.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn l1_same_prefix_and_symlink_alias_conflict() {
    let h = Harness::new();
    let (tool, fifo) = gated_tool(&h);
    let jobs = JobRegistry::default();
    let service = h.service(jobs.clone(), &tool);
    let alias = h.dir.path().join("alias");
    symlink(&h.prefix_root, &alias).unwrap();

    let mut first = service
        .start_install(PROFILE.to_string(), raw_prefix(&h), packages())
        .await
        .unwrap();
    wait_for_line(&mut first, "installing dotnet48").await;

    for path in [raw_prefix(&h), alias.to_string_lossy().into_owned()] {
        let error = service
            .start_install(PROFILE.to_string(), path, packages())
            .await
            .err()
            .expect("a second install on the prefix must conflict");
        assert_eq!(error.kind(), AppErrorKind::Conflict);
        assert_eq!(error.message(), msg_already_installing(&h.prefix));
    }
    assert_eq!(jobs.job_counts().0, 1, "only the first job runs");

    open_gate(fifo).await;
    drain(&mut first).await;
    let installs = read_calls(&h.calls)
        .into_iter()
        .filter(|call| call.ends_with("dotnet48"))
        .count();
    assert_eq!(installs, 1, "the installer ran exactly once");
    assert_eq!(jobs.job_counts(), (0, 0));
}

// L2: another prefix is not blocked (no global lock any more).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn l2_other_prefix_runs_in_parallel() {
    let h = Harness::new();
    let (tool, fifo) = gated_tool(&h);
    let jobs = JobRegistry::default();
    let service = h.service(jobs.clone(), &tool);
    let other = h.dir.path().join("other");
    write_system_reg(&other.join("pfx"), WinVersion::Win7);
    let other_dir = h.dir.path().join("other-tool");
    fs::create_dir_all(&other_dir).unwrap();
    let other_calls = other_dir.join("calls.txt");
    let other_tool = fake_tool(&other_dir, &other_calls, &ToolScript::default());
    let (other_canonical, _) = prefix_job_key(&other.to_string_lossy()).unwrap();
    assert_ne!(other_canonical, h.prefix);
    // Same registry, seams that expect the other prefix.
    let other_service = h.service_with(
        jobs.clone(),
        Arc::new(MetadataJournalStore(h.metadata.clone())),
        Arc::new(CountingRestorer::new(
            Arc::new(CoreRestorer),
            &other_canonical,
        )),
        Arc::new(FakeInUse::new(false, &other_canonical)),
        Arc::new(FakeProbe::winetricks(&other_tool)),
    );

    let mut first = service
        .start_install(PROFILE.to_string(), raw_prefix(&h), packages())
        .await
        .unwrap();
    wait_for_line(&mut first, "installing dotnet48").await;

    let mut second = other_service
        .start_install(
            PROFILE.to_string(),
            other.to_string_lossy().into_owned(),
            packages(),
        )
        .await
        .expect("a different prefix must not conflict");
    let (_, completion) = drain(&mut second).await;
    assert!(completion.outcome.install_succeeded);
    assert_eq!(read_calls(&other_calls), vec!["-q dotnet48", "-q win7"]);
    assert_eq!(jobs.job_counts().0, 1, "the first install still runs");

    open_gate(fifo).await;
    drain(&mut first).await;
    assert_eq!(jobs.job_counts(), (0, 0));
}

// L3: repair conflicts with a running install; startup recovery skips the
// busy prefix and leaves its row for the holder.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn l3_repair_conflicts_and_startup_skips_busy_prefix() {
    let h = Harness::new();
    let (tool, fifo) = gated_tool(&h);
    let jobs = JobRegistry::default();
    let service = h.service(jobs.clone(), &tool);

    let mut first = service
        .start_install(PROFILE.to_string(), raw_prefix(&h), packages())
        .await
        .unwrap();
    wait_for_line(&mut first, "installing dotnet48").await;
    let row = h.row().expect("the install recorded its pending row");
    assert_eq!(row.state, "pending");

    let error = service
        .repair(raw_prefix(&h))
        .await
        .expect_err("repair must conflict with the running install");
    assert_eq!(error.kind(), AppErrorKind::Conflict);

    let results = service.recover_on_startup().await;
    assert_eq!(
        results,
        vec![(h.prefix.clone(), RecoveryResult::SkippedBusy)]
    );
    assert_eq!(h.row().map(|row| row.state).as_deref(), Some("pending"));
    assert_eq!(
        read_calls(&h.calls),
        vec!["-q dotnet48"],
        "no restore while the install holds the prefix"
    );

    open_gate(fifo).await;
    drain(&mut first).await;
    assert!(h.row().is_none());
}

// Any prefix-keyed job (run-executable, update, install-game) blocks an
// install, and an install blocks them, through the one shared key.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn other_prefix_keyed_jobs_share_the_key() {
    let h = Harness::new();
    let (tool, fifo) = gated_tool(&h);
    let jobs = JobRegistry::default();
    let service = h.service(jobs.clone(), &tool);
    let (_, key) = prefix_job_key(&raw_prefix(&h)).unwrap();

    let mut install = service
        .start_install(PROFILE.to_string(), raw_prefix(&h), packages())
        .await
        .unwrap();
    wait_for_line(&mut install, "installing dotnet48").await;
    let error = jobs
        .start(
            JobSpec::new("run-executable").with_key(key.clone()),
            |_ctx| async { Outcome::Succeeded },
        )
        .err()
        .expect("a run-executable job on the prefix must conflict");
    assert_eq!(error.kind(), AppErrorKind::Conflict);
    open_gate(fifo).await;
    drain(&mut install).await;

    // The other direction: a held update-style key refuses the install
    // before anything runs.
    fs::remove_file(&h.calls).unwrap();
    let (release, released) = tokio::sync::oneshot::channel::<()>();
    let update = jobs
        .start(
            JobSpec::new("update-game").with_key(key),
            |_ctx| async move {
                let _ = released.await;
                Outcome::Succeeded
            },
        )
        .unwrap();
    let error = service
        .start_install(PROFILE.to_string(), raw_prefix(&h), packages())
        .await
        .err()
        .expect("an install on a busy prefix must conflict");
    assert_eq!(error.kind(), AppErrorKind::Conflict);
    assert_eq!(error.message(), msg_already_installing(&h.prefix));
    assert!(read_calls(&h.calls).is_empty());
    release.send(()).unwrap();
    let mut events = update.into_events();
    while events.recv().await.is_some() {}
}

// A pending row left by a crash is restored by the next install first (J1c
// through the service, under the install's own key).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn install_recovers_pending_row_before_installing() {
    let h = Harness::new();
    let tool = h.tool(&Default::default());
    h.seed_row(&pending_row(&h.prefix, &tool));
    let service = h.service(JobRegistry::default(), &tool);

    let mut run = service
        .start_install(PROFILE.to_string(), raw_prefix(&h), packages())
        .await
        .unwrap();
    drain(&mut run).await;

    // Recovery restores win10 first; the install then captures win10.
    assert_eq!(
        read_calls(&h.calls),
        vec!["-q win10", "-q dotnet48", "-q win10"]
    );
    assert!(h.row().is_none());
    let status = service.repair_status(raw_prefix(&h)).await.unwrap();
    assert_eq!(
        status.last_recovery.map(|recovery| recovery.outcome),
        Some(super::super::RecoveryOutcome::Restored)
    );
}
