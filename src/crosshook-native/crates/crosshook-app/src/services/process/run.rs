//! A started process job as seen by its caller: the job's log stream, then
//! one typed completion.
//!
//! [`start_run`] starts the job body with a completion sender and a
//! `started` sender. [`StartingRun::started`] waits for the `started` signal,
//! so the caller holds the run (and can relay its events) before the body
//! produces any output. The body sends its completion before returning its
//! [`Outcome`], and the registry publishes `Finished` only after that, so
//! [`ProcessRun::next`] always finds the completion once `Finished` arrives.

use std::future::Future;

use tokio::sync::oneshot;

use crate::jobs::{JobContext, JobEventKind, JobEvents, JobId, JobRegistry, JobSpec, Outcome};
use crate::log_tail::LogBatch;
use crate::{AppError, AppErrorKind};

/// One event of a [`ProcessRun`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunEvent<C> {
    /// Log output (or a lag/truncation notice).
    Log(LogBatch),
    /// The job finished; `None` when it ended without sending a completion
    /// (cancelled before completing, or panicked).
    Finished(Option<C>),
}

/// A running process job: its id, its event stream and its typed completion.
///
/// Dropping a run before it yielded [`RunEvent::Finished`] leaves the job
/// running; a background task then waits for it to finish and reaps it.
pub struct ProcessRun<C> {
    run_id: JobId,
    events: Option<JobEvents>,
    completion: Option<oneshot::Receiver<C>>,
    jobs: JobRegistry,
}

impl<C: Send + 'static> ProcessRun<C> {
    /// The job id, used as the wire `run_id`.
    pub fn run_id(&self) -> &str {
        self.run_id.as_str()
    }

    /// The next event: `Log` batches in order, then exactly one `Finished`
    /// carrying the completion, then `None`. Progress events are skipped. The
    /// job is reaped before `Finished` is returned.
    pub async fn next(&mut self) -> Option<RunEvent<C>> {
        let events = self.events.as_mut()?;
        // The registry always publishes `Finished` before closing; an early
        // close (`None`) is treated the same way.
        while let Some(event) = events.recv().await {
            match event.kind {
                JobEventKind::Log(batch) => return Some(RunEvent::Log(batch)),
                JobEventKind::Progress(_) => {}
                JobEventKind::Finished(_) => break,
            }
        }
        self.events = None;
        if let Err(error) = self.jobs.reap(&self.run_id) {
            tracing::debug!(%error, "process job was already reaped");
        }
        let completion = match self.completion.take() {
            Some(receiver) => receiver.await.ok(),
            None => None,
        };
        Some(RunEvent::Finished(completion))
    }
}

impl<C> Drop for ProcessRun<C> {
    fn drop(&mut self) {
        let Some(mut events) = self.events.take() else {
            return;
        };
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            // No runtime to wait on; the registry evicts the finished job.
            return;
        };
        let jobs = self.jobs.clone();
        let id = self.run_id.clone();
        drop(runtime.spawn(async move {
            while events.recv().await.is_some() {}
            if let Err(error) = jobs.reap(&id) {
                tracing::debug!(%error, "process job was already reaped");
            }
        }));
    }
}

/// A job started by [`start_run`] whose body has not yet signalled
/// `started`.
pub(crate) struct StartingRun<C> {
    run: ProcessRun<C>,
    started_rx: oneshot::Receiver<()>,
}

impl<C: Send + 'static> StartingRun<C> {
    /// Waits until the body signals `started` and returns the run.
    ///
    /// # Errors
    /// When the body ends without signalling `started`, the run is drained
    /// and reaped, and its outcome becomes the error: `Failed(e)` → `e`,
    /// `Cancelled` → `Cancelled`, and a body that succeeded without starting
    /// → `Internal`.
    pub(crate) async fn started(self) -> Result<ProcessRun<C>, AppError> {
        let Self {
            mut run,
            started_rx,
        } = self;
        if started_rx.await.is_ok() {
            return Ok(run);
        }
        let Some(mut events) = run.events.take() else {
            return Err(not_started());
        };
        let mut outcome = None;
        while let Some(event) = events.recv().await {
            if let JobEventKind::Finished(finished) = event.kind {
                outcome = Some(finished);
            }
        }
        if let Err(error) = run.jobs.reap(&run.run_id) {
            tracing::debug!(%error, "process job was already reaped");
        }
        Err(match outcome {
            Some(Outcome::Failed(error)) => error,
            Some(Outcome::Cancelled) | None => {
                AppError::new(AppErrorKind::Cancelled, "the process job was cancelled")
            }
            Some(Outcome::Succeeded) => not_started(),
        })
    }
}

fn not_started() -> AppError {
    AppError::new(
        AppErrorKind::Internal,
        "the process job ended before it started",
    )
}

/// Starts `body` as a job.
///
/// The body gets its [`JobContext`], the completion sender and the `started`
/// sender. It must send `started` before producing any output (typically
/// right before spawning), send its completion before returning, and return
/// the job [`Outcome`].
///
/// # Errors
/// The registry's start errors (`Conflict` when `spec.key` is held,
/// `Unavailable` without a runtime); nothing runs then.
pub(crate) fn start_run<C, F, Fut>(
    jobs: &JobRegistry,
    spec: JobSpec,
    body: F,
) -> Result<StartingRun<C>, AppError>
where
    C: Send + 'static,
    F: FnOnce(JobContext, oneshot::Sender<C>, oneshot::Sender<()>) -> Fut + Send + 'static,
    Fut: Future<Output = Outcome> + Send + 'static,
{
    let (completion_tx, completion_rx) = oneshot::channel();
    let (started_tx, started_rx) = oneshot::channel();
    let handle = jobs.start(spec, move |ctx| body(ctx, completion_tx, started_tx))?;
    let run_id = handle.id().clone();
    Ok(StartingRun {
        run: ProcessRun {
            run_id,
            events: Some(handle.into_events()),
            completion: Some(completion_rx),
            jobs: jobs.clone(),
        },
        started_rx,
    })
}
