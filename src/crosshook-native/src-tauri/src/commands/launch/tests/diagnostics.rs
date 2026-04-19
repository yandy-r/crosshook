use crate::commands::launch::diagnostics::diagnostic_method_for_log;
use crosshook_core::launch::{METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH};

#[test]
fn diagnostic_method_uses_proton_run_for_trainer_runner_logs() {
    let log_tail = "[steam-helper] Delegating trainer leg to steam-host-trainer-runner.sh\n[steam-trainer-runner] trainer_launch_mode=direct_proton\n";

    assert_eq!(
        diagnostic_method_for_log(METHOD_STEAM_APPLAUNCH, log_tail),
        METHOD_PROTON_RUN
    );
}

#[test]
fn diagnostic_method_keeps_steam_for_plain_helper_logs() {
    let log_tail = "[steam-helper] Launching Steam AppID 12345\n";

    assert_eq!(
        diagnostic_method_for_log(METHOD_STEAM_APPLAUNCH, log_tail),
        METHOD_STEAM_APPLAUNCH
    );
}
