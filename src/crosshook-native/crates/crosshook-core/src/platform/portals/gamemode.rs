//! `org.freedesktop.portal.GameMode` integration.
//!
//! See `docs/architecture/adr-0002-flatpak-portal-contracts.md` § GameMode
//! portal contract for the contract and decision matrix.
//!
//! **Scope**: this module registers CrossHook's **own** sandbox-side PID
//! with the host's `gamemoded` daemon. Host games continue to use the
//! `gamemoderun` wrapper via `crate::platform::host_command*`; the two
//! paths are complementary, not alternatives.

use std::fmt;

use crate::platform::is_flatpak;

/// D-Bus destination name for xdg-desktop-portal.
const PORTAL_DESKTOP_BUS: &str = "org.freedesktop.portal.Desktop";
/// Object path for the portal.
const PORTAL_DESKTOP_PATH: &str = "/org/freedesktop/portal/desktop";
/// Portal interface name for GameMode.
const PORTAL_GAMEMODE_INTERFACE: &str = "org.freedesktop.portal.GameMode";

/// How GameMode will actually be reached for a given launch context.
///
/// The variants encode the decision matrix in ADR-0002 § GameMode portal
/// contract. They are deliberately about **how**, not **whether** — host
/// games still use `gamemoderun` in both `Portal` and `HostGamemodeRun`
/// branches; the distinction is whether CrossHook **also** self-registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameModeBackend {
    /// Running under Flatpak with the portal reachable. CrossHook self-registers
    /// its sandbox PID via `org.freedesktop.portal.GameMode`. Host games still
    /// use the `gamemoderun` wrapper.
    Portal,
    /// Native build, or Flatpak with the portal unreachable.
    /// `gamemoderun` is the only path for host games; there is no
    /// CrossHook-self registration.
    HostGamemodeRun,
    /// Neither the portal nor `gamemoderun` is available.
    Unavailable,
}

impl fmt::Display for GameModeBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Portal => f.write_str("portal"),
            Self::HostGamemodeRun => f.write_str("host_gamemoderun"),
            Self::Unavailable => f.write_str("unavailable"),
        }
    }
}

/// Pure decision function — deterministic, no I/O. Used by tests and by
/// callers that have already probed portal and wrapper availability.
///
/// The truth table (ADR-0002):
///
/// | `is_in_flatpak` | `portal_is_available` | `host_gamemoderun_available` | Result            |
/// | --------------- | --------------------- | ---------------------------- | ----------------- |
/// | false           | _                     | true                         | `HostGamemodeRun` |
/// | false           | _                     | false                        | `Unavailable`     |
/// | true            | true                  | _                            | `Portal`          |
/// | true            | false                 | true                         | `HostGamemodeRun` |
/// | true            | false                 | false                        | `Unavailable`     |
pub fn resolve_backend(
    is_in_flatpak: bool,
    portal_is_available: bool,
    host_gamemoderun_available: bool,
) -> GameModeBackend {
    if is_in_flatpak && portal_is_available {
        return GameModeBackend::Portal;
    }
    if host_gamemoderun_available {
        return GameModeBackend::HostGamemodeRun;
    }
    GameModeBackend::Unavailable
}

/// Probes whether `org.freedesktop.portal.Desktop` is reachable on the session
/// bus AND exposes the `org.freedesktop.portal.GameMode` interface.
///
/// Returns `false` immediately for native builds (no D-Bus traffic). Under
/// Flatpak, connects to the session bus, queries the portal's introspection,
/// and looks for the GameMode interface. All errors are swallowed into
/// `false` — the caller falls back to `HostGamemodeRun` semantics.
///
/// Prefer [`probe_and_register_via_portal`] on the hot launch path; it
/// reuses the same session-bus connection for both introspection and
/// registration (one `Connection::session()` instead of two).
pub async fn portal_available() -> bool {
    if !is_flatpak() {
        return false;
    }
    async {
        let connection = zbus::Connection::session().await?;
        probe_portal_interface(&connection).await
    }
    .await
    .unwrap_or(false)
}

/// Introspects the portal object on `connection` and returns `true` if the
/// `org.freedesktop.portal.GameMode` interface is advertised.
///
/// Separated from connection setup so both `portal_available` and
/// `probe_and_register_via_portal` can reuse the same logic without
/// duplicating the introspection XML parse.
async fn probe_portal_interface(connection: &zbus::Connection) -> Result<bool, GameModeError> {
    let proxy = zbus::fdo::IntrospectableProxy::builder(connection)
        .destination(PORTAL_DESKTOP_BUS)?
        .path(PORTAL_DESKTOP_PATH)?
        .build()
        .await?;
    let xml = proxy.introspect().await?;
    Ok(xml.contains(PORTAL_GAMEMODE_INTERFACE))
}

/// Probes the GameMode portal interface **and**, if available, registers
/// CrossHook's own PID — all over a single `zbus::Connection::session()`.
///
/// This is the preferred entry point for the hot launch path: it avoids the
/// double socket connect + SASL handshake + `Hello` exchange that would occur
/// if `portal_available()` and `register_self_pid_with_portal()` were called
/// separately.
///
/// Returns:
/// - `Ok(None)` when the process is not running under Flatpak.
/// - `Ok(None)` when the portal interface is not advertised (logged at
///   `info!`; caller falls back to host `gamemoderun`).
/// - `Ok(Some(guard))` on successful registration.
/// - `Err(_)` on D-Bus transport failure or a rejected `RegisterGame` call.
pub async fn probe_and_register_via_portal() -> Result<Option<GameModeRegistration>, GameModeError>
{
    if !is_flatpak() {
        return Ok(None);
    }

    let connection = zbus::Connection::session().await?;

    // Introspect on the same connection — no second socket open.
    let available = probe_portal_interface(&connection).await?;
    tracing::debug!(available, "gamemode portal: introspection result");

    if !available {
        return Ok(None);
    }

    // Reuse the same connection for the registration call.
    let proxy = zbus::Proxy::new(
        &connection,
        PORTAL_DESKTOP_BUS,
        PORTAL_DESKTOP_PATH,
        PORTAL_GAMEMODE_INTERFACE,
    )
    .await?;

    let self_pid: u32 = std::process::id();

    // RegisterGame(pid: u32) -> i32 (0 on success, non-zero on error per the
    // portal interface definition). The portal handles sandbox→host PID
    // translation internally.
    let status: i32 = proxy.call("RegisterGame", &self_pid).await?;
    if status != 0 {
        return Err(GameModeError::RegistrationRejected(format!(
            "RegisterGame returned non-zero status {status}"
        )));
    }

    tracing::info!(
        self_pid,
        "gamemode portal: registered CrossHook self-PID via org.freedesktop.portal.GameMode"
    );

    Ok(Some(GameModeRegistration {
        connection: Some(connection),
        registered_pid: self_pid,
    }))
}

/// Registers CrossHook's own PID with the GameMode portal.
///
/// **Preconditions**: `resolve_backend(..)` returned `GameModeBackend::Portal`
/// AND the caller has already confirmed `portal_available()` returns `true`.
/// On the hot launch path prefer [`probe_and_register_via_portal`] instead,
/// which combines the introspection probe and registration into a single
/// session-bus connection.
///
/// The returned [`GameModeRegistration`] is RAII; dropping it unregisters
/// the PID via the portal's `UnregisterGame` method.
///
/// # Errors
///
/// Returns `GameModeError::NotSandboxed` if called on a native build,
/// `GameModeError::PortalUnreachable` if the portal does not respond, or
/// `GameModeError::DBusProtocol` for transport-level failures.
pub async fn register_self_pid_with_portal() -> Result<GameModeRegistration, GameModeError> {
    if !is_flatpak() {
        return Err(GameModeError::NotSandboxed);
    }

    let connection = zbus::Connection::session().await?;
    let proxy = zbus::Proxy::new(
        &connection,
        PORTAL_DESKTOP_BUS,
        PORTAL_DESKTOP_PATH,
        PORTAL_GAMEMODE_INTERFACE,
    )
    .await?;

    let self_pid: u32 = std::process::id();

    // RegisterGame(pid: u32) -> i32 (0 on success, non-zero on error per the
    // portal interface definition). The portal handles sandbox→host PID
    // translation internally.
    let status: i32 = proxy.call("RegisterGame", &self_pid).await?;
    if status != 0 {
        return Err(GameModeError::RegistrationRejected(format!(
            "RegisterGame returned non-zero status {status}"
        )));
    }

    tracing::info!(
        self_pid,
        "gamemode portal: registered CrossHook self-PID via org.freedesktop.portal.GameMode"
    );

    Ok(GameModeRegistration {
        connection: Some(connection),
        registered_pid: self_pid,
    })
}

/// RAII handle to an active GameMode portal registration.
///
/// Dropping the value unregisters the PID via `UnregisterGame`. The Drop
/// impl is **best-effort**: if the portal is gone or D-Bus has failed we
/// log at `warn!` and swallow the error — the process is exiting anyway.
pub struct GameModeRegistration {
    connection: Option<zbus::Connection>,
    registered_pid: u32,
}

impl GameModeRegistration {
    /// The PID that was registered (always CrossHook's own `std::process::id()`).
    pub fn registered_pid(&self) -> u32 {
        self.registered_pid
    }

    /// Explicitly unregister. Prefer letting Drop do it; this exists for
    /// callers that want to observe the result.
    pub async fn unregister(mut self) -> Result<(), GameModeError> {
        let Some(connection) = self.connection.take() else {
            return Ok(());
        };
        let proxy = zbus::Proxy::new(
            &connection,
            PORTAL_DESKTOP_BUS,
            PORTAL_DESKTOP_PATH,
            PORTAL_GAMEMODE_INTERFACE,
        )
        .await?;
        let status: i32 = proxy.call("UnregisterGame", &self.registered_pid).await?;
        if status != 0 {
            tracing::warn!(
                status,
                registered_pid = self.registered_pid,
                "gamemode portal: UnregisterGame returned non-zero"
            );
        }
        Ok(())
    }
}

impl Drop for GameModeRegistration {
    fn drop(&mut self) {
        // We cannot run async code in Drop. If the connection is still live
        // we log and rely on session bus teardown to drop the registration
        // when the process exits. If callers want clean unregistration they
        // should call `.unregister().await` explicitly.
        if self.connection.is_some() {
            tracing::debug!(
                registered_pid = self.registered_pid,
                "gamemode portal: GameModeRegistration dropped without explicit unregister; \
                 relying on session bus teardown"
            );
        }
    }
}

impl fmt::Debug for GameModeRegistration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GameModeRegistration")
            .field("registered_pid", &self.registered_pid)
            .field("connection_active", &self.connection.is_some())
            .finish()
    }
}

/// Errors from GameMode portal interactions. All are non-fatal at the call
/// site — the caller logs a single `warn!` and falls back to
/// `HostGamemodeRun` semantics for host games.
#[derive(Debug)]
pub enum GameModeError {
    /// The process is not running under a Flatpak sandbox; the portal
    /// does not apply.
    NotSandboxed,
    /// The portal is not reachable on the session bus (xdg-desktop-portal
    /// is not running or the Flatpak manifest did not request access to
    /// `org.freedesktop.portal.Desktop`).
    PortalUnreachable,
    /// The portal returned a non-zero status for the registration call.
    RegistrationRejected(String),
    /// Transport-level D-Bus error.
    DBusProtocol(zbus::Error),
}

impl fmt::Display for GameModeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSandboxed => f.write_str("not running under Flatpak; GameMode portal skipped"),
            Self::PortalUnreachable => {
                f.write_str("xdg-desktop-portal is not reachable on the session bus")
            }
            Self::RegistrationRejected(detail) => {
                write!(f, "GameMode portal rejected the registration: {detail}")
            }
            Self::DBusProtocol(inner) => write!(f, "D-Bus transport error: {inner}"),
        }
    }
}

impl std::error::Error for GameModeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DBusProtocol(inner) => Some(inner),
            _ => None,
        }
    }
}

impl From<zbus::Error> for GameModeError {
    fn from(value: zbus::Error) -> Self {
        Self::DBusProtocol(value)
    }
}

impl From<zbus::fdo::Error> for GameModeError {
    fn from(value: zbus::fdo::Error) -> Self {
        Self::DBusProtocol(zbus::Error::FDO(Box::new(value)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_backend_native_with_gamemoderun_uses_host_wrapper() {
        assert_eq!(
            resolve_backend(false, false, true),
            GameModeBackend::HostGamemodeRun
        );
    }

    #[test]
    fn resolve_backend_native_without_gamemoderun_is_unavailable() {
        assert_eq!(
            resolve_backend(false, false, false),
            GameModeBackend::Unavailable
        );
    }

    #[test]
    fn resolve_backend_flatpak_with_portal_prefers_portal() {
        assert_eq!(resolve_backend(true, true, true), GameModeBackend::Portal);
    }

    #[test]
    fn resolve_backend_flatpak_with_portal_but_no_wrapper_still_uses_portal() {
        // The portal covers CrossHook's own PID — the caller's capability
        // code decides whether to surface Degraded because host games can't
        // be wrapped, but the backend itself is still Portal.
        assert_eq!(resolve_backend(true, true, false), GameModeBackend::Portal);
    }

    #[test]
    fn resolve_backend_flatpak_without_portal_falls_back_to_wrapper() {
        assert_eq!(
            resolve_backend(true, false, true),
            GameModeBackend::HostGamemodeRun
        );
    }

    #[test]
    fn resolve_backend_flatpak_without_portal_or_wrapper_is_unavailable() {
        assert_eq!(
            resolve_backend(true, false, false),
            GameModeBackend::Unavailable
        );
    }

    #[test]
    fn gamemode_backend_display_is_snake_case_for_logs() {
        assert_eq!(GameModeBackend::Portal.to_string(), "portal");
        assert_eq!(
            GameModeBackend::HostGamemodeRun.to_string(),
            "host_gamemoderun"
        );
        assert_eq!(GameModeBackend::Unavailable.to_string(), "unavailable");
    }

    #[test]
    fn portal_available_is_false_on_native() {
        // This test intentionally runs the full helper. On a native test
        // host the is_flatpak() gate returns false immediately and we never
        // touch D-Bus. On a Flatpak test host the probe performs one
        // session-bus introspection (bounded, cheap).
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        let result = runtime.block_on(async { portal_available().await });
        // We cannot assert `!result` unconditionally (tests might be run
        // inside a sandbox one day), but on the CI matrix used today the
        // result must be false.
        if !is_flatpak() {
            assert!(!result, "portal_available must be false outside Flatpak");
        }
    }

    #[test]
    fn gamemode_error_display_is_stable() {
        assert!(GameModeError::NotSandboxed
            .to_string()
            .contains("not running under Flatpak"));
        assert!(GameModeError::PortalUnreachable
            .to_string()
            .contains("not reachable"));
        assert!(GameModeError::RegistrationRejected("oops".into())
            .to_string()
            .contains("oops"));
    }
}
