//! The application owner handle: runtime, event bus, and task spawning.

use std::future::Future;
use std::sync::Arc;

use tokio::runtime::Handle;
use tokio_util::sync::CancellationToken;

use crate::error::AppError;
use crate::events::{EventBus, EventReceiver, DEFAULT_EVENT_CAPACITY};
use crate::runtime::AppRuntime;

/// Shared state behind every [`App`] clone.
struct Inner {
    runtime: AppRuntime,
    events: EventBus,
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
/// # Shutdown
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
    /// Creates an app owning a new multi-thread Tokio runtime with the
    /// default event-bus capacity.
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
        Ok(Self {
            inner: Arc::new(Inner {
                runtime: AppRuntime::owned()?,
                events: EventBus::new(capacity)?,
                shutdown: CancellationToken::new(),
            }),
        })
    }

    /// Creates an app reusing an ambient Tokio runtime via its [`Handle`].
    /// The host runtime is never shut down by this app. This constructor cannot
    /// fail: the event bus uses the always-valid default capacity.
    pub fn with_handle(handle: Handle) -> Self {
        Self {
            inner: Arc::new(Inner {
                runtime: AppRuntime::borrowed(handle),
                events: EventBus::default(),
                shutdown: CancellationToken::new(),
            }),
        }
    }

    /// Returns the shared event bus.
    pub fn events(&self) -> &EventBus {
        &self.inner.events
    }

    /// Subscribes to the shared event bus.
    pub fn subscribe(&self) -> EventReceiver {
        self.inner.events.subscribe()
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

#[cfg(test)]
mod tests;
