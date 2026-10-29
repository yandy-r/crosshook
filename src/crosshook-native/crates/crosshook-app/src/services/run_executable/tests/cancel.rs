//! Owner-checked `cancel` (TERM → grace → KILL) and `stop` (immediate group
//! KILL plus prefix sweep); the throwaway prefix is removed either way.

use std::io::Read;
use std::path::Path;
use std::time::{Duration, Instant};

use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;

use super::{key_of, open_gate, Fixture, GATED};
use crate::jobs::DEFAULT_KILL_GRACE;
use crate::services::process::JobOwner;
use crate::services::run_executable::RunCompletion;

/// Upper bound for the escaped process to die after a stop; only reached
/// when the sweep is broken (the test then fails instead of hanging).
const ESCAPEE_DEATH_BOUND: Duration = Duration::from_secs(30);

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
    // Rendezvous files live outside the prefix, which cleanup deletes.
    let alive = fixture.root.join("alive");
    let hold = fixture.root.join("hold");
    let pid_file = fixture.root.join("escapee.pid");
    // TERM is ignored, so only an immediate KILL ends the run before the
    // grace period. A descendant leaves the group (setsid) but keeps the
    // prefix in its environment: only the prefix sweep can reach it. It
    // records its pid and holds `alive` open for writing until it dies.
    fixture.install_proton(&format!(
        "trap '' TERM\n\
         mkfifo '{alive}' '{hold}'\n\
         setsid sh -c 'echo $$ > \"$0\"; exec 3>\"$1\"; read x < \"$2\"' '{pid}' '{alive}' '{hold}' &\n\
         {GATED}",
        alive = alive.display(),
        hold = hold.display(),
        pid = pid_file.display(),
    ));
    let owner = JobOwner::new("main");
    let (response, mut run) = fixture.start("main", fixture.request()).await;
    let prefix = response.result.resolved_prefix_path.clone();
    let key = key_of(&prefix);
    run.wait_ready(&prefix).await;
    // Rendezvous with the escapee's writer: it is alive, outside the group,
    // and its pid is recorded from here on.
    let mut alive_reader = tokio::task::spawn_blocking(move || std::fs::File::open(alive))
        .await
        .unwrap()
        .unwrap();
    let escapee: i32 = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();

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

    // EOF on `alive`: the escaped descendant is dead (killed by the sweep).
    let eof = tokio::time::timeout(
        ESCAPEE_DEATH_BOUND,
        tokio::task::spawn_blocking(move || {
            let mut rest = Vec::new();
            alive_reader.read_to_end(&mut rest).map(|_| rest)
        }),
    )
    .await;
    let Ok(eof) = eof else {
        // Unblock the reader so the runtime can shut down, then fail.
        let _ = kill(Pid::from_raw(escapee), Signal::SIGKILL);
        panic!("the escaped descendant survived the stop: no prefix sweep");
    };
    assert!(eof.unwrap().unwrap().is_empty());
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
