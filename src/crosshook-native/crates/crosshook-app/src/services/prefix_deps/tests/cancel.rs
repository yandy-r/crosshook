//! C1: cancelling an install terminates its whole process group and still
//! restores the captured Windows version (I8), then resolves the row.

use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;

use crosshook_core::prefix_deps::PrefixVersionRestoreState;

use super::helpers::{read_calls, Harness, PROFILE};
use super::install::{drain, gated_tool, packages, raw_prefix, wait_for_line};
use crate::jobs::{JobEventKind, JobId, JobRegistry, Outcome};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c1_cancel_terminates_group_and_still_restores() {
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
    assert_eq!(install.restore_state, PrefixVersionRestoreState::Succeeded);
    // The install ran once, then the restore of the captured version.
    assert_eq!(read_calls(&h.calls), vec!["-q dotnet48", "-q win7"]);
    assert!(h.row().is_none(), "the verified restore deleted the row");

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
