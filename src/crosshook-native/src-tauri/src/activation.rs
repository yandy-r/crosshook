//! Single-instance activation for the Classic shell.
//!
//! The first instance raises its main window when a second launch calls
//! `org.freedesktop.Application.Activate`; a second instance asks the first
//! to do so and exits.

use crosshook_app::instance_lock::{
    activate_running_instance, ActivationError, ActivationHandler, ActivationRequest,
    ActivationStatus, ACTIVATE_TIMEOUT,
};
use std::sync::mpsc;
use std::time::Duration;

use tauri::{AppHandle, Manager};

const MAIN_WINDOW: &str = "main";
/// How long the handler waits for the main thread to confirm the raise; well
/// inside the 2 s activation budget of the second launch.
const RAISE_CONFIRM_TIMEOUT: Duration = Duration::from_secs(1);

/// Raises the Classic main window on activation requests.
pub(crate) struct TauriActivation(AppHandle);

impl TauriActivation {
    pub(crate) fn new(handle: AppHandle) -> Self {
        Self(handle)
    }
}

impl ActivationHandler for TauriActivation {
    /// Runs the raise on the main thread and waits (bounded by
    /// [`RAISE_CONFIRM_TIMEOUT`]) for its result, so the second launch is told
    /// "activated" only after the window was shown and focused.
    fn activate(&self, request: ActivationRequest) -> ActivationStatus {
        // No window means the app is closing: tell the caller instead of
        // reporting a raise that cannot happen.
        if self.0.get_webview_window(MAIN_WINDOW).is_none() {
            tracing::info!("activation refused: the main window is gone (shutting down)");
            return ActivationStatus::NoWindow;
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let handle = self.0.clone();
        let scheduled = self.0.run_on_main_thread(move || {
            let _ = sender.send(raise_main_window(&handle, &request));
        });
        if let Err(error) = scheduled {
            tracing::warn!(%error, "could not schedule window activation");
            return ActivationStatus::NoWindow;
        }
        match receiver.recv_timeout(RAISE_CONFIRM_TIMEOUT) {
            Ok(status) => status,
            Err(_) => {
                tracing::warn!("window activation was not confirmed in time");
                ActivationStatus::RaiseFailed
            }
        }
    }
}

/// Un-minimizes, shows and focuses the main window (main thread only).
fn raise_main_window(handle: &AppHandle, request: &ActivationRequest) -> ActivationStatus {
    let Some(window) = handle.get_webview_window(MAIN_WINDOW) else {
        tracing::info!("activation refused: the main window is gone (shutting down)");
        return ActivationStatus::NoWindow;
    };
    if let Err(error) = window.unminimize().and_then(|()| window.show()) {
        tracing::warn!(%error, "could not show the main window on activation");
        return ActivationStatus::RaiseFailed;
    }
    // tao's `set_focus` presents with GDK_CURRENT_TIME and no token; the
    // launcher's token must reach GTK via `set_startup_id`, which calls
    // xdg_activation_v1_activate on Wayland and ends startup notification on
    // X11.
    #[cfg(target_os = "linux")]
    if let Some(id) = request.startup_id() {
        match window.gtk_window() {
            Ok(gtk_window) => {
                use gtk::prelude::GtkWindowExt;
                gtk_window.set_startup_id(id);
            }
            Err(error) => tracing::warn!(%error, "no GTK window for activation token"),
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = request;
    if let Err(error) = window.set_focus() {
        tracing::warn!(%error, "could not focus the main window on activation");
        return ActivationStatus::RaiseFailed;
    }
    tracing::info!("activated by a second CrossHook launch");
    ActivationStatus::Activated
}

/// Second launch: activates the running instance and exits 0, or prints why
/// it could not and exits 1.
pub(crate) fn exit_second_instance() -> ! {
    match activate_running_instance(ActivationRequest::from_env(), ACTIVATE_TIMEOUT) {
        Ok(()) => {
            eprintln!("CrossHook is already running; activated the existing window.");
            std::process::exit(0);
        }
        Err(error) => {
            eprintln!("{}", failure_message(&error));
            std::process::exit(1);
        }
    }
}

/// What a failed second launch tells the user.
fn failure_message(error: &ActivationError) -> String {
    const RETRY: &str = "close it or end the process and try again.";
    match error {
        ActivationError::TimedOut => format!(
            "CrossHook is already running but did not respond within {} s; {RETRY}",
            ACTIVATE_TIMEOUT.as_secs()
        ),
        ActivationError::ShuttingDown => {
            "The existing CrossHook is shutting down; try again in a few seconds.".to_owned()
        }
        // The name is claimed after the lock, so no owner also covers a first
        // start that is still migrating.
        ActivationError::NoOwner | ActivationError::Starting => {
            "The existing CrossHook is starting up or shutting down; try again in a few seconds."
                .to_owned()
        }
        ActivationError::RaiseFailed => {
            "CrossHook is already running but could not bring its window to the front.".to_owned()
        }
        ActivationError::Rejected(detail) => format!(
            "CrossHook is already running but the activation request was rejected ({detail}); {RETRY}"
        ),
        ActivationError::Bus(detail) => format!(
            "CrossHook is already running but could not be reached over the session bus \
             ({detail}); {RETRY}"
        ),
    }
}

#[cfg(test)]
mod tests;
