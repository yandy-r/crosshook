use crate::profile::{
    profile_to_shareable_toml, profile_to_shareable_toml_with_options, GameProfile, HookStage,
    LaunchHook, ShareableTomlOptions,
};

use super::fixtures::sample_profile;

#[test]
fn shareable_toml_starts_with_comment_header() {
    let profile = sample_profile();
    let toml = profile_to_shareable_toml("elden-ring", &profile).unwrap();
    assert!(toml.starts_with("# CrossHook Profile: elden-ring\n"));
    assert!(toml.contains("# To use this profile, save this file as:"));
    assert!(toml.contains("~/.config/crosshook/profiles/elden-ring.toml"));
}

#[test]
fn shareable_toml_roundtrips_through_parser() {
    let profile = sample_profile();
    let toml = profile_to_shareable_toml("elden-ring", &profile).unwrap();
    let parsed: GameProfile = toml::from_str(&toml).unwrap();
    assert_eq!(parsed, profile);
}

#[test]
fn shareable_toml_strips_hooks_by_default() {
    let mut profile = sample_profile();
    profile.pre_launch_hooks = vec![LaunchHook {
        id: "pre-1".to_string(),
        name: "Overlay".to_string(),
        path: "/opt/hooks/pre.sh".to_string(),
        stage: HookStage::PreLaunch,
        enabled: true,
    }];
    let toml = profile_to_shareable_toml("elden-ring", &profile).unwrap();
    assert!(!toml.contains("pre_launch_hooks"));
    assert!(!toml.contains("/opt/hooks/pre.sh"));
}

#[test]
fn shareable_toml_include_hooks_opt_in() {
    let mut profile = sample_profile();
    profile.pre_launch_hooks = vec![LaunchHook {
        id: "pre-1".to_string(),
        name: "Overlay".to_string(),
        path: "/opt/hooks/pre.sh".to_string(),
        stage: HookStage::PreLaunch,
        enabled: true,
    }];
    let toml = profile_to_shareable_toml_with_options(
        "elden-ring",
        &profile,
        ShareableTomlOptions {
            include_hooks: true,
        },
    )
    .unwrap();
    assert!(toml.contains("pre_launch_hooks"));
    assert!(toml.contains("/opt/hooks/pre.sh"));
}

#[test]
fn shareable_toml_with_empty_name_still_valid() {
    let profile = GameProfile::default();
    let toml = profile_to_shareable_toml("", &profile).unwrap();
    assert!(toml.starts_with("# CrossHook Profile: \n"));
    let parsed: GameProfile = toml::from_str(&toml).unwrap();
    assert_eq!(parsed, profile);
}
