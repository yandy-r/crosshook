use crate::commands::launch::shared::{
    build_injection_log_event, suppression_summary_line, transform_launch_log_line_for_ui,
    InjectionLogLevel, InjectionLogSessionKind, InjectionLogSource, LaunchLogRelayState,
};
use crosshook_core::launch::{LaunchSessionRegistry, SessionKind};

#[test]
fn launch_log_ui_shows_first_gamescope_xdg_backend_line_then_suppresses_repeats() {
    let mut state = LaunchLogRelayState::default();
    let line = "[gamescope] [\u{1b}[0;31mError\u{1b}[0m] \u{1b}[0;37mxdg_backend:\u{1b}[0m Compositor released us but we were not acquired. Oh no.";

    let first = transform_launch_log_line_for_ui(&mut state, line);
    let second = transform_launch_log_line_for_ui(&mut state, line);
    let third = transform_launch_log_line_for_ui(&mut state, line);

    assert_eq!(first, vec![line.to_string()]);
    assert_eq!(
        second,
        vec![String::from(
            "[crosshook] Suppressing repeated gamescope xdg_backend console noise. The raw launch log still contains every line."
        )]
    );
    assert!(third.is_empty());
    assert_eq!(state.gamescope_xdg_backend_suppressed, 2);
}

#[test]
fn launch_log_ui_suppression_summary_reports_suppressed_count() {
    let mut state = LaunchLogRelayState::default();
    state.gamescope_xdg_backend_suppressed = 329;

    let summary = suppression_summary_line(&state).expect("summary line");
    assert!(summary.contains("329 repeated gamescope xdg_backend lines"));
}

#[test]
fn injection_log_event_constructor_builds_display_safe_structured_payload() {
    let registry = LaunchSessionRegistry::default();
    let (session_id, _rx) = registry.register(SessionKind::Trainer, "elden-ring");

    let event = build_injection_log_event(
        "2026-06-05T12:34:56Z",
        Some("  Elden Ring  "),
        session_id,
        SessionKind::Trainer,
        InjectionLogLevel::Warning,
        InjectionLogSource::Injection,
        "DLL injection engine is not available; stored hook configuration was not applied.",
        true,
    );

    assert_eq!(event.timestamp, "2026-06-05T12:34:56Z");
    assert_eq!(event.profile_name, "Elden Ring");
    assert_eq!(event.session_id, session_id.to_string());
    assert_eq!(event.session_kind, InjectionLogSessionKind::Trainer);
    assert_eq!(event.level, InjectionLogLevel::Warning);
    assert_eq!(event.source, InjectionLogSource::Injection);
    assert!(event.hook_id.is_none());
    assert!(event.hook_name.is_none());
    assert!(event.unsupported_runtime);

    let json = serde_json::to_value(&event).expect("serialize event");
    assert_eq!(json["session_kind"], "trainer");
    assert_eq!(json["level"], "warning");
    assert_eq!(json["source"], "injection");
    assert_eq!(json["unsupported_runtime"], true);

    let fallback = build_injection_log_event(
        "2026-06-05T12:35:00Z",
        Some("   "),
        session_id,
        SessionKind::Game,
        InjectionLogLevel::Info,
        InjectionLogSource::Runtime,
        "Trainer lifecycle complete.",
        false,
    );

    assert_eq!(fallback.profile_name, "Unknown profile");
    assert_eq!(fallback.session_kind, InjectionLogSessionKind::Game);
    let fallback_json = serde_json::to_value(&fallback).expect("serialize fallback event");
    assert!(
        fallback_json.get("unsupported_runtime").is_none(),
        "false unsupported_runtime should be omitted from the event payload: {fallback_json}"
    );
}
