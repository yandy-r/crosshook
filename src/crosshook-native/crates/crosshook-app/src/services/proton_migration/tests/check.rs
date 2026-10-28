//! `check` tests: advisory Steam path (A-5), display sanitization (A-4) and
//! stale-profile suggestions.

use std::fs;
use std::path::Path;

use crosshook_core::profile::migration::ProtonPathField;
use tempfile::TempDir;

use super::{proton_file, Harness};
use crate::presenter::display_path_tilde;

/// Tempdir placed under `$HOME` when that is a writable directory (so the
/// A-4 tilde form is observable), else under the system temp dir. `HOME` is
/// only read, never changed.
fn steam_tempdir() -> TempDir {
    std::env::var("HOME")
        .ok()
        .and_then(|home| tempfile::tempdir_in(home).ok())
        .unwrap_or_else(|| tempfile::tempdir().unwrap())
}

/// `<root>/steamapps` plus an installed `<root>/compatibilitytools.d/<name>/proton`.
fn steam_root_with_tool(root: &Path, tool: &str) -> String {
    fs::create_dir_all(root.join("steamapps")).unwrap();
    proton_file(&root.join("compatibilitytools.d"), tool)
        .to_string_lossy()
        .into_owned()
}

#[tokio::test]
async fn supplied_steam_root_with_steamapps_is_scanned() {
    let harness = Harness::new();
    let steam = steam_tempdir();
    let tool = steam_root_with_tool(steam.path(), "GE-Proton9-21");

    let result = harness
        .service
        .check(Some(steam.path().to_string_lossy().into_owned()))
        .await
        .unwrap();

    assert!(result.installed_proton_versions.iter().any(
        |install| install.name == "GE-Proton9-21" && install.path == display_path_tilde(&tool)
    ));
    assert_eq!(result.profiles_scanned, 0);
}

#[tokio::test]
async fn supplied_path_without_steamapps_is_not_honored() {
    let harness = Harness::new();
    let bogus = tempfile::tempdir().unwrap();
    // Has a tool directory but no `steamapps`: the A-5 filter must reject it.
    proton_file(
        &bogus.path().join("compatibilitytools.d"),
        "Marker-Proton1-1",
    );

    for supplied in [
        Some(bogus.path().to_string_lossy().into_owned()),
        Some("   ".to_string()),
        Some(String::new()),
        None,
    ] {
        let result = harness.service.check(supplied).await.unwrap();
        assert!(
            result
                .installed_proton_versions
                .iter()
                .all(|install| install.name != "Marker-Proton1-1"),
            "fallback path must be used, not the supplied one"
        );
    }
}

#[tokio::test]
async fn supplied_path_is_trimmed_before_the_steamapps_check() {
    let harness = Harness::new();
    let steam = steam_tempdir();
    steam_root_with_tool(steam.path(), "GE-Proton9-21");

    // The A-5 check trims; discovery trims too, so the root is still found.
    let padded = format!(" {} ", steam.path().display());
    let result = harness.service.check(Some(padded)).await.unwrap();

    assert!(result
        .installed_proton_versions
        .iter()
        .any(|install| install.name == "GE-Proton9-21"));
}

#[tokio::test]
async fn stale_profiles_get_suggestions_and_unmatched_with_sanitized_paths() {
    let harness = Harness::new();
    let steam = steam_tempdir();
    let tool = steam_root_with_tool(steam.path(), "GE-Proton9-21");
    let stale = steam
        .path()
        .join("old")
        .join("GE-Proton9-20")
        .join("proton");
    let lost = steam.path().join("old").join("Zzz-Tool1").join("proton");
    harness.seed_proton_run("Alpha", &stale.to_string_lossy());
    harness.seed_proton_run("Beta", &lost.to_string_lossy());

    let result = harness
        .service
        .check(Some(steam.path().to_string_lossy().into_owned()))
        .await
        .unwrap();

    assert_eq!(result.profiles_scanned, 2);
    assert_eq!(result.affected_count, 2);
    assert_eq!(result.suggestions.len(), 1);
    let suggestion = &result.suggestions[0];
    assert_eq!(suggestion.profile_name, "Alpha");
    assert_eq!(suggestion.field, ProtonPathField::RuntimeProtonPath);
    assert_eq!(suggestion.new_proton_name, "GE-Proton9-21");
    assert_eq!(
        suggestion.old_path,
        display_path_tilde(&stale.to_string_lossy())
    );
    assert_eq!(suggestion.new_path, display_path_tilde(&tool));
    assert_eq!(result.unmatched.len(), 1);
    assert_eq!(result.unmatched[0].profile_name, "Beta");
    assert_eq!(
        result.unmatched[0].stale_path,
        display_path_tilde(&lost.to_string_lossy())
    );

    if let Ok(home) = std::env::var("HOME") {
        if steam.path().starts_with(&home) {
            // The real home prefix never leaves the service.
            assert!(suggestion.old_path.starts_with("~/"));
            assert!(suggestion.new_path.starts_with("~/"));
            assert!(result.unmatched[0].stale_path.starts_with("~/"));
            assert!(result
                .installed_proton_versions
                .iter()
                .filter(|install| install.name == "GE-Proton9-21")
                .all(|install| install.path.starts_with("~/")));
        }
    }
}

#[tokio::test]
async fn existing_proton_path_is_not_flagged() {
    let harness = Harness::new();
    let steam = steam_tempdir();
    let tool = steam_root_with_tool(steam.path(), "GE-Proton9-21");
    harness.seed_proton_run("Alpha", &tool);

    let result = harness
        .service
        .check(Some(steam.path().to_string_lossy().into_owned()))
        .await
        .unwrap();

    assert_eq!(result.profiles_scanned, 1);
    assert_eq!(result.affected_count, 0);
    assert!(result.suggestions.is_empty());
    assert!(result.unmatched.is_empty());
}
