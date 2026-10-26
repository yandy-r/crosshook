//! Session registration, spawn, stream task and cancel consumer (Task 3.1).
//!
//! Both entry points build a command, then hand it here. This module registers
//! the session before spawn, tees the child into its event stream, and starts
//! the one cancel consumer. It never signals the child.

use std::io;

use crosshook_core::launch::{
    build_launch_hook_execution_context, run_pre_launch_hooks, session_profile_key_for_name,
    LaunchHookExecutionContext, LaunchSessionRegistry, SessionId, SessionKind, TeardownReason,
    WatchdogOutcome,
};
use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::LaunchHook;
use tokio::process::{Child, Command};
use tokio::sync::broadcast;

use super::finalizer::SessionGuard;
use super::gamescope::{choose_cancel_consumer, spawn_cancel_consumer};
use super::portal::try_register_gamemode_portal_for_launch;
use super::prepare::Prepared;
use super::streaming::stream_session;
use super::types::{
    EventPublisher, InjectionLevel, InjectionLogData, InjectionSource, SessionContext,
};
use super::{run_blocking, LaunchEvents, LaunchService, LaunchSession, LaunchStartError};
use crate::presenter::display_path_tilde;
use crate::{AppError, AppErrorKind};

const TRAINER_UNSUPPORTED: &str =
    "DLL injection engine is not available; stored hook configuration was not applied.";

impl LaunchService {
    /// Runs portal registration, pre-launch hooks, spawn, and session streaming.
    ///
    /// The session is registered before spawn and removed again when spawn
    /// fails, without a history row. The portal guard is dropped on those
    /// early failures. The stream task starts before the cancel consumer.
    ///
    /// # Errors
    /// Hook-context preparation or process spawn failed. A trainer spawn
    /// failure includes the injection rows published before the process started.
    pub(super) async fn run_session(
        &self,
        prepared: Prepared,
        mut command: Command,
    ) -> Result<LaunchSession, LaunchStartError> {
        let Prepared {
            request,
            kind,
            execution_method,
            mut warnings,
            coexistence,
            log_path,
            gamescope,
        } = prepared;

        // Dropped, not unregistered, when a later step returns before the
        // session task owns it (I-22).
        let portal_guard =
            try_register_gamemode_portal_for_launch(&request, execution_method).await;

        let hook_context = build_launch_hook_execution_context(&request).map_err(|error| {
            AppError::new(
                AppErrorKind::Io,
                format!("failed to prepare launch hook context: {error}"),
            )
        })?;
        warnings
            .extend(run_pre_launch_hooks_blocking(&request.pre_launch_hooks, &hook_context).await?);

        let watchdog_outcome = WatchdogOutcome::new();
        let profile_key = session_profile_key_for_name(request.profile_name.as_deref());
        let (session_id, cancel_rx) = register_session(&self.registry, kind, profile_key);

        // Subscribed before spawn, so a presenter cannot miss the first event.
        let (publisher, mut events) = EventPublisher::channel(session_id);
        if kind == SessionKind::Trainer {
            publish_trainer(
                &publisher,
                request.profile_name.as_deref(),
                InjectionLevel::Info,
                InjectionSource::Trainer,
                "Trainer launch requested.",
                false,
            );
        }

        let child = match self.spawner.spawn(&mut command) {
            Ok(child) => child,
            Err(error) => {
                return Err(fail_spawn(
                    &self.registry,
                    kind,
                    execution_method,
                    error,
                    &publisher,
                    request.profile_name.as_deref(),
                    session_id,
                    &mut events,
                ));
            }
        };
        let child_pid = child.id();

        if kind == SessionKind::Trainer {
            publish_trainer(
                &publisher,
                request.profile_name.as_deref(),
                InjectionLevel::Info,
                InjectionSource::Trainer,
                "Trainer process started.",
                false,
            );
            publish_trainer(
                &publisher,
                request.profile_name.as_deref(),
                InjectionLevel::Warning,
                InjectionSource::Injection,
                TRAINER_UNSUPPORTED,
                true,
            );
        }

        let operation_id = record_launch_started(
            &self.metadata,
            request.profile_name.as_deref(),
            execution_method,
            &display_path_tilde(&log_path.to_string_lossy()),
        )
        .await;

        let ctx = SessionContext {
            session_id,
            kind,
            method: execution_method,
            profile_name: request.profile_name.clone(),
            steam_app_id: request.steam.app_id.clone(),
            steam_client_path: request.steam.steam_client_install_path.clone(),
            trainer_host_path: non_empty_trimmed(&request.trainer_host_path),
            log_path: log_path.clone(),
            operation_id,
            watchdog_outcome: watchdog_outcome.clone(),
            coexistence,
            post_exit_hooks: request.post_exit_hooks.clone(),
            hook_context,
            portal_guard,
            metadata: self.metadata.clone(),
            registry: self.registry.clone(),
        };

        // Stream first, then the cancel consumer (I-29). Neither path kills.
        drop(spawn_session_task(ctx, child, publisher));
        let consumer = choose_cancel_consumer(&gamescope, &request, kind, child_pid);
        spawn_cancel_consumer(
            consumer,
            session_id,
            watchdog_outcome,
            cancel_rx,
            self.background.clone(),
        );

        Ok(LaunchSession {
            id: session_id,
            kind,
            warnings,
            log_path,
            events,
        })
    }
}

fn register_session(
    registry: &LaunchSessionRegistry,
    kind: SessionKind,
    profile_key: String,
) -> (SessionId, broadcast::Receiver<TeardownReason>) {
    match kind {
        SessionKind::Game => registry.register(kind, profile_key),
        SessionKind::Trainer => {
            let (session_id, cancel_rx, parent_id) = registry.register_and_link_to_parent_of_kind(
                kind,
                profile_key.clone(),
                SessionKind::Game,
            );
            match parent_id {
                Some(parent) => tracing::info!(
                    trainer_session = %session_id,
                    parent_session = %parent,
                    profile_key = %profile_key,
                    "launch session: trainer linked to game parent"
                ),
                None => tracing::debug!(
                    trainer_session = %session_id,
                    profile_key = %profile_key,
                    "launch session: trainer registered without parent game"
                ),
            }
            (session_id, cancel_rx)
        }
    }
}

fn fail_spawn(
    registry: &LaunchSessionRegistry,
    kind: SessionKind,
    execution_method: &str,
    error: io::Error,
    publisher: &EventPublisher,
    profile_name: Option<&str>,
    session_id: SessionId,
    events: &mut LaunchEvents,
) -> LaunchStartError {
    if kind == SessionKind::Trainer {
        publish_trainer(
            publisher,
            profile_name,
            InjectionLevel::Error,
            InjectionSource::Trainer,
            "Trainer launch failed before the trainer process started.",
            false,
        );
    }
    registry.deregister(session_id);
    let message = match kind {
        SessionKind::Game => format!("failed to launch helper: {error}"),
        SessionKind::Trainer => {
            format!("failed to launch trainer (method={execution_method}): {error}")
        }
    };
    LaunchStartError {
        error: AppError::new(AppErrorKind::Io, message),
        events: events.drain_ready(),
    }
}

fn publish_trainer(
    publisher: &EventPublisher,
    profile_name: Option<&str>,
    level: InjectionLevel,
    source: InjectionSource,
    message: &str,
    unsupported_runtime: bool,
) {
    publisher.injection(InjectionLogData::new(
        profile_name,
        SessionKind::Trainer,
        level,
        source,
        message,
        unsupported_runtime,
    ));
}

async fn run_pre_launch_hooks_blocking(
    hooks: &[LaunchHook],
    context: &LaunchHookExecutionContext,
) -> Result<Vec<crosshook_core::launch::LaunchValidationIssue>, AppError> {
    let hooks = hooks.to_vec();
    let context = context.clone();
    run_blocking(move || Ok(run_pre_launch_hooks(&hooks, &context))).await
}

/// Writes the start row only after a successful spawn. A store or task
/// failure is logged and yields no operation id, so the launch still proceeds.
async fn record_launch_started(
    metadata: &MetadataStore,
    profile_name: Option<&str>,
    method: &'static str,
    log_path: &str,
) -> Option<String> {
    let metadata = metadata.clone();
    let profile_name = profile_name.map(str::to_owned);
    let log_path = log_path.to_owned();
    let recorded = run_blocking(move || {
        match metadata.record_launch_started(profile_name.as_deref(), method, Some(&log_path)) {
            Ok(operation_id) => Ok(operation_id),
            Err(error) => {
                tracing::warn!(%error, "record_launch_started failed");
                Ok(String::new())
            }
        }
    })
    .await;

    match recorded {
        Ok(operation_id) if !operation_id.is_empty() => Some(operation_id),
        Ok(_) => None,
        Err(error) => {
            tracing::warn!(%error, "metadata spawn_blocking join failed");
            None
        }
    }
}

fn non_empty_trimmed(path: &str) -> Option<String> {
    let path = path.trim();
    if path.is_empty() {
        None
    } else {
        Some(path.to_string())
    }
}

pub(super) fn spawn_session_task(
    ctx: SessionContext,
    child: Child,
    publisher: EventPublisher,
) -> tokio::task::JoinHandle<()> {
    let log_path = ctx.log_path.clone();
    // The guard is built before the spawn: if the task is dropped before it
    // is first polled, `Drop` still tears the session down.
    let guard = SessionGuard::new(ctx, publisher.clone());
    tokio::spawn(async move {
        let status = stream_session(child, &log_path, &publisher).await;
        guard.finish(status).await;
    })
}
