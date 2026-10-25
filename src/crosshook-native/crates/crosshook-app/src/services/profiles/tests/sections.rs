//! Section and preset save tests: section isolation, name trimming parity,
//! revision sources, degraded metadata and preset-origin rows.

use std::fs;

use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::{GameProfile, MangoHudConfig};

use super::helpers::{steam_profile, Harness, Metadata};
use crate::services::profiles::{
    CommandArgumentsPayload, CommitWarning, LaunchOptimizationsPayload,
};
use crate::{AppErrorKind, AppEvent, ProfileChangeReason};

const NAME: &str = "Alpha";

fn seed(harness: &Harness) -> GameProfile {
    let home = harness.root.path().to_string_lossy().into_owned();
    let mut profile = steam_profile(&home);
    profile
        .extra
        .insert("future_key".to_string(), "keep".into());
    harness.profiles.save(NAME, &profile).unwrap();
    profile
}

fn changed(reason: ProfileChangeReason) -> Option<AppEvent> {
    Some(AppEvent::ProfilesChanged {
        reason,
        names: vec![NAME.to_string()],
    })
}

fn optimizations(ids: &[&str]) -> LaunchOptimizationsPayload {
    LaunchOptimizationsPayload {
        enabled_option_ids: ids.iter().map(ToString::to_string).collect(),
        switch_active_preset: None,
    }
}

/// Sources of the stored revisions of `name`, newest first.
fn revision_sources(metadata: &MetadataStore, name: &str) -> Vec<String> {
    let profile_id = metadata.lookup_profile_id(name).unwrap().unwrap();
    metadata
        .list_config_revisions(&profile_id, None)
        .unwrap()
        .into_iter()
        .map(|row| row.source)
        .collect()
}

/// `(origin, source_bundled_preset_id)` of a stored preset-origin row.
fn preset_origin(
    metadata: &MetadataStore,
    name: &str,
    preset_name: &str,
) -> Option<(String, Option<String>)> {
    let profile_id = metadata.lookup_profile_id(name).unwrap()?;
    metadata
        .with_sqlite_conn("read preset origin", |conn| {
            Ok(conn
                .query_row(
                    "SELECT origin, source_bundled_preset_id FROM profile_launch_preset_metadata \
                     WHERE profile_id = ?1 AND preset_name = ?2",
                    [profile_id.as_str(), preset_name],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .ok())
        })
        .unwrap()
}

#[tokio::test]
async fn launch_optimizations_update_only_the_launch_section() {
    let mut harness = Harness::new(Metadata::Enabled);
    let profile = seed(&harness);
    let payload = optimizations(&["disable_steam_input", "use_gamemode"]);

    let commit = harness
        .writer
        .save_launch_optimizations(format!("  {NAME}  "), payload.clone())
        .await
        .unwrap();

    assert!(commit.warnings.is_empty(), "{:?}", commit.warnings);
    let loaded = harness.profiles.load(NAME).unwrap();
    assert_eq!(loaded.game, profile.game);
    assert_eq!(loaded.trainer, profile.trainer);
    assert_eq!(loaded.injection, profile.injection);
    assert_eq!(loaded.steam, profile.steam);
    assert_eq!(loaded.runtime, profile.runtime);
    assert_eq!(loaded.launch.method, profile.launch.method);
    assert_eq!(
        loaded.launch.optimizations.enabled_option_ids,
        payload.enabled_option_ids
    );
    assert_eq!(
        revision_sources(&harness.metadata, NAME),
        ["launch_optimization_save"]
    );
    // The trimmed name is published.
    assert_eq!(
        harness.next_event().await,
        changed(ProfileChangeReason::Saved)
    );
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn launch_optimizations_reject_missing_and_blank_profiles() {
    let mut harness = Harness::new(Metadata::Enabled);

    let missing = harness
        .writer
        .save_launch_optimizations("missing-profile".into(), optimizations(&["use_gamemode"]))
        .await
        .unwrap_err();
    let blank = harness
        .writer
        .save_launch_optimizations("   ".into(), optimizations(&["use_gamemode"]))
        .await
        .unwrap_err();

    assert_eq!(missing.kind(), AppErrorKind::NotFound);
    assert!(missing.message().contains("profile file not found"));
    assert_eq!(blank.kind(), AppErrorKind::Validation { field: "name" });
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn section_save_keeps_disk_extras_and_records_manual_save() {
    let mut harness = Harness::new(Metadata::Enabled);
    seed(&harness);
    let config = MangoHudConfig {
        enabled: true,
        fps_limit: Some(90),
        ..Default::default()
    };

    harness
        .writer
        .save_mangohud_config(NAME.into(), config.clone())
        .await
        .unwrap();

    let loaded = harness.profiles.load(NAME).unwrap();
    assert_eq!(loaded.launch.mangohud, config);
    let raw = fs::read_to_string(harness.profiles.base_path.join(format!("{NAME}.toml"))).unwrap();
    assert!(raw.contains("future_key"), "{raw}");
    assert_eq!(revision_sources(&harness.metadata, NAME), ["manual_save"]);
    assert_eq!(
        harness.next_event().await,
        changed(ProfileChangeReason::Saved)
    );
}

#[tokio::test]
async fn section_save_without_metadata_saves_and_warns() {
    let mut harness = Harness::new(Metadata::Disabled);
    seed(&harness);
    let mut config = harness.profiles.load(NAME).unwrap().launch.gamescope;
    config.enabled = true;

    let commit = harness
        .writer
        .save_trainer_gamescope_config(NAME.into(), config.clone())
        .await
        .unwrap();

    assert_eq!(commit.warnings, [CommitWarning::MetadataUnavailable]);
    assert_eq!(commit.new_revision_id, None);
    let loaded = harness.profiles.load(NAME).unwrap();
    assert_eq!(loaded.launch.trainer_gamescope, config);
    assert_eq!(
        harness.next_event().await,
        changed(ProfileChangeReason::Saved)
    );
}

#[tokio::test]
async fn command_arguments_save_and_reject_without_writing() {
    let mut harness = Harness::new(Metadata::Enabled);
    seed(&harness);
    let payload = CommandArgumentsPayload {
        enabled_argument_ids: vec![" force_vulkan ".into()],
        custom_args: vec!["-windowed".into()],
    };

    let rejected = harness
        .writer
        .save_command_arguments(NAME.into(), payload.clone(), Some("native".into()))
        .await
        .unwrap_err();
    assert_eq!(
        rejected.kind(),
        AppErrorKind::Validation { field: "launch" }
    );
    assert_eq!(harness.next_event().await, None);

    harness
        .writer
        .save_command_arguments(format!(" {NAME} "), payload, None)
        .await
        .unwrap();

    let arguments = harness
        .profiles
        .load(NAME)
        .unwrap()
        .launch
        .command_arguments;
    assert_eq!(arguments.enabled_argument_ids, ["force_vulkan"]);
    assert_eq!(arguments.custom_args, ["-windowed"]);
    assert_eq!(
        revision_sources(&harness.metadata, NAME),
        ["launch_command_arguments_save"]
    );
    assert_eq!(
        harness.next_event().await,
        changed(ProfileChangeReason::Saved)
    );
}

#[tokio::test]
async fn bundled_preset_on_unindexed_profile_records_origin() {
    let mut harness = Harness::new(Metadata::Enabled);
    // Written straight to disk: no metadata row exists yet.
    seed(&harness);
    assert_eq!(harness.metadata.lookup_profile_id(NAME).unwrap(), None);
    let presets = harness
        .writer
        .list_bundled_optimization_presets()
        .await
        .unwrap();
    let preset = presets
        .iter()
        .find(|preset| preset.preset_id == "nvidia_performance")
        .unwrap();

    let commit = harness
        .writer
        .apply_bundled_optimization_preset(NAME.into(), " nvidia_performance ".into())
        .await
        .unwrap();

    assert!(commit.warnings.is_empty(), "{:?}", commit.warnings);
    let output = commit.into_output();
    assert!(output.extra.is_empty());
    assert_eq!(output.launch.active_preset, "bundled/nvidia_performance");
    assert_eq!(
        output.launch.optimizations.enabled_option_ids,
        preset.enabled_option_ids
    );
    assert_eq!(
        preset_origin(&harness.metadata, NAME, "bundled/nvidia_performance"),
        Some(("bundled".into(), Some("nvidia_performance".into())))
    );
    assert_eq!(revision_sources(&harness.metadata, NAME), ["preset_apply"]);
    assert_eq!(
        harness.next_event().await,
        changed(ProfileChangeReason::PresetApplied)
    );
}

#[tokio::test]
async fn bundled_presets_fail_closed_without_metadata() {
    let mut harness = Harness::new(Metadata::Disabled);
    seed(&harness);

    let listed = harness
        .writer
        .list_bundled_optimization_presets()
        .await
        .unwrap_err();
    let applied = harness
        .writer
        .apply_bundled_optimization_preset(NAME.into(), "nvidia_performance".into())
        .await
        .unwrap_err();

    assert_eq!(listed.kind(), AppErrorKind::Unavailable);
    assert_eq!(applied.kind(), AppErrorKind::Unavailable);
    assert!(harness
        .profiles
        .load(NAME)
        .unwrap()
        .launch
        .presets
        .is_empty());
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn manual_preset_records_user_origin_and_rejects_reserved_names() {
    let mut harness = Harness::new(Metadata::Enabled);
    seed(&harness);

    let reserved = harness
        .writer
        .save_manual_optimization_preset(NAME.into(), "bundled/x".into(), Vec::new())
        .await
        .unwrap_err();
    assert_eq!(
        reserved.kind(),
        AppErrorKind::Validation { field: "profile" }
    );
    assert_eq!(harness.next_event().await, None);

    let output = harness
        .writer
        .save_manual_optimization_preset(
            NAME.into(),
            "  Fast  ".into(),
            vec!["use_gamemode".into()],
        )
        .await
        .unwrap()
        .into_output();

    assert_eq!(output.launch.active_preset, "Fast");
    assert_eq!(
        output.launch.presets["Fast"].enabled_option_ids,
        ["use_gamemode"]
    );
    assert_eq!(
        preset_origin(&harness.metadata, NAME, "Fast"),
        Some(("user".into(), None))
    );
    assert_eq!(
        harness.next_event().await,
        changed(ProfileChangeReason::PresetApplied)
    );
}

#[tokio::test]
async fn manual_preset_saves_without_metadata() {
    let mut harness = Harness::new(Metadata::Disabled);
    seed(&harness);

    let commit = harness
        .writer
        .save_manual_optimization_preset(NAME.into(), "Fast".into(), vec!["use_gamemode".into()])
        .await
        .unwrap();

    assert_eq!(commit.warnings, [CommitWarning::MetadataUnavailable]);
    assert_eq!(
        harness.profiles.load(NAME).unwrap().launch.active_preset,
        "Fast"
    );
    assert_eq!(
        harness.next_event().await,
        changed(ProfileChangeReason::PresetApplied)
    );
}
