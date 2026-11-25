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
//! drop-safe wrap-up, `queries` read-only commands, `sessions` running-session
//! events and snapshot, `steam_watch` Steam game exit watch, `prepare`/`run`
//! shared start path, `game`/`trainer` builder dispatch.

mod diagnostics;
mod finalizer;
mod game;
mod gamescope;
mod portal;
mod prepare;
mod queries;
mod run;
mod sessions;
mod snapshot;
mod spawner;
mod steam_watch;
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
#[cfg(test)]
use crosshook_core::runtime_helpers::HelperRoots;
use crosshook_core::runtime_helpers::{self, RuntimeHelper};
use crosshook_core::settings::SettingsStore;

use super::BackgroundGrantHolder;
use crate::events::EventBus;
use crate::jobs::LogDir;
use crate::{AppError, AppErrorKind};
use sessions::session_observer;
use steam_watch::{default_probe, ProcessProbe};

pub use diagnostics::read_log_tail;
pub use gamescope::{gamescope_required, pid_capture_required};
pub use sessions::{RunningSession, RunningSessions};
pub(crate) use spawner::{ProcessSpawner, TokioSpawner};
pub use streaming::stream_child_log;
pub(crate) use types::{InjectionLevel, InjectionSource, LaunchEnv};
pub use types::{
    InjectionLogData, LaunchEvent, LaunchEvents, LaunchSession, LaunchStartError,
    LAUNCH_EVENT_CAPACITY,
};

pub(super) use crate::services::run_blocking;

/// Where the service looks for the Steam runtime helper scripts.
#[derive(Debug, Clone)]
pub enum HelperSource {
    /// Roots detected for the current process (production).
    Detect,
    /// Fixed roots, so tests never depend on the install layout.
    #[cfg(test)]
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
    /// Game-executable probe of the Steam game watch (test seam).
    probe: ProcessProbe,
    background: Option<Arc<BackgroundGrantHolder>>,
    spawner: Arc<dyn ProcessSpawner>,
    helpers: HelperSource,
    env: Option<LaunchEnv>,
    /// Where [`crate::jobs::create_log_path_in`] creates the session log: the
    /// private directory resolved at bootstrap. Tests pass a tempdir-backed
    /// one so no test touches the real runtime or state directory.
    log_dir: LogDir,
}

impl LaunchService {
    /// Builds the service over the shared stores. `events` is the app bus
    /// that receives [`AppEvent::SessionStarted`](crate::AppEvent::SessionStarted)
    /// / [`AppEvent::SessionExited`](crate::AppEvent::SessionExited) for every
    /// registry change. `background` is the Background-portal grant holder the
    /// gamescope watchdog waits on, if any. `log_dir` is the private directory
    /// session logs are created in. Production defaults: [`TokioSpawner`],
    /// detected helper roots and a detected [`LaunchEnv`].
    pub fn new(
        profiles: ProfileStore,
        settings: SettingsStore,
        metadata: MetadataStore,
        events: EventBus,
        background: Option<Arc<BackgroundGrantHolder>>,
        log_dir: LogDir,
    ) -> Self {
        Self {
            profiles,
            settings,
            metadata,
            registry: Arc::new(LaunchSessionRegistry::with_observer(session_observer(
                events,
            ))),
            probe: default_probe(),
            background,
            spawner: Arc::new(TokioSpawner),
            helpers: HelperSource::Detect,
            env: None,
            log_dir,
        }
    }

    /// Replaces the Steam game watch's process probe (test seam).
    #[cfg(test)]
    #[must_use]
    pub(super) fn with_probe(mut self, probe: ProcessProbe) -> Self {
        self.probe = probe;
        self
    }

    /// Replaces the process spawner (test seam).
    #[cfg(test)]
    #[must_use]
    pub fn with_spawner(mut self, spawner: Arc<dyn ProcessSpawner>) -> Self {
        self.spawner = spawner;
        self
    }

    /// Fixes the host facts instead of detecting them (test seam).
    #[cfg(test)]
    #[must_use]
    pub fn with_env(mut self, env: LaunchEnv) -> Self {
        self.env = Some(env);
        self
    }

    /// Replaces where runtime helpers are resolved from (test seam).
    #[cfg(test)]
    #[must_use]
    pub fn with_helper_source(mut self, helpers: HelperSource) -> Self {
        self.helpers = helpers;
        self
    }

    /// Resolves a runtime helper script from the given [`HelperSource`].
    ///
    /// Separated from `&self` so callers can run filesystem probing inside
    /// [`run_blocking`] without holding a borrow across the blocking boundary.
    ///
    /// # Errors
    /// [`AppErrorKind::Unavailable`] when no candidate location holds an
    /// acceptable helper.
    pub(super) fn resolve_helper_with(
        helpers: &HelperSource,
        helper: RuntimeHelper,
    ) -> Result<PathBuf, AppError> {
        let resolved = match helpers {
            HelperSource::Detect => runtime_helpers::resolve(helper),
            #[cfg(test)]
            HelperSource::Roots(roots) => runtime_helpers::resolve_with(roots, helper),
        };
        resolved.map_err(|error| AppError::new(AppErrorKind::Unavailable, error.to_string()))
    }

    /// The session registry shared by every launch of this service.
    #[cfg(test)]
    pub(super) fn registry(&self) -> &Arc<LaunchSessionRegistry> {
        &self.registry
    }
}

// Presenters share the service across threads (Tauri `.manage()` requires
// `Send + Sync`); this fires at compile time if a field loses either bound.
static_assertions::assert_impl_all!(LaunchService: Send, Sync);
