//! Tauri-side integration for `org.freedesktop.portal.Background.RequestBackground`.
//!
//! Holds the RAII [`BackgroundGrant`] from
//! [`crosshook_core::platform::portals::background`] for the lifetime of the
//! Tauri app and exposes a Tauri command / state API so the frontend can
//! render the watchdog-protection capability status.
//!
//! See `docs/architecture/adr-0002-flatpak-portal-contracts.md` § Background
//! portal contract.

use std::sync::Mutex;

use crosshook_core::platform::portals::background::{self, BackgroundError, BackgroundGrant};
use serde::Serialize;

/// Runtime state of CrossHook's Background portal grant.
///
/// Mirrors the decision matrix in ADR-0002: derived from
/// `background_supported()` plus the grant result. Native builds omit this
/// row (the UI renders nothing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundProtectionState {
    /// Native build — portal does not apply.
    NotApplicable,
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
}

#[derive(Debug)]
struct BackgroundGrantState {
    protection: BackgroundProtectionState,
    grant: Option<BackgroundGrant>,
}

impl BackgroundGrantHolder {
    /// Creates a holder with the initial state determined by whether we are
    /// running under Flatpak. The actual grant is stored later via
    /// [`BackgroundGrantHolder::store_result`] once the async portal call
    /// completes.
    pub fn new() -> Self {
        let initial = if background::background_supported() {
            BackgroundProtectionState::Unavailable
        } else {
            BackgroundProtectionState::NotApplicable
        };
        Self {
            inner: Mutex::new(BackgroundGrantState {
                protection: initial,
                grant: None,
            }),
        }
    }

    /// Stores the result of a `request_background` call.
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
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.protection = protection;
        state.grant = grant;
    }

    /// Returns the current protection state for IPC callers (frontend).
    pub fn protection_state(&self) -> BackgroundProtectionState {
        let state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.protection
    }

    /// Returns true while we hold an active grant. Used by the watchdog
    /// spawn site to log whether the launch is running with protection.
    pub fn has_active_grant(&self) -> bool {
        let state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.grant.is_some()
    }
}

impl Default for BackgroundGrantHolder {
    fn default() -> Self {
        Self::new()
    }
}

/// Tauri command: report the current background-protection state so the
/// host-tool dashboard can surface it as a capability row.
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
    }

    #[test]
    fn storing_not_sandboxed_error_yields_not_applicable() {
        let holder = BackgroundGrantHolder::new();
        holder.store_result(Err(BackgroundError::NotSandboxed));
        assert_eq!(
            holder.protection_state(),
            BackgroundProtectionState::NotApplicable
        );
    }

    #[test]
    fn storing_denied_error_yields_degraded() {
        let holder = BackgroundGrantHolder::new();
        holder.store_result(Err(BackgroundError::PortalDenied));
        assert_eq!(
            holder.protection_state(),
            BackgroundProtectionState::Degraded
        );
    }
}
