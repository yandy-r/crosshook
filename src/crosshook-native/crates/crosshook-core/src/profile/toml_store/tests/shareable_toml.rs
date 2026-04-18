use crate::profile::{profile_to_shareable_toml, GameProfile};

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
fn shareable_toml_with_empty_name_still_valid() {
    let profile = GameProfile::default();
    let toml = profile_to_shareable_toml("", &profile).unwrap();
    assert!(toml.starts_with("# CrossHook Profile: \n"));
    let parsed: GameProfile = toml::from_str(&toml).unwrap();
    assert_eq!(parsed, profile);
}
