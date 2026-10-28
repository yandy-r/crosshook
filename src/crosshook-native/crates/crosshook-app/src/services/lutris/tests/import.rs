//! Preview and import over a real Lutris YAML library and the shared writer.

use crosshook_core::profile::LutrisImportOutcome;

use super::helpers::{write_game_config, Harness};
use crate::{AppEvent, ProfileChangeReason};

fn imported(names: &[&str]) -> Option<AppEvent> {
    Some(AppEvent::ProfilesChanged {
        reason: ProfileChangeReason::Imported,
        names: names.iter().map(ToString::to_string).collect(),
    })
}

#[tokio::test]
async fn prepare_import_previews_library_without_writing() {
    let mut harness = Harness::new();
    let library = harness.lutris_library();

    let preview = harness
        .service
        .prepare_import(Some(library.clone()))
        .await
        .unwrap();

    assert_eq!(preview.lutris_root, Some(library));
    assert_eq!(preview.entries.len(), 1);
    let entry = &preview.entries[0];
    assert_eq!(entry.suggested_name, "sample-game");
    assert_eq!(entry.game_name, "Sample Game");
    assert!(entry.importable);
    assert!(harness.profiles.list().unwrap().is_empty());
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn prepare_import_missing_root_is_an_empty_diagnostic_preview() {
    let harness = Harness::new();
    let missing = harness.root.path().join("no-lutris-here");

    let preview = harness.service.prepare_import(Some(missing)).await.unwrap();

    assert!(preview.entries.is_empty());
    assert_eq!(preview.lutris_root, None);
    assert!(!preview.diagnostics.is_empty());
}

#[tokio::test]
async fn import_profiles_writes_names_and_publishes_one_event() {
    let mut harness = Harness::new();
    let library = harness.lutris_library();
    write_game_config(&library, "other-game", "Other Game");
    let preview = harness.service.prepare_import(Some(library)).await.unwrap();
    assert_eq!(preview.entries.len(), 2);

    let result = harness
        .service
        .import_profiles(preview.entries)
        .await
        .unwrap();

    assert_eq!(result.imported_count, 2);
    assert_eq!(result.skipped_count, 0);
    assert_eq!(result.failed_count, 0);
    let mut names: Vec<String> = result
        .results
        .iter()
        .map(|entry| entry.profile_name.clone().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["other-game", "sample-game"]);
    assert_eq!(harness.profiles.list().unwrap().len(), 2);

    let Some(AppEvent::ProfilesChanged {
        reason: ProfileChangeReason::Imported,
        names: mut event_names,
    }) = harness.next_event().await
    else {
        panic!("expected one Imported event");
    };
    event_names.sort();
    assert_eq!(event_names, names);
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn importing_the_same_entry_again_gets_a_unique_name() {
    let mut harness = Harness::new();
    let library = harness.lutris_library();
    let entries = harness
        .service
        .prepare_import(Some(library.clone()))
        .await
        .unwrap()
        .entries;
    harness
        .service
        .import_profiles(entries.clone())
        .await
        .unwrap();
    assert_eq!(harness.next_event().await, imported(&["sample-game"]));

    let result = harness.service.import_profiles(entries).await.unwrap();

    assert_eq!(result.imported_count, 1);
    assert_eq!(
        result.results[0].profile_name.as_deref(),
        Some("sample-game-2")
    );
    assert_eq!(harness.next_event().await, imported(&["sample-game-2"]));

    let preview = harness.service.prepare_import(Some(library)).await.unwrap();
    assert_eq!(preview.entries[0].suggested_name, "sample-game-3");
}

#[tokio::test]
async fn non_importable_entries_are_skipped_without_an_event() {
    let mut harness = Harness::new();
    let library = harness.lutris_library();
    let mut entries = harness
        .service
        .prepare_import(Some(library))
        .await
        .unwrap()
        .entries;
    entries[0].importable = false;

    let result = harness.service.import_profiles(entries).await.unwrap();

    assert_eq!(result.imported_count, 0);
    assert_eq!(result.skipped_count, 1);
    assert_eq!(result.results[0].outcome, LutrisImportOutcome::Skipped);
    assert!(harness.profiles.list().unwrap().is_empty());
    assert_eq!(harness.next_event().await, None);
}
