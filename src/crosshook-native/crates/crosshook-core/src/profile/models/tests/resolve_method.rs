#![cfg(test)]

use crate::launch::request::{METHOD_NATIVE, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH};

use super::super::*;
use super::fixtures::*;

#[test]
fn resolve_launch_method_prefers_explicit_method() {
    let mut profile = sample_profile();
    profile.launch.method = METHOD_NATIVE.to_string();
    profile.steam.enabled = true;

    assert_eq!(resolve_launch_method(&profile), METHOD_NATIVE);
}

#[test]
fn resolve_launch_method_falls_back_to_steam_enabled() {
    let mut profile = sample_profile();
    profile.launch.method.clear();
    profile.steam.enabled = true;

    assert_eq!(resolve_launch_method(&profile), METHOD_STEAM_APPLAUNCH);
}

#[test]
fn resolve_launch_method_falls_back_to_proton_for_windows_games() {
    let mut profile = sample_profile();
    profile.launch.method.clear();
    profile.steam.enabled = false;

    assert_eq!(resolve_launch_method(&profile), METHOD_PROTON_RUN);
}

#[test]
fn resolve_launch_method_falls_back_to_native_for_non_windows_games() {
    let mut profile = sample_profile();
    profile.launch.method.clear();
    profile.steam.enabled = false;
    profile.game.executable_path = "/games/test.sh".to_string();

    assert_eq!(resolve_launch_method(&profile), METHOD_NATIVE);
}
