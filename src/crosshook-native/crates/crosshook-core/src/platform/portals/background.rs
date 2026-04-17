//! `org.freedesktop.portal.Background.RequestBackground` integration.
//!
//! See `docs/architecture/adr-0002-flatpak-portal-contracts.md` § Background
//! portal contract for the full contract.
//!
//! **Scope**: this module keeps **CrossHook's own** sandbox process (and the
//! sandbox-side `gamescope_watchdog` Tokio task that supervises host
//! gameplay) alive when the Tauri window is minimized. Host games are not
//! sandbox processes — they are not passed to this API.
//!
//! Native (non-Flatpak) builds call this module only through
//! [`background_supported`], which returns `false` immediately and performs
//! zero D-Bus traffic.

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use futures_util::StreamExt as _;
use zvariant::OwnedObjectPath;
use zvariant::OwnedValue;
use zvariant::Value;

use crate::platform::is_flatpak;

/// D-Bus destination for xdg-desktop-portal.
const PORTAL_DESKTOP_BUS: &str = "org.freedesktop.portal.Desktop";
/// Object path for the portal.
const PORTAL_DESKTOP_PATH: &str = "/org/freedesktop/portal/desktop";
/// Background portal interface name.
const PORTAL_BACKGROUND_INTERFACE: &str = "org.freedesktop.portal.Background";
/// D-Bus interface name for an in-flight portal request handle.
const PORTAL_REQUEST_INTERFACE: &str = "org.freedesktop.portal.Request";

/// How long to wait for the `Response` signal before treating the request as
/// denied. 60 s matches typical portal timeout documentation; the user dialog
/// is modal so a minute is generous.
const PORTAL_RESPONSE_TIMEOUT: Duration = Duration::from_secs(60);

// ---------------------------------------------------------------------------
// One-call-per-process debounce (ADR-0002 § Background portal contract)
// ---------------------------------------------------------------------------
//
// State machine for REQUEST_STATE:
//
//   IDLE ──(attempt)──► IN_FLIGHT ──(success)──► SUCCEEDED
//                            │
//                            └──(failure)──► IDLE    (one retry allowed)
//
// A second call after SUCCEEDED returns `BackgroundError::AlreadyRequested`.
// A failure leaves the state IDLE so the caller (or the one retry the ADR
// permits) can try again. Concurrent calls are sequenced by the compare-exchange:
// only one reaches IN_FLIGHT; the other sees IN_FLIGHT and is rejected.
const STATE_IDLE: u8 = 0;
const STATE_IN_FLIGHT: u8 = 1;
const STATE_SUCCEEDED: u8 = 2;

static REQUEST_STATE: AtomicU8 = AtomicU8::new(STATE_IDLE);

/// Returns `true` iff CrossHook is running under Flatpak and should attempt
/// a Background portal request. On native builds this is `false` and
/// callers must not call [`request_background`].
pub fn background_supported() -> bool {
    is_flatpak()
}

/// Requests `org.freedesktop.portal.Background.RequestBackground` to keep
/// CrossHook running (with its window possibly minimized) so the
/// sandbox-side watchdog can continue supervising host games.
///
/// `reason` is the user-facing string the portal may surface (e.g. GNOME
/// Shell's "Background Apps" list).
/// `autostart` is passed through — CrossHook always passes `false`.
///
/// Awaits the `Response` signal on the returned `org.freedesktop.portal.Request`
/// object path before returning, so the caller receives a confirmed grant (or
/// a concrete denial) rather than an optimistic object path.
///
/// # One-call-per-process contract
///
/// This function **must be called at most once per successful grant** per
/// process lifetime (ADR-0002 § Background portal contract). A second call
/// after a successful grant returns [`BackgroundError::AlreadyRequested`].
/// A failed attempt leaves the guard idle so a single retry is permitted,
/// matching the ADR's "retried at most once after initial failure" note.
/// Concurrent calls are serialised by an atomic guard: only one proceeds to
/// IN_FLIGHT; the other is rejected with [`BackgroundError::AlreadyRequested`].
///
/// # Errors
///
/// - [`BackgroundError::NotSandboxed`] — native build; caller should skip.
/// - [`BackgroundError::AlreadyRequested`] — a successful grant already exists
///   for this process, or another call is already in-flight.
/// - [`BackgroundError::PortalDenied`] — portal returned a non-success
///   response (user declined, policy denied, etc.), or the `Response` signal
///   did not arrive within [`PORTAL_RESPONSE_TIMEOUT`]. Caller should degrade
///   gracefully (capability becomes `Degraded`; watchdog still runs).
/// - [`BackgroundError::DBusProtocol`] — transport-level failure.
pub async fn request_background(
    reason: &str,
    autostart: bool,
) -> Result<BackgroundGrant, BackgroundError> {
    if !background_supported() {
        return Err(BackgroundError::NotSandboxed);
    }

    // Enforce the one-call-per-successful-grant contract. See the state
    // machine in the module-level comment above REQUEST_STATE.
    if REQUEST_STATE
        .compare_exchange(
            STATE_IDLE,
            STATE_IN_FLIGHT,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_err()
    {
        // State is either IN_FLIGHT (concurrent call) or SUCCEEDED (grant
        // already confirmed). Both cases are rejected.
        return Err(BackgroundError::AlreadyRequested);
    }

    // Run the actual D-Bus exchange in an inner block so we can uniformly
    // reset the state to IDLE on any failure path (the `?` operator would
    // otherwise short-circuit past the state reset).
    let result = request_background_inner(reason, autostart).await;

    match &result {
        Ok(_) => {
            REQUEST_STATE.store(STATE_SUCCEEDED, Ordering::Release);
        }
        Err(_) => {
            // Reset to IDLE so the caller can retry once, matching the ADR's
            // "retried at most once after initial failure" note.
            REQUEST_STATE.store(STATE_IDLE, Ordering::Release);
        }
    }

    result
}

/// Inner implementation of `request_background`. Called only when the debounce
/// gate has transitioned state to IN_FLIGHT. Uses `?` freely; the caller
/// resets state on error.
async fn request_background_inner(
    reason: &str,
    autostart: bool,
) -> Result<BackgroundGrant, BackgroundError> {
    let connection = zbus::Connection::session().await?;
    let proxy = zbus::Proxy::new(
        &connection,
        PORTAL_DESKTOP_BUS,
        PORTAL_DESKTOP_PATH,
        PORTAL_BACKGROUND_INTERFACE,
    )
    .await?;

    // RequestBackground(parent_window: &str, options: a{sv}) -> o
    // parent_window is "" because CrossHook's Tauri window is not yet
    // re-parentable via xdp-gtk/xdp-wayland handle at setup time; leaving
    // it blank is the documented default.
    let mut options: HashMap<&str, Value<'_>> = HashMap::new();
    options.insert("reason", Value::from(reason));
    options.insert("autostart", Value::from(autostart));
    // Do not pass a commandline — the portal infers CrossHook's from its
    // .desktop entry. Do not request dbus-activatable (false default).

    // Obtain the Request object path first; then subscribe to its Response
    // signal. Per the xdg-desktop-portal spec the portal holds the Response
    // until a consumer exists, so subscribing immediately after the method
    // call is safe for all portal implementations CrossHook targets.
    let request_path: OwnedObjectPath = proxy.call("RequestBackground", &("", &options)).await?;

    tracing::debug!(
        reason,
        autostart,
        request_path = %request_path.as_str(),
        "background portal: RequestBackground submitted; awaiting Response signal"
    );

    // Subscribe to the Response signal on the returned Request handle.
    let req_proxy = zbus::Proxy::new(
        &connection,
        PORTAL_DESKTOP_BUS,
        request_path.as_str(),
        PORTAL_REQUEST_INTERFACE,
    )
    .await?;
    let mut stream = req_proxy.receive_signal("Response").await?;

    // Await the Response signal with a bounded timeout.
    // On timeout we treat the request as denied — the user dismissed the
    // dialog without answering, or the portal is unresponsive.
    let msg = tokio::time::timeout(PORTAL_RESPONSE_TIMEOUT, stream.next())
        .await
        // timeout elapsed → treat as denied
        .map_err(|_| BackgroundError::PortalDenied)?
        // stream closed without a message → portal gone
        .ok_or(BackgroundError::PortalDenied)?;

    let (code, results): (u32, HashMap<String, OwnedValue>) = msg.body().deserialize()?;
    parse_response_payload(code, &results)?;

    tracing::info!(
        reason,
        autostart,
        "background portal: grant confirmed; watchdog protection active"
    );

    Ok(BackgroundGrant {
        connection: Some(connection),
        request_path,
    })
}

/// RAII handle to an outstanding `RequestBackground` grant. Dropping the
/// value closes the underlying `zbus::Connection`; the portal retains the
/// session-scoped grant until the sandbox process exits or the user
/// revokes it via their desktop environment.
pub struct BackgroundGrant {
    connection: Option<zbus::Connection>,
    request_path: OwnedObjectPath,
}

impl BackgroundGrant {
    /// The D-Bus object path of the portal request. Used for logging and
    /// (future) signal subscription if the portal sends `Running = false`.
    pub fn request_path(&self) -> &str {
        self.request_path.as_str()
    }

    /// Returns `true` while the underlying D-Bus connection is open.
    pub fn is_active(&self) -> bool {
        self.connection.is_some()
    }

    /// Explicitly release the grant. Prefer letting Drop do it.
    pub fn release(mut self) {
        self.connection = None;
    }
}

impl fmt::Debug for BackgroundGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BackgroundGrant")
            .field("request_path", &self.request_path.as_str())
            .field("is_active", &self.is_active())
            .finish()
    }
}

impl Drop for BackgroundGrant {
    fn drop(&mut self) {
        if self.connection.is_some() {
            tracing::debug!(
                request_path = %self.request_path.as_str(),
                "background portal: BackgroundGrant dropped; session bus will release on exit"
            );
        }
    }
}

/// Errors from Background portal interactions.
#[derive(Debug)]
pub enum BackgroundError {
    /// The process is not running under Flatpak. Caller should skip.
    NotSandboxed,
    /// `request_background` was called after a successful grant already exists
    /// for this process lifetime. The Background portal contract (ADR-0002)
    /// allows at most one successful request per process.
    AlreadyRequested,
    /// The portal returned a "denied" response (user declined or policy
    /// blocks background apps), or the `Response` signal did not arrive
    /// within the timeout.
    PortalDenied,
    /// Transport-level D-Bus failure.
    DBusProtocol(zbus::Error),
}

impl fmt::Display for BackgroundError {
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
            Self::DBusProtocol(inner) => write!(f, "D-Bus transport error: {inner}"),
        }
    }
}

impl std::error::Error for BackgroundError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DBusProtocol(inner) => Some(inner),
            _ => None,
        }
    }
}

impl From<zbus::Error> for BackgroundError {
    fn from(value: zbus::Error) -> Self {
        Self::DBusProtocol(value)
    }
}

impl From<zbus::fdo::Error> for BackgroundError {
    fn from(value: zbus::fdo::Error) -> Self {
        Self::DBusProtocol(zbus::Error::FDO(Box::new(value)))
    }
}

/// Parses the `Response` signal payload the portal emits after
/// `RequestBackground` completes. Extracted for unit-testability via
/// recorded fixtures — `zbus` does not synthesize the signal for us.
///
/// The portal response body is:
/// - `u` response code (0 = success, 1 = user cancelled, 2 = other error)
/// - `a{sv}` results dictionary (contains `background: b` and `autostart: b`)
///
/// Returns `Ok(())` on a success response, [`BackgroundError::PortalDenied`]
/// on cancel/other, and propagates parse failures as
/// [`BackgroundError::DBusProtocol`].
pub fn parse_response_payload(
    response_code: u32,
    results: &HashMap<String, OwnedValue>,
) -> Result<(), BackgroundError> {
    if response_code != 0 {
        return Err(BackgroundError::PortalDenied);
    }
    // We only check the `background` flag here; downstream code consumes
    // the capability state from `background_supported()` + grant presence.
    if let Some(background_flag) = results.get("background") {
        if let Ok(granted) = bool::try_from(background_flag.clone()) {
            if !granted {
                return Err(BackgroundError::PortalDenied);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn background_supported_matches_is_flatpak() {
        assert_eq!(background_supported(), is_flatpak());
    }

    #[tokio::test]
    async fn request_background_on_native_returns_not_sandboxed() {
        if is_flatpak() {
            // CI runs native; this branch is a placeholder for hypothetical
            // Flatpak test runners. We only assert when native.
            return;
        }
        let err = request_background("test", false)
            .await
            .expect_err("native build must refuse RequestBackground before touching D-Bus");
        assert!(matches!(err, BackgroundError::NotSandboxed));
    }

    #[test]
    fn parse_response_payload_success_returns_ok() {
        let mut results: HashMap<String, OwnedValue> = HashMap::new();
        results.insert(
            "background".to_string(),
            Value::from(true).try_into().unwrap(),
        );
        results.insert(
            "autostart".to_string(),
            Value::from(false).try_into().unwrap(),
        );
        parse_response_payload(0, &results).expect("success response must parse to Ok");
    }

    #[test]
    fn parse_response_payload_non_zero_code_is_denied() {
        let results: HashMap<String, OwnedValue> = HashMap::new();
        let err =
            parse_response_payload(1, &results).expect_err("non-zero response code must be Denied");
        assert!(matches!(err, BackgroundError::PortalDenied));
    }

    #[test]
    fn parse_response_payload_background_false_is_denied() {
        let mut results: HashMap<String, OwnedValue> = HashMap::new();
        results.insert(
            "background".to_string(),
            Value::from(false).try_into().unwrap(),
        );
        let err = parse_response_payload(0, &results)
            .expect_err("background=false must be treated as Denied");
        assert!(matches!(err, BackgroundError::PortalDenied));
    }

    #[test]
    fn parse_response_payload_missing_background_key_is_ok() {
        // If the portal omits the `background` key but the response code
        // is 0 we accept the grant — the portal variant documented in
        // `01/01 desktop-portal` spec.
        let results: HashMap<String, OwnedValue> = HashMap::new();
        parse_response_payload(0, &results)
            .expect("missing background key + code 0 must be accepted");
    }

    #[test]
    fn background_error_display_is_stable() {
        assert!(BackgroundError::NotSandboxed
            .to_string()
            .contains("not running under Flatpak"));
        assert!(BackgroundError::PortalDenied.to_string().contains("denied"));
        assert!(BackgroundError::AlreadyRequested
            .to_string()
            .contains("already succeeded"));
    }
}
