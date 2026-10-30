//! The job registry: start, subscribe, cancel, and reap supervised jobs.

mod events;

use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tokio::runtime::Handle;
use tokio::sync::{broadcast, watch};
use tokio_util::sync::{CancellationToken, WaitForCancellationFuture};

pub use self::events::{JobEvents, JobHandle};
use super::types::{JobEvent, JobEventKind, JobId, JobKey, JobProgress, JobSpec, Outcome};
use crate::error::{AppError, AppErrorKind};
use crate::events::{DEFAULT_EVENT_CAPACITY, MAX_EVENT_CAPACITY};
use crate::log_tail::LogBatch;

/// Default number of finished, unreaped outcomes the registry retains.
pub const DEFAULT_JOB_RETENTION: usize = 64;

/// Registry limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobRegistryConfig {
    /// Per-job broadcast capacity (events buffered per receiver before it
    /// lags). Must be between 1 and [`MAX_EVENT_CAPACITY`].
    pub event_capacity: usize,
    /// Finished outcomes kept for late subscribers until reaped; the oldest is
    /// evicted beyond this cap. `0` retains nothing.
    pub retention: usize,
}

impl Default for JobRegistryConfig {
    fn default() -> Self {
        Self {
            event_capacity: DEFAULT_EVENT_CAPACITY,
            retention: DEFAULT_JOB_RETENTION,
        }
    }
}

struct RunningJob {
    key: Option<JobKey>,
    cancel: CancellationToken,
    tx: broadcast::Sender<JobEvent>,
}

#[derive(Default)]
struct State {
    running: HashMap<JobId, RunningJob>,
    keys: HashSet<JobKey>,
    finished: HashMap<JobId, Outcome>,
    /// Finish order of `finished` entries, oldest first.
    finished_order: VecDeque<JobId>,
}

struct Shared {
    config: JobRegistryConfig,
    /// Runtime to spawn on; `None` uses the ambient runtime at `start`.
    runtime: Option<Handle>,
    /// Parent of every job token.
    root: CancellationToken,
    state: Mutex<State>,
    /// Number of running jobs, updated under the `state` lock.
    running: watch::Sender<usize>,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        // The lock is never held across user code, so a poisoned state is
        // still consistent.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Supervisor for long-running jobs. Cheap to [`Clone`]; clones share state.
///
/// Jobs are cooperative: the registry never drops a running job future on
/// cancel. A job observes [`JobContext::cancelled`] and returns
/// [`Outcome::Cancelled`] (process jobs use
/// [`supervise_child`](super::supervise_child)). Only runtime shutdown drops a
/// job mid-flight, which still finishes it as `Cancelled`.
#[derive(Clone)]
pub struct JobRegistry {
    shared: Arc<Shared>,
}

impl Default for JobRegistry {
    /// Default limits, ambient runtime, no parent token.
    fn default() -> Self {
        Self::build(JobRegistryConfig::default(), None, CancellationToken::new())
    }
}

impl JobRegistry {
    /// Creates a registry that spawns on the ambient Tokio runtime at
    /// [`start`](Self::start).
    ///
    /// # Errors
    /// `Validation { field: "event_capacity" }` when the capacity is zero or
    /// above [`MAX_EVENT_CAPACITY`].
    pub fn new(config: JobRegistryConfig) -> Result<Self, AppError> {
        validate(config)?;
        Ok(Self::build(config, None, CancellationToken::new()))
    }

    /// Registry owned by an [`App`](crate::App): spawns on `runtime` and
    /// derives every job token from `parent` (the app shutdown token).
    pub(crate) fn for_app(runtime: Handle, parent: &CancellationToken) -> Self {
        Self::build(
            JobRegistryConfig::default(),
            Some(runtime),
            parent.child_token(),
        )
    }

    fn build(config: JobRegistryConfig, runtime: Option<Handle>, root: CancellationToken) -> Self {
        Self {
            shared: Arc::new(Shared {
                config,
                runtime,
                root,
                state: Mutex::new(State::default()),
                running: watch::Sender::new(0),
            }),
        }
    }

    /// Starts `job` and returns its handle, already subscribed to its events.
    ///
    /// The channel and first receiver are created and the job is recorded
    /// before the task is spawned, so even an instantly finishing job is
    /// observed by the handle. The job's result converts into an [`Outcome`]
    /// (`Outcome` itself or `Result<(), AppError>`). A panic finishes the job
    /// as `Failed` (`Internal`).
    ///
    /// # Errors
    /// - `Conflict` when `spec.key` is held by a running job; nothing spawns.
    /// - `Unavailable` when the registry has no runtime and none is ambient.
    pub fn start<F, Fut, R>(&self, spec: JobSpec, job: F) -> Result<JobHandle, AppError>
    where
        F: FnOnce(JobContext) -> Fut + Send + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: Into<Outcome>,
    {
        let runtime = match &self.shared.runtime {
            Some(handle) => handle.clone(),
            None => Handle::try_current().map_err(|_| {
                AppError::new(AppErrorKind::Unavailable, "no async runtime for jobs")
            })?,
        };

        let id = JobId::new_v4();
        let cancel = self.shared.root.child_token();
        let (tx, rx) = broadcast::channel(self.shared.config.event_capacity);
        {
            let mut state = self.shared.lock();
            if let Some(key) = &spec.key {
                if !state.keys.insert(key.clone()) {
                    return Err(AppError::new(
                        AppErrorKind::Conflict,
                        format!("a {} job is already running for this target", spec.kind),
                    ));
                }
            }
            state.running.insert(
                id.clone(),
                RunningJob {
                    key: spec.key,
                    cancel: cancel.clone(),
                    tx: tx.clone(),
                },
            );
            self.shared.running.send_replace(state.running.len());
        }

        let publisher = Publisher::new(tx);
        let context = JobContext {
            id: id.clone(),
            cancel: cancel.clone(),
            publisher: publisher.clone(),
        };
        // Built outside the task so that a task dropped before its first poll
        // (runtime shutting down) still finishes the job.
        let mut guard = FinishGuard {
            shared: Arc::clone(&self.shared),
            id: id.clone(),
            publisher: Some(publisher),
        };
        let kind = spec.kind;
        drop(runtime.spawn(async move {
            tracing::debug!(job_kind = kind, "job started");
            let outcome = job(context).await.into();
            guard.finish(outcome);
        }));

        Ok(JobHandle {
            events: JobEvents::live(id.clone(), rx),
            id,
            cancel,
        })
    }

    /// Subscribes to a job's events.
    ///
    /// Running: a live receiver (events from now on, then `Finished`).
    /// Finished but unreaped: a receiver that yields the retained `Finished`
    /// once.
    ///
    /// # Errors
    /// `NotFound` for unknown, reaped, or evicted ids.
    pub fn subscribe(&self, id: &JobId) -> Result<JobEvents, AppError> {
        let state = self.shared.lock();
        if let Some(job) = state.running.get(id) {
            return Ok(JobEvents::live(id.clone(), job.tx.subscribe()));
        }
        match state.finished.get(id) {
            Some(outcome) => Ok(JobEvents::replay(JobEvent {
                job_id: id.clone(),
                kind: JobEventKind::Finished(outcome.clone()),
            })),
            None => Err(not_found()),
        }
    }

    /// Requests cancellation of one running job. Cancelling an already
    /// finished (unreaped) job is a no-op.
    ///
    /// # Errors
    /// `NotFound` for unknown, reaped, or evicted ids; no other job is touched.
    pub fn cancel(&self, id: &JobId) -> Result<(), AppError> {
        let state = self.shared.lock();
        if let Some(job) = state.running.get(id) {
            job.cancel.cancel();
            Ok(())
        } else if state.finished.contains_key(id) {
            Ok(())
        } else {
            Err(not_found())
        }
    }

    /// Returns the outcome of a finished, unreaped job, or `None` while it
    /// runs.
    ///
    /// # Errors
    /// `NotFound` for unknown, reaped, or evicted ids.
    pub fn outcome(&self, id: &JobId) -> Result<Option<Outcome>, AppError> {
        let state = self.shared.lock();
        if state.running.contains_key(id) {
            return Ok(None);
        }
        state
            .finished
            .get(id)
            .cloned()
            .map(Some)
            .ok_or_else(not_found)
    }

    /// Forgets a finished job and returns its outcome.
    ///
    /// # Errors
    /// `NotFound` for unknown or already reaped ids; `Conflict` while the job
    /// is still running.
    pub fn reap(&self, id: &JobId) -> Result<Outcome, AppError> {
        let mut state = self.shared.lock();
        if state.running.contains_key(id) {
            return Err(AppError::new(
                AppErrorKind::Conflict,
                "job is still running",
            ));
        }
        let outcome = state.finished.remove(id).ok_or_else(not_found)?;
        state.finished_order.retain(|finished| finished != id);
        Ok(outcome)
    }

    /// Whether `key` is held by a running job. Released before that job's
    /// `Finished` event is published.
    pub fn is_key_held(&self, key: &JobKey) -> bool {
        self.shared.lock().keys.contains(key)
    }

    /// Resolves once no job is running (immediately when idle). Jobs started
    /// while waiting are waited for as well. Callers bound the wait, e.g. with
    /// [`tokio::time::timeout`]; a job that ignores cancellation never ends.
    pub async fn wait_idle(&self) {
        let mut running = self.shared.running.subscribe();
        // The sender lives in `self.shared`, so the channel never closes here.
        let _ = running.wait_for(|count| *count == 0).await;
    }

    /// `(running, finished-unreaped)` job counts, for tests asserting that a
    /// helper reaps its jobs.
    #[cfg(test)]
    pub(crate) fn job_counts(&self) -> (usize, usize) {
        let state = self.shared.lock();
        (state.running.len(), state.finished.len())
    }
}

fn validate(config: JobRegistryConfig) -> Result<(), AppError> {
    if config.event_capacity == 0 || config.event_capacity > MAX_EVENT_CAPACITY {
        return Err(AppError::validation(
            "event_capacity",
            "job event capacity must be between 1 and MAX_EVENT_CAPACITY",
        ));
    }
    Ok(())
}

fn not_found() -> AppError {
    AppError::new(AppErrorKind::NotFound, "job not found")
}

/// The job's event sender, closed exactly once by [`FinishGuard`].
///
/// Every publish takes the lock, so closing and sending `Finished` under it
/// makes `Finished` the last event; publishes from [`JobContext`] clones that
/// outlive the job are dropped silently.
#[derive(Clone)]
struct Publisher(Arc<Mutex<Option<broadcast::Sender<JobEvent>>>>);

impl Publisher {
    fn new(tx: broadcast::Sender<JobEvent>) -> Self {
        Self(Arc::new(Mutex::new(Some(tx))))
    }

    fn lock(&self) -> MutexGuard<'_, Option<broadcast::Sender<JobEvent>>> {
        // Only `send`/`take` run under the lock; a poisoned slot is consistent.
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn publish(&self, event: JobEvent) {
        if let Some(tx) = self.lock().as_ref() {
            // Never blocks; with no receivers the event is simply discarded.
            let _ = tx.send(event);
        }
    }

    /// Publishes `last` and closes the channel; later publishes are no-ops.
    fn close_with(&self, last: JobEvent) {
        let mut slot = self.lock();
        if let Some(tx) = slot.take() {
            let _ = tx.send(last);
        }
    }
}

/// Finishes a job exactly once: on normal completion, panic, or drop.
struct FinishGuard {
    shared: Arc<Shared>,
    id: JobId,
    publisher: Option<Publisher>,
}

impl FinishGuard {
    fn finish(&mut self, outcome: Outcome) {
        let Some(publisher) = self.publisher.take() else {
            return;
        };
        {
            let mut state = self.shared.lock();
            if let Some(job) = state.running.remove(&self.id) {
                if let Some(key) = job.key {
                    state.keys.remove(&key);
                }
            }
            self.shared.running.send_replace(state.running.len());
            let retention = self.shared.config.retention;
            if retention > 0 {
                state.finished.insert(self.id.clone(), outcome.clone());
                state.finished_order.push_back(self.id.clone());
                while state.finished_order.len() > retention {
                    if let Some(oldest) = state.finished_order.pop_front() {
                        state.finished.remove(&oldest);
                    }
                }
            }
        }
        // Published after the key is released and the outcome stored, so a
        // caller reacting to `Finished` can restart the key and query it.
        // Closing drops the last sender: receivers see `Finished`, then end.
        publisher.close_with(JobEvent {
            job_id: self.id.clone(),
            kind: JobEventKind::Finished(outcome),
        });
    }
}

impl Drop for FinishGuard {
    fn drop(&mut self) {
        if self.publisher.is_none() {
            return;
        }
        let outcome = if std::thread::panicking() {
            // Never surface the panic payload; it may embed user paths.
            Outcome::Failed(AppError::new(
                AppErrorKind::Internal,
                "job failed unexpectedly",
            ))
        } else {
            Outcome::Cancelled
        };
        self.finish(outcome);
    }
}

/// Handed to a job: its id, cancellation, and non-blocking event publishing.
/// Terminal `Finished` events are published by the registry only; once the
/// job finished, publishing through any (cloned) context is a silent no-op.
#[derive(Clone)]
pub struct JobContext {
    id: JobId,
    cancel: CancellationToken,
    publisher: Publisher,
}

impl JobContext {
    /// This job's id.
    pub fn id(&self) -> &JobId {
        &self.id
    }

    /// This job's cancellation token (a child of the registry token).
    pub fn cancel_token(&self) -> &CancellationToken {
        &self.cancel
    }

    /// Whether cancellation was requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    /// Resolves once cancellation is requested.
    pub fn cancelled(&self) -> WaitForCancellationFuture<'_> {
        self.cancel.cancelled()
    }

    /// Publishes log lines; an empty batch publishes nothing.
    pub fn log(&self, lines: Vec<String>) {
        if !lines.is_empty() {
            self.log_batch(LogBatch::Lines(lines));
        }
    }

    /// Publishes a log batch as-is (e.g. forwarded from a tailer).
    pub fn log_batch(&self, batch: LogBatch) {
        self.publish(JobEventKind::Log(batch));
    }

    /// Publishes a progress report.
    pub fn progress(&self, progress: JobProgress) {
        self.publish(JobEventKind::Progress(progress));
    }

    fn publish(&self, kind: JobEventKind) {
        self.publisher.publish(JobEvent {
            job_id: self.id.clone(),
            kind,
        });
    }
}
