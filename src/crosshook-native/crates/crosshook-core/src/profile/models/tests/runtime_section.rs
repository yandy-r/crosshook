#![cfg(test)]

use super::super::*;
use super::fixtures::*;

#[test]
fn runtime_section_is_empty_returns_false_when_only_steam_app_id_set() {
    let section = RuntimeSection {
        steam_app_id: "1245620".to_string(),
        ..RuntimeSection::default()
    };
    assert!(
        !section.is_empty(),
        "is_empty() must return false when steam_app_id is set"
    );
}

#[test]
fn runtime_section_is_empty_returns_true_when_all_fields_empty() {
    assert!(RuntimeSection::default().is_empty());
}

#[test]
fn runtime_steam_app_id_roundtrips_through_toml() {
    let mut profile = sample_profile();
    profile.runtime.steam_app_id = "1245620".to_string();
    let serialized = toml::to_string_pretty(&profile).expect("serialize");
    assert!(
        serialized.contains("steam_app_id"),
        "serialized TOML must contain steam_app_id: {serialized}"
    );
    let parsed: GameProfile = toml::from_str(&serialized).expect("deserialize");
    assert_eq!(parsed.runtime.steam_app_id, "1245620");
}

#[test]
fn runtime_steam_app_id_empty_omitted_from_toml() {
    let profile = sample_profile();
    let serialized = toml::to_string_pretty(&profile).expect("serialize");
    assert!(
        !serialized.contains("steam_app_id"),
        "empty steam_app_id must be omitted from TOML: {serialized}"
    );
}

#[test]
fn runtime_section_umu_game_id_roundtrip() {
    let section = RuntimeSection {
        prefix_path: "/pfx".to_string(),
        proton_path: "/opt/proton".to_string(),
        working_directory: String::new(),
        steam_app_id: String::new(),
        umu_game_id: "custom-42".to_string(),
        umu_preference: None,
    };
    let toml = toml::to_string(&section).unwrap();
    assert!(toml.contains("umu_game_id = \"custom-42\""));
    let parsed: RuntimeSection = toml::from_str(&toml).unwrap();
    assert_eq!(parsed.umu_game_id, "custom-42");
}

#[test]
fn runtime_section_is_empty_considers_umu_game_id() {
    let mut section = RuntimeSection::default();
    assert!(section.is_empty());
    section.umu_game_id = "x".to_string();
    assert!(!section.is_empty());
    section.umu_game_id = "   ".to_string();
    assert!(section.is_empty()); // whitespace-only trims to empty
    section.umu_game_id = String::new();
    assert!(section.is_empty());
}
