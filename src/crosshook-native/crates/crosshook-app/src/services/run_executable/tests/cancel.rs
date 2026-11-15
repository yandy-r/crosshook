//! Owner-checked `cancel` (TERM → grace → KILL) and `stop` (immediate group
//! KILL), each followed by the prefix sweep; the throwaway prefix is removed
//! only when nothing survived the sweep.

use std::path::Path;
use std::time::Instant;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use super::{key_of, open_gate, Fixture, GATED};
use crate::jobs::{processes_using_prefix, PrefixSweep, DEFAULT_KILL_GRACE};
use crate::services::process::test_support::EscapeeFiles;
use crate::services::process::JobOwner;
use crate::services::run_executable::cleanup::CleanupOutcome;
use crate::services::run_executable::RunCompletion;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_own_terminates_and_removes_the_throwaway_prefix() {
    let fixture = Fixture::new();
    fixture.install_proton(GATED);
    let owner = JobOwner::new("main");
    let (response, mut run) = fixture.start("main", fixture.request()).await;
    let prefix = response.result.resolved_prefix_path.clone();
    let key = key_of(&prefix);
    run.wait_ready(&prefix).await;

    assert!(fixture.service.cancel(&owner));
    let (lines, completion) = run.drain().await;
    assert!(
        lines.is_empty(),
        "the gated child never said done: {lines:?}"
    );
    assert_eq!(
        completion,
        Some(RunCompletion {
            code: None,
            signal: Some(15)
        })
    );
    assert!(!Path::new(&prefix).exists());
    assert!(!fixture.jobs.is_key_held(&key));
    assert_eq!(fixture.jobs.job_counts(), (0, 0));
    // The finished run's slot is gone.
    assert!(!fixture.service.cancel(&owner));
    assert!(!fixture.service.stop(&owner));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_own_kills_at_once_sweeps_the_prefix_and_removes_it() {
    let fixture = Fixture::new();
    // TERM is ignored, so only an immediate KILL ends the run before the
    // grace period.
    fixture.install_proton(&format!(
        "trap '' TERM\n{}\n{GATED}",
        EscapeeFiles::in_dir(&fixture.root).script()
    ));
    let owner = JobOwner::new("main");
    let (response, mut run) = fixture.start("main", fixture.request()).await;
    let prefix = response.result.resolved_prefix_path.clone();
    let key = key_of(&prefix);
    run.wait_ready(&prefix).await;
    let escapee = EscapeeFiles::in_dir(&fixture.root).attach().await;

    let stopped_at = Instant::now();
    assert!(fixture.service.stop(&owner));
    let (lines, completion) = run.drain().await;
    assert!(
        stopped_at.elapsed() < DEFAULT_KILL_GRACE,
        "stop must not wait for the TERM grace"
    );
    assert!(lines.is_empty(), "the child never said done: {lines:?}");
    assert_eq!(
        completion,
        Some(RunCompletion {
            code: None,
            signal: Some(9)
        })
    );
    assert!(!Path::new(&prefix).exists());
    assert!(!fixture.jobs.is_key_held(&key));
    assert_eq!(fixture.jobs.job_counts(), (0, 0));
    assert!(
        escapee.died().await,
        "the escaped descendant survived the stop: no prefix sweep"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_sweeps_a_setsid_descendant_before_removing_the_prefix() {
    let fixture = Fixture::new();
    fixture.install_proton(&format!(
        "{}\n{GATED}",
        EscapeeFiles::in_dir(&fixture.root).script()
    ));
    let owner = JobOwner::new("main");
    let (response, mut run) = fixture.start("main", fixture.request()).await;
    let prefix = response.result.resolved_prefix_path.clone();
    let key = key_of(&prefix);
    run.wait_ready(&prefix).await;
    let escapee = EscapeeFiles::in_dir(&fixture.root).attach().await;

    assert!(fixture.service.cancel(&owner));
    let (lines, completion) = run.drain().await;

    assert!(lines.is_empty(), "nothing is left to report: {lines:?}");
    assert_eq!(
        completion,
        Some(RunCompletion {
            code: None,
            signal: Some(15)
        })
    );
    assert!(processes_using_prefix(Path::new(&prefix)).is_empty());
    assert!(!Path::new(&prefix).exists());
    assert!(!fixture.jobs.is_key_held(&key));
    assert!(
        escapee.died().await,
        "the escaped descendant survived the cancel: no prefix sweep"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_with_survivors_keeps_the_throwaway_prefix_and_says_so() {
    let mut fixture = Fixture::new();
    let removals = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&removals);
    fixture.service = fixture
        .service
        .clone()
        .with_sweep(|_| PrefixSweep::Incomplete { survivors: 2 })
        .with_cleanup(move |_, _| {
            counted.fetch_add(1, Ordering::SeqCst);
            CleanupOutcome::Removed
        });
    fixture.install_proton(GATED);
    let owner = JobOwner::new("main");
    let (response, mut run) = fixture.start("main", fixture.request()).await;
    let prefix = response.result.resolved_prefix_path.clone();
    run.wait_ready(&prefix).await;

    assert!(fixture.service.cancel(&owner));
    let (lines, _) = run.drain().await;

    assert_eq!(
        lines,
        vec![
            "2 process(es) still use the prefix after the run ended.".to_string(),
            "The temporary prefix was not cleaned up because processes still use it.".to_string(),
        ]
    );
    assert!(Path::new(&prefix).exists(), "the prefix was deleted");
    assert_eq!(removals.load(Ordering::SeqCst), 0);
    assert!(!fixture.jobs.is_key_held(&key_of(&prefix)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_on_a_user_prefix_sweeps_it_and_never_deletes_it() {
    let fixture = Fixture::new();
    let user_prefix = fixture.root.join("user-prefix");
    std::fs::create_dir(&user_prefix).unwrap();
    fixture.install_proton(&format!(
        "{}\n{GATED}",
        EscapeeFiles::in_dir(&fixture.root).script()
    ));
    let owner = JobOwner::new("main");
    let (response, mut run) = fixture
        .start("main", fixture.request_with_prefix(&user_prefix))
        .await;
    let prefix = response.result.resolved_prefix_path.clone();
    run.wait_ready(&prefix).await;
    let escapee = EscapeeFiles::in_dir(&fixture.root).attach().await;

    assert!(fixture.service.cancel(&owner));
    let (_, completion) = run.drain().await;

    assert_eq!(completion.and_then(|done| done.signal), Some(15));
    assert!(user_prefix.exists(), "a user prefix is never deleted");
    assert!(escapee.died().await, "the user prefix was not swept");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn foreign_cancel_and_stop_are_refused_and_leave_the_job_running() {
    let fixture = Fixture::new();
    fixture.install_proton(GATED);
    let stranger = JobOwner::new("other-window");
    // No active run at all.
    assert!(!fixture.service.cancel(&stranger));
    assert!(!fixture.service.stop(&stranger));

    let (response, mut run) = fixture.start("main", fixture.request()).await;
    let prefix = response.result.resolved_prefix_path.clone();
    run.wait_ready(&prefix).await;

    assert!(!fixture.service.cancel(&stranger));
    assert!(!fixture.service.stop(&stranger));
    assert!(fixture.jobs.is_key_held(&key_of(&prefix)));
    assert_eq!(fixture.jobs.job_counts(), (1, 0));

    // The run is untouched: it still completes naturally.
    open_gate(&prefix).await;
    assert_eq!(run.next_line().await, "done");
    let (_, completion) = run.drain().await;
    assert_eq!(
        completion,
        Some(RunCompletion {
            code: Some(0),
            signal: None
        })
    );
    assert!(!Path::new(&prefix).exists());
    assert_eq!(fixture.jobs.job_counts(), (0, 0));
}
