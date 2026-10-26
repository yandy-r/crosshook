//! Launch pipeline service: validation, spawn, log streaming and session
//! finalization for game and trainer launches, independent of any UI toolkit.
//!
//! [`LaunchService::start_game`] and [`LaunchService::start_trainer`] return a
//! [`LaunchSession`] whose [`LaunchEvents`] stream was subscribed before the
//! child was spawned. Presenters relay the events; they never see the log
//! file, the registry or the finalizer.
//!
//! Module ownership (one file per slice, each under 500 lines):
//! `gamescope` predicate and watchdog choice, `warnings`/`portal`/`diagnostics`
//! helpers, `streaming` stdio tee, `snapshot` post-exit effects, `finalizer`
//! drop-safe wrap-up, `queries` read-only commands, `prepare`/`run` shared
//! start path, `game`/`trainer` builder dispatch.

mod diagnostics;
mod finalizer;
mod game;
mod gamescope;
mod portal;
mod prepare;
mod queries;
mod run;
mod snapshot;
mod spawner;
mod streaming;
mod trainer;
mod types;
mod warnings;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::Arc;

use crosshook_core::launch::LaunchSessionRegistry;
use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::ProfileStore;
use crosshook_core::runtime_helpers::{self, HelperRoots, RuntimeHelper};
use crosshook_core::settings::SettingsStore;

use super::BackgroundGrantHolder;
use crate::{AppError, AppErrorKind};

pub use diagnostics::read_log_tail;
pub use gamescope::{execution_method, gamescope_required, pid_capture_required};
pub use spawner::{ProcessSpawner, TokioSpawner};
pub use streaming::stream_child_log;
pub use types::{
    InjectionLevel, InjectionLogData, InjectionSource, LaunchEnv, LaunchEvent, LaunchEvents,
    LaunchSession, LaunchStartError, LAUNCH_EVENT_CAPACITY,
};

pub(super) use crate::services::run_blocking;

/// Where the service looks for the Steam runtime helper scripts.
#[derive(Debug, Clone)]
pub enum HelperSource {
    /// Roots detected for the current process (production).
    Detect,
    /// Fixed roots, so tests never depend on the install layout.
    Roots(HelperRoots),
}

/// Async application service owning the launch pipeline.
///
/// Clones share the stores, the session registry and the spawner. The service
/// holds no toolkit handle: presenters pass requests in and relay the returned
/// event streams.
#[derive(Clone)]
pub struct LaunchService {
    profiles: ProfileStore,
    settings: SettingsStore,
    metadata: MetadataStore,
    registry: Arc<LaunchSessionRegistry>,
    background: Option<Arc<BackgroundGrantHolder>>,
    spawner: Arc<dyn ProcessSpawner>,
    helpers: HelperSource,
    env: Option<LaunchEnv>,
}

impl LaunchService {
    /// Builds the service over the shared stores. `background` is the
    /// Background-portal grant holder the gamescope watchdog waits on, if any.
    /// Production defaults: [`TokioSpawner`], detected helper roots and a
    /// detected [`LaunchEnv`].
    pub fn new(
        profiles: ProfileStore,
        settings: SettingsStore,
        metadata: MetadataStore,
        background: Option<Arc<BackgroundGrantHolder>>,
    ) -> Self {
        Self {
            profiles,
            settings,
            metadata,
            registry: Arc::new(LaunchSessionRegistry::new()),
            background,
            spawner: Arc::new(TokioSpawner),
            helpers: HelperSource::Detect,
            env: None,
        }
    }

    /// Replaces the process spawner (test seam).
    #[must_use]
    pub fn with_spawner(mut self, spawner: Arc<dyn ProcessSpawner>) -> Self {
        self.spawner = spawner;
        self
    }

    /// Fixes the host facts instead of detecting them (test seam).
    #[must_use]
    pub fn with_env(mut self, env: LaunchEnv) -> Self {
        self.env = Some(env);
        self
    }

    /// Replaces where runtime helpers are resolved from (test seam).
    #[must_use]
    pub fn with_helper_source(mut self, helpers: HelperSource) -> Self {
        self.helpers = helpers;
        self
    }

    /// The fixed [`LaunchEnv`], or the detected one.
    pub(super) fn launch_env(&self) -> LaunchEnv {
        self.env.unwrap_or_else(LaunchEnv::detect)
    }

    /// Resolves a runtime helper script.
    ///
    /// # Errors
    /// [`AppErrorKind::Unavailable`] when no candidate location holds an
    /// acceptable helper.
    pub(super) fn resolve_helper(&self, helper: RuntimeHelper) -> Result<PathBuf, AppError> {
        let resolved = match &self.helpers {
            HelperSource::Detect => runtime_helpers::resolve(helper),
            HelperSource::Roots(roots) => runtime_helpers::resolve_with(roots, helper),
        };
        resolved.map_err(|error| AppError::new(AppErrorKind::Unavailable, error.to_string()))
    }

    /// The session registry shared by every launch of this service.
    pub(super) fn registry(&self) -> &Arc<LaunchSessionRegistry> {
        &self.registry
    }
}

// Presenters share the service across threads (Tauri `.manage()` requires
// `Send + Sync`); this fires at compile time if a field loses either bound.
static_assertions::assert_impl_all!(LaunchService: Send, Sync);
