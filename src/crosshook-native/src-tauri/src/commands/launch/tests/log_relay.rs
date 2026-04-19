use crate::commands::launch::shared::{
    suppression_summary_line, transform_launch_log_line_for_ui, LaunchLogRelayState,
};

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
