#![cfg(test)]

use super::super::*;
use super::fixtures::*;

#[test]
fn legacy_profile_without_injection_keys_defaults_to_config() {
    let toml = r#"
[game]
executable_path = "/games/test.exe"

[launch]
method = "proton-run"
"#;

    let mut parsed: GameProfile = toml::from_str(toml).expect("deserialize");
    parsed.normalize_injection();

    assert!(parsed.injection.loaded_hooks.is_empty());
    assert!(parsed.injection.dll_paths.is_empty());
    assert!(parsed.injection.inject_on_launch.is_empty());
    assert_eq!(parsed.injection.method, InjectionMethod::Disabled);
    assert_eq!(parsed.injection.stage, InjectionStage::TrainerLaunch);
    assert_eq!(parsed.injection.timeout_ms, 0);
    assert_eq!(
        parsed.injection.fallback,
        InjectionFallback::WarnAndContinue
    );
}

#[test]
fn legacy_dll_arrays_normalize_to_canonical_loaded_hooks() {
    let toml = r#"
[game]
executable_path = "/games/test.exe"

[injection]
dll_paths = ["/dlls/overlay.dll", "/dlls/metrics.dll", ""]
inject_on_launch = [true]
"#;

    let mut parsed: GameProfile = toml::from_str(toml).expect("deserialize");
    parsed.normalize_injection();

    assert_eq!(parsed.injection.loaded_hooks.len(), 2);
    assert_eq!(parsed.injection.loaded_hooks[0].id, "legacy-dll-1");
    assert_eq!(parsed.injection.loaded_hooks[0].name, "overlay");
    assert_eq!(parsed.injection.loaded_hooks[0].path, "/dlls/overlay.dll");
    assert!(parsed.injection.loaded_hooks[0].enabled);
    assert_eq!(parsed.injection.loaded_hooks[1].id, "legacy-dll-2");
    assert_eq!(parsed.injection.loaded_hooks[1].name, "metrics");
    assert_eq!(parsed.injection.loaded_hooks[1].path, "/dlls/metrics.dll");
    assert!(!parsed.injection.loaded_hooks[1].enabled);
    assert_eq!(
        parsed.injection.dll_paths,
        vec!["/dlls/overlay.dll", "/dlls/metrics.dll"]
    );
    assert_eq!(parsed.injection.inject_on_launch, vec![true, false]);
}

#[test]
fn canonical_loaded_hooks_refresh_legacy_mirrors() {
    let mut profile = sample_profile();
    profile.injection.loaded_hooks = vec![
        LoadedDllHook {
            id: "dll-a".to_string(),
            name: "Overlay".to_string(),
            path: "/dlls/overlay.dll".to_string(),
            enabled: true,
        },
        LoadedDllHook {
            id: "dll-b".to_string(),
            name: "Metrics".to_string(),
            path: "/dlls/metrics.dll".to_string(),
            enabled: false,
        },
    ];
    profile.injection.dll_paths = vec!["/legacy/old.dll".to_string()];
    profile.injection.inject_on_launch = vec![false, false, false];

    profile.normalize_injection();

    assert_eq!(
        profile.injection.dll_paths,
        vec!["/dlls/overlay.dll", "/dlls/metrics.dll"]
    );
    assert_eq!(profile.injection.inject_on_launch, vec![true, false]);
}

#[test]
fn malformed_loaded_hook_missing_identity_dropped() {
    let toml = r#"
[game]
executable_path = "/games/test.exe"

[injection]

[[injection.loaded_hooks]]
name = "Missing ID"
path = "/dlls/missing-id.dll"
enabled = true

[[injection.loaded_hooks]]
id = "dll-good"
name = "Good"
path = "/dlls/good.dll"
enabled = true
"#;

    let mut parsed: GameProfile = toml::from_str(toml).expect("deserialize");
    assert_eq!(parsed.injection.loaded_hooks.len(), 2);

    parsed.normalize_injection();

    assert_eq!(parsed.injection.loaded_hooks.len(), 1);
    assert_eq!(parsed.injection.loaded_hooks[0].id, "dll-good");
    assert_eq!(parsed.injection.dll_paths, vec!["/dlls/good.dll"]);
    assert_eq!(parsed.injection.inject_on_launch, vec![true]);
}

#[test]
fn injection_config_uses_snake_case_toml_values() {
    let mut profile = sample_profile();
    profile.injection.method = InjectionMethod::LoadLibrary;
    profile.injection.stage = InjectionStage::GameProcessReady;
    profile.injection.timeout_ms = 2500;
    profile.injection.fallback = InjectionFallback::AbortLaunch;

    let serialized = toml::to_string_pretty(&profile).expect("serialize");

    assert!(serialized.contains(r#"method = "load_library""#));
    assert!(serialized.contains(r#"stage = "game_process_ready""#));
    assert!(serialized.contains("timeout_ms = 2500"));
    assert!(serialized.contains(r#"fallback = "abort_launch""#));
}

#[test]
fn unknown_injection_config_value_is_rejected() {
    let toml = r#"
[game]
executable_path = "/games/test.exe"

[injection]
method = "kernel_magic"
"#;

    let result = toml::from_str::<GameProfile>(toml);

    assert!(
        result.is_err(),
        "expected deserialization to reject unknown injection method"
    );
}
