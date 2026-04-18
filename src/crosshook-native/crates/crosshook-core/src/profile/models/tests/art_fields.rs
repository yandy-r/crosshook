#![cfg(test)]

use super::super::*;
use super::fixtures::*;

#[test]
fn local_override_game_section_not_empty_when_portrait_art_set() {
    let section = LocalOverrideGameSection {
        custom_portrait_art_path: "/art/portrait.png".to_string(),
        ..LocalOverrideGameSection::default()
    };
    assert!(!section.is_empty());
}

#[test]
fn local_override_game_section_not_empty_when_background_art_set() {
    let section = LocalOverrideGameSection {
        custom_background_art_path: "/art/bg.png".to_string(),
        ..LocalOverrideGameSection::default()
    };
    assert!(!section.is_empty());
}

#[test]
fn storage_profile_moves_portrait_and_background_to_local_override() {
    let mut profile = sample_profile();
    profile.game.custom_portrait_art_path = "/art/portrait.png".to_string();
    profile.game.custom_background_art_path = "/art/bg.png".to_string();

    let storage = profile.storage_profile();
    assert!(storage.game.custom_portrait_art_path.is_empty());
    assert!(storage.game.custom_background_art_path.is_empty());
    assert_eq!(
        storage.local_override.game.custom_portrait_art_path,
        "/art/portrait.png"
    );
    assert_eq!(
        storage.local_override.game.custom_background_art_path,
        "/art/bg.png"
    );
}

#[test]
fn effective_profile_merges_portrait_and_background_from_local_override() {
    let mut profile = sample_profile();
    profile.local_override.game.custom_portrait_art_path = "/override/portrait.png".to_string();
    profile.local_override.game.custom_background_art_path = "/override/bg.png".to_string();

    let effective = profile.effective_profile();
    assert_eq!(
        effective.game.custom_portrait_art_path,
        "/override/portrait.png"
    );
    assert_eq!(
        effective.game.custom_background_art_path,
        "/override/bg.png"
    );
}

#[test]
fn portrait_and_background_art_paths_roundtrip_through_toml() {
    let mut profile = sample_profile();
    profile.game.custom_portrait_art_path = "/art/portrait.png".to_string();
    profile.game.custom_background_art_path = "/art/bg.png".to_string();

    let serialized = toml::to_string_pretty(&profile).expect("serialize");
    assert!(serialized.contains("custom_portrait_art_path"));
    assert!(serialized.contains("custom_background_art_path"));

    let parsed: GameProfile = toml::from_str(&serialized).expect("deserialize");
    assert_eq!(parsed.game.custom_portrait_art_path, "/art/portrait.png");
    assert_eq!(parsed.game.custom_background_art_path, "/art/bg.png");
}

#[test]
fn empty_portrait_and_background_art_paths_omitted_from_toml() {
    let profile = sample_profile();
    let serialized = toml::to_string_pretty(&profile).expect("serialize");
    assert!(!serialized.contains("custom_portrait_art_path"));
    assert!(!serialized.contains("custom_background_art_path"));
}
