#![cfg(test)]

use super::super::*;
use super::fixtures::*;

#[test]
fn resolve_launch_method_prefers_explicit_method() {
    let mut profile = sample_profile();
    profile.launch.method = "native".to_string();
    profile.steam.enabled = true;

    assert_eq!(resolve_launch_method(&profile), "native");
}

#[test]
fn resolve_launch_method_falls_back_to_steam_enabled() {
    let mut profile = sample_profile();
    profile.launch.method.clear();
    profile.steam.enabled = true;

    assert_eq!(resolve_launch_method(&profile), "steam_applaunch");
}

#[test]
fn resolve_launch_method_falls_back_to_proton_for_windows_games() {
    let mut profile = sample_profile();
    profile.launch.method.clear();
    profile.steam.enabled = false;

    assert_eq!(resolve_launch_method(&profile), "proton_run");
}

#[test]
fn resolve_launch_method_falls_back_to_native_for_non_windows_games() {
    let mut profile = sample_profile();
    profile.launch.method.clear();
    profile.steam.enabled = false;
    profile.game.executable_path = "/games/test.sh".to_string();

    assert_eq!(resolve_launch_method(&profile), "native");
}
