//! Unit tests for `crate::profile::normalize`.

use std::collections::BTreeMap;

use super::*;
use crate::launch::catalog::{OptimizationEntry, DEFAULT_CATALOG_TOML};
use crate::profile::{GameSection, LaunchSection, LauncherSection, RuntimeSection};

#[test]
fn normalizes_umu_store_and_codename_for_edit_and_save() {
    let mut profile = GameProfile::default();
    profile.runtime.umu_store = "  GOG  ".to_string();
    profile.runtime.umu_codename = "  Cyberpunk_2077  ".to_string();

    let edited = normalize_profile_for_edit(profile.clone());
    assert_eq!(edited.runtime.umu_store, "gog");
    assert_eq!(edited.runtime.umu_codename, "Cyberpunk_2077");

    let saved = normalize_profile(profile);
    assert_eq!(saved.runtime.umu_store, "gog");
    assert_eq!(saved.runtime.umu_codename, "Cyberpunk_2077");
}

#[test]
fn drops_umu_hints_with_control_chars_and_caps_long_values() {
    let mut profile = GameProfile::default();
    profile.runtime.umu_store = "go\ng".to_string();
    profile.runtime.umu_codename = "x".repeat(140);

    let normalized = normalize_profile_for_edit(profile);
    assert_eq!(normalized.runtime.umu_store, "");
    assert_eq!(normalized.runtime.umu_codename.chars().count(), 128);
}

#[test]
fn caps_unicode_umu_hint_without_splitting_utf8() {
    let mut profile = GameProfile::default();
    profile.runtime.umu_codename = "龍".repeat(140);

    let normalized = normalize_profile_for_edit(profile);
    assert_eq!(normalized.runtime.umu_codename.chars().count(), 128);
}

#[test]
fn trims_deduplicates_command_argument_ids_and_preserves_control_char_custom_args() {
    let mut profile = GameProfile::default();
    profile.launch.command_arguments = LaunchCommandArgumentsSection {
        enabled_argument_ids: vec![
            "  force_vulkan  ".to_string(),
            "force_vulkan".to_string(),
            String::new(),
        ],
        custom_args: vec![
            "   ".to_string(),
            "  --dx11  ".to_string(),
            "\t".to_string(),
            "--bad\0arg".to_string(),
        ],
        extra: toml::Table::new(),
    };

    let edited = normalize_profile_for_edit(profile.clone());
    assert_eq!(
        edited.launch.command_arguments.enabled_argument_ids,
        vec!["force_vulkan"]
    );
    assert_eq!(
        edited.launch.command_arguments.custom_args,
        vec!["", "--dx11", "", "--bad\0arg"]
    );

    let saved = normalize_profile(profile);
    assert_eq!(
        saved.launch.command_arguments.enabled_argument_ids,
        vec!["force_vulkan"]
    );
    assert_eq!(
        saved.launch.command_arguments.custom_args,
        vec!["--dx11", "--bad\0arg"]
    );
}

#[test]
fn round_trips_populated_command_arguments_through_edit_normalization() {
    let mut profile = GameProfile::default();
    profile.launch.command_arguments = LaunchCommandArgumentsSection {
        enabled_argument_ids: vec!["force_vulkan".to_string(), "skip_launcher".to_string()],
        custom_args: vec!["-- %command%".to_string(), "+set sv_cheats 1".to_string()],
        extra: toml::Table::new(),
    };

    let edited = normalize_profile_for_edit(profile);
    assert_eq!(
        edited.launch.command_arguments.enabled_argument_ids,
        vec!["force_vulkan", "skip_launcher"]
    );
    assert_eq!(
        edited.launch.command_arguments.custom_args,
        vec!["-- %command%", "+set sv_cheats 1"]
    );
}

#[test]
fn derives_game_and_launcher_display_names_with_suffix_stripping() {
    let mut from_exe = GameProfile {
        game: GameSection {
            executable_path: r"C:\Games\My.Game.exe".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };
    from_exe.steam.launcher = LauncherSection {
        display_name: "  Custom Name - Trainer  ".to_string(),
        ..Default::default()
    };
    let saved = normalize_profile(from_exe);
    assert_eq!(saved.game.name, "My.Game");
    assert_eq!(saved.steam.launcher.display_name, "Custom Name");
    assert_eq!(saved.launch.method, "proton_run");
    assert!(!saved.steam.enabled);

    let mut from_trainer = GameProfile::default();
    from_trainer.trainer.path = "/trainers/Elden Ring - Trainer.exe".to_string();
    let saved_trainer = normalize_profile(from_trainer);
    assert_eq!(saved_trainer.game.name, "");
    assert_eq!(saved_trainer.steam.launcher.display_name, "Elden Ring");
}

#[test]
fn normalizes_presets_active_preset_and_catalog_filtering() {
    let mut profile = GameProfile {
        launch: LaunchSection {
            active_preset: "  Fast  ".to_string(),
            optimizations: LaunchOptimizationsSection {
                enabled_option_ids: vec!["stale".to_string()],
                extra: toml::Table::new(),
            },
            presets: BTreeMap::from([
                (
                    "   ".to_string(),
                    LaunchOptimizationsSection {
                        enabled_option_ids: vec!["ignored".to_string()],
                        extra: toml::Table::new(),
                    },
                ),
                (
                    " Fast ".to_string(),
                    LaunchOptimizationsSection {
                        enabled_option_ids: vec![
                            "use_gamemode".to_string(),
                            "unknown_opt".to_string(),
                            "use_gamemode".to_string(),
                        ],
                        extra: toml::Table::new(),
                    },
                ),
            ]),
            ..Default::default()
        },
        runtime: RuntimeSection {
            prefix_path: "  /pfx  ".to_string(),
            proton_path: "  /proton  ".to_string(),
            working_directory: "  /work  ".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };
    profile.steam.enabled = true;

    let lenient = normalize_profile(profile.clone());
    assert_eq!(lenient.launch.method, "steam_applaunch");
    assert!(lenient.steam.enabled);
    assert_eq!(lenient.launch.active_preset, "Fast");
    assert!(!lenient.launch.presets.contains_key(""));
    assert_eq!(
        lenient.launch.optimizations.enabled_option_ids,
        vec!["use_gamemode", "unknown_opt"]
    );
    assert_eq!(lenient.runtime.prefix_path, "/pfx");
    assert_eq!(lenient.runtime.proton_path, "/proton");
    assert_eq!(lenient.runtime.working_directory, "/work");

    let catalog = OptimizationCatalog::from_entries(vec![OptimizationEntry {
        id: "use_gamemode".to_string(),
        applies_to_method: "proton_run".to_string(),
        env: Vec::new(),
        wrappers: Vec::new(),
        conflicts_with: Vec::new(),
        required_binary: String::new(),
        label: "GameMode".to_string(),
        description: String::new(),
        help_text: String::new(),
        category: "performance".to_string(),
        target_gpu_vendor: String::new(),
        advanced: false,
        community: false,
        applicable_methods: Vec::new(),
    }]);
    let strict = normalize_profile_with_catalog(profile, Some(&catalog));
    assert_eq!(
        strict.launch.optimizations.enabled_option_ids,
        vec!["use_gamemode"]
    );
    assert!(!DEFAULT_CATALOG_TOML.is_empty());
}
