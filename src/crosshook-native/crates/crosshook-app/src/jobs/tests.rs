//! Tests for the job registry: start/finish races, keys, lag, late
//! subscribers, ownership, panics, and retention.

use std::time::Duration;

use tokio::sync::oneshot;

use super::{
    JobEvent, JobEventKind, JobHandle, JobId, JobKey, JobProgress, JobRegistry, JobRegistryConfig,
    JobSpec, Outcome,
};
use crate::log_tail::LogBatch;
use crate::{AppError, AppErrorKind};

const TIMEOUT: Duration = Duration::from_secs(10);

async fn next(handle: &mut JobHandle) -> Option<JobEvent> {
    tokio::time::timeout(TIMEOUT, handle.recv())
        .await
        .expect("event within timeout")
}

/// Drains the handle and returns the `Finished` outcome; asserts the stream
/// ends after it.
async fn finished(handle: &mut JobHandle) -> Outcome {
    loop {
        match next(handle)
            .await
            .expect("stream ended before Finished")
            .kind
        {
            JobEventKind::Finished(outcome) => {
                assert_eq!(next(handle).await, None, "events after Finished");
                return outcome;
            }
            _ => continue,
        }
    }
}

async fn panics(message: &'static str) -> Outcome {
    panic!("{message}")
}

fn registry_with(event_capacity: usize, retention: usize) -> JobRegistry {
    JobRegistry::new(JobRegistryConfig {
        event_capacity,
        retention,
    })
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn instantly_finishing_job_is_always_observed() {
    let registry = JobRegistry::default();
    for _ in 0..1_000 {
        let mut handle = registry
            .start(JobSpec::new("noop"), |_ctx| async { Outcome::Succeeded })
            .unwrap();
        let event = next(&mut handle).await.expect("Finished");
        assert_eq!(event.job_id, *handle.id());
        assert_eq!(event.kind, JobEventKind::Finished(Outcome::Succeeded));
        assert_eq!(next(&mut handle).await, None);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn result_jobs_map_to_outcomes() {
    let registry = JobRegistry::default();
    let mut ok = registry
        .start(JobSpec::new("ok"), |_ctx| async { Ok::<(), AppError>(()) })
        .unwrap();
    assert_eq!(finished(&mut ok).await, Outcome::Succeeded);

    let mut failed = registry
        .start(JobSpec::new("err"), |_ctx| async {
            Err(AppError::validation("x", "bad"))
        })
        .unwrap();
    match finished(&mut failed).await {
        Outcome::Failed(error) => assert_eq!(error.kind(), AppErrorKind::Validation { field: "x" }),
        other => panic!("expected Failed, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn active_key_conflicts_and_is_reusable_after_finished() {
    let registry = JobRegistry::default();
    let key = JobKey::new("/prefixes/game");
    let (release, released) = oneshot::channel::<()>();
    let mut first = registry
        .start(JobSpec::new("first").with_key(key.clone()), |_ctx| async {
            let _ = released.await;
            Outcome::Succeeded
        })
        .unwrap();

    let error = registry
        .start(JobSpec::new("second").with_key(key.clone()), |_ctx| {
            panics("conflicting job must never run")
        })
        .err()
        .expect("conflict");
    assert_eq!(error.kind(), AppErrorKind::Conflict);

    // A different key is unaffected.
    let mut other = registry
        .start(
            JobSpec::new("other").with_key(JobKey::new("/prefixes/other")),
            |_ctx| async { Outcome::Succeeded },
        )
        .unwrap();
    assert_eq!(finished(&mut other).await, Outcome::Succeeded);

    release.send(()).unwrap();
    assert_eq!(finished(&mut first).await, Outcome::Succeeded);

    // Reusable immediately on Finished, no sleep.
    let mut again = registry
        .start(JobSpec::new("again").with_key(key), |_ctx| async {
            Outcome::Succeeded
        })
        .unwrap();
    assert_eq!(finished(&mut again).await, Outcome::Succeeded);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lagging_receiver_gets_dropped_count_then_tail_and_finished() {
    let registry = registry_with(4, 8);
    let (go, wait_go) = oneshot::channel::<()>();
    let mut handle = registry
        .start(JobSpec::new("flood"), |ctx| async move {
            let _ = wait_go.await;
            for i in 0..100u64 {
                ctx.progress(JobProgress {
                    current: i,
                    total: Some(100),
                    message: None,
                });
            }
            Outcome::Succeeded
        })
        .unwrap();
    let id = handle.id().clone();
    // A second subscriber tells us when Finished has been sent, without
    // reading from the handle (it lags too, but Finished is never lost).
    let mut witness = registry.subscribe(&id).unwrap();
    go.send(()).unwrap();
    tokio::time::timeout(TIMEOUT, async {
        while let Some(event) = witness.recv().await {
            if matches!(event.kind, JobEventKind::Finished(_)) {
                break;
            }
        }
    })
    .await
    .unwrap();

    let first = next(&mut handle).await.unwrap();
    assert_eq!(first.job_id, id);
    let JobEventKind::Log(LogBatch::Dropped { n }) = first.kind else {
        panic!("expected Dropped, got {first:?}");
    };
    // 101 events into capacity 4: 97 dropped.
    assert_eq!(n, 97);

    let mut progress = Vec::new();
    let outcome = loop {
        let event = next(&mut handle).await.unwrap();
        assert_eq!(event.job_id, id);
        match event.kind {
            JobEventKind::Progress(p) => progress.push(p.current),
            JobEventKind::Finished(outcome) => break outcome,
            JobEventKind::Log(batch) => panic!("unexpected log {batch:?}"),
        }
    };
    assert_eq!(progress, vec![97, 98, 99]);
    assert_eq!(outcome, Outcome::Succeeded);
    assert_eq!(next(&mut handle).await, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn leaked_context_cannot_bury_or_outlive_finished() {
    let registry = registry_with(1, 8);
    let (go, wait_go) = oneshot::channel::<()>();
    let (flooded, wait_flooded) = oneshot::channel::<()>();
    let (_keep_alive, hold) = oneshot::channel::<()>();
    let mut handle = registry
        .start(JobSpec::new("leak"), |ctx| async move {
            let leaked = ctx.clone();
            drop(tokio::spawn(async move {
                let _ = wait_go.await;
                for i in 0..16u64 {
                    leaked.progress(JobProgress {
                        current: i,
                        total: None,
                        message: None,
                    });
                }
                let _ = flooded.send(());
                // Keeps the leaked context alive past the job.
                let _ = hold.await;
                drop(leaked);
            }));
            Outcome::Succeeded
        })
        .unwrap();
    let id = handle.id().clone();
    tokio::time::timeout(TIMEOUT, async {
        while registry.outcome(&id).unwrap().is_none() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    go.send(()).unwrap();
    tokio::time::timeout(TIMEOUT, wait_flooded)
        .await
        .unwrap()
        .unwrap();

    // The handle never read: whatever lagged, `Finished` is last, then `None`.
    assert_eq!(finished(&mut handle).await, Outcome::Succeeded);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_subscriber_sees_later_events() {
    let registry = JobRegistry::default();
    let (go, wait_go) = oneshot::channel::<()>();
    let handle = registry
        .start(JobSpec::new("logs"), |ctx| async move {
            let _ = wait_go.await;
            ctx.log(vec!["hello".to_owned()]);
            ctx.log(Vec::new());
            Outcome::Succeeded
        })
        .unwrap();
    let mut events = registry.subscribe(handle.id()).unwrap();
    assert_eq!(events.job_id(), handle.id());
    go.send(()).unwrap();
    let log = tokio::time::timeout(TIMEOUT, events.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        log.kind,
        JobEventKind::Log(LogBatch::Lines(vec!["hello".to_owned()]))
    );
    let done = tokio::time::timeout(TIMEOUT, events.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(done.kind, JobEventKind::Finished(Outcome::Succeeded));
    assert_eq!(events.recv().await, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn late_subscribe_replays_finished_until_reaped() {
    let registry = JobRegistry::default();
    let mut handle = registry
        .start(JobSpec::new("late"), |_ctx| async {
            Err(AppError::validation("late", "nope"))
        })
        .unwrap();
    let outcome = finished(&mut handle).await;
    let id = handle.id().clone();

    for _ in 0..2 {
        let mut late = registry.subscribe(&id).unwrap();
        let event = late.recv().await.unwrap();
        assert_eq!(event.job_id, id);
        assert_eq!(event.kind, JobEventKind::Finished(outcome.clone()));
        assert_eq!(late.recv().await, None);
    }
    assert_eq!(registry.outcome(&id).unwrap(), Some(outcome.clone()));

    assert_eq!(registry.reap(&id).unwrap(), outcome);
    assert_eq!(
        registry.subscribe(&id).err().unwrap().kind(),
        AppErrorKind::NotFound
    );
    assert_eq!(
        registry.reap(&id).unwrap_err().kind(),
        AppErrorKind::NotFound
    );
    assert_eq!(
        registry.cancel(&id).unwrap_err().kind(),
        AppErrorKind::NotFound
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reap_of_running_job_conflicts() {
    let registry = JobRegistry::default();
    let mut handle = registry
        .start(JobSpec::new("wait"), |ctx| async move {
            ctx.cancelled().await;
            Outcome::Cancelled
        })
        .unwrap();
    assert_eq!(registry.outcome(handle.id()).unwrap(), None);
    assert_eq!(
        registry.reap(handle.id()).unwrap_err().kind(),
        AppErrorKind::Conflict
    );
    handle.cancel();
    assert_eq!(finished(&mut handle).await, Outcome::Cancelled);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_is_scoped_to_the_owning_job() {
    let registry = JobRegistry::default();
    let wait_cancel = |ctx: super::JobContext| async move {
        ctx.log(vec![ctx.id().to_string()]);
        ctx.cancelled().await;
        Outcome::Cancelled
    };
    let mut a = registry.start(JobSpec::new("a"), wait_cancel).unwrap();
    let mut b = registry.start(JobSpec::new("b"), wait_cancel).unwrap();
    assert_ne!(a.id(), b.id());

    let unknown = JobId::from_string(uuid::Uuid::new_v4().to_string());
    assert_eq!(
        registry.cancel(&unknown).unwrap_err().kind(),
        AppErrorKind::NotFound
    );

    registry.cancel(a.id()).unwrap();
    let a_id = a.id().clone();
    let mut a_events = Vec::new();
    loop {
        let event = next(&mut a).await.unwrap();
        assert_eq!(event.job_id, a_id, "foreign event on handle A");
        let done = matches!(event.kind, JobEventKind::Finished(_));
        a_events.push(event.kind);
        if done {
            break;
        }
    }
    assert_eq!(
        a_events,
        vec![
            JobEventKind::Log(LogBatch::Lines(vec![a_id.to_string()])),
            JobEventKind::Finished(Outcome::Cancelled),
        ]
    );

    // B is untouched: still running, token not cancelled.
    assert!(!b.cancel_token().is_cancelled());
    assert_eq!(registry.outcome(b.id()).unwrap(), None);
    let b_log = next(&mut b).await.unwrap();
    assert_eq!(b_log.job_id, *b.id());

    b.cancel();
    assert_eq!(finished(&mut b).await, Outcome::Cancelled);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn panicking_job_fails_instead_of_hanging() {
    let registry = JobRegistry::default();
    let key = JobKey::new("panic-key");
    let mut handle = registry
        .start(JobSpec::new("panic").with_key(key.clone()), |_ctx| {
            panics("boom /home/user/secret")
        })
        .unwrap();
    match finished(&mut handle).await {
        Outcome::Failed(error) => {
            assert_eq!(error.kind(), AppErrorKind::Internal);
            assert!(!error.message().contains("secret"));
        }
        other => panic!("expected Failed, got {other:?}"),
    }
    // Key released after the panic.
    let mut again = registry
        .start(JobSpec::new("again").with_key(key), |_ctx| async {
            Outcome::Succeeded
        })
        .unwrap();
    assert_eq!(finished(&mut again).await, Outcome::Succeeded);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retention_evicts_oldest_finished() {
    let registry = registry_with(8, 2);
    let mut ids = Vec::new();
    for _ in 0..3 {
        let mut handle = registry
            .start(JobSpec::new("r"), |_ctx| async { Outcome::Succeeded })
            .unwrap();
        finished(&mut handle).await;
        ids.push(handle.id().clone());
    }
    assert_eq!(
        registry.subscribe(&ids[0]).err().unwrap().kind(),
        AppErrorKind::NotFound
    );
    assert_eq!(registry.outcome(&ids[1]).unwrap(), Some(Outcome::Succeeded));
    assert_eq!(registry.outcome(&ids[2]).unwrap(), Some(Outcome::Succeeded));

    // Reaping frees a slot: the next finish keeps both remaining jobs.
    registry.reap(&ids[1]).unwrap();
    let mut handle = registry
        .start(JobSpec::new("r"), |_ctx| async { Outcome::Succeeded })
        .unwrap();
    finished(&mut handle).await;
    assert!(registry.outcome(&ids[2]).is_ok());
    assert!(registry.outcome(handle.id()).is_ok());
}

#[test]
fn invalid_capacity_is_rejected() {
    for capacity in [0, crate::MAX_EVENT_CAPACITY + 1] {
        let error = JobRegistry::new(JobRegistryConfig {
            event_capacity: capacity,
            retention: 1,
        })
        .err()
        .unwrap();
        assert_eq!(
            error.kind(),
            AppErrorKind::Validation {
                field: "event_capacity"
            }
        );
    }
}

#[test]
fn start_without_runtime_is_unavailable() {
    let error = JobRegistry::default()
        .start(JobSpec::new("x"), |_ctx| async { Outcome::Succeeded })
        .err()
        .unwrap();
    assert_eq!(error.kind(), AppErrorKind::Unavailable);
}

#[test]
fn job_ids_are_uuid_v4() {
    let id = JobId::new_v4();
    let parsed = uuid::Uuid::parse_str(id.as_str()).unwrap();
    assert_eq!(parsed.get_version_num(), 4);
    assert_eq!(id.to_string(), id.as_str());
}
