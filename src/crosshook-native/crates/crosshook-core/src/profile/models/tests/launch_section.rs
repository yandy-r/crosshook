#![cfg(test)]

use crate::launch::request::METHOD_PROTON_RUN;

use super::super::*;
use super::fixtures::*;

#[test]
fn normalize_preset_selection_clears_unknown_active_preset() {
    let mut launch = LaunchSection::default();
    launch.active_preset = "missing".to_string();
    launch.optimizations.enabled_option_ids = vec!["use_gamemode".to_string()];
    launch.normalize_preset_selection();
    assert!(launch.active_preset.is_empty());
    assert_eq!(
        launch.optimizations.enabled_option_ids,
        vec!["use_gamemode".to_string()]
    );
}

#[test]
fn launch_presets_toml_roundtrip() {
    use std::collections::BTreeMap;

    let mut launch = LaunchSection::default();
    launch.method = METHOD_PROTON_RUN.to_string();
    launch.optimizations.enabled_option_ids = vec!["use_gamemode".to_string()];
    launch.active_preset = "quality".to_string();
    let mut presets = BTreeMap::new();
    presets.insert(
        "performance".to_string(),
        LaunchOptimizationsSection {
            enabled_option_ids: vec!["disable_steam_input".to_string()],
        },
    );
    presets.insert(
        "quality".to_string(),
        LaunchOptimizationsSection {
            enabled_option_ids: vec!["enable_hdr".to_string()],
        },
    );
    launch.presets = presets;

    let profile = GameProfile {
        launch,
        ..GameProfile::default()
    };
    let serialized = toml::to_string_pretty(&profile).expect("serialize");
    let parsed: GameProfile = toml::from_str(&serialized).expect("deserialize");
    assert_eq!(parsed.launch.presets.len(), 2);
    assert_eq!(parsed.launch.active_preset, "quality");
    assert_eq!(
        parsed.launch.optimizations.enabled_option_ids,
        vec!["use_gamemode".to_string()]
    );
}

#[test]
fn custom_env_vars_empty_omitted_from_toml_and_roundtrips() {
    let profile = sample_profile();
    let serialized = toml::to_string_pretty(&profile).expect("serialize");
    assert!(
        !serialized.contains("custom_env_vars"),
        "expected empty map skipped: {serialized}"
    );
    let parsed: GameProfile = toml::from_str(&serialized).expect("deserialize");
    assert!(parsed.launch.custom_env_vars.is_empty());
}

#[test]
fn custom_env_vars_nonempty_toml_roundtrip() {
    use std::collections::BTreeMap;

    let mut profile = sample_profile();
    profile.launch.custom_env_vars = BTreeMap::from([
        ("DXVK_ASYNC".to_string(), "1".to_string()),
        ("MANGOHUD".to_string(), "1".to_string()),
    ]);
    let serialized = toml::to_string_pretty(&profile).expect("serialize");
    assert!(serialized.contains("custom_env_vars"));
    let parsed: GameProfile = toml::from_str(&serialized).expect("deserialize");
    assert_eq!(
        parsed.launch.custom_env_vars,
        profile.launch.custom_env_vars
    );
}

#[test]
fn trainer_gamescope_default_omitted_from_profile_toml() {
    let profile = sample_profile();
    let serialized = toml::to_string_pretty(&profile).expect("serialize");
    assert!(
        !serialized.contains("[launch.trainer_gamescope]"),
        "default GamescopeConfig should be omitted from TOML output: {serialized}"
    );
}

#[test]
fn trainer_gamescope_roundtrip() {
    let mut profile = sample_profile();
    profile.launch.trainer_gamescope = GamescopeConfig {
        enabled: true,
        internal_width: Some(800),
        internal_height: Some(400),
        fullscreen: true,
        grab_cursor: true,
        ..GamescopeConfig::default()
    };
    let serialized = toml::to_string_pretty(&profile).expect("serialize");
    assert!(serialized.contains("[launch.trainer_gamescope]"));
    let parsed: GameProfile = toml::from_str(&serialized).expect("deserialize");
    assert_eq!(
        parsed.launch.trainer_gamescope,
        profile.launch.trainer_gamescope
    );
}

#[test]
fn launch_section_resolved_trainer_gamescope_auto_generates_windowed() {
    let mut launch = LaunchSection::default();
    launch.gamescope = GamescopeConfig {
        enabled: true,
        fullscreen: true,
        borderless: true,
        internal_width: Some(1920),
        internal_height: Some(1080),
        ..GamescopeConfig::default()
    };
    launch.trainer_gamescope = GamescopeConfig::default();

    let resolved = launch.resolved_trainer_gamescope();

    assert!(resolved.enabled);
    assert!(!resolved.fullscreen);
    assert!(!resolved.borderless);
    assert_eq!(resolved.internal_width, Some(1920));
    assert_eq!(resolved.internal_height, Some(1080));
}

#[test]
fn launch_section_resolved_trainer_gamescope_prefers_trainer_when_enabled() {
    let mut launch = LaunchSection::default();
    launch.gamescope = GamescopeConfig {
        enabled: true,
        internal_width: Some(1920),
        internal_height: Some(1080),
        ..GamescopeConfig::default()
    };
    launch.trainer_gamescope = GamescopeConfig {
        enabled: true,
        internal_width: Some(800),
        internal_height: Some(600),
        ..GamescopeConfig::default()
    };

    assert_eq!(
        launch.resolved_trainer_gamescope(),
        launch.trainer_gamescope
    );
}

#[test]
fn launch_section_resolved_trainer_gamescope_returns_default_when_game_disabled() {
    let mut launch = LaunchSection::default();
    launch.gamescope = GamescopeConfig::default();
    launch.trainer_gamescope = GamescopeConfig::default();

    let resolved = launch.resolved_trainer_gamescope();

    assert_eq!(resolved, GamescopeConfig::default());
}

// --- Parity: LaunchSection::resolved_trainer_gamescope == LaunchRequest::resolved_trainer_gamescope ---

#[test]
fn resolved_trainer_gamescope_parity_explicit_enabled() {
    // Branch 1: explicit trainer override with enabled=true
    let game_cfg = GamescopeConfig {
        enabled: true,
        fullscreen: true,
        internal_width: Some(1920),
        internal_height: Some(1080),
        ..GamescopeConfig::default()
    };
    let trainer_cfg = GamescopeConfig {
        enabled: true,
        fullscreen: false,
        internal_width: Some(800),
        internal_height: Some(600),
        ..GamescopeConfig::default()
    };

    let mut launch = LaunchSection::default();
    launch.gamescope = game_cfg.clone();
    launch.trainer_gamescope = trainer_cfg.clone();

    let request = crate::launch::LaunchRequest {
        gamescope: game_cfg,
        trainer_gamescope: Some(trainer_cfg),
        ..crate::launch::LaunchRequest::default()
    };

    assert_eq!(
        launch.resolved_trainer_gamescope(),
        request.resolved_trainer_gamescope(),
        "explicit-enabled branch must produce equal results"
    );
}

#[test]
fn resolved_trainer_gamescope_parity_disabled_override_auto_derive() {
    // Branch 2: trainer override disabled (or None) → auto-derive windowed from game
    let game_cfg = GamescopeConfig {
        enabled: true,
        fullscreen: true,
        borderless: false,
        internal_width: Some(2560),
        internal_height: Some(1440),
        frame_rate_limit: Some(60),
        ..GamescopeConfig::default()
    };

    // LaunchSection uses a non-enabled trainer_gamescope (disabled explicit override)
    let mut launch = LaunchSection::default();
    launch.gamescope = game_cfg.clone();
    launch.trainer_gamescope = GamescopeConfig::default(); // enabled=false

    // LaunchRequest uses None (no override) — semantically equivalent: auto-derive
    let request = crate::launch::LaunchRequest {
        gamescope: game_cfg,
        trainer_gamescope: None,
        ..crate::launch::LaunchRequest::default()
    };

    assert_eq!(
        launch.resolved_trainer_gamescope(),
        request.resolved_trainer_gamescope(),
        "disabled-override auto-derive branch must produce equal results"
    );
}

#[test]
fn resolved_trainer_gamescope_parity_disabled_game_default() {
    // Branch 3: game gamescope disabled → both return GamescopeConfig::default()
    let mut launch = LaunchSection::default();
    launch.gamescope = GamescopeConfig::default(); // enabled=false
    launch.trainer_gamescope = GamescopeConfig::default();

    let request = crate::launch::LaunchRequest {
        gamescope: GamescopeConfig::default(),
        trainer_gamescope: None,
        ..crate::launch::LaunchRequest::default()
    };

    assert_eq!(
        launch.resolved_trainer_gamescope(),
        request.resolved_trainer_gamescope(),
        "disabled-game default branch must produce equal results"
    );
}

#[test]
fn network_isolation_defaults_true_when_absent_from_toml() {
    let toml = r#"
[game]
executable_path = "/games/x.exe"
[trainer]
path = "/t/y.exe"
type = "fling"
[launch]
"#;
    let toml = toml.to_string() + &format!(r#"method = "{METHOD_PROTON_RUN}""#);
    let p: GameProfile = toml::from_str(&toml).expect("deserialize");
    assert!(p.launch.network_isolation);
}

#[test]
fn network_isolation_false_roundtrips_through_toml() {
    let mut p = sample_profile();
    p.launch.network_isolation = false;
    let s = toml::to_string_pretty(&p).expect("serialize");
    assert!(s.contains("network_isolation = false"));
    let back: GameProfile = toml::from_str(&s).expect("deserialize");
    assert!(!back.launch.network_isolation);
}

#[test]
fn network_isolation_true_omitted_from_toml() {
    let mut p = sample_profile();
    p.launch.network_isolation = true;
    let s = toml::to_string_pretty(&p).expect("serialize");
    assert!(
        !s.contains("network_isolation"),
        "true (default) should be omitted: {s}"
    );
}

#[test]
fn launch_section_default_has_network_isolation_true() {
    let launch = LaunchSection::default();
    assert!(launch.network_isolation);
}
