use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crosshook_core::community::{GitInvocation, GitOutput, GitRunError, TapGitRunner};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use super::helpers::{tap, write_manifest, Harness};
use crate::jobs::{JobEventKind, JobHandle, Outcome};
use crate::AppErrorKind;

pub(super) async fn finished(mut handle: JobHandle) -> Outcome {
    tokio::time::timeout(Duration::from_secs(10), async move {
        while let Some(event) = handle.recv().await {
            if let JobEventKind::Finished(outcome) = event.kind {
                return outcome;
            }
        }
        panic!("job event stream closed before Finished")
    })
    .await
    .expect("job did not finish")
}

struct GateRunner {
    cancel: CancellationToken,
    release: CancellationToken,
    calls: Arc<AtomicUsize>,
    started: mpsc::UnboundedSender<()>,
}
impl TapGitRunner for GateRunner {
    fn run<'a>(
        &'a self,
        _invocation: GitInvocation,
    ) -> Pin<Box<dyn Future<Output = Result<GitOutput, GitRunError>> + Send + 'a>> {
        Box::pin(async move {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                let _ = self.started.send(());
                tokio::select! {
                    () = self.cancel.cancelled() => return Err(GitRunError::Cancelled),
                    () = self.release.cancelled() => {},
                }
            }
            Ok(GitOutput {
                success: true,
                stdout: "0123456789abcdef".into(),
                stderr: String::new(),
            })
        })
    }
    fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }
}

#[tokio::test(flavor = "current_thread")]
async fn community_sync_is_nonblocking_and_queued_job_cancels_without_conflict() {
    let release = CancellationToken::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let (tx, mut started) = mpsc::unbounded_channel();
    let factory = {
        let release = release.clone();
        let calls = calls.clone();
        Arc::new(move |cancel| {
            Arc::new(GateRunner {
                cancel,
                release: release.clone(),
                calls: calls.clone(),
                started: tx.clone(),
            }) as Arc<dyn TapGitRunner>
        })
    };
    let harness = Harness::with_runner_factory(factory);
    let subscription = tap("https://example.invalid/community.git");
    harness.service.add_tap(subscription.clone()).await.unwrap();
    let workspace = harness.taps.resolve_workspace(&subscription).unwrap();
    write_manifest(
        &workspace.local_path.join("community-profile.json"),
        "Queued Game",
    );
    let service = harness.service.clone();
    let waiter = tokio::spawn(async move { service.sync().await });
    tokio::time::timeout(Duration::from_secs(5), started.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(harness.service.jobs().job_counts(), (1, 0));
    // This task can run on the same single-thread executor while git is gated.
    let (progress_tx, progress_rx) = oneshot::channel();
    tokio::spawn(async move {
        progress_tx.send(()).unwrap();
    });
    tokio::time::timeout(Duration::from_secs(1), progress_rx)
        .await
        .unwrap()
        .unwrap();
    let queued = harness.service.start_sync().unwrap();
    let queued_id = queued.id().clone();
    assert_eq!(harness.service.jobs().job_counts(), (2, 0));
    queued.cancel();
    assert_eq!(finished(queued).await, Outcome::Cancelled);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(harness.service.sync_result(&queued_id).unwrap().is_none());
    harness.service.jobs().reap(&queued_id).unwrap();
    release.cancel();
    tokio::time::timeout(Duration::from_secs(5), waiter)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(harness.service.jobs().job_counts(), (0, 0));
}

struct SleepRunner {
    cancel: CancellationToken,
    started: Arc<Mutex<Option<oneshot::Sender<u32>>>>,
}
impl TapGitRunner for SleepRunner {
    fn run<'a>(
        &'a self,
        invocation: GitInvocation,
    ) -> Pin<Box<dyn Future<Output = Result<GitOutput, GitRunError>> + Send + 'a>> {
        Box::pin(async move {
            let started = self.started.lock().unwrap().take();
            let invocation = GitInvocation {
                args: vec!["60".into()],
                directory: None,
                ..invocation
            };
            super::super::runner::run_test_program(&self.cancel, "sleep", invocation, started).await
        })
    }
    fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }
}

#[tokio::test]
async fn community_cancelled_fetch_kills_child_without_cached_success_or_metadata_writes() {
    let (tx, rx) = oneshot::channel();
    let started = Arc::new(Mutex::new(Some(tx)));
    let factory = Arc::new(move |cancel| {
        Arc::new(SleepRunner {
            cancel,
            started: started.clone(),
        }) as Arc<dyn TapGitRunner>
    });
    let harness = Harness::with_runner_factory(factory);
    let subscription = tap("https://example.invalid/cached.git");
    harness.service.add_tap(subscription.clone()).await.unwrap();
    let workspace = harness.taps.resolve_workspace(&subscription).unwrap();
    write_manifest(
        &workspace.local_path.join("community-profile.json"),
        "Must Not Index",
    );
    let handle = harness.service.start_sync().unwrap();
    let id = handle.id().clone();
    let pid = tokio::time::timeout(Duration::from_secs(5), rx)
        .await
        .unwrap()
        .unwrap();
    handle.cancel();
    assert_eq!(finished(handle).await, Outcome::Cancelled);
    assert!(
        !std::path::Path::new(&format!("/proc/{pid}")).exists(),
        "supervision must reap its child"
    );
    assert!(harness.service.sync_result(&id).unwrap().is_none());
    assert!(harness
        .metadata
        .list_community_tap_profiles(None)
        .unwrap()
        .is_empty());
    assert!(harness
        .metadata
        .lookup_community_tap_id(&subscription.url, "")
        .unwrap()
        .is_none());
    harness.service.jobs().reap(&id).unwrap();
}

#[tokio::test]
async fn community_offline_rejects_before_constructing_runner_and_reaps_failure() {
    let harness = Harness::with_runner_factory(Arc::new(|_| {
        panic!("offline sync must not construct a runner")
    }));
    harness
        .settings
        .update(|settings| {
            settings.offline_mode = true;
            Ok::<_, ()>(())
        })
        .unwrap()
        .unwrap();
    let error = harness.service.sync().await.unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Unavailable);
    assert_eq!(error.message(), "offline mode is enabled");
    assert_eq!(harness.service.jobs().job_counts(), (0, 0));
}

#[tokio::test]
async fn community_dropped_waiter_does_not_leak_result_or_job() {
    let release = CancellationToken::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let (tx, mut started) = mpsc::unbounded_channel();
    let factory = {
        let release = release.clone();
        Arc::new(move |cancel| {
            Arc::new(GateRunner {
                cancel,
                release: release.clone(),
                calls: calls.clone(),
                started: tx.clone(),
            }) as Arc<dyn TapGitRunner>
        })
    };
    let harness = Harness::with_runner_factory(factory);
    let subscription = tap("https://example.invalid/abandoned.git");
    harness.service.add_tap(subscription.clone()).await.unwrap();
    let workspace = harness.taps.resolve_workspace(&subscription).unwrap();
    write_manifest(
        &workspace.local_path.join("community-profile.json"),
        "Abandoned Game",
    );
    let service = harness.service.clone();
    let waiter = tokio::spawn(async move { service.sync().await });
    tokio::time::timeout(Duration::from_secs(5), started.recv())
        .await
        .unwrap()
        .unwrap();
    waiter.abort();
    assert!(waiter.await.unwrap_err().is_cancelled());
    release.cancel();
    tokio::time::timeout(Duration::from_secs(5), async {
        while harness.service.jobs().job_counts() != (0, 0) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(harness.service.sync_results.lock().unwrap().is_empty());
}

#[tokio::test]
async fn community_start_prunes_result_slots_evicted_by_registry() {
    let mut harness = Harness::new();
    harness.service.jobs = crate::jobs::JobRegistry::new(crate::jobs::JobRegistryConfig {
        retention: 1,
        ..crate::jobs::JobRegistryConfig::default()
    })
    .unwrap();
    let first = harness.service.start_sync().unwrap();
    let first_id = first.id().clone();
    assert_eq!(finished(first).await, Outcome::Succeeded);
    let second = harness.service.start_sync().unwrap();
    assert_eq!(finished(second).await, Outcome::Succeeded);
    assert!(harness.service.jobs().outcome(&first_id).is_err());
    let third = harness.service.start_sync().unwrap();
    assert!(!harness
        .service
        .sync_results
        .lock()
        .unwrap()
        .contains_key(&first_id));
    assert_eq!(finished(third).await, Outcome::Succeeded);
}
