//! Bundle-level export tests.

use std::fs;
use std::path::Path;

use tempfile::tempdir;

use super::*;
use crate::profile::ProfileStore;
use crate::settings::SettingsStore;

#[test]
fn redact_home_paths_replaces_home_directory() {
    let home = env::var("HOME").unwrap();
    let text = format!("path = \"{home}/games/elden-ring.exe\"");
    let redacted = redact_home_paths(&text);
    assert!(redacted.contains("~/games/elden-ring.exe"));
    assert!(!redacted.contains(&home));
}

#[test]
fn redact_home_paths_preserves_text_without_home() {
    let text = "path = \"/opt/games/elden-ring.exe\"";
    let redacted = redact_home_paths(text);
    assert_eq!(redacted, text);
}

#[test]
fn diagnostic_bundle_error_display_formats_correctly() {
    let err = DiagnosticBundleError::Archive("test error".to_string());
    assert_eq!(err.to_string(), "archive error: test error");

    let err = DiagnosticBundleError::Io {
        action: "create",
        path: PathBuf::from("/tmp/test"),
        source: io::Error::new(io::ErrorKind::PermissionDenied, "denied"),
    };
    assert!(err.to_string().contains("create"));
    assert!(err.to_string().contains("/tmp/test"));
}

#[test]
fn export_diagnostic_bundle_produces_valid_archive() {
    let temp = tempdir().unwrap();
    let profiles_dir = temp.path().join("profiles");
    let settings_dir = temp.path().join("settings");
    let output_dir = temp.path().join("output");
    fs::create_dir_all(&profiles_dir).unwrap();
    fs::create_dir_all(&settings_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    fs::write(
        profiles_dir.join("test-game.toml"),
        "[game]\nname = \"Test Game\"\nexecutable_path = \"/games/test.exe\"\n",
    )
    .unwrap();

    let profile_store = ProfileStore::with_base_path(profiles_dir);
    let settings_store = SettingsStore::with_base_path(settings_dir);
    let options = DiagnosticBundleOptions {
        redact_paths: false,
        output_dir: Some(output_dir),
        metadata_status: Some(MetadataStatus::Ok),
        metadata_backup_files: Vec::new(),
        extra_files: vec![DiagnosticBundleFile {
            name: "ui.log".to_string(),
            contents: b"hello".to_vec(),
        }],
        recent_files_path: None,
    };

    let result = export_diagnostic_bundle(&profile_store, &settings_store, &options).unwrap();

    // Verify the archive file exists.
    assert!(Path::new(&result.archive_path).exists());
    assert!(result.archive_path.ends_with(".tar.gz"));
    assert_eq!(result.summary.profile_count, 1);
    assert!(result
        .summary
        .crosshook_version
        .contains(env!("CARGO_PKG_VERSION")));

    // Decompress and verify archive contents.
    let archive_file = File::open(&result.archive_path).unwrap();
    let decoder = flate2::read::GzDecoder::new(archive_file);
    let mut archive = tar::Archive::new(decoder);
    let entry_paths: Vec<String> = archive
        .entries()
        .unwrap()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            Some(entry.path().ok()?.to_string_lossy().into_owned())
        })
        .collect();

    // Check that essential files are present.
    assert!(entry_paths.iter().any(|p| p.ends_with("system-info.txt")));
    assert!(entry_paths
        .iter()
        .any(|p| p.ends_with("crosshook-info.txt")));
    assert!(entry_paths
        .iter()
        .any(|p| p.ends_with("health-summary.json")));
    assert!(entry_paths
        .iter()
        .any(|p| p.ends_with("metadata-status.json")));
    assert!(entry_paths
        .iter()
        .any(|p| p.ends_with("steam-diagnostics.txt")));
    assert!(entry_paths
        .iter()
        .any(|p| p.ends_with("proton-installs.json")));
    assert!(entry_paths.iter().any(|p| p.ends_with("settings.toml")));
    assert!(entry_paths
        .iter()
        .any(|p| p.contains("profiles/test-game.toml")));
    assert!(entry_paths.iter().any(|p| p.ends_with("/app/ui.log")));
}

const TEST_KEY: &str = "sgdb-test-key-0123456789abcdef";

/// Exports a bundle with `settings` as settings.toml and returns `(path, text)` entries.
pub(super) fn export_entries(
    settings: &str,
    redact_paths: bool,
    extra_files: Vec<DiagnosticBundleFile>,
) -> Vec<(String, String)> {
    let temp = tempdir().unwrap();
    let settings_dir = temp.path().join("settings");
    fs::create_dir_all(&settings_dir).unwrap();
    let settings_store = SettingsStore::with_base_path(settings_dir);
    fs::write(settings_store.settings_path(), settings).unwrap();
    let profile_store = ProfileStore::with_base_path(temp.path().join("profiles"));
    let options = DiagnosticBundleOptions {
        redact_paths,
        output_dir: Some(temp.path().join("output")),
        metadata_status: Some(MetadataStatus::Ok),
        extra_files,
        ..Default::default()
    };

    let result = export_diagnostic_bundle(&profile_store, &settings_store, &options).unwrap();
    read_entries(&result.archive_path)
}

/// Returns `(path, text)` for every entry of the bundle at `archive_path`.
pub(super) fn read_entries(archive_path: &str) -> Vec<(String, String)> {
    let decoder = flate2::read::GzDecoder::new(File::open(archive_path).unwrap());
    let mut archive = tar::Archive::new(decoder);
    archive
        .entries()
        .unwrap()
        .map(|entry| {
            let mut entry = entry.unwrap();
            let path = entry.path().unwrap().to_string_lossy().into_owned();
            let mut contents = Vec::new();
            io::Read::read_to_end(&mut entry, &mut contents).unwrap();
            (path, String::from_utf8_lossy(&contents).into_owned())
        })
        .collect()
}

/// Text of the single entry whose path ends with `suffix`.
pub(super) fn entry<'a>(entries: &'a [(String, String)], suffix: &str) -> &'a str {
    let (_, text) = entries
        .iter()
        .find(|(path, _)| path.ends_with(suffix))
        .unwrap_or_else(|| panic!("{suffix} entry"));
    text
}

fn assert_no_key(entries: &[(String, String)]) {
    for (path, text) in entries {
        assert!(
            !text.contains(TEST_KEY),
            "{path} leaks the SteamGridDB API key"
        );
    }
}

#[test]
fn export_diagnostic_bundle_redacts_steamgriddb_api_key_in_settings() {
    let settings = format!("last_used_profile = \"daily\"\nsteamgriddb_api_key = \"{TEST_KEY}\"\n");
    for redact_paths in [false, true] {
        let entries = export_entries(&settings, redact_paths, Vec::new());
        assert_no_key(&entries);
        let (_, text) = entries
            .iter()
            .find(|(path, _)| path.ends_with("/settings.toml"))
            .expect("settings.toml entry");
        assert!(text.contains("steamgriddb_api_key = \"<redacted>\""));
        assert!(text.contains("last_used_profile = \"daily\""));
    }
}

#[test]
fn export_diagnostic_bundle_scrubs_key_from_malformed_settings_and_other_entries() {
    // Unquoted value: the TOML parse error would otherwise quote this line.
    let settings = format!("steamgriddb_api_key = {TEST_KEY}\n");
    let extra_files = vec![DiagnosticBundleFile {
        name: "ui.log".to_string(),
        contents: format!("request failed for key {TEST_KEY}").into_bytes(),
    }];

    let entries = export_entries(&settings, false, extra_files);

    assert_no_key(&entries);
    let (_, ui_log) = entries
        .iter()
        .find(|(path, _)| path.ends_with("/app/ui.log"))
        .expect("ui.log entry");
    assert_eq!(ui_log, "request failed for key <redacted>");
}

#[test]
fn bundle_file_name_validation_rejects_unsafe_names() {
    for bad in ["", ".", "..", "a/b", "a\\b", "../x", "nul\0"] {
        assert!(!is_valid_bundle_file_name(bad), "{bad:?} must be rejected");
    }
    assert!(is_valid_bundle_file_name("ui.log"));
    assert!(is_valid_bundle_file_name("..hidden"));
}

#[test]
fn export_rejects_invalid_extra_file_name_without_writing() {
    let temp = tempdir().unwrap();
    let output_dir = temp.path().join("output");
    let options = DiagnosticBundleOptions {
        output_dir: Some(output_dir.clone()),
        extra_files: vec![DiagnosticBundleFile {
            name: "../escape".to_string(),
            contents: Vec::new(),
        }],
        ..Default::default()
    };
    let profile_store = ProfileStore::with_base_path(temp.path().join("profiles"));
    let settings_store = SettingsStore::with_base_path(temp.path().join("settings"));
    assert!(export_diagnostic_bundle(&profile_store, &settings_store, &options).is_err());
    assert!(!output_dir.exists());
}
