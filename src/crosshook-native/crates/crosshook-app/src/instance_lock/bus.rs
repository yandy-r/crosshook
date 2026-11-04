//! Session-bus half of the single-instance guard: owning
//! `dev.crosshook.CrossHook`, exporting `org.freedesktop.Application`, and
//! activating a running instance from a second launch.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures_core::Stream;
use zbus::fdo::DBusProxy;
use zbus::names::{BusName, WellKnownName};
use zbus::zvariant::{OwnedValue, Value};

use super::relay::{ActivationRelay, RelayOutcome};
use super::{APP_ID, OBJECT_PATH};

const APPLICATION_IFACE: &str = "org.freedesktop.Application";
const ACTIVATION_TOKEN: &str = "activation-token";
const DESKTOP_STARTUP_ID: &str = "desktop-startup-id";
/// Upper bound on the Phase B bus claim (connect + name request).
const CLAIM_TIMEOUT: Duration = Duration::from_secs(2);
/// Upper bound on releasing the bus name at exit or handoff.
const RELEASE_TIMEOUT: Duration = Duration::from_millis(500);
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

/// What an activation handler did with a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationStatus {
    /// The main window was raised.
    Activated,
    /// There is no window to raise (the instance is shutting down).
    NoWindow,
    /// A window exists but raising it failed or was not confirmed in time.
    RaiseFailed,
}

/// Implemented by each UI to raise its main window.
///
/// Called on a blocking-pool thread. Implementations hand the work to their
/// own main thread and wait for its result, bounded (well under the 2 s
/// activation budget), so the second launch is only told "activated" after
/// the raise happened.
pub trait ActivationHandler: Send + Sync + 'static {
    /// Un-minimizes, shows and focuses the main window.
    fn activate(&self, request: ActivationRequest) -> ActivationStatus;
}

/// D-Bus error names `Activate` replies with, so the caller can tell them
/// apart from errors of an unrelated process that owns the name.
const ERROR_SHUTTING_DOWN: &str = "dev.crosshook.CrossHook.Error.ShuttingDown";
const ERROR_STARTING: &str = "dev.crosshook.CrossHook.Error.Starting";
const ERROR_RAISE_FAILED: &str = "dev.crosshook.CrossHook.Error.RaiseFailed";

/// `Activate` failures, sent as the error names above.
#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "dev.crosshook.CrossHook.Error")]
enum ActivateReply {
    #[zbus(error)]
    ZBus(zbus::Error),
    /// No window (exiting), or startup failed before the UI existed.
    ShuttingDown(String),
    /// The UI did not finish starting within the reply budget.
    Starting(String),
    /// The raise failed.
    RaiseFailed(String),
}

struct ApplicationIface {
    relay: Arc<ActivationRelay>,
}

#[zbus::interface(name = "org.freedesktop.Application")]
impl ApplicationIface {
    /// `Activate(a{sv} platform_data)`: raise the main window. The reply is
    /// sent only after the UI handled the request (or failed to).
    async fn activate(
        &self,
        platform_data: HashMap<String, OwnedValue>,
    ) -> Result<(), ActivateReply> {
        let request = ActivationRequest::from_platform_data(&platform_data);
        match self.relay.request(request).await {
            RelayOutcome::Handled(ActivationStatus::Activated) => Ok(()),
            RelayOutcome::Handled(ActivationStatus::NoWindow) | RelayOutcome::Closed => Err(
                ActivateReply::ShuttingDown("CrossHook is shutting down".to_owned()),
            ),
            RelayOutcome::Handled(ActivationStatus::RaiseFailed) => Err(
                ActivateReply::RaiseFailed("the window could not be raised".to_owned()),
            ),
            RelayOutcome::StillStarting | RelayOutcome::Busy => Err(ActivateReply::Starting(
                "CrossHook is still starting".to_owned(),
            )),
        }
    }
}

/// An owned session-bus name; the connection serves `Activate` while held.
pub struct BusClaim {
    connection: zbus::Connection,
    name: String,
}

impl BusClaim {
    /// Releases the name and closes the connection, bounded by
    /// [`RELEASE_TIMEOUT`]; on timeout, dropping the connection gives the
    /// name up anyway.
    pub(crate) async fn release(self) {
        let release = self.connection.release_name(self.name.as_str());
        match tokio::time::timeout(RELEASE_TIMEOUT, release).await {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => tracing::warn!(%error, "failed to release the instance bus name"),
            Err(_) => tracing::warn!(
                timeout_ms = RELEASE_TIMEOUT.as_millis(),
                "instance bus name release timed out; closing the connection instead"
            ),
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
    let relay = Arc::new(ActivationRelay::new(tokio::runtime::Handle::current()));
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
    /// The running CrossHook is shutting down (or failed to start).
    #[error("the running instance is shutting down")]
    ShuttingDown,
    /// The running CrossHook is still starting its UI.
    #[error("the running instance is still starting")]
    Starting,
    /// The running CrossHook could not raise its window.
    #[error("the running instance could not raise its window")]
    RaiseFailed,
    /// The owner answered `Activate` with an unexpected error (for example a
    /// process that is not CrossHook owns the name).
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

/// One deadline covers connecting, waiting for an owner and the call.
/// Running out while no one owns the name is [`ActivationError::NoOwner`];
/// running out during the call is [`ActivationError::TimedOut`]. If the
/// owner goes away between the ownership check and the call, the call is
/// retried once a new owner appears, within the same deadline.
async fn activate_at(
    name: &str,
    request: ActivationRequest,
    timeout: Duration,
) -> Result<(), ActivationError> {
    let deadline = tokio::time::Instant::now() + timeout;
    let attempt = async {
        let mut session = Session::connect(name).await?;
        loop {
            session.wait_for_owner(deadline).await?;
            match session.call(&request).await {
                Err(CallError::NoOwner) => continue,
                Err(CallError::Failed(error)) => return Err(error),
                Ok(()) => return Ok(()),
            }
        }
    };
    tokio::time::timeout_at(deadline, attempt)
        .await
        .unwrap_or(Err(ActivationError::TimedOut))
}

fn bus(error: zbus::Error) -> ActivationError {
    ActivationError::Bus(sanitized(error))
}

enum CallError {
    /// The name had no owner when the call was routed.
    NoOwner,
    Failed(ActivationError),
}

/// A session-bus connection watching `NameOwnerChanged` for one name.
struct Session<'a> {
    connection: zbus::Connection,
    dbus: DBusProxy<'a>,
    changes: zbus::fdo::NameOwnerChangedStream,
    name: WellKnownName<'static>,
}

impl Session<'_> {
    /// Connects and subscribes before any ownership check, so an owner
    /// appearing in between is not missed.
    async fn connect(name: &str) -> Result<Self, ActivationError> {
        let name = WellKnownName::try_from(name)
            .map_err(|e| bus(e.into()))?
            .into_owned();
        let connection = zbus::Connection::session().await.map_err(bus)?;
        let dbus = DBusProxy::new(&connection).await.map_err(bus)?;
        let changes = dbus
            .receive_name_owner_changed_with_args(&[(0, name.as_str())])
            .await
            .map_err(bus)?;
        Ok(Self {
            connection,
            dbus,
            changes,
            name,
        })
    }

    /// Returns once the name has an owner; [`ActivationError::NoOwner`] if
    /// none appears before `deadline`.
    async fn wait_for_owner(
        &mut self,
        deadline: tokio::time::Instant,
    ) -> Result<(), ActivationError> {
        let owned = self
            .dbus
            .name_has_owner(BusName::from(self.name.clone()))
            .await
            .map_err(|error| bus(error.into()))?;
        if owned {
            return Ok(());
        }
        let changes = &mut self.changes;
        let wait = async {
            loop {
                let Some(signal) = next(changes).await else {
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
        tokio::time::timeout_at(deadline, wait)
            .await
            .unwrap_or(Err(ActivationError::NoOwner))
    }

    async fn call(&self, request: &ActivationRequest) -> Result<(), CallError> {
        let reply = self
            .connection
            .call_method(
                Some(self.name.clone()),
                OBJECT_PATH,
                Some(APPLICATION_IFACE),
                "Activate",
                &(request.platform_data(),),
            )
            .await;
        match reply {
            Ok(_) => Ok(()),
            Err(error) => Err(classify(error)),
        }
    }
}

/// Maps a failed `Activate` call: our own error names first, then "nobody
/// owns the name" (retried), then anything else as an opaque rejection.
fn classify(error: zbus::Error) -> CallError {
    let zbus::Error::MethodError(name, detail, _) = &error else {
        return CallError::Failed(bus(error));
    };
    let detail = detail.as_deref().map(sanitized).unwrap_or_default();
    CallError::Failed(match name.as_str() {
        ERROR_SHUTTING_DOWN => ActivationError::ShuttingDown,
        ERROR_STARTING => ActivationError::Starting,
        ERROR_RAISE_FAILED => ActivationError::RaiseFailed,
        "org.freedesktop.DBus.Error.ServiceUnknown"
        | "org.freedesktop.DBus.Error.NameHasNoOwner" => {
            return CallError::NoOwner;
        }
        other => ActivationError::Rejected(format!("{other}: {detail}")),
    })
}

/// `StreamExt::next` without a `futures-util` dependency.
fn next<S: Stream + Unpin>(stream: &mut S) -> impl Future<Output = Option<S::Item>> + '_ {
    std::future::poll_fn(move |cx| Pin::new(&mut *stream).poll_next(cx))
}
