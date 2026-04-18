use super::super::content::build_trainer_script_content;
use super::fixtures::make_gamescope_request;
use crate::profile::{GamescopeConfig, TrainerLoadingMode};
use crate::settings::UmuPreference;

#[test]
fn gamescope_disabled_script_unchanged() {
    let request = make_gamescope_request(
        GamescopeConfig::default(),
        TrainerLoadingMode::SourceDirectory,
    );
    let content = build_trainer_script_content(&request, "Test Game");
    assert!(!content.contains("_GAMESCOPE_ARGS"));
    assert!(!content.contains("_GS_PREFIX"));
    assert!(!content.contains("gamescope"));
    assert!(content.contains(r#"exec "$PROTON" run "$trainer_host_path""#));
    assert!(content.contains(r#"exec umu-run "$trainer_host_path""#));
}

#[test]
fn gamescope_enabled_source_directory_wraps_exec() {
    let request = make_gamescope_request(
        GamescopeConfig {
            enabled: true,
            internal_width: Some(800),
            internal_height: Some(400),
            fullscreen: true,
            ..Default::default()
        },
        TrainerLoadingMode::SourceDirectory,
    );
    let content = build_trainer_script_content(&request, "Test Game");
    assert!(content.contains("_GAMESCOPE_ARGS=("));
    assert!(content.contains("'-f'"));
    assert!(content.contains("'-w' '800'"));
    assert!(content.contains("'-h' '400'"));
    assert!(content.contains("GAMESCOPE_WAYLAND_DISPLAY"));
    assert!(content.contains(r#"exec "${_GS_PREFIX[@]}" "$PROTON" run "$trainer_host_path""#));
    assert!(content.contains(r#"exec "${_GS_PREFIX[@]}" umu-run "$trainer_host_path""#));
}

#[test]
fn gamescope_enabled_copy_to_prefix_wraps_exec() {
    let request = make_gamescope_request(
        GamescopeConfig {
            enabled: true,
            fullscreen: true,
            ..Default::default()
        },
        TrainerLoadingMode::CopyToPrefix,
    );
    let content = build_trainer_script_content(&request, "Test Game");
    assert!(content.contains("_GAMESCOPE_ARGS=("));
    assert!(
        content.contains(r#"exec "${_GS_PREFIX[@]}" "$PROTON" run "$staged_trainer_windows_path""#)
    );
    assert!(content.contains(r#"exec "${_GS_PREFIX[@]}" umu-run "$staged_trainer_windows_path""#));
}

#[test]
fn gamescope_allow_nested_skips_guard() {
    let request = make_gamescope_request(
        GamescopeConfig {
            enabled: true,
            allow_nested: true,
            fullscreen: true,
            ..Default::default()
        },
        TrainerLoadingMode::SourceDirectory,
    );
    let content = build_trainer_script_content(&request, "Test Game");
    assert!(!content.contains("GAMESCOPE_WAYLAND_DISPLAY"));
    assert!(content.contains("_GS_PREFIX=("));
}

#[test]
fn gamescope_fullscreen_always_present() {
    let request = make_gamescope_request(
        GamescopeConfig {
            enabled: true,
            fullscreen: false,
            internal_width: Some(1280),
            internal_height: Some(720),
            ..Default::default()
        },
        TrainerLoadingMode::SourceDirectory,
    );
    let content = build_trainer_script_content(&request, "Test Game");
    assert!(
        content.contains("'-f'"),
        "fullscreen flag must always be present for trainer gamescope"
    );
}

#[test]
fn gamescope_script_does_not_override_display() {
    let request = make_gamescope_request(
        GamescopeConfig {
            enabled: true,
            fullscreen: true,
            ..Default::default()
        },
        TrainerLoadingMode::SourceDirectory,
    );
    let content = build_trainer_script_content(&request, "Test Game");
    assert!(!content.contains("pgrep -x wineserver"));
    assert!(!content.contains("DISPLAY="));
    assert!(content.contains("gamescope"));
}

#[test]
fn network_isolation_enabled_generates_runtime_probe_and_exec() {
    let mut request = make_gamescope_request(
        GamescopeConfig::default(),
        TrainerLoadingMode::SourceDirectory,
    );
    request.network_isolation = true;
    let content = build_trainer_script_content(&request, "Test Game");
    assert!(content.contains("# Network isolation: enabled"));
    assert!(content.contains("if unshare --net true"));
    assert!(content.contains("_NET_PREFIX=(unshare --net)"));
    assert!(content.contains("WARNING: unshare --net unavailable"));
    assert!(content.contains(r#"exec "${_NET_PREFIX[@]}" "$PROTON" run"#));
    assert!(content.contains(r#"exec "${_NET_PREFIX[@]}" umu-run"#));
}

#[test]
fn network_isolation_disabled_no_unshare_in_exec() {
    let mut request = make_gamescope_request(
        GamescopeConfig::default(),
        TrainerLoadingMode::SourceDirectory,
    );
    request.network_isolation = false;
    let content = build_trainer_script_content(&request, "Test Game");
    assert!(content.contains("# Network isolation: disabled"));
    assert!(!content.contains("unshare"));
}

#[test]
fn network_isolation_with_gamescope_uses_both_prefixes() {
    let mut request = make_gamescope_request(
        GamescopeConfig {
            enabled: true,
            fullscreen: true,
            ..Default::default()
        },
        TrainerLoadingMode::CopyToPrefix,
    );
    request.network_isolation = true;
    let content = build_trainer_script_content(&request, "Test Game");
    assert!(content.contains(r#"exec "${_GS_PREFIX[@]}" "${_NET_PREFIX[@]}" "$PROTON" run"#));
    assert!(content.contains(r#"exec "${_GS_PREFIX[@]}" "${_NET_PREFIX[@]}" umu-run"#));
}

#[test]
fn build_exec_line_emits_umu_probe_and_dual_exec() {
    let request = make_gamescope_request(
        GamescopeConfig::default(),
        TrainerLoadingMode::SourceDirectory,
    );
    let content = build_trainer_script_content(&request, "Test Game");
    assert!(content.contains("command -v umu-run"));
    assert!(content.contains("_UMU_AVAILABLE=1"));
    assert!(content.contains(r#"exec "$PROTON" run"#));
    assert!(content.contains("exec umu-run"));
}

#[test]
fn proton_preference_bypasses_umu_run_for_exports() {
    let mut request = make_gamescope_request(
        GamescopeConfig::default(),
        TrainerLoadingMode::SourceDirectory,
    );
    request.umu_preference = UmuPreference::Proton;
    request.umu_game_id = "custom-77".to_string();
    request.runtime_steam_app_id = "9000".to_string();

    let content = build_trainer_script_content(&request, "Test Game");

    assert!(!content.contains("command -v umu-run"));
    assert!(!content.contains("exec umu-run"));
    assert!(!content.contains("export GAMEID="));
    assert!(content.contains("unset GAMEID PROTON_VERB PROTONPATH"));
    assert!(content.contains(r#"exec "$PROTON" run "$trainer_host_path""#));
}

#[test]
fn umu_export_emits_trainer_env_contract_with_runtime_precedence() {
    let mut request = make_gamescope_request(
        GamescopeConfig::default(),
        TrainerLoadingMode::SourceDirectory,
    );
    request.umu_preference = UmuPreference::Umu;
    request.steam_app_id = "12345".to_string();
    request.runtime_steam_app_id = "67890".to_string();
    request.umu_game_id = "custom-override".to_string();

    let content = build_trainer_script_content(&request, "Test Game");

    assert!(content.contains("export GAMEID='custom-override'"));
    assert!(content.contains("export PROTON_VERB='runinprefix'"));
    assert!(content.contains("export PROTONPATH='/opt/proton'"));
    assert!(content.contains("WARNING: umu preference requested but umu-run is unavailable"));
    assert!(content.contains("unset GAMEID PROTON_VERB PROTONPATH"));
}
