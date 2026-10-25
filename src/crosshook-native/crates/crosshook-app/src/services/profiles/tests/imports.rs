//! Import facade tests: Lutris and community imports write under the writer
//! lock and publish one `Imported` event with the concrete names; legacy
//! import records an `Import` revision; duplicate and rename capture
//! revisions and lineage.

use std::fs;
use std::path::PathBuf;

use crosshook_core::profile::{
    CommunityProfileManifest, CommunityProfileMetadata, LutrisImportEntry, LutrisImportOutcome,
};

use super::helpers::{revision_sources, steam_profile, sync_row, Harness, Metadata};
use crate::{AppErrorKind, AppEvent, ProfileChangeReason};

fn imported(names: &[&str]) -> Option<AppEvent> {
    Some(AppEvent::ProfilesChanged {
        reason: ProfileChangeReason::Imported,
        names: names.iter().map(ToString::to_string).collect(),
    })
}

fn home(harness: &Harness) -> String {
    harness.root.path().to_string_lossy().into_owned()
}

fn lutris_entry(harness: &Harness, name: &str, importable: bool) -> LutrisImportEntry {
    LutrisImportEntry {
        source_path: PathBuf::from(format!("/lutris/{name}.yml")),
        suggested_name: name.to_string(),
        game_name: name.to_string(),
        runner: "wine".to_string(),
        mapped: steam_profile(&home(harness)),
        warnings: Vec::new(),
        importable,
    }
}

/// Health snapshot cached for `name`, so invalidation can be observed.
fn cache_health(harness: &Harness, name: &str) -> String {
    let profile_id = harness.metadata.lookup_profile_id(name).unwrap().unwrap();
    harness
        .metadata
        .upsert_health_snapshot(&profile_id, "healthy", 0, "2026-01-01T00:00:00Z")
        .unwrap();
    profile_id
}

/// Writes a community manifest (as the exporter writes it, with an explicit
/// `schema_version`) and returns its path.
fn community_manifest(harness: &Harness, game_version: &str, trainer_version: &str) -> PathBuf {
    let metadata = CommunityProfileMetadata {
        game_name: "Community Game".to_string(),
        game_version: game_version.to_string(),
        trainer_version: trainer_version.to_string(),
        ..CommunityProfileMetadata::default()
    };
    let manifest = CommunityProfileManifest::new(metadata, steam_profile(&home(harness)));
    let mut value = serde_json::to_value(&manifest).unwrap();
    value["schema_version"] = manifest.schema_version.into();
    let path = harness.root.path().join("tap").join("community.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, serde_json::to_string_pretty(&value).unwrap()).unwrap();
    path
}

#[tokio::test]
async fn lutris_import_writes_records_and_publishes_imported_names() {
    let mut harness = Harness::new(Metadata::Enabled);
    harness
        .writer
        .save("taken".to_string(), steam_profile(&home(&harness)))
        .await
        .unwrap();
    assert!(harness.next_event().await.is_some());
    let taken_id = cache_health(&harness, "taken");
    let entries = vec![
        lutris_entry(&harness, "taken", true),
        lutris_entry(&harness, "skipped", false),
    ];

    let commit = harness.writer.import_lutris(entries).await.unwrap();

    let result = commit.output;
    assert_eq!(result.imported_count, 1);
    assert_eq!(result.skipped_count, 1);
    assert_eq!(result.results[0].outcome, LutrisImportOutcome::Imported);
    let name = result.results[0].profile_name.clone().unwrap();
    assert_eq!(name, "taken-2", "unique name derived under the lock");
    assert!(harness.profiles.profile_exists(&name));
    assert!(commit.warnings.is_empty(), "{:?}", commit.warnings);
    assert_eq!(revision_sources(&harness.metadata, &name), ["import"]);
    assert_eq!(sync_row(&harness.metadata, &name).0, "lutris_import");
    assert!(
        harness
            .metadata
            .lookup_health_snapshot(&taken_id)
            .unwrap()
            .is_some(),
        "other profiles keep their cached health"
    );
    assert_eq!(harness.next_event().await, imported(&[&name]));
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn lutris_import_with_nothing_written_is_silent() {
    let mut harness = Harness::new(Metadata::Enabled);
    let entries = vec![lutris_entry(&harness, "skipped", false)];

    let result = harness
        .writer
        .import_lutris(entries)
        .await
        .unwrap()
        .into_output();

    assert_eq!(result.imported_count, 0);
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn community_import_records_seeds_version_and_publishes_imported() {
    let mut harness = Harness::new(Metadata::Enabled);
    let path = community_manifest(&harness, "1.2.3", "v4");

    let commit = harness.writer.import_community(path).await.unwrap();

    let result = commit.output;
    assert_eq!(result.profile_name, "community-game");
    assert!(harness.profiles.profile_exists(&result.profile_name));
    assert!(commit.warnings.is_empty(), "{:?}", commit.warnings);
    assert_eq!(
        revision_sources(&harness.metadata, &result.profile_name),
        ["import"]
    );
    assert_eq!(
        sync_row(&harness.metadata, &result.profile_name).0,
        "import"
    );
    let profile_id = harness
        .metadata
        .lookup_profile_id(&result.profile_name)
        .unwrap()
        .unwrap();
    let snapshot = harness
        .metadata
        .lookup_latest_version_snapshot(&profile_id)
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.human_game_ver.as_deref(), Some("1.2.3"));
    assert_eq!(snapshot.trainer_version.as_deref(), Some("v4"));
    assert_eq!(snapshot.status, "untracked");
    assert_eq!(harness.next_event().await, imported(&["community-game"]));
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn community_import_without_versions_skips_the_version_seed() {
    let harness = Harness::new(Metadata::Enabled);
    let path = community_manifest(&harness, "", "");

    let result = harness
        .writer
        .import_community(path)
        .await
        .unwrap()
        .into_output();

    let profile_id = harness
        .metadata
        .lookup_profile_id(&result.profile_name)
        .unwrap()
        .unwrap();
    assert!(harness
        .metadata
        .lookup_latest_version_snapshot(&profile_id)
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn invalid_community_manifest_is_rejected_without_event() {
    let mut harness = Harness::new(Metadata::Enabled);
    let path = harness.root.path().join("broken.json");
    fs::write(&path, "{\"schema_version\": 1}").unwrap();

    let error = harness.writer.import_community(path).await.unwrap_err();

    assert_eq!(
        error.kind(),
        AppErrorKind::Validation {
            field: "community_profile"
        }
    );
    assert!(harness.profiles.list().unwrap().is_empty());
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn legacy_import_records_import_revision_and_publishes_trimmed_name() {
    let mut harness = Harness::new(Metadata::Enabled);
    let legacy_dir = harness.root.path().join("legacy");
    fs::create_dir_all(&legacy_dir).unwrap();
    let path = legacy_dir.join("Old Game.profile");
    fs::write(&path, "GamePath=C:\\Games\\old.exe\nLaunchMethod=native\n").unwrap();

    harness.writer.import_legacy(path).await.unwrap();

    assert!(harness.profiles.profile_exists("Old Game"));
    assert_eq!(revision_sources(&harness.metadata, "Old Game"), ["import"]);
    assert_eq!(sync_row(&harness.metadata, "Old Game").0, "import");
    assert_eq!(harness.next_event().await, imported(&["Old Game"]));
}

#[tokio::test]
async fn duplicate_records_copy_revision_lineage_and_event() {
    let mut harness = Harness::new(Metadata::Enabled);
    harness
        .writer
        .save("Alpha".to_string(), steam_profile(&home(&harness)))
        .await
        .unwrap();
    assert!(harness.next_event().await.is_some());
    let source_id = harness
        .metadata
        .lookup_profile_id("Alpha")
        .unwrap()
        .unwrap();

    let copy = harness
        .writer
        .duplicate(" Alpha ".to_string())
        .await
        .unwrap()
        .into_output();

    assert_eq!(copy.name, "Alpha (Copy)");
    assert_eq!(
        revision_sources(&harness.metadata, &copy.name),
        ["manual_save"]
    );
    assert_eq!(
        sync_row(&harness.metadata, &copy.name),
        ("app_duplicate".to_string(), Some(source_id))
    );
    assert_eq!(
        harness.next_event().await,
        Some(AppEvent::ProfilesChanged {
            reason: ProfileChangeReason::Duplicated,
            names: vec![copy.name],
        })
    );
}

#[tokio::test]
async fn rename_captures_a_revision_under_the_new_name() {
    let harness = Harness::new(Metadata::Enabled);
    harness
        .writer
        .save("Alpha".to_string(), steam_profile(&home(&harness)))
        .await
        .unwrap();

    harness
        .writer
        .rename("Alpha".to_string(), "Beta".to_string())
        .await
        .unwrap();

    // The display-name rewrite is a new snapshot on top of the save.
    assert_eq!(
        revision_sources(&harness.metadata, "Beta"),
        ["manual_save", "manual_save"]
    );
}
