use std::collections::BTreeMap;

use crate::profile::{CollectionDefaultsSection, GameProfile};

const PROFILE_WITH_UNKNOWN_FIELDS: &str = r#"
root_future = 1979-05-27T07:32:00Z

[game]
name = "Game"
future_ratio = 1.25

[trainer]
future_trainer = "x"

[injection]
future_injection = true

[[injection.loaded_hooks]]
id = "dll-a"
path = "/dll/a.dll"
future_dll = 2.5

[steam]
future_steam = "s"

[steam.launcher]
future_launcher = "l"

[runtime]
future_runtime = { nested = 1979-05-27 }

[launch]
method = "proton_run"
active_preset = "fast"
future_launch = 0.5

[launch.optimizations]
enabled_option_ids = ["old"]
future_active = "keep"

[launch.command_arguments]
future_args = "a"

[launch.presets.fast]
enabled_option_ids = ["fast"]
future_preset = 9.75

[launch.gamescope]
future_gamescope = 1.5

[launch.trainer_gamescope]
future_trainer_gamescope = 2

[launch.mangohud]
future_mangohud = "m"

[local_override]
future_local = "keep"

[local_override.game]
future_game = 1

[local_override.trainer]
future_trainer = 2

[local_override.steam]
future_steam = 3

[local_override.runtime]
future_runtime = 4

[future_section]
value = 1.5

[[pre_launch_hooks]]
id = "hook-a"
name = "Hook"
path = "/hook"
future_hook = 1979-05-27T00:32:00-07:00
"#;

fn parsed() -> GameProfile {
    toml::from_str(PROFILE_WITH_UNKNOWN_FIELDS).expect("profile parses")
}

fn reparsed(profile: &GameProfile) -> toml::Table {
    toml::from_str(&toml::to_string(profile).expect("profile serializes")).expect("valid TOML")
}

#[test]
fn unknown_toml_round_trips_recursively_without_type_loss() {
    let table = reparsed(&parsed());

    assert!(table["root_future"].is_datetime());
    assert_eq!(table["game"]["future_ratio"].as_float(), Some(1.25));
    assert_eq!(table["future_section"]["value"].as_float(), Some(1.5));
    assert!(table["runtime"]["future_runtime"]["nested"].is_datetime());
    assert_eq!(
        table["injection"]["loaded_hooks"][0]["future_dll"].as_float(),
        Some(2.5)
    );
    assert_eq!(
        table["launch"]["presets"]["fast"]["future_preset"].as_float(),
        Some(9.75)
    );
    assert_eq!(
        table["launch"]["gamescope"]["future_gamescope"].as_float(),
        Some(1.5)
    );
    assert!(table["pre_launch_hooks"][0]["future_hook"].is_datetime());
    assert_eq!(
        table["local_override"]["runtime"]["future_runtime"].as_integer(),
        Some(4)
    );
}

#[test]
fn extra_only_sections_and_extra_only_local_override_are_not_skipped() {
    let profile: GameProfile = toml::from_str(
        "[runtime]\nfuture = 1\n[launch.gamescope]\nfuture = 2\n[local_override.game]\nfuture = 3\n",
    )
    .unwrap();
    let table = reparsed(&profile.portable_profile());

    assert_eq!(table["runtime"]["future"].as_integer(), Some(1));
    assert_eq!(table["launch"]["gamescope"]["future"].as_integer(), Some(2));
    assert_eq!(
        table["local_override"]["game"]["future"].as_integer(),
        Some(3)
    );
}

#[test]
fn malformed_known_field_is_still_rejected() {
    let err = toml::from_str::<GameProfile>("[launch]\nnetwork_isolation = \"no\"\nfuture = 1\n");
    assert!(err.is_err());
}

#[test]
fn storage_effective_and_portable_keep_extras_but_drop_known_local_values() {
    let mut profile = parsed();
    profile.local_override.game.executable_path = "/local/game.exe".into();
    profile.normalize_injection();
    profile.launch.normalize_preset_selection();

    let effective = profile.effective_profile();
    assert_eq!(
        effective.local_override.extra["future_local"].as_str(),
        Some("keep")
    );
    assert_eq!(
        effective.launch.optimizations.extra["future_active"].as_str(),
        Some("keep")
    );
    assert!(!effective
        .launch
        .optimizations
        .extra
        .contains_key("future_preset"));
    assert_eq!(
        effective.injection.loaded_hooks[0].extra["future_dll"].as_float(),
        Some(2.5)
    );

    let storage = profile.storage_profile();
    assert_eq!(
        storage.local_override.game.executable_path,
        "/local/game.exe"
    );
    assert_eq!(
        storage.local_override.extra["future_local"].as_str(),
        Some("keep")
    );

    let portable = profile.portable_profile();
    assert_eq!(portable.local_override.game.executable_path, "");
    assert_eq!(
        portable.local_override.game.extra["future_game"].as_integer(),
        Some(1)
    );
    assert_eq!(portable.local_override.extra, profile.local_override.extra);
}

#[test]
fn collection_defaults_replace_known_fields_without_leaking_collection_extras() {
    let profile = parsed();
    let defaults: CollectionDefaultsSection = toml::from_str(
        "future_collection = 1\n[optimizations]\nenabled_option_ids = [\"collection\"]\nfuture_active = \"collection\"\n[gamescope]\nenabled = true\nfuture_gamescope = 3.5\n",
    )
    .unwrap();

    let merged = profile.effective_profile_with(Some(&defaults));

    assert!(!merged.launch.extra.contains_key("future_collection"));
    assert_eq!(merged.launch.extra["future_launch"].as_float(), Some(0.5));
    assert_eq!(
        merged.launch.optimizations.enabled_option_ids,
        ["collection"]
    );
    assert_eq!(
        merged.launch.optimizations.extra["future_active"].as_str(),
        Some("keep")
    );
    assert!(merged.launch.gamescope.enabled);
    assert_eq!(
        merged.launch.gamescope.extra["future_gamescope"].as_float(),
        Some(1.5)
    );
}

#[test]
fn clear_and_preserve_extras_are_recursive_and_identity_aware() {
    let existing = parsed();
    let mut incoming = existing.clone();
    incoming.clear_extra();
    assert!(!toml::to_string(&incoming).unwrap().contains("future_"));

    incoming.game.name = "Edited".into();
    incoming.injection.loaded_hooks[0].id = "dll-new".into();
    incoming
        .pre_launch_hooks
        .insert(0, incoming.pre_launch_hooks[0].clone());
    incoming.pre_launch_hooks[0].id = "hook-new".into();
    incoming.launch.presets = BTreeMap::from([(
        "renamed".to_string(),
        incoming.launch.presets["fast"].clone(),
    )]);

    incoming.preserve_extra_from(&existing);

    assert_eq!(incoming.game.name, "Edited");
    assert_eq!(incoming.game.extra["future_ratio"].as_float(), Some(1.25));
    assert!(incoming.extra["root_future"].is_datetime());
    assert!(incoming.injection.loaded_hooks[0].extra.is_empty());
    assert!(incoming.pre_launch_hooks[0].extra.is_empty());
    assert!(incoming.pre_launch_hooks[1].extra["future_hook"].is_datetime());
    assert!(incoming.launch.presets["renamed"].extra.is_empty());
    assert_eq!(
        incoming.local_override.runtime.extra["future_runtime"].as_integer(),
        Some(4)
    );
}

#[test]
fn collection_extra_only_sections_survive_clear_copy_and_toml() {
    let existing: CollectionDefaultsSection = toml::from_str(
        "future = 1979-05-27\n[optimizations]\nfuture = 1.5\n[gamescope]\nfuture = 2.5\n[trainer_gamescope]\nfuture = 3.5\n[mangohud]\nfuture = 4.5\n",
    )
    .unwrap();
    assert!(!existing.is_empty());
    let mut incoming = existing.clone();
    incoming.clear_extra();
    assert!(!toml::to_string(&incoming).unwrap().contains("future"));
    incoming.preserve_extra_from(&existing);
    let reparsed: CollectionDefaultsSection =
        toml::from_str(&toml::to_string(&incoming).unwrap()).unwrap();
    assert_eq!(reparsed, existing);
    let extra_only: CollectionDefaultsSection = toml::from_str("future = 1.5").unwrap();
    assert!(!extra_only.is_empty());
}

#[test]
fn identity_less_hooks_never_inherit_extras_by_position() {
    let existing = parsed();
    let mut incoming = existing.clone();
    incoming.clear_extra();
    let mut new_launch = incoming.pre_launch_hooks[0].clone();
    new_launch.id.clear();
    incoming.pre_launch_hooks.insert(0, new_launch);
    let mut new_dll = incoming.injection.loaded_hooks[0].clone();
    new_dll.id.clear();
    incoming.injection.loaded_hooks.insert(0, new_dll);

    incoming.preserve_extra_from(&existing);

    assert!(incoming.pre_launch_hooks[0].extra.is_empty());
    assert!(incoming.injection.loaded_hooks[0].extra.is_empty());
    assert_eq!(
        incoming.pre_launch_hooks[1].extra,
        existing.pre_launch_hooks[0].extra
    );
    assert_eq!(
        incoming.injection.loaded_hooks[1].extra,
        existing.injection.loaded_hooks[0].extra
    );

    // Empty IDs on disk are not a usable identity either.
    let mut no_ids = existing.clone();
    for hook in &mut no_ids.pre_launch_hooks {
        hook.id.clear();
    }
    for hook in &mut no_ids.injection.loaded_hooks {
        hook.id.clear();
    }
    let mut incoming = no_ids.clone();
    incoming.clear_extra();
    incoming.pre_launch_hooks.reverse();
    incoming.injection.loaded_hooks.reverse();
    incoming.preserve_extra_from(&no_ids);
    assert!(incoming
        .pre_launch_hooks
        .iter()
        .all(|hook| hook.extra.is_empty()));
    assert!(incoming
        .injection
        .loaded_hooks
        .iter()
        .all(|hook| hook.extra.is_empty()));
}
