//! Launch-service data types: the per-session event stream, the session
//! handle returned to presenters, the start error, and the crate-private
//! publisher and finalizer context shared by the leaf modules.
//!
//! Every session owns one bounded [`tokio::sync::broadcast`] channel created
//! *before* the child is spawned (subscribe-before-start), so no event can be
//! published without a receiver having been given the chance to see it.

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use crosshook_core::launch::{
    is_inside_gamescope_session, CoexistenceAdvisoryRecord, DiagnosticReport,
    LaunchHookExecutionContext, LaunchSessionRegistry, LaunchValidationIssue, SessionId,
    SessionKind, WatchdogOutcome,
};
use crosshook_core::metadata::MetadataStore;
use crosshook_core::platform;
use crosshook_core::platform::portals::gamemode::GameModeRegistration;
use crosshook_core::profile::LaunchHook;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};

use crate::log_tail::LogBatch;
use crate::AppError;

/// Capacity of each session's event channel. A subscriber that falls further
/// behind sees a [`LogBatch::Dropped`] notice instead of the skipped events.
pub const LAUNCH_EVENT_CAPACITY: usize = 256;

/// One event of a launch session's stream.
///
/// [`LaunchEvent::Exited`] is published exactly once per session, after every
/// other event.
#[derive(Debug, Clone, PartialEq)]
pub enum LaunchEvent {
    /// Raw log output, including empty lines.
    Log {
        /// Session that produced the output.
        session_id: SessionId,
        /// The coalesced lines, or a dropped-events notice.
        batch: LogBatch,
    },
    /// A runtime injection row (trainer sessions only).
    Injection {
        /// Session the row belongs to.
        session_id: SessionId,
        /// Display-safe row data.
        event: InjectionLogData,
    },
    /// The post-exit diagnostic, sanitized for display.
    Diagnostic {
        /// Session that was analyzed.
        session_id: SessionId,
        /// The diagnostic report.
        report: Box<DiagnosticReport>,
    },
    /// The child exited; always the last event of a session.
    Exited {
        /// Session that exited.
        session_id: SessionId,
        /// Exit code, when the child exited normally.
        code: Option<i32>,
        /// Terminating signal, when the child was killed by one.
        signal: Option<i32>,
    },
}

impl LaunchEvent {
    /// The session this event belongs to.
    pub fn session_id(&self) -> SessionId {
        match self {
            Self::Log { session_id, .. }
            | Self::Injection { session_id, .. }
            | Self::Diagnostic { session_id, .. }
            | Self::Exited { session_id, .. } => *session_id,
        }
    }
}

/// Severity of an [`InjectionLogData`] row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectionLevel {
    /// Informational progress.
    Info,
    /// Recoverable problem.
    Warning,
    /// Failure.
    Error,
}

/// Origin of an [`InjectionLogData`] row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectionSource {
    /// The trainer program itself.
    Trainer,
    /// The injection step.
    Injection,
    /// The runtime or launcher.
    Runtime,
}

/// Display-safe data of one injection-log row.
///
/// `message` must stay scoped and sanitized: no raw helper output, environment
/// dumps or unsanitized filesystem paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InjectionLogData {
    /// When the row was produced; presenters format the timestamp.
    pub at: SystemTime,
    /// Profile name, trimmed, or `"Unknown profile"`.
    pub profile_name: String,
    /// Kind of the session that produced the row.
    pub session_kind: SessionKind,
    /// Row severity.
    pub level: InjectionLevel,
    /// Row origin.
    pub source: InjectionSource,
    /// Sanitized message text.
    pub message: String,
    /// Whether the row reports an unsupported runtime.
    pub unsupported_runtime: bool,
}

impl InjectionLogData {
    /// Builds a row stamped with the current time. `profile_name` is trimmed;
    /// `None` or a blank name becomes `"Unknown profile"`.
    pub fn new(
        profile_name: Option<&str>,
        kind: SessionKind,
        level: InjectionLevel,
        source: InjectionSource,
        message: impl Into<String>,
        unsupported_runtime: bool,
    ) -> Self {
        Self {
            at: SystemTime::now(),
            profile_name: profile_name
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .unwrap_or("Unknown profile")
                .to_string(),
            session_kind: kind,
            level,
            source,
            message: message.into(),
            unsupported_runtime,
        }
    }
}

/// A started launch: the session identity plus its event stream.
#[derive(Debug)]
pub struct LaunchSession {
    /// Unique id of this session.
    pub id: SessionId,
    /// Whether this is a game or a trainer session.
    pub kind: SessionKind,
    /// Non-fatal warnings collected before the spawn.
    pub warnings: Vec<LaunchValidationIssue>,
    /// Log file the child's output is teed into.
    pub log_path: PathBuf,
    /// The session's event stream, subscribed before the child was spawned.
    pub events: LaunchEvents,
}

/// Receiving end of one session's event stream.
#[derive(Debug)]
pub struct LaunchEvents {
    id: SessionId,
    rx: broadcast::Receiver<LaunchEvent>,
    done: bool,
}

impl LaunchEvents {
    /// The session this stream belongs to.
    pub fn session_id(&self) -> SessionId {
        self.id
    }

    /// Next event of the session.
    ///
    /// A subscriber that lagged sees a [`LogBatch::Dropped`] log event for the
    /// skipped events; the terminal [`LaunchEvent::Exited`] is never skipped.
    /// Returns `None` after `Exited` was delivered or the channel closed.
    pub async fn recv(&mut self) -> Option<LaunchEvent> {
        if self.done {
            return None;
        }
        match self.rx.recv().await {
            Ok(event) => Some(self.observe(event)),
            Err(RecvError::Lagged(n)) => Some(self.dropped(n)),
            Err(RecvError::Closed) => {
                self.done = true;
                None
            }
        }
    }

    /// Events already queued, without waiting. Used to recover pre-spawn
    /// events when a launch fails to start.
    pub fn drain_ready(&mut self) -> Vec<LaunchEvent> {
        let mut ready = Vec::new();
        while !self.done {
            match self.rx.try_recv() {
                Ok(event) => ready.push(self.observe(event)),
                Err(TryRecvError::Lagged(n)) => ready.push(self.dropped(n)),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Closed) => self.done = true,
            }
        }
        ready
    }

    fn observe(&mut self, event: LaunchEvent) -> LaunchEvent {
        if matches!(event, LaunchEvent::Exited { .. }) {
            self.done = true;
        }
        event
    }

    fn dropped(&self, n: u64) -> LaunchEvent {
        LaunchEvent::Log {
            session_id: self.id,
            batch: LogBatch::Dropped { n },
        }
    }
}

/// Failure to start a launch, with any events published before the failure
/// (for example pre-spawn injection rows) so the presenter can still show them.
#[derive(Debug)]
pub struct LaunchStartError {
    /// Sanitized failure.
    pub error: AppError,
    /// Events published before the failure, in order.
    pub events: Vec<LaunchEvent>,
}

impl From<AppError> for LaunchStartError {
    fn from(error: AppError) -> Self {
        Self {
            error,
            events: Vec::new(),
        }
    }
}

impl fmt::Display for LaunchStartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.error, f)
    }
}

impl std::error::Error for LaunchStartError {}

/// Host facts the launch predicates depend on, injectable for tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchEnv {
    /// Running inside the Flatpak sandbox.
    pub is_flatpak: bool,
    /// The process runs inside a gamescope compositor session.
    pub inside_gamescope_session: bool,
}

impl LaunchEnv {
    /// Reads the facts from the current process environment.
    pub fn detect() -> Self {
        Self {
            is_flatpak: platform::is_flatpak(),
            inside_gamescope_session: is_inside_gamescope_session(),
        }
    }
}

/// Sending end of one session's event stream.
#[derive(Clone)]
pub(super) struct EventPublisher {
    id: SessionId,
    tx: broadcast::Sender<LaunchEvent>,
}

impl EventPublisher {
    /// Creates the session channel and its first receiver, before any spawn.
    pub(super) fn channel(id: SessionId) -> (Self, LaunchEvents) {
        let (tx, rx) = broadcast::channel(LAUNCH_EVENT_CAPACITY);
        (
            Self { id, tx },
            LaunchEvents {
                id,
                rx,
                done: false,
            },
        )
    }

    /// Publishes `event`; never blocks. Having no receivers is not an error:
    /// the session keeps running when the presenter dropped its stream.
    pub(super) fn publish(&self, event: LaunchEvent) {
        let _ = self.tx.send(event);
    }

    pub(super) fn log(&self, batch: LogBatch) {
        self.publish(LaunchEvent::Log {
            session_id: self.id,
            batch,
        });
    }

    pub(super) fn injection(&self, data: InjectionLogData) {
        self.publish(LaunchEvent::Injection {
            session_id: self.id,
            event: data,
        });
    }

    pub(super) fn diagnostic(&self, report: DiagnosticReport) {
        self.publish(LaunchEvent::Diagnostic {
            session_id: self.id,
            report: Box::new(report),
        });
    }

    pub(super) fn exited(&self, code: Option<i32>, signal: Option<i32>) {
        self.publish(LaunchEvent::Exited {
            session_id: self.id,
            code,
            signal,
        });
    }
}

/// Everything the session finalizer needs once the child has exited.
pub(super) struct SessionContext {
    pub session_id: SessionId,
    pub kind: SessionKind,
    /// Execution method the session was launched with.
    pub method: &'static str,
    pub profile_name: Option<String>,
    pub steam_app_id: String,
    pub steam_client_path: String,
    pub trainer_host_path: Option<String>,
    pub log_path: PathBuf,
    pub operation_id: Option<String>,
    pub watchdog_outcome: WatchdogOutcome,
    pub coexistence: Vec<CoexistenceAdvisoryRecord>,
    pub post_exit_hooks: Vec<LaunchHook>,
    pub hook_context: LaunchHookExecutionContext,
    pub portal_guard: Option<GameModeRegistration>,
    pub metadata: MetadataStore,
    pub registry: Arc<LaunchSessionRegistry>,
}
