//! Tauri-side integration for `org.freedesktop.portal.Background.RequestBackground`.
//!
//! Holds the RAII [`BackgroundGrant`] from
//! [`crosshook_core::platform::portals::background`] for the lifetime of the
//! Tauri app and exposes a Tauri command / state API so the frontend can
//! render the watchdog-protection capability status.
//!
//! See `docs/architecture/adr-0002-flatpak-portal-contracts.md` § Background
//! portal contract.

use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use crosshook_core::platform::portals::background::{self, BackgroundError, BackgroundGrant};
use serde::Serialize;
use tokio::sync::Notify;

/// Runtime state of CrossHook's Background portal grant.
///
/// Mirrors the decision matrix in ADR-0002: derived from
/// `background_supported()` plus the grant result. Native builds omit this
/// row (the UI renders nothing).
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

/// Tauri-managed holder for the portal grant. The underlying
/// [`BackgroundGrant`] is dropped when the app exits.
pub struct BackgroundGrantHolder {
    inner: Mutex<BackgroundGrantState>,
    /// Fires `notify_waiters` once when [`BackgroundGrantHolder::store_result`]
    /// is called. Callers that need to synchronize with the one-time portal
    /// init (e.g., the watchdog spawn site) can await it via
    /// [`BackgroundGrantHolder::wait_for_initialization`].
    ready: Arc<Notify>,
}

#[derive(Debug)]
struct BackgroundGrantState {
    protection: BackgroundProtectionState,
    grant: Option<BackgroundGrant>,
    /// `true` once [`BackgroundGrantHolder::store_result`] has been called.
    /// Before that, `protection` carries `Pending` (Flatpak) or
    /// `NotApplicable` (native), and the `grant` is `None`.
    initialized: bool,
}

impl BackgroundGrantHolder {
    /// Creates a holder with the initial state determined by whether we are
    /// running under Flatpak.
    ///
    /// Under Flatpak the initial state is [`BackgroundProtectionState::Pending`]
    /// — the outcome of the one-time `request_background` call is not yet
    /// known. Native builds initialize directly to
    /// [`BackgroundProtectionState::NotApplicable`] and are considered
    /// initialized from the start.
    pub fn new() -> Self {
        let (protection, initialized) = if background::background_supported() {
            (BackgroundProtectionState::Pending, false)
        } else {
            (BackgroundProtectionState::NotApplicable, true)
        };
        let holder = Self {
            inner: Mutex::new(BackgroundGrantState {
                protection,
                grant: None,
                initialized,
            }),
            ready: Arc::new(Notify::new()),
        };
        if initialized {
            // Native builds: no init to wait for. Pre-arm the notifier so
            // any late awaiter completes immediately.
            holder.ready.notify_waiters();
        }
        holder
    }

    /// Stores the result of a `request_background` call and signals all
    /// awaiters of [`BackgroundGrantHolder::wait_for_initialization`].
    ///
    /// Must be called exactly once per process lifetime. Subsequent calls
    /// overwrite the stored state but are harmless — the notify is
    /// idempotent.
    ///
    /// Successful grants update the holder to `Available` and keep the
    /// RAII handle alive until the holder is dropped. Denials update to
    /// `Degraded`. D-Bus transport failures update to `Unavailable`.
    /// Native builds always map to `NotApplicable`.
    pub fn store_result(&self, result: Result<BackgroundGrant, BackgroundError>) {
        let (protection, grant) = match result {
            Ok(grant) => (BackgroundProtectionState::Available, Some(grant)),
            Err(BackgroundError::NotSandboxed) => (BackgroundProtectionState::NotApplicable, None),
            Err(BackgroundError::PortalDenied) => (BackgroundProtectionState::Degraded, None),
            Err(BackgroundError::DBusProtocol(_)) => (BackgroundProtectionState::Unavailable, None),
        };
        {
            let mut state = self
                .inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.protection = protection;
            state.grant = grant;
            state.initialized = true;
        }
        self.ready.notify_waiters();
    }

    /// Returns the current protection state for IPC callers (frontend).
    ///
    /// Readers receive [`BackgroundProtectionState::Pending`] while the
    /// one-time portal request is in flight. Use
    /// [`BackgroundGrantHolder::wait_for_initialization`] to synchronize.
    pub fn protection_state(&self) -> BackgroundProtectionState {
        let state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.protection
    }

    /// Returns `true` while we hold an active grant. Used by the watchdog
    /// spawn site to log whether the launch is running with protection.
    pub fn has_active_grant(&self) -> bool {
        let state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.grant.is_some()
    }

    /// Returns `true` once [`BackgroundGrantHolder::store_result`] has been
    /// called. Native builds always return `true` from construction.
    pub fn is_initialized(&self) -> bool {
        let state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.initialized
    }

    /// Awaits the one-time `request_background` call with a timeout and
    /// returns the resolved protection state.
    ///
    /// If already initialized returns immediately. Otherwise subscribes to
    /// `ready` and waits up to `timeout` for
    /// [`BackgroundGrantHolder::store_result`] to fire. On timeout returns
    /// the current state (still [`BackgroundProtectionState::Pending`]).
    ///
    /// Call sites (e.g. the watchdog spawn task) use this to synchronize
    /// their logging/decisions with the one-time portal init without
    /// blocking startup.
    pub async fn wait_for_initialization(&self, timeout: Duration) -> BackgroundProtectionState {
        if self.is_initialized() {
            return self.protection_state();
        }
        // Subscribe before the final check to avoid missing a notification
        // that fires between the check and the await.
        let notified = self.ready.notified();
        if self.is_initialized() {
            return self.protection_state();
        }
        let _ = tokio::time::timeout(timeout, notified).await;
        self.protection_state()
    }
}

impl Default for BackgroundGrantHolder {
    fn default() -> Self {
        Self::new()
    }
}

/// Tauri command: report the current background-protection state so the
/// host-tool dashboard can surface it as a capability row.
///
/// Returns [`BackgroundProtectionState::Pending`] transiently at startup
/// under Flatpak while the one-time `request_background` call is in flight;
/// the frontend should treat Pending as an indeterminate state and refresh.
#[tauri::command]
pub fn get_background_protection_state(
    holder: tauri::State<'_, BackgroundGrantHolder>,
) -> BackgroundProtectionState {
    holder.protection_state()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_holder_is_not_applicable_on_native_builds() {
        // This test is only meaningful in the native test harness.
        if crosshook_core::platform::is_flatpak() {
            return;
        }
        let holder = BackgroundGrantHolder::new();
        assert_eq!(
            holder.protection_state(),
            BackgroundProtectionState::NotApplicable
        );
        assert!(!holder.has_active_grant());
        assert!(
            holder.is_initialized(),
            "native builds must be considered initialized at construction"
        );
    }

    #[test]
    fn storing_not_sandboxed_error_yields_not_applicable() {
        let holder = BackgroundGrantHolder::new();
        holder.store_result(Err(BackgroundError::NotSandboxed));
        assert_eq!(
            holder.protection_state(),
            BackgroundProtectionState::NotApplicable
        );
        assert!(holder.is_initialized());
    }

    #[test]
    fn storing_denied_error_yields_degraded() {
        let holder = BackgroundGrantHolder::new();
        holder.store_result(Err(BackgroundError::PortalDenied));
        assert_eq!(
            holder.protection_state(),
            BackgroundProtectionState::Degraded
        );
        assert!(holder.is_initialized());
    }

    fn current_thread_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime")
    }

    #[test]
    fn wait_for_initialization_returns_immediately_on_native() {
        if crosshook_core::platform::is_flatpak() {
            return;
        }
        let holder = BackgroundGrantHolder::new();
        let rt = current_thread_runtime();
        let state = rt.block_on(async {
            holder
                .wait_for_initialization(Duration::from_millis(10))
                .await
        });
        assert_eq!(state, BackgroundProtectionState::NotApplicable);
    }

    #[test]
    fn wait_for_initialization_times_out_when_store_result_never_runs() {
        // This behavioural assertion only fires on Flatpak test hosts
        // because the `Pending` path is gated on `background_supported()`.
        if !crosshook_core::platform::is_flatpak() {
            return;
        }
        let holder = BackgroundGrantHolder::new();
        let rt = current_thread_runtime();
        let state = rt.block_on(async {
            holder
                .wait_for_initialization(Duration::from_millis(10))
                .await
        });
        assert_eq!(state, BackgroundProtectionState::Pending);
    }

    #[test]
    fn wait_for_initialization_unblocks_when_store_result_fires() {
        let holder = Arc::new(BackgroundGrantHolder::new());
        let holder2 = Arc::clone(&holder);
        let rt = current_thread_runtime();
        rt.block_on(async move {
            let handle = tokio::spawn(async move {
                holder2
                    .wait_for_initialization(Duration::from_secs(5))
                    .await
            });
            tokio::task::yield_now().await;
            holder.store_result(Err(BackgroundError::PortalDenied));
            let state = handle.await.expect("waiter task should not panic");
            // Native builds already return `NotApplicable` from construction
            // (the holder pre-arms the notifier), so the waiter sees that
            // state even though the explicit `store_result` was a denial.
            // Flatpak builds see the denial → `Degraded`.
            assert!(matches!(
                state,
                BackgroundProtectionState::NotApplicable | BackgroundProtectionState::Degraded
            ));
        });
    }
}
