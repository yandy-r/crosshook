use crosshook_app::instance_lock::APP_ID;

use super::*;

/// With `app.enableGTKAppId` on, tao would register a GApplication that owns
/// `dev.crosshook.CrossHook` and exports its own `org.freedesktop.Application`,
/// competing with the single-instance bus name. Keep it off (D22).
#[test]
fn gtk_app_id_stays_disabled_and_identifier_matches_bus_name() {
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../../tauri.conf.json")).expect("valid tauri.conf.json");
    let enable = &config["app"]["enableGTKAppId"];
    assert!(
        enable.is_null() || enable == &serde_json::Value::Bool(false),
        "app.enableGTKAppId must stay absent or false, found {enable}"
    );
    assert_eq!(config["identifier"], APP_ID);
}

#[test]
fn failure_messages_distinguish_timeout_shutdown_and_bus() {
    let timeout = failure_message(&ActivationError::TimedOut);
    assert!(timeout.starts_with("CrossHook is already running but did not respond within 2 s"));
    assert!(timeout.contains("close it or end the process"));
    assert_eq!(
        failure_message(&ActivationError::Rejected(
            "CrossHook is shutting down".into()
        )),
        "The existing CrossHook is shutting down; try again in a few seconds."
    );
    assert_eq!(
        failure_message(&ActivationError::NoOwner),
        "The existing CrossHook is starting up or shutting down; try again in a few seconds."
    );
    let bus = failure_message(&ActivationError::Bus("no socket".into()));
    assert!(bus.contains("could not be reached over the session bus (no socket)"));
}
