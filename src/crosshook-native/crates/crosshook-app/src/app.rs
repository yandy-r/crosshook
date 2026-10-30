//! The application owner handle: runtime, event bus, and task spawning.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use tokio::runtime::Handle;
use tokio_util::sync::CancellationToken;

use crate::error::AppError;
use crate::events::{EventBus, EventReceiver, DEFAULT_EVENT_CAPACITY};
use crate::jobs::{JobRegistry, DEFAULT_KILL_GRACE};
use crate::runtime::AppRuntime;
use crate::services::process::child::DRAIN_GRACE;

/// Shared state behind every [`App`] clone.
struct Inner {
    runtime: AppRuntime,
    events: EventBus,
    jobs: JobRegistry,
    shutdown: CancellationToken,
}

impl Drop for Inner {
    fn drop(&mut self) {
        // Wake app-owned tasks so their wrappers resolve `Cancelled` instead of
        // waiting on a runtime that is going away (owned) or surviving (borrowed).
        self.shutdown.cancel();
    }
}

/// Application owner handle. Cheap to [`Clone`]; all clones share one runtime
/// and event bus. Dropping the last clone shuts the app down.
///
/// Shells construct the fully initialized application with [`App::bootstrap`],
/// which returns a [`BootstrappedApp`](crate::bootstrap::BootstrappedApp). The
/// runtime-only constructors remain useful for embedded jobs and do not
/// initialize stores or startup snapshots.
///
/// # Shutdown
/// - Explicit ([`App::shutdown`]): cancels every job and app task, then waits
///   (bounded) for running jobs to finish, so supervised child process groups
///   are terminated before the shell exits the process. Shells whose event
///   loop ends with `process::exit` (Tauri) must call it: drop never runs.
/// - Owned runtime ([`App::new`]): the runtime is consumed with
///   [`tokio::runtime::Runtime::shutdown_background`] when the last clone
///   drops. This is safe to do from inside another Tokio runtime. Tasks are
///   dropped without another poll, so work that cannot tolerate being dropped
///   mid-await (non-abortable blocking work) is unsupported.
/// - Borrowed runtime ([`App::with_handle`]): the host runtime is never shut
///   down; only tasks spawned through this app are cancelled.
///
/// # Caution
/// Do not capture an `App` clone inside a task that never ends: the clone
/// keeps the owner alive, so the app never shuts down.
pub struct App {
    inner: Arc<Inner>,
}

impl Clone for App {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl App {
    /// Default bound for [`App::shutdown`]: the process-group kill grace
    /// ([`DEFAULT_KILL_GRACE`]), the bounded log-pipe drain after the group
    /// exited ([`DRAIN_GRACE`]), plus one second for supervised jobs to reap
    /// their children and publish `Finished`. Quitting while a process job
    /// runs can therefore delay the exit by up to this long.
    pub const DEFAULT_SHUTDOWN_TIMEOUT: Duration = DEFAULT_KILL_GRACE
        .saturating_add(DRAIN_GRACE)
        .saturating_add(Duration::from_secs(1));

    /// Creates an app owning a new multi-thread Tokio runtime with the
    /// default event-bus capacity.
    /// This creates a runtime-only owner; use [`App::bootstrap`] for stores
    /// and services after completing pre-runtime initialization.
    ///
    /// # Errors
    /// Fails if the runtime cannot be built (e.g. thread spawn failure).
    pub fn new() -> Result<Self, AppError> {
        Self::with_event_capacity(DEFAULT_EVENT_CAPACITY)
    }

    /// Like [`App::new`] with an explicit event-bus capacity.
    ///
    /// # Errors
    /// Fails if the runtime cannot be built or `capacity` is invalid
    /// (zero or above the maximum).
    pub fn with_event_capacity(capacity: usize) -> Result<Self, AppError> {
        let events = EventBus::new(capacity)?;
        Ok(Self::from_parts(AppRuntime::owned()?, events))
    }

    /// Creates an app reusing an ambient Tokio runtime via its [`Handle`].
    /// The host runtime is never shut down by this app. This constructor cannot
    /// fail: the event bus uses the always-valid default capacity.
    pub fn with_handle(handle: Handle) -> Self {
        Self::from_parts(AppRuntime::borrowed(handle), EventBus::default())
    }

    fn from_parts(runtime: AppRuntime, events: EventBus) -> Self {
        let shutdown = CancellationToken::new();
        let jobs = JobRegistry::for_app(runtime.handle().clone(), &shutdown);
        Self {
            inner: Arc::new(Inner {
                runtime,
                events,
                jobs,
                shutdown,
            }),
        }
    }

    /// Returns the shared event bus.
    pub fn events(&self) -> &EventBus {
        &self.inner.events
    }

    /// Returns a clone of the app runtime's [`Handle`], for spawning
    /// service-owned background tasks that follow the app's lifetime.
    pub(crate) fn runtime_handle(&self) -> Handle {
        self.inner.runtime.handle().clone()
    }

    /// Returns the app's job registry. Jobs spawn on the app runtime and their
    /// cancellation tokens are children of the app shutdown token, so app
    /// shutdown cancels every running job.
    pub fn jobs(&self) -> &JobRegistry {
        &self.inner.jobs
    }

    /// Subscribes to the shared event bus.
    pub fn subscribe(&self) -> EventReceiver {
        self.inner.events.subscribe()
    }

    /// Shuts the app down: cancels the app shutdown token (every job token
    /// and every [`App::spawn`] task) and waits until no job is running or
    /// `timeout` elapses. Returns whether every job finished in time.
    ///
    /// Process jobs terminate their process groups on cancel (`SIGTERM`, then
    /// `SIGKILL` after [`DEFAULT_KILL_GRACE`]) and drain their pipes for at
    /// most [`DRAIN_GRACE`], so [`App::DEFAULT_SHUTDOWN_TIMEOUT`] lets them
    /// finish. A job that ignores cancellation cannot extend the wait past
    /// `timeout`; the caller then exits with that job still running.
    ///
    /// The wait runs on the app runtime, so the returned future polls on any
    /// executor. Calling it again only waits again; the app stays cancelled.
    pub async fn shutdown(&self, timeout: Duration) -> bool {
        self.inner.shutdown.cancel();
        let jobs = self.inner.jobs.clone();
        let wait = self
            .inner
            .runtime
            .handle()
            .spawn(async move { tokio::time::timeout(timeout, jobs.wait_idle()).await });
        matches!(wait.await, Ok(Ok(())))
    }

    /// Spawns `f` on the app runtime immediately and returns a future that
    /// resolves to `f`'s output.
    ///
    /// The returned future is `Send + 'static` and polls fine on any executor
    /// (no ambient Tokio context required). If the app shuts down before `f`
    /// completes, the task is cancelled and the future resolves to
    /// [`AppErrorKind::Cancelled`](crate::AppErrorKind::Cancelled).
    ///
    /// Dropping the returned future does not stop the task: it detaches and
    /// keeps running on the app runtime until it completes or the app shuts
    /// down.
    pub fn spawn<F, T>(&self, f: F) -> impl Future<Output = Result<T, AppError>> + Send + 'static
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let shutdown = self.inner.shutdown.clone();
        let join = self.inner.runtime.handle().spawn(async move {
            tokio::select! {
                () = shutdown.cancelled() => None,
                value = f => Some(value),
            }
        });
        async move {
            match join.await {
                Ok(Some(value)) => Ok(value),
                Ok(None) => Err(AppError::cancelled()),
                Err(join_error) => Err(AppError::from(join_error)),
            }
        }
    }
}

static_assertions::assert_impl_all!(App: Send, Sync);
#[cfg(test)]
mod tests;
