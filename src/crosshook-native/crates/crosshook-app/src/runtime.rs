//! Runtime ownership: either a runtime this crate built, or a borrowed handle.

use tokio::runtime::{Builder, Handle, Runtime};

use crate::error::AppError;

/// Owns a Tokio runtime or merely wraps a [`Handle`] to a host's runtime.
pub(crate) struct AppRuntime {
    handle: Handle,
    owned: Option<Runtime>,
}

impl AppRuntime {
    /// Builds a multi-thread runtime owned by this value.
    pub(crate) fn owned() -> Result<Self, AppError> {
        let runtime = Builder::new_multi_thread()
            .thread_name("crosshook-app")
            .enable_all()
            .build()?;
        Ok(Self {
            handle: runtime.handle().clone(),
            owned: Some(runtime),
        })
    }

    /// Wraps a host runtime; never shuts it down.
    pub(crate) fn borrowed(handle: Handle) -> Self {
        Self {
            handle,
            owned: None,
        }
    }

    pub(crate) fn handle(&self) -> &Handle {
        &self.handle
    }
}

impl Drop for AppRuntime {
    fn drop(&mut self) {
        // `shutdown_background` never blocks, so it is safe to call from a
        // thread that is itself inside a Tokio runtime (plain `drop` would panic).
        if let Some(runtime) = self.owned.take() {
            runtime.shutdown_background();
        }
    }
}
