//! C1: cancelling an install (only app shutdown does) terminates its whole
//! process group and skips the restore; C2: it also sweeps processes that
//! left the group but still use the prefix: a restore started during shutdown
//! would outlive the process, so the `pending` row stays for startup
//! recovery.

use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;

use crosshook_core::prefix_deps::PrefixVersionRestoreState;

use super::helpers::{read_calls, Harness, ToolScript, PROFILE};
use super::install::{drain, gated_tool, make_fifo, packages, raw_prefix, wait_for_line};
use crate::jobs::{processes_using_prefix, JobEventKind, JobId, JobRegistry, Outcome};
use crate::services::process::test_support::EscapeeFiles;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c1_cancel_terminates_group_and_leaves_row_pending() {
    let h = Harness::new();
    let (tool, fifo) = gated_tool(&h);
    let jobs = JobRegistry::default();
    let service = h.service(jobs.clone(), &tool);

    let mut run = service
        .start_install(PROFILE.to_string(), raw_prefix(&h), packages())
        .await
        .unwrap();
    wait_for_line(&mut run, "installing dotnet48").await;
    assert_eq!(h.row().map(|row| row.state).as_deref(), Some("pending"));
    let id = JobId::from_string(run.run_id());
    let mut observer = jobs.subscribe(&id).unwrap();

    jobs.cancel(&id).unwrap();
    let (_, completion) = drain(&mut run).await;

    let mut outcome = None;
    while let Some(event) = observer.recv().await {
        if let JobEventKind::Finished(finished) = event.kind {
            outcome = Some(finished);
        }
    }
    assert_eq!(outcome, Some(Outcome::Cancelled));

    let install = &completion.outcome;
    assert!(!install.install_succeeded);
    assert_eq!(install.install_exit_code, None, "ended by a signal");
    assert_eq!(install.restore_state, PrefixVersionRestoreState::Pending);
    assert_eq!(install.restore_error, None);
    // The install ran once; no restore was started.
    assert_eq!(read_calls(&h.calls), vec!["-q dotnet48"]);
    assert_eq!(
        h.row().map(|row| row.state).as_deref(),
        Some("pending"),
        "the row stays for startup recovery"
    );

    // The gate reader (`cat`, a group member, not the leader) is gone too:
    // a non-blocking writer finds no reader on the fifo.
    let error = OpenOptions::new()
        .write(true)
        .custom_flags(nix::libc::O_NONBLOCK)
        .open(&fifo)
        .expect_err("the group member reading the fifo must be terminated");
    assert_eq!(error.raw_os_error(), Some(nix::libc::ENXIO));
    assert_eq!(jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c2_cancel_sweeps_a_setsid_descendant_left_on_the_prefix() {
    let h = Harness::new();
    let files_dir = tempfile::tempdir().unwrap();
    let escapee = EscapeeFiles::in_dir(files_dir.path());
    let tool = h.tool(&ToolScript {
        install_gate: Some(make_fifo(h.dir.path(), "gate")),
        before_install: Some(escapee.script()),
        ..ToolScript::default()
    });
    let jobs = JobRegistry::default();
    let service = h.service(jobs.clone(), &tool);

    let mut run = service
        .start_install(PROFILE.to_string(), raw_prefix(&h), packages())
        .await
        .unwrap();
    wait_for_line(&mut run, "installing dotnet48").await;
    let escapee = escapee.attach().await;

    jobs.cancel(&JobId::from_string(run.run_id())).unwrap();
    let (_, completion) = drain(&mut run).await;

    assert!(!completion.outcome.install_succeeded);
    assert!(processes_using_prefix(h.pfx()).is_empty());
    assert!(escapee.died().await, "the escapee survived the cancel");
    assert_eq!(jobs.job_counts(), (0, 0));
}
