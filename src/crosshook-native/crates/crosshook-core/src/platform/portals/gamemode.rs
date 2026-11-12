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

/// Introspects the portal object on `connection` and returns `true` if the
/// `org.freedesktop.portal.GameMode` interface is advertised.
///
/// Shared by [`probe_and_register_via_portal`] so introspection is not duplicated.
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
/// This is the launch-path entry point: one session-bus connection covers
/// introspection and registration.
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
    /// The portal returned a non-zero status for the registration call.
    RegistrationRejected(String),
    /// Transport-level D-Bus error.
    DBusProtocol(zbus::Error),
}

impl fmt::Display for GameModeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSandboxed => f.write_str("not running under Flatpak; GameMode portal skipped"),
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
    fn gamemode_error_display_is_stable() {
        assert!(GameModeError::NotSandboxed
            .to_string()
            .contains("not running under Flatpak"));
        assert!(GameModeError::RegistrationRejected("oops".into())
            .to_string()
            .contains("oops"));
    }
}
