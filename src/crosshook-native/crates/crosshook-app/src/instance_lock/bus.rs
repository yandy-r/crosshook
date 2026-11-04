//! Session-bus half of the single-instance guard: owning
//! `dev.crosshook.CrossHook`, exporting `org.freedesktop.Application`, and
//! activating a running instance from a second launch.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use futures_core::Stream;
use zbus::fdo::DBusProxy;
use zbus::names::{BusName, WellKnownName};
use zbus::zvariant::{OwnedValue, Value};

use super::{APP_ID, OBJECT_PATH};

const APPLICATION_IFACE: &str = "org.freedesktop.Application";
const ACTIVATION_TOKEN: &str = "activation-token";
const DESKTOP_STARTUP_ID: &str = "desktop-startup-id";
/// Upper bound on the Phase B bus claim (connect + name request).
const CLAIM_TIMEOUT: Duration = Duration::from_secs(2);
/// Longest accepted activation token or startup id, in bytes.
const MAX_TOKEN_LEN: usize = 512;

/// Window-activation details forwarded from a second launch.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActivationRequest {
    /// Wayland `xdg-activation` token (`XDG_ACTIVATION_TOKEN`).
    pub activation_token: Option<String>,
    /// X11 startup-notification id (`DESKTOP_STARTUP_ID`).
    pub desktop_startup_id: Option<String>,
}

fn accepted(value: &str) -> Option<String> {
    let ok =
        !value.is_empty() && value.len() <= MAX_TOKEN_LEN && !value.chars().any(char::is_control);
    ok.then(|| value.to_owned())
}

impl ActivationRequest {
    /// Reads the launcher's tokens from this process's environment (read-only).
    pub fn from_env() -> Self {
        let read = |key: &str| std::env::var(key).ok().as_deref().and_then(accepted);
        Self {
            activation_token: read("XDG_ACTIVATION_TOKEN"),
            desktop_startup_id: read("DESKTOP_STARTUP_ID"),
        }
    }

    /// Parses untrusted `Activate` platform data: only the two known keys
    /// with string values of at most 512 bytes and no control characters are
    /// kept; everything else is ignored.
    pub fn from_platform_data(data: &HashMap<String, OwnedValue>) -> Self {
        let read = |key: &str| {
            data.get(key)
                .and_then(|value| <&str>::try_from(&**value).ok())
                .and_then(accepted)
        };
        Self {
            activation_token: read(ACTIVATION_TOKEN),
            desktop_startup_id: read(DESKTOP_STARTUP_ID),
        }
    }

    /// The id to hand to the window system: the activation token first,
    /// then the X11 startup id.
    pub fn startup_id(&self) -> Option<&str> {
        self.activation_token
            .as_deref()
            .or(self.desktop_startup_id.as_deref())
    }

    pub(super) fn platform_data(&self) -> HashMap<&'static str, Value<'_>> {
        [
            (ACTIVATION_TOKEN, &self.activation_token),
            (DESKTOP_STARTUP_ID, &self.desktop_startup_id),
        ]
        .into_iter()
        .filter_map(|(key, value)| value.as_deref().map(|value| (key, Value::from(value))))
        .collect()
    }
}

/// Whether an activation request could be honoured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationStatus {
    /// The window raise was scheduled.
    Activated,
    /// There is no window to raise (the instance is shutting down); the
    /// caller gets an `org.freedesktop.DBus.Error.Failed` reply.
    NoWindow,
}

/// Implemented by each UI to raise its main window.
///
/// Called on a Tokio worker thread; implementations marshal the work to
/// their own main thread and return without waiting for it.
pub trait ActivationHandler: Send + Sync + 'static {
    /// Un-minimizes, shows and focuses the main window.
    fn activate(&self, request: ActivationRequest) -> ActivationStatus;
}

/// Reply text for an activation the instance could not honour.
const NO_WINDOW: &str = "CrossHook is shutting down";

#[derive(Default)]
struct RelayState {
    handler: Option<Arc<dyn ActivationHandler>>,
    pending: Option<ActivationRequest>,
}

/// Buffers the latest activation until the UI installs its handler, because
/// the bus name is owned before any window exists.
#[derive(Default)]
pub(crate) struct ActivationRelay {
    state: Mutex<RelayState>,
}

impl ActivationRelay {
    /// Forwards `request` to the handler, or keeps it (latest wins) until one
    /// is installed (that counts as activated: the window is about to exist).
    pub(crate) fn deliver(&self, request: ActivationRequest) -> ActivationStatus {
        let handler = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            match &state.handler {
                Some(handler) => Arc::clone(handler),
                None => {
                    state.pending = Some(request);
                    return ActivationStatus::Activated;
                }
            }
        };
        handler.activate(request)
    }

    /// Installs the handler and delivers the pending request, if any.
    pub(crate) fn set_handler(&self, handler: Arc<dyn ActivationHandler>) {
        let pending = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            state.handler = Some(Arc::clone(&handler));
            state.pending.take()
        };
        if let Some(request) = pending {
            let _ = handler.activate(request);
        }
    }
}

struct ApplicationIface {
    relay: Arc<ActivationRelay>,
}

#[zbus::interface(name = "org.freedesktop.Application")]
impl ApplicationIface {
    /// `Activate(a{sv} platform_data)`: raise the main window, or fail when
    /// there is none (shutting down).
    fn activate(&self, platform_data: HashMap<String, OwnedValue>) -> zbus::fdo::Result<()> {
        match self
            .relay
            .deliver(ActivationRequest::from_platform_data(&platform_data))
        {
            ActivationStatus::Activated => Ok(()),
            ActivationStatus::NoWindow => Err(zbus::fdo::Error::Failed(NO_WINDOW.to_owned())),
        }
    }
}

/// An owned session-bus name; the connection serves `Activate` while held.
pub struct BusClaim {
    connection: zbus::Connection,
    name: String,
}

impl BusClaim {
    /// Releases the name and closes the connection.
    pub(crate) async fn release(self) {
        if let Err(error) = self.connection.release_name(self.name.as_str()).await {
            tracing::warn!(%error, "failed to release the instance bus name");
        }
    }
}

/// Result of claiming the instance bus name.
pub enum ClaimOutcome {
    /// This process now owns the name.
    Owned(BusClaim),
    /// Another process owns it: a CrossHook instance is already running.
    AlreadyOwned,
    /// No usable session bus (sanitized detail).
    Unavailable(String),
}

fn sanitized(error: impl std::fmt::Display) -> String {
    crate::presenter::sanitize_message(&error.to_string())
}

async fn claim_at(name: &str, relay: Arc<ActivationRelay>) -> ClaimOutcome {
    let build = async {
        zbus::connection::Builder::session()?
            .serve_at(OBJECT_PATH, ApplicationIface { relay })?
            .name(name)?
            .allow_name_replacements(false)
            .replace_existing_names(false)
            .build()
            .await
    };
    match tokio::time::timeout(CLAIM_TIMEOUT, build).await {
        Ok(Ok(connection)) => ClaimOutcome::Owned(BusClaim {
            connection,
            name: name.to_owned(),
        }),
        Ok(Err(zbus::Error::NameTaken)) => ClaimOutcome::AlreadyOwned,
        Ok(Err(error)) => ClaimOutcome::Unavailable(sanitized(error)),
        Err(_) => ClaimOutcome::Unavailable("session bus timed out".to_owned()),
    }
}

/// Owns `dev.crosshook.CrossHook` (`DoNotQueue`, no replacement) and serves
/// `Activate` through `relay`, bounded by 2 s. Must run on a live runtime.
pub(crate) async fn claim_bus_name(relay: Arc<ActivationRelay>) -> ClaimOutcome {
    claim_at(APP_ID, relay).await
}

/// Test seam: claims an arbitrary (unique) name with `handler` installed.
#[cfg(feature = "test-seams")]
#[doc(hidden)]
pub async fn claim_bus_name_at(name: &str, handler: impl ActivationHandler) -> ClaimOutcome {
    let relay = Arc::new(ActivationRelay::default());
    relay.set_handler(Arc::new(handler));
    claim_at(name, relay).await
}

/// Why a second launch could not activate the running instance.
#[derive(Debug, thiserror::Error)]
pub enum ActivationError {
    /// No usable session bus.
    #[error("session bus unavailable: {0}")]
    Bus(String),
    /// The name had no owner for the whole budget: the running instance is
    /// still starting or already shutting down (it released the name).
    #[error("no instance owns the bus name")]
    NoOwner,
    /// The owner did not answer `Activate` within the budget.
    #[error("timed out")]
    TimedOut,
    /// The owner answered `Activate` with an error.
    #[error("activation rejected: {0}")]
    Rejected(String),
}

/// Asks the running instance to raise its window. Synchronous: builds its
/// own current-thread runtime, and returns [`ActivationError::Bus`] when
/// called from inside a Tokio runtime. When the name has no owner yet (the
/// first instance is still in Phase A), waits for it within the same
/// `timeout`.
pub fn activate_running_instance(
    request: ActivationRequest,
    timeout: Duration,
) -> Result<(), ActivationError> {
    if tokio::runtime::Handle::try_current().is_ok() {
        return Err(ActivationError::Bus(
            "called inside a Tokio runtime".to_owned(),
        ));
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| ActivationError::Bus(sanitized(error)))?;
    runtime.block_on(activate_at(APP_ID, request, timeout))
}

/// Test seam: [`activate_running_instance`]'s body against an arbitrary name.
#[cfg(feature = "test-seams")]
#[doc(hidden)]
pub async fn activate_named(
    name: &str,
    request: ActivationRequest,
    timeout: Duration,
) -> Result<(), ActivationError> {
    activate_at(name, request, timeout).await
}

/// One deadline covers connecting, waiting for an owner and the call. Running
/// out while no one owns the name is [`ActivationError::NoOwner`]; running
/// out during the call is [`ActivationError::TimedOut`].
async fn activate_at(
    name: &str,
    request: ActivationRequest,
    timeout: Duration,
) -> Result<(), ActivationError> {
    let deadline = tokio::time::Instant::now() + timeout;
    let call = tokio::time::timeout_at(deadline, connect_and_wait(name, deadline)).await;
    let (connection, well_known) = call.unwrap_or(Err(ActivationError::TimedOut))?;
    tokio::time::timeout_at(deadline, call_activate(&connection, well_known, &request))
        .await
        .unwrap_or(Err(ActivationError::TimedOut))
}

fn bus(error: zbus::Error) -> ActivationError {
    ActivationError::Bus(sanitized(error))
}

/// Connects and waits (until `deadline`) for `name` to have an owner.
async fn connect_and_wait(
    name: &str,
    deadline: tokio::time::Instant,
) -> Result<(zbus::Connection, WellKnownName<'static>), ActivationError> {
    let well_known = WellKnownName::try_from(name).map_err(|e| bus(e.into()))?;
    let connection = zbus::Connection::session().await.map_err(bus)?;
    let dbus = DBusProxy::new(&connection).await.map_err(bus)?;
    // Subscribe before asking, so an owner appearing in between is not missed.
    let mut changes = dbus
        .receive_name_owner_changed_with_args(&[(0, name)])
        .await
        .map_err(bus)?;
    let owned = dbus
        .name_has_owner(BusName::from(well_known.clone()))
        .await
        .map_err(|error| bus(error.into()))?;
    if !owned {
        let wait_for_owner = async {
            loop {
                let Some(signal) = next(&mut changes).await else {
                    return Err(ActivationError::Bus("bus connection closed".to_owned()));
                };
                let has_owner = signal
                    .args()
                    .is_ok_and(|args| args.new_owner().as_ref().is_some());
                if has_owner {
                    return Ok(());
                }
            }
        };
        tokio::time::timeout_at(deadline, wait_for_owner)
            .await
            .unwrap_or(Err(ActivationError::NoOwner))?;
    }
    Ok((connection, well_known.into_owned()))
}

async fn call_activate(
    connection: &zbus::Connection,
    well_known: WellKnownName<'static>,
    request: &ActivationRequest,
) -> Result<(), ActivationError> {
    connection
        .call_method(
            Some(well_known),
            OBJECT_PATH,
            Some(APPLICATION_IFACE),
            "Activate",
            &(request.platform_data(),),
        )
        .await
        .map(drop)
        .map_err(|error| match error {
            zbus::Error::MethodError(..) | zbus::Error::FDO(_) => {
                ActivationError::Rejected(sanitized(error))
            }
            other => bus(other),
        })
}

/// `StreamExt::next` without a `futures-util` dependency.
fn next<S: Stream + Unpin>(stream: &mut S) -> impl Future<Output = Option<S::Item>> + '_ {
    std::future::poll_fn(move |cx| Pin::new(&mut *stream).poll_next(cx))
}
