//! Flatpak Background portal (`org.freedesktop.portal.Background`) grant
//! holder.
//!
//! Holds the RAII grant returned by [`BackgroundPortal::request`] for the app
//! lifetime and exposes the resolved [`BackgroundProtectionState`] so the UI
//! can render the watchdog-protection capability. The portal is abstracted
//! behind [`BackgroundPortal`] so tests can drive state transitions without a
//! D-Bus session; production uses [`XdgBackgroundPortal`].
//!
//! UI integration of the protection state is deferred: the
//! `get_background_protection_state` command is registered but has no
//! frontend consumer yet (ADR-0002 § Capability integration).
//!
//! See `docs/architecture/adr-0002-flatpak-portal-contracts.md` § Background
//! portal contract.

use std::fmt;
use std::future::Future;
use std::sync::Mutex;
use std::time::Duration;

use crosshook_core::platform::portals::background::{self, BackgroundError};
use serde::Serialize;
use tokio::sync::Notify;

/// User-facing reason passed to `RequestBackground` at startup.
pub const BACKGROUND_REQUEST_REASON: &str =
    "keep CrossHook running during game launches so the watchdog can clean up gamescope on exit";

/// Runtime state of CrossHook's Background portal grant.
///
/// Mirrors the decision matrix in ADR-0002: derived from
/// [`BackgroundPortal::supported`] plus the grant result. Native builds omit
/// this row (the UI renders nothing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundProtectionState {
    /// Native build — portal does not apply. Initial state under native.
    NotApplicable,
    /// Flatpak build, but the `RequestBackground` call has not resolved yet.
    /// Transient initial state under Flatpak; distinct from `Unavailable` so
    /// consumers can distinguish "request in flight" from "request failed".
    Pending,
    /// Flatpak + grant returned successfully. Watchdog survives window
    /// minimize.
    Available,
    /// Flatpak + portal reachable but the request was denied. The watchdog
    /// still runs but may not survive a long minimize.
    Degraded,
    /// Flatpak + portal unreachable. No protection at all.
    Unavailable,
}

/// Opaque RAII grant handle; dropping it releases the grant. Implemented for
/// every `Send + Sync + Debug` type so fakes need no D-Bus types.
pub trait BackgroundGrantHandle: Send + Sync + fmt::Debug + 'static {}

impl<T: Send + Sync + fmt::Debug + 'static> BackgroundGrantHandle for T {}

/// Portal-agnostic `RequestBackground` failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackgroundRequestError {
    /// Not running under Flatpak; the request is a no-op.
    NotSandboxed,
    /// A grant already exists or a request is in flight for this process.
    AlreadyRequested,
    /// The portal denied the request (or the response timed out).
    PortalDenied,
    /// Transport-level failure (message from the transport).
    Transport(String),
}

impl fmt::Display for BackgroundRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSandboxed => {
                f.write_str("not running under Flatpak; RequestBackground is a no-op")
            }
            Self::AlreadyRequested => f.write_str(
                "RequestBackground already succeeded for this process; \
                 must not be called again",
            ),
            Self::PortalDenied => f.write_str("xdg-desktop-portal denied the background request"),
            Self::Transport(message) => write!(f, "D-Bus transport error: {message}"),
        }
    }
}

impl std::error::Error for BackgroundRequestError {}

impl From<BackgroundError> for BackgroundRequestError {
    fn from(error: BackgroundError) -> Self {
        match error {
            BackgroundError::NotSandboxed => Self::NotSandboxed,
            BackgroundError::AlreadyRequested => Self::AlreadyRequested,
            BackgroundError::PortalDenied => Self::PortalDenied,
            BackgroundError::DBusProtocol(inner) => Self::Transport(inner.to_string()),
        }
    }
}

/// Result of a `RequestBackground` call.
pub type BackgroundRequestResult = Result<Box<dyn BackgroundGrantHandle>, BackgroundRequestError>;

/// Background portal backend.
pub trait BackgroundPortal: Send + Sync {
    /// `true` when the portal applies (Flatpak); native builds return `false`.
    fn supported(&self) -> bool;

    /// Requests background permission. `reason` is the user-facing string the
    /// portal may surface; `autostart` is passed through.
    fn request(
        &self,
        reason: &str,
        autostart: bool,
    ) -> impl Future<Output = BackgroundRequestResult> + Send;
}

/// Production portal over `crosshook_core::platform::portals::background`.
/// Native builds short-circuit inside the core call and make zero D-Bus calls.
#[derive(Debug, Clone, Copy, Default)]
pub struct XdgBackgroundPortal;

impl BackgroundPortal for XdgBackgroundPortal {
    fn supported(&self) -> bool {
        background::background_supported()
    }

    async fn request(&self, reason: &str, autostart: bool) -> BackgroundRequestResult {
        background::request_background(reason, autostart)
            .await
            .map(|grant| Box::new(grant) as Box<dyn BackgroundGrantHandle>)
            .map_err(BackgroundRequestError::from)
    }
}

/// App-lifetime holder for the portal grant. The grant is dropped (released)
/// when the holder is dropped.
pub struct BackgroundGrantHolder {
    inner: Mutex<BackgroundGrantState>,
    /// Fires `notify_waiters` once when [`BackgroundGrantHolder::store_result`]
    /// is called. Callers that need to synchronize with the one-time portal
    /// init (e.g., the watchdog spawn site) can await it via
    /// [`BackgroundGrantHolder::wait_for_initialization`].
    ready: Notify,
}

#[derive(Debug)]
struct BackgroundGrantState {
    protection: BackgroundProtectionState,
    grant: Option<Box<dyn BackgroundGrantHandle>>,
    /// `true` once [`BackgroundGrantHolder::store_result`] has been called.
    /// Before that, `protection` carries `Pending` (Flatpak) or
    /// `NotApplicable` (native), and the `grant` is `None`.
    initialized: bool,
}

// Toolkit shells share the holder across threads (e.g. Tauri `.manage()`
// requires `Send + Sync`). This assertion fires at compile time so a grant
// type that loses `Send + Sync` (the production grant relies on
// `zbus::Connection: Send + Sync`, guaranteed since zbus >= 5) surfaces here.
static_assertions::assert_impl_all!(BackgroundGrantHolder: Send, Sync);

impl BackgroundGrantHolder {
    /// Creates a holder for the production portal ([`XdgBackgroundPortal`]).
    pub fn new() -> Self {
        Self::for_portal(&XdgBackgroundPortal)
    }

    /// Creates a holder whose initial state follows `portal.supported()`.
    ///
    /// Supported portals start [`BackgroundProtectionState::Pending`] — the
    /// outcome of the one-time request is not yet known. Unsupported portals
    /// (native builds) start [`BackgroundProtectionState::NotApplicable`] and
    /// are considered initialized from the start.
    pub fn for_portal(portal: &impl BackgroundPortal) -> Self {
        let (protection, initialized) = if portal.supported() {
            (BackgroundProtectionState::Pending, false)
        } else {
            (BackgroundProtectionState::NotApplicable, true)
        };
        Self {
            inner: Mutex::new(BackgroundGrantState {
                protection,
                grant: None,
                initialized,
            }),
            ready: Notify::new(),
        }
    }

    /// Performs the one-time `RequestBackground` call through `portal` and
    /// stores the result. Unsupported portals short-circuit inside
    /// `request` (no D-Bus traffic) and resolve to `NotApplicable`.
    ///
    /// Logs `NotSandboxed` at debug level and any other failure once at warn
    /// level; launches always continue.
    pub async fn request_and_store(&self, portal: &impl BackgroundPortal) {
        let result = portal.request(BACKGROUND_REQUEST_REASON, false).await;
        if let Err(error) = &result {
            match error {
                BackgroundRequestError::NotSandboxed => {
                    tracing::debug!(
                        "background portal: not running under Flatpak; skipping RequestBackground"
                    );
                }
                other => {
                    tracing::warn!(
                        %other,
                        "background portal: RequestBackground failed; watchdog protection degraded"
                    );
                }
            }
        }
        self.store_result(result);
    }

    /// Stores the result of a `RequestBackground` call and signals all
    /// awaiters of [`BackgroundGrantHolder::wait_for_initialization`].
    ///
    /// Must be called exactly once per process lifetime. A later
    /// `AlreadyRequested` result never discards a held grant (see below);
    /// other results replace the stored state. The notify is idempotent.
    ///
    /// Successful grants update the holder to `Available` and keep the
    /// RAII handle alive until the holder is dropped. Denials update to
    /// `Degraded`. Transport failures update to `Unavailable`.
    /// `NotSandboxed` always maps to `NotApplicable`. `AlreadyRequested`
    /// (a contract violation) keeps the current state and grant when a grant
    /// is held, and only falls back to `Unavailable` when none is.
    pub fn store_result(&self, result: BackgroundRequestResult) {
        {
            let mut state = self.lock();
            let (protection, grant) = match result {
                Ok(grant) => (BackgroundProtectionState::Available, Some(grant)),
                Err(BackgroundRequestError::NotSandboxed) => {
                    (BackgroundProtectionState::NotApplicable, None)
                }
                Err(BackgroundRequestError::PortalDenied) => {
                    (BackgroundProtectionState::Degraded, None)
                }
                Err(BackgroundRequestError::Transport(_)) => {
                    (BackgroundProtectionState::Unavailable, None)
                }
                Err(BackgroundRequestError::AlreadyRequested) => {
                    tracing::warn!(
                        "store_result called after RequestBackground already succeeded \
                         or is in-flight; this is a programming error"
                    );
                    if state.grant.is_some() {
                        // The prior grant is still valid; keep it and its state.
                        return;
                    }
                    // No grant to fall back on: report conservatively rather
                    // than leave a stale `Pending`.
                    (BackgroundProtectionState::Unavailable, None)
                }
            };
            state.protection = protection;
            state.grant = grant;
            state.initialized = true;
        }
        self.ready.notify_waiters();
    }

    /// Returns the current protection state.
    ///
    /// Readers receive [`BackgroundProtectionState::Pending`] while the
    /// one-time portal request is in flight. Use
    /// [`BackgroundGrantHolder::wait_for_initialization`] to synchronize.
    pub fn protection_state(&self) -> BackgroundProtectionState {
        self.lock().protection
    }

    /// Returns `true` while we hold an active grant. Used by the watchdog
    /// spawn site to log whether the launch is running with protection.
    pub fn has_active_grant(&self) -> bool {
        self.lock().grant.is_some()
    }

    /// Returns `true` once [`BackgroundGrantHolder::store_result`] has been
    /// called. Unsupported (native) holders return `true` from construction.
    pub fn is_initialized(&self) -> bool {
        self.lock().initialized
    }

    /// Awaits the one-time `RequestBackground` call with a timeout and
    /// returns the resolved protection state.
    ///
    /// If already initialized returns immediately. Otherwise subscribes to
    /// `ready` and waits up to `timeout` for
    /// [`BackgroundGrantHolder::store_result`] to fire. On timeout returns
    /// the current state (still [`BackgroundProtectionState::Pending`]).
    pub async fn wait_for_initialization(&self, timeout: Duration) -> BackgroundProtectionState {
        if self.is_initialized() {
            return self.protection_state();
        }
        // Pin and enable the Notified future *before* the double-check so that
        // any `notify_waiters` call that fires between enable() and the await
        // is captured. Without enable(), a notification fired after creation
        // but before the future is first polled would be silently dropped.
        let mut notified = std::pin::pin!(self.ready.notified());
        notified.as_mut().enable();
        if self.is_initialized() {
            return self.protection_state();
        }
        let _ = tokio::time::timeout(timeout, notified).await;
        self.protection_state()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BackgroundGrantState> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Default for BackgroundGrantHolder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
