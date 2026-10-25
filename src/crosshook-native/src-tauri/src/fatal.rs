use crosshook_app::bootstrap::BootstrapError;
use crosshook_app::presenter::display_path_tilde;
use std::time::Duration;
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

/// Upper bound on how long the fatal dialog may keep a window-less process alive.
const DIALOG_WATCHDOG: Duration = Duration::from_secs(5 * 60);

/// Displays a native error without creating a webview; always exits unsuccessfully.
pub(crate) fn show_and_exit(mut context: tauri::Context<tauri::Wry>, error: BootstrapError) -> ! {
    tracing::error!(%error, "CrossHook startup failed");
    eprintln!("CrossHook could not start: {error}");
    let mut message = error.user_message();
    if let Some(dirs) = crosshook_core::platform::base_dirs() {
        message.push_str("\n\nData directory: ");
        message.push_str(&display_path_tilde(
            &dirs.data_local_dir().join("crosshook").to_string_lossy(),
        ));
    }
    if let Ok(path) = crosshook_core::logging::log_file_path() {
        if path.is_file() {
            message.push_str("\nSee the log at ");
            message.push_str(&display_path_tilde(&path.to_string_lossy()));
        }
    }
    context.config_mut().app.windows.clear();
    // The dialog callback may never fire (the rfd GTK backend runs its own
    // thread), which would leave this window-less process running forever.
    // The watchdog guarantees the documented exit code 1 after five minutes.
    std::thread::spawn(|| {
        std::thread::sleep(DIALOG_WATCHDOG); // allow: startup-path bounded fatal-dialog watchdog, no runtime
        eprintln!("CrossHook: startup error dialog timed out; exiting");
        std::process::exit(1);
    });
    // Pinned Tao calls expect() when GTK cannot initialize (for example without
    // a display). Catch that build panic too, so the fallback still exits 1.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        tauri::Builder::default()
            .plugin(tauri_plugin_dialog::init())
            .setup(move |app| {
                let handle = app.handle().clone();
                app.dialog()
                    .message(message)
                    .kind(MessageDialogKind::Error)
                    .title("CrossHook could not start")
                    .show(move |_| handle.exit(1));
                Ok(())
            })
            .build(context)
    }));
    match result {
        Ok(Ok(app)) => app.run(|_, _| {}),
        Ok(Err(fallback_error)) => {
            eprintln!("CrossHook: could not display startup error: {fallback_error}");
        }
        Err(_) => eprintln!("CrossHook: could not initialize the startup error dialog"),
    }
    std::process::exit(1);
}
