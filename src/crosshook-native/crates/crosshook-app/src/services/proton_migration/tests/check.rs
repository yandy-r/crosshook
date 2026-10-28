//! `check` tests: advisory Steam path (A-5), display sanitization (A-4) and
//! stale-profile suggestions.

use std::fs;
use std::path::Path;

use crosshook_core::profile::migration::{MigrationScanResult, ProtonPathField};

use super::{proton_file, Harness};

/// Deterministic stand-in for the home presenter: rewrites a `home` prefix to
/// `~` without reading or touching the process environment.
fn tilde_under(home: &Path) -> impl Fn(&str) -> String + Send + 'static {
    let home = home.to_string_lossy().into_owned();
    move |path| match path.strip_prefix(&home) {
        Some(rest) => format!("~{rest}"),
        None => path.to_string(),
    }
}

/// `<root>/steamapps` plus an installed `<root>/compatibilitytools.d/<name>/proton`.
fn steam_root_with_tool(root: &Path, tool: &str) -> String {
    fs::create_dir_all(root.join("steamapps")).unwrap();
    proton_file(&root.join("compatibilitytools.d"), tool)
        .to_string_lossy()
        .into_owned()
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn has_tool(result: &MigrationScanResult, name: &str) -> bool {
    result
        .installed_proton_versions
        .iter()
        .any(|install| install.name == name)
}

/// Runs `check_with` using `fallback` as the injected default Steam root.
async fn check(
    harness: &Harness,
    supplied: Option<String>,
    fallback: &Path,
    home: &Path,
) -> MigrationScanResult {
    let fallback = path_string(fallback);
    harness
        .service
        .check_with(supplied, async move { fallback }, tilde_under(home))
        .await
        .unwrap()
}

#[tokio::test]
async fn supplied_steam_root_with_steamapps_is_scanned() {
    let harness = Harness::new();
    let steam = tempfile::tempdir().unwrap();
    let fallback = tempfile::tempdir().unwrap();
    let tool = steam_root_with_tool(steam.path(), "Supplied-Proton1-1");
    steam_root_with_tool(fallback.path(), "Fallback-Proton1-1");

    let result = check(
        &harness,
        Some(path_string(steam.path())),
        fallback.path(),
        steam.path(),
    )
    .await;

    assert!(result.installed_proton_versions.iter().any(|install| {
        install.name == "Supplied-Proton1-1" && install.path == tilde_under(steam.path())(&tool)
    }));
    assert!(!has_tool(&result, "Fallback-Proton1-1"));
    assert_eq!(result.profiles_scanned, 0);
}

#[tokio::test]
async fn supplied_path_without_steamapps_falls_back_to_default() {
    let harness = Harness::new();
    let bogus = tempfile::tempdir().unwrap();
    let fallback = tempfile::tempdir().unwrap();
    // Has a tool directory but no `steamapps`: the A-5 filter must reject it.
    proton_file(
        &bogus.path().join("compatibilitytools.d"),
        "Bogus-Proton1-1",
    );
    steam_root_with_tool(fallback.path(), "Fallback-Proton1-1");

    for supplied in [
        Some(path_string(bogus.path())),
        Some("   ".to_string()),
        Some(String::new()),
        None,
    ] {
        let result = check(&harness, supplied, fallback.path(), fallback.path()).await;
        assert!(
            has_tool(&result, "Fallback-Proton1-1"),
            "the injected fallback must be used"
        );
        assert!(!has_tool(&result, "Bogus-Proton1-1"));
    }
}

#[tokio::test]
async fn supplied_path_is_trimmed_before_the_steamapps_check() {
    let harness = Harness::new();
    let steam = tempfile::tempdir().unwrap();
    let fallback = tempfile::tempdir().unwrap();
    steam_root_with_tool(steam.path(), "Supplied-Proton1-1");
    steam_root_with_tool(fallback.path(), "Fallback-Proton1-1");

    // Without trimming, `" <root> /steamapps"` is not a directory and the
    // fallback would be used instead of the supplied root.
    let padded = format!(" {} ", steam.path().display());
    let result = check(&harness, Some(padded), fallback.path(), steam.path()).await;

    assert!(has_tool(&result, "Supplied-Proton1-1"));
    assert!(!has_tool(&result, "Fallback-Proton1-1"));
}

#[tokio::test]
async fn stale_profiles_get_suggestions_and_unmatched_with_sanitized_paths() {
    let harness = Harness::new();
    let steam = tempfile::tempdir().unwrap();
    let fallback = tempfile::tempdir().unwrap();
    let tool = steam_root_with_tool(steam.path(), "GE-Proton9-21");
    let stale = steam
        .path()
        .join("old")
        .join("GE-Proton9-20")
        .join("proton");
    let lost = steam.path().join("old").join("Zzz-Tool1").join("proton");
    harness.seed_proton_run("Alpha", &stale.to_string_lossy());
    harness.seed_proton_run("Beta", &lost.to_string_lossy());

    // The injected home is the steam tempdir, so every path is under it.
    let result = check(
        &harness,
        Some(path_string(steam.path())),
        fallback.path(),
        steam.path(),
    )
    .await;

    assert_eq!(result.profiles_scanned, 2);
    assert_eq!(result.affected_count, 2);
    assert_eq!(result.suggestions.len(), 1);
    let suggestion = &result.suggestions[0];
    assert_eq!(suggestion.profile_name, "Alpha");
    assert_eq!(suggestion.field, ProtonPathField::RuntimeProtonPath);
    assert_eq!(suggestion.new_proton_name, "GE-Proton9-21");
    assert_eq!(suggestion.old_path, "~/old/GE-Proton9-20/proton");
    assert_eq!(
        suggestion.new_path,
        "~/compatibilitytools.d/GE-Proton9-21/proton"
    );
    assert_eq!(result.unmatched.len(), 1);
    assert_eq!(result.unmatched[0].profile_name, "Beta");
    assert_eq!(result.unmatched[0].stale_path, "~/old/Zzz-Tool1/proton");

    let installed: Vec<_> = result
        .installed_proton_versions
        .iter()
        .filter(|install| install.name == "GE-Proton9-21")
        .collect();
    assert_eq!(installed.len(), 1);
    assert_eq!(
        installed[0].path,
        "~/compatibilitytools.d/GE-Proton9-21/proton"
    );
    // The home prefix never leaves the service.
    let home = path_string(steam.path());
    assert!(!tool.is_empty() && tool.starts_with(&home));
    assert!(!suggestion.new_path.contains(&home));
    assert!(!suggestion.old_path.contains(&home));
}

#[tokio::test]
async fn existing_proton_path_is_not_flagged() {
    let harness = Harness::new();
    let steam = tempfile::tempdir().unwrap();
    let fallback = tempfile::tempdir().unwrap();
    let tool = steam_root_with_tool(steam.path(), "GE-Proton9-21");
    harness.seed_proton_run("Alpha", &tool);

    let result = check(
        &harness,
        Some(path_string(steam.path())),
        fallback.path(),
        steam.path(),
    )
    .await;

    assert_eq!(result.profiles_scanned, 1);
    assert_eq!(result.affected_count, 0);
    assert!(result.suggestions.is_empty());
    assert!(result.unmatched.is_empty());
}
