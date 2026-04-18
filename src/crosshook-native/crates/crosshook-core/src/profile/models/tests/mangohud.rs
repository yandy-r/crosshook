#![cfg(test)]

use super::super::*;
use super::fixtures::*;

#[test]
fn mangohud_config_default_omitted_from_profile_toml() {
    let profile = sample_profile();
    let serialized = toml::to_string_pretty(&profile).expect("serialize");
    assert!(
        !serialized.contains("[launch.mangohud]"),
        "default MangoHudConfig should be omitted from TOML output: {serialized}"
    );
}

#[test]
fn mangohud_config_roundtrip() {
    let mut profile = sample_profile();
    profile.launch.mangohud = MangoHudConfig {
        enabled: true,
        fps_limit: Some(144),
        gpu_stats: true,
        cpu_stats: true,
        ram: false,
        frametime: true,
        battery: false,
        watt: false,
        position: Some(MangoHudPosition::TopRight),
    };
    let serialized = toml::to_string_pretty(&profile).expect("serialize");
    let parsed: GameProfile = toml::from_str(&serialized).expect("deserialize");
    assert_eq!(parsed.launch.mangohud, profile.launch.mangohud);
}
