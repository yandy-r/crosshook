use crate::commands::launch::{
    build_steam_launch_options_command, check_game_running, check_gamescope_session, launch_game,
    launch_platform_status, launch_reset_sessions, launch_trainer, list_launch_history_for_profile,
    list_running_profiles, preview_launch, validate_launch,
};

#[test]
fn command_names_match_expected_ipc_contract() {
    // Async commands cannot be cast to fn pointers; naming them pins the
    // exported symbol (and thus the IPC command name).
    let _ = launch_game;
    let _ = launch_trainer;
    let _ = launch_reset_sessions;
    let _ = validate_launch;
    let _ = preview_launch;
    let _ = build_steam_launch_options_command;
    let _ = check_gamescope_session;
    let _ = launch_platform_status;
    let _ = check_game_running;
    let _ = list_running_profiles;
    let _ = list_launch_history_for_profile;
}
