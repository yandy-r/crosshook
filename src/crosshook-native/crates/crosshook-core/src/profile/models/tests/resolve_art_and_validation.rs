#![cfg(test)]

use super::super::*;
use super::fixtures::*;

// --- resolve_art_app_id ---

#[test]
fn resolve_art_app_id_prefers_steam_app_id_when_both_set() {
    let mut profile = sample_profile();
    profile.steam.app_id = "111111".to_string();
    profile.runtime.steam_app_id = "222222".to_string();
    assert_eq!(resolve_art_app_id(&profile), "111111");
}

#[test]
fn resolve_art_app_id_falls_back_to_runtime_when_steam_empty() {
    let mut profile = sample_profile();
    profile.steam.app_id = String::new();
    profile.runtime.steam_app_id = "1245620".to_string();
    assert_eq!(resolve_art_app_id(&profile), "1245620");
}

#[test]
fn resolve_art_app_id_returns_empty_when_neither_set() {
    let profile = sample_profile();
    assert_eq!(resolve_art_app_id(&profile), "");
}

#[test]
fn resolve_art_app_id_trims_whitespace() {
    let mut profile = sample_profile();
    profile.steam.app_id = "  ".to_string();
    profile.runtime.steam_app_id = " 1245620 ".to_string();
    assert_eq!(resolve_art_app_id(&profile), "1245620");
}

// --- validate_steam_app_id ---

#[test]
fn validate_steam_app_id_accepts_empty_string() {
    assert!(validate_steam_app_id("").is_ok());
}

#[test]
fn validate_steam_app_id_accepts_valid_ids() {
    assert!(validate_steam_app_id("1245620").is_ok());
    assert!(validate_steam_app_id("570").is_ok());
    assert!(validate_steam_app_id("730").is_ok());
    assert!(validate_steam_app_id("123456789012").is_ok()); // 12 digits — max
}

#[test]
fn validate_steam_app_id_rejects_non_numeric() {
    assert!(validate_steam_app_id("abc").is_err());
    assert!(validate_steam_app_id("123abc").is_err());
    assert!(validate_steam_app_id("12.3").is_err());
    assert!(validate_steam_app_id("12 3").is_err());
}

#[test]
fn validate_steam_app_id_rejects_more_than_12_digits() {
    assert!(validate_steam_app_id("1234567890123").is_err()); // 13 digits
}

#[test]
fn validate_steam_app_id_accepts_exactly_12_digits() {
    assert!(validate_steam_app_id("123456789012").is_ok());
}
