//! Profile name validation at every mutating facade: surrounding spaces are
//! trimmed before any effect, so events and metadata use the stored name;
//! traversal names are rejected before any side effect.

use std::fs;
use std::path::PathBuf;

use crosshook_core::profile::migration::{ApplyMigrationRequest, ProtonPathField};
use crosshook_core::profile::{
    GamescopeConfig, LutrisImportEntry, LutrisImportOutcome, MangoHudConfig,
};
use crosshook_core::protondb::AcceptSuggestionRequest;

use super::helpers::{steam_profile, Harness, Metadata};
use crate::services::profiles::{CommandArgumentsPayload, LaunchOptimizationsPayload};
use crate::{AppError, AppErrorKind, AppEvent, ProfileChangeReason};

const TRAVERSAL: &str = "../x";

fn home(harness: &Harness) -> String {
    harness.root.path().to_string_lossy().into_owned()
}

fn changed(reason: ProfileChangeReason, name: &str) -> Option<AppEvent> {
    Some(AppEvent::ProfilesChanged {
        reason,
        names: vec![name.to_string()],
    })
}

fn assert_validation(error: &AppError) {
    assert!(
        matches!(error.kind(), AppErrorKind::Validation { .. }),
        "expected validation, got {error:?}"
    );
}

/// Metadata row names (live and deleted).
fn metadata_names(harness: &Harness) -> Vec<String> {
    harness
        .metadata
        .with_sqlite_conn("list profile rows", |conn| {
            let mut statement = conn
                .prepare("SELECT current_filename FROM profiles ORDER BY current_filename")
                .unwrap();
            let names = statement
                .query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<Vec<String>, _>>()
                .unwrap();
            Ok(names)
        })
        .unwrap()
}

#[tokio::test]
async fn padded_names_are_trimmed_for_effects_and_events() {
    let mut harness = Harness::new(Metadata::Enabled);

    harness
        .writer
        .save(" Alpha ".to_string(), steam_profile(&home(&harness)))
        .await
        .unwrap();
    assert_eq!(
        harness.next_event().await,
        changed(ProfileChangeReason::Saved, "Alpha")
    );
    assert_eq!(metadata_names(&harness), ["Alpha"]);
    assert!(harness.profiles.base_path.join("Alpha.toml").exists());

    harness
        .writer
        .save_mangohud_config(" Alpha ".to_string(), MangoHudConfig::default())
        .await
        .unwrap();
    assert_eq!(
        harness.next_event().await,
        changed(ProfileChangeReason::Saved, "Alpha")
    );

    harness
        .writer
        .accept_protondb_suggestion(AcceptSuggestionRequest::EnvVar {
            profile_name: " Alpha ".to_string(),
            env_key: "DXVK_ASYNC".to_string(),
            env_value: "1".to_string(),
        })
        .await
        .unwrap();
    assert_eq!(
        harness.next_event().await,
        changed(ProfileChangeReason::Saved, "Alpha")
    );

    harness
        .writer
        .save_manual_optimization_preset(" Alpha ".to_string(), "mine".to_string(), Vec::new())
        .await
        .unwrap();
    assert_eq!(
        harness.next_event().await,
        changed(ProfileChangeReason::PresetApplied, "Alpha")
    );
    assert_eq!(
        metadata_names(&harness),
        ["Alpha"],
        "no second metadata row"
    );

    harness.writer.delete(" Alpha ".to_string()).await.unwrap();
    assert_eq!(
        harness.next_event().await,
        changed(ProfileChangeReason::Deleted, "Alpha")
    );
    assert!(
        harness
            .metadata
            .lookup_profile_id("Alpha")
            .unwrap()
            .is_none(),
        "the trimmed name soft-deletes the row"
    );
}

#[tokio::test]
async fn padded_migration_name_is_trimmed() {
    let mut harness = Harness::new(Metadata::Enabled);
    let mut profile = steam_profile(&home(&harness));
    profile.steam.proton_path = "/stale/GE-Proton9-4/proton".to_string();
    harness.profiles.save("Alpha", &profile).unwrap();
    let proton_dir = harness.root.path().join("GE-Proton9-7");
    fs::create_dir_all(&proton_dir).unwrap();
    let new_path = proton_dir.join("proton");
    fs::write(&new_path, b"#!/bin/sh\n").unwrap();

    harness
        .writer
        .apply_proton_migration(ApplyMigrationRequest {
            profile_name: " Alpha ".to_string(),
            field: ProtonPathField::SteamProtonPath,
            new_path: new_path.to_string_lossy().into_owned(),
        })
        .await
        .unwrap();

    assert_eq!(
        harness.next_event().await,
        changed(ProfileChangeReason::Migrated, "Alpha")
    );
    assert_eq!(metadata_names(&harness), ["Alpha"]);
}

fn lutris_entry(harness: &Harness, suggested_name: &str) -> LutrisImportEntry {
    LutrisImportEntry {
        source_path: PathBuf::from("/lutris/game.yml"),
        suggested_name: suggested_name.to_string(),
        game_name: "Game".to_string(),
        runner: "wine".to_string(),
        mapped: steam_profile(&home(harness)),
        warnings: Vec::new(),
        importable: true,
    }
}

#[tokio::test]
async fn padded_lutris_name_neither_overwrites_nor_splits_metadata() {
    let mut harness = Harness::new(Metadata::Enabled);
    let mut existing = steam_profile(&home(&harness));
    existing.game.name = "Existing".to_string();
    harness
        .writer
        .save("foo".to_string(), existing)
        .await
        .unwrap();
    assert!(harness.next_event().await.is_some());
    let path = harness.profiles.base_path.join("foo.toml");
    let before = fs::read(&path).unwrap();

    let result = harness
        .writer
        .import_lutris(vec![lutris_entry(&harness, " foo ")])
        .await
        .unwrap()
        .into_output();

    assert_eq!(result.imported_count, 1);
    assert_eq!(result.results[0].profile_name.as_deref(), Some("foo-2"));
    assert_eq!(
        fs::read(&path).unwrap(),
        before,
        "existing profile untouched"
    );
    assert_eq!(metadata_names(&harness), ["foo", "foo-2"]);
    assert_eq!(
        harness.next_event().await,
        Some(AppEvent::ProfilesChanged {
            reason: ProfileChangeReason::Imported,
            names: vec!["foo-2".to_string()],
        })
    );
}

#[tokio::test]
async fn invalid_lutris_name_fails_only_that_entry() {
    let mut harness = Harness::new(Metadata::Enabled);

    let result = harness
        .writer
        .import_lutris(vec![
            lutris_entry(&harness, TRAVERSAL),
            lutris_entry(&harness, "good"),
        ])
        .await
        .unwrap()
        .into_output();

    assert_eq!(result.failed_count, 1);
    assert_eq!(result.imported_count, 1);
    assert_eq!(result.results[0].outcome, LutrisImportOutcome::Failed);
    assert_eq!(result.results[0].profile_name, None);
    assert_eq!(result.results[1].profile_name.as_deref(), Some("good"));
    assert_eq!(harness.profiles.list().unwrap(), ["good"]);
    assert_eq!(metadata_names(&harness), ["good"]);
    assert_eq!(
        harness.next_event().await,
        Some(AppEvent::ProfilesChanged {
            reason: ProfileChangeReason::Imported,
            names: vec!["good".to_string()],
        })
    );
}

#[tokio::test]
async fn traversal_save_is_rejected_before_art_import() {
    let mut harness = Harness::new(Metadata::Enabled);
    // A real PNG outside the managed media dir: a save that reached the
    // art import would copy it and rewrite the path.
    let art = harness.root.path().join("cover.png");
    fs::write(
        &art,
        [
            0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0x0D, b'I', b'H', b'D', b'R',
        ],
    )
    .unwrap();
    let mut profile = steam_profile(&home(&harness));
    profile.game.custom_cover_art_path = art.to_string_lossy().into_owned();
    let media_before = media_listing();

    let error = harness
        .writer
        .save(TRAVERSAL.to_string(), profile)
        .await
        .unwrap_err();

    assert_validation(&error);
    assert_eq!(media_listing(), media_before, "no art copied");
    assert!(harness.profiles.list().unwrap().is_empty());
    assert!(metadata_names(&harness).is_empty());
    assert_eq!(harness.next_event().await, None);
}

/// Files under the managed covers directory (empty when it does not exist).
fn media_listing() -> Vec<std::path::PathBuf> {
    let Some(dirs) = crosshook_core::platform::base_dirs() else {
        return Vec::new();
    };
    let covers = dirs
        .data_local_dir()
        .join("crosshook")
        .join("media")
        .join("covers");
    let Ok(entries) = fs::read_dir(covers) else {
        return Vec::new();
    };
    let mut paths: Vec<_> = entries.map(|entry| entry.unwrap().path()).collect();
    paths.sort();
    paths
}

#[tokio::test]
async fn traversal_names_are_rejected_by_every_facade_without_events() {
    let mut harness = Harness::new(Metadata::Enabled);
    let name = || TRAVERSAL.to_string();
    let writer = harness.writer.clone();

    let errors = [
        writer.delete(name()).await.unwrap_err(),
        writer.duplicate(name()).await.unwrap_err(),
        writer
            .save_launch_optimizations(name(), LaunchOptimizationsPayload::default())
            .await
            .unwrap_err(),
        writer
            .save_command_arguments(name(), CommandArgumentsPayload::default(), None)
            .await
            .unwrap_err(),
        writer
            .save_mangohud_config(name(), MangoHudConfig::default())
            .await
            .unwrap_err(),
        writer
            .save_gamescope_config(name(), GamescopeConfig::default())
            .await
            .unwrap_err(),
        writer
            .save_trainer_gamescope_config(name(), GamescopeConfig::default())
            .await
            .unwrap_err(),
        writer
            .save_manual_optimization_preset(name(), "mine".to_string(), Vec::new())
            .await
            .unwrap_err(),
        writer
            .apply_bundled_optimization_preset(name(), "preset".to_string())
            .await
            .unwrap_err(),
        writer
            .accept_protondb_suggestion(AcceptSuggestionRequest::EnvVar {
                profile_name: name(),
                env_key: "DXVK_ASYNC".to_string(),
                env_value: "1".to_string(),
            })
            .await
            .unwrap_err(),
        writer
            .apply_proton_migration(ApplyMigrationRequest {
                profile_name: name(),
                field: ProtonPathField::SteamProtonPath,
                new_path: "/nonexistent/proton".to_string(),
            })
            .await
            .unwrap_err(),
        writer.config_rollback(name(), 1).await.unwrap_err(),
    ];

    for error in &errors {
        assert_validation(error);
    }
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn legacy_import_rejects_invalid_file_stem_without_writing() {
    let mut harness = Harness::new(Metadata::Enabled);
    let legacy_dir = harness.root.path().join("legacy");
    fs::create_dir_all(&legacy_dir).unwrap();
    // `..profile` has the file stem `.`, which is not a valid profile name.
    let path = legacy_dir.join("..profile");
    fs::write(&path, "GamePath=C:\\Games\\old.exe\n").unwrap();

    let error = harness.writer.import_legacy(path).await.unwrap_err();

    assert_validation(&error);
    assert!(harness.profiles.list().unwrap().is_empty());
    assert_eq!(harness.next_event().await, None);
}
