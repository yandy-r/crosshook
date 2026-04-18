#![cfg(test)]

use super::super::*;
use super::fixtures::*;

#[test]
fn profile_toml_without_trainer_type_deserializes_unknown() {
    let toml = r#"
[game]
executable_path = "/games/x.exe"

[trainer]
path = "/t/y.exe"
type = "fling"
"#;
    let p: GameProfile = toml::from_str(toml).expect("deserialize");
    assert_eq!(p.trainer.trainer_type, "unknown");
}

#[test]
fn profile_trainer_type_roundtrip_toml() {
    let mut p = sample_profile();
    p.trainer.trainer_type = "aurora".to_string();
    let s = toml::to_string_pretty(&p).expect("serialize");
    let back: GameProfile = toml::from_str(&s).expect("deserialize");
    assert_eq!(back.trainer.trainer_type, "aurora");
}

#[test]
fn trainer_section_roundtrip_with_required_protontricks() {
    let section = TrainerSection {
        required_protontricks: vec!["vcrun2019".to_string(), "dotnet48".to_string()],
        ..Default::default()
    };
    let toml_str = toml::to_string_pretty(&section).unwrap();
    let deserialized: TrainerSection = toml::from_str(&toml_str).unwrap();
    assert_eq!(
        deserialized.required_protontricks,
        section.required_protontricks
    );
}

#[test]
fn trainer_section_roundtrip_without_required_protontricks() {
    let section = TrainerSection::default();
    let toml_str = toml::to_string_pretty(&section).unwrap();
    assert!(
        !toml_str.contains("required_protontricks"),
        "empty vec should be skipped in serialization"
    );
    let deserialized: TrainerSection = toml::from_str(&toml_str).unwrap();
    assert!(deserialized.required_protontricks.is_empty());
}

#[test]
fn trainer_section_deserialize_without_field() {
    // Simulate existing TOML that doesn't have the field (backward compatibility)
    let toml_str = r#"
path = "/some/path"
type = "fling"
loading_mode = "source_directory"
"#;
    let section: TrainerSection = toml::from_str(toml_str).unwrap();
    assert!(section.required_protontricks.is_empty());
}

#[test]
fn local_override_trainer_section_with_extra_protontricks() {
    let section = LocalOverrideTrainerSection {
        path: String::new(),
        extra_protontricks: vec!["xact".to_string()],
    };
    assert!(!section.is_empty());
    let toml_str = toml::to_string_pretty(&section).unwrap();
    assert!(toml_str.contains("extra_protontricks"));
}

#[test]
fn local_override_trainer_section_empty_when_both_empty() {
    let section = LocalOverrideTrainerSection::default();
    assert!(section.is_empty());
}
