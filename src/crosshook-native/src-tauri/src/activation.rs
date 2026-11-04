//! Single-instance activation for the Classic shell.
//!
//! The first instance raises its main window when a second launch calls
//! `org.freedesktop.Application.Activate`; a second instance asks the first
//! to do so and exits.

use crosshook_app::instance_lock::{
    activate_running_instance, ActivationError, ActivationHandler, ActivationRequest,
    ActivationStatus, ACTIVATE_TIMEOUT,
};
use tauri::{AppHandle, Manager};

const MAIN_WINDOW: &str = "main";

/// Raises the Classic main window on activation requests.
pub(crate) struct TauriActivation(AppHandle);

impl TauriActivation {
    pub(crate) fn new(handle: AppHandle) -> Self {
        Self(handle)
    }
}

impl ActivationHandler for TauriActivation {
    fn activate(&self, request: ActivationRequest) -> ActivationStatus {
        // No window means the app is closing: tell the caller instead of
        // reporting a raise that cannot happen.
        if self.0.get_webview_window(MAIN_WINDOW).is_none() {
            tracing::info!("activation refused: the main window is gone (shutting down)");
            return ActivationStatus::NoWindow;
        }
        let handle = self.0.clone();
        let scheduled = self.0.run_on_main_thread(move || {
            let Some(window) = handle.get_webview_window(MAIN_WINDOW) else {
                tracing::warn!("activation requested before the main window exists");
                return;
            };
            if let Err(error) = window.unminimize().and_then(|()| window.show()) {
                tracing::warn!(%error, "could not show the main window on activation");
            }
            // tao's `set_focus` presents with GDK_CURRENT_TIME and no token;
            // the launcher's token must reach GTK via `set_startup_id`, which
            // calls xdg_activation_v1_activate on Wayland and ends startup
            // notification on X11.
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
            let _ = &request;
            if let Err(error) = window.set_focus() {
                tracing::warn!(%error, "could not focus the main window on activation");
            }
            tracing::info!("activated by a second CrossHook launch");
        });
        match scheduled {
            Ok(()) => ActivationStatus::Activated,
            Err(error) => {
                tracing::warn!(%error, "could not schedule window activation");
                ActivationStatus::NoWindow
            }
        }
    }
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
    match error {
        ActivationError::TimedOut => format!(
            "CrossHook is already running but did not respond within {} s; \
             close it or end the process and try again.",
            ACTIVATE_TIMEOUT.as_secs()
        ),
        ActivationError::Rejected(_) => {
            "The existing CrossHook is shutting down; try again in a few seconds.".to_owned()
        }
        // The name is claimed after the lock, so no owner also covers a first
        // start that is still migrating.
        ActivationError::NoOwner => {
            "The existing CrossHook is starting up or shutting down; try again in a few seconds."
                .to_owned()
        }
        ActivationError::Bus(detail) => format!(
            "CrossHook is already running but could not be reached over the session bus \
             ({detail}); close it or end the process and try again."
        ),
    }
}

#[cfg(test)]
mod tests;
