//! Preview vs. import authorization over real tempdir tap workspaces.

use std::fs;

use super::helpers::{write_manifest, Harness};
use crate::{AppErrorKind, AppEvent, ProfileChangeReason};

const URL: &str = "https://example.invalid/tap.git";

fn path_validation() -> AppErrorKind {
    AppErrorKind::Validation { field: "path" }
}

fn imported(names: &[&str]) -> Option<AppEvent> {
    Some(AppEvent::ProfilesChanged {
        reason: ProfileChangeReason::Imported,
        names: names.iter().map(ToString::to_string).collect(),
    })
}

#[tokio::test]
async fn prepare_import_accepts_json_outside_any_tap() {
    let mut harness = Harness::new();
    harness.subscribe_workspace(URL);
    let outside = harness.root.path().join("downloads/shared.json");
    write_manifest(&outside, "Outside Game");

    let preview = harness
        .service
        .prepare_import(outside.clone())
        .await
        .unwrap();

    assert_eq!(preview.profile_name, "outside-game");
    assert_eq!(preview.source_path, outside.canonicalize().unwrap());
    assert_eq!(preview.manifest.metadata.game_name, "Outside Game");
    assert!(
        harness.profiles.list().unwrap().is_empty(),
        "preview must not write"
    );
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn prepare_import_accepts_json_without_any_subscribed_tap() {
    let harness = Harness::new();
    let file = harness.root.path().join("loose.JSON");
    write_manifest(&file, "Loose Game");

    let preview = harness.service.prepare_import(file).await.unwrap();

    assert_eq!(preview.profile_name, "loose-game");
}

#[tokio::test]
async fn prepare_import_rejects_non_regular_and_non_json_paths() {
    let harness = Harness::new();
    let directory = harness.root.path().join("dir.json");
    fs::create_dir_all(&directory).unwrap();
    let text = harness.root.path().join("manifest.txt");
    write_manifest(&text, "Text Game");
    let missing = harness.root.path().join("missing.json");

    let error = harness.service.prepare_import(directory).await.unwrap_err();
    assert_eq!(error.kind(), path_validation());
    let error = harness.service.prepare_import(text).await.unwrap_err();
    assert_eq!(error.kind(), path_validation());
    let error = harness.service.prepare_import(missing).await.unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::NotFound);
}

#[tokio::test]
async fn import_profile_rejects_json_outside_every_tap() {
    let mut harness = Harness::new();
    harness.subscribe_workspace(URL);
    let outside = harness.root.path().join("downloads/shared.json");
    write_manifest(&outside, "Outside Game");

    let error = harness.service.import_profile(outside).await.unwrap_err();

    assert_eq!(error.kind(), path_validation());
    assert_eq!(
        error.message(),
        "community profile path is not inside a known tap workspace"
    );
    assert!(harness.profiles.list().unwrap().is_empty());
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn import_profile_rejects_json_when_no_tap_is_subscribed() {
    let mut harness = Harness::new();
    let file = harness.root.path().join("loose.json");
    write_manifest(&file, "Loose Game");

    let error = harness.service.import_profile(file).await.unwrap_err();

    assert_eq!(error.kind(), path_validation());
    assert!(harness.profiles.list().unwrap().is_empty());
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn import_profile_rejects_traversal_even_when_it_ends_inside_a_tap() {
    let mut harness = Harness::new();
    let workspace = harness.subscribe_workspace(URL);
    write_manifest(
        &workspace.join("profiles/game/community-profile.json"),
        "Nested Game",
    );
    let outside = harness.root.path().join("outside/escape.json");
    write_manifest(&outside, "Escaped Game");

    let ends_outside = workspace.join("..").join("..").join("outside/escape.json");
    let ends_inside = workspace.join("profiles/../profiles/game/community-profile.json");

    for candidate in [ends_outside, ends_inside] {
        let error = harness.service.import_profile(candidate).await.unwrap_err();
        assert_eq!(error.kind(), path_validation());
    }
    assert!(harness.profiles.list().unwrap().is_empty());
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn import_profile_imports_nested_tap_json_through_the_writer() {
    let mut harness = Harness::new();
    let workspace = harness.subscribe_workspace(URL);
    let manifest_path = workspace.join("profiles/deep/game/community-profile.json");
    write_manifest(&manifest_path, "Nested Game");

    let result = harness
        .service
        .import_profile(manifest_path.clone())
        .await
        .unwrap();

    assert_eq!(result.profile_name, "nested-game");
    assert_eq!(result.source_path, manifest_path.canonicalize().unwrap());
    assert_eq!(result.manifest.metadata.game_name, "Nested Game");
    assert_eq!(harness.profiles.list().unwrap(), ["nested-game"]);
    assert!(harness.profiles.load("nested-game").is_ok());
    assert_eq!(harness.next_event().await, imported(&["nested-game"]));
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn import_profile_rejects_non_regular_and_non_json_paths_inside_a_tap() {
    let mut harness = Harness::new();
    let workspace = harness.subscribe_workspace(URL);
    let directory = workspace.join("profiles/dir.json");
    fs::create_dir_all(&directory).unwrap();
    let text = workspace.join("profiles/manifest.txt");
    write_manifest(&text, "Text Game");
    let missing = workspace.join("profiles/missing.json");

    for candidate in [directory, text] {
        let error = harness.service.import_profile(candidate).await.unwrap_err();
        assert_eq!(error.kind(), path_validation());
    }
    let error = harness.service.import_profile(missing).await.unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::NotFound);
    assert!(harness.profiles.list().unwrap().is_empty());
    assert_eq!(harness.next_event().await, None);
}

#[cfg(unix)]
#[tokio::test]
async fn import_profile_rejects_a_tap_symlink_that_leads_outside() {
    let mut harness = Harness::new();
    let workspace = harness.subscribe_workspace(URL);
    let outside = harness.root.path().join("outside/secret.json");
    write_manifest(&outside, "Secret Game");
    let link = workspace.join("profiles/link.json");
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&outside, &link).unwrap();

    let error = harness.service.import_profile(link).await.unwrap_err();

    assert_eq!(error.kind(), path_validation());
    assert!(harness.profiles.list().unwrap().is_empty());
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn import_accepts_a_relative_file_in_the_second_subscribed_workspace() {
    let mut harness = Harness::new();
    let first = super::helpers::tap(URL);
    let second = super::helpers::tap("https://example.invalid/second.git");
    harness.set_taps(vec![first.clone(), second.clone()]);
    fs::create_dir_all(harness.workspace_path(&first)).unwrap();
    let candidate = std::path::PathBuf::from("profiles/nested.json");
    write_manifest(
        &harness.workspace_path(&second).join(&candidate),
        "Second Game",
    );
    let result = harness.service.import_profile(candidate).await.unwrap();
    assert_eq!(result.profile_name, "second-game");
    assert_eq!(harness.next_event().await, imported(&["second-game"]));
}

#[tokio::test]
async fn export_uses_the_selected_destination_without_a_tap_restriction() {
    let harness = Harness::new();
    let workspace = harness.subscribe_workspace(URL);
    let source = workspace.join("profile.json");
    write_manifest(&source, "Export Game");
    harness.service.import_profile(source).await.unwrap();
    let destination = harness.root.path().join("chosen/export.json");
    let result = harness
        .service
        .export_profile(" export-game ".into(), destination.to_str().unwrap().into())
        .await
        .unwrap();
    assert_eq!(result.profile_name, "export-game");
    assert_eq!(result.output_path, destination);
    assert!(result.output_path.is_file());
}
