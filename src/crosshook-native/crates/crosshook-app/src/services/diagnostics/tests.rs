//! Tests for diagnostic export: file list, attachment validation, redaction.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::ProfileStore;
use crosshook_core::settings::SettingsStore;
use tempfile::tempdir;

use super::{DiagnosticAttachment, DiagnosticsRequest, DiagnosticsService};
use crate::AppErrorKind;

fn service(root: &Path) -> DiagnosticsService {
    DiagnosticsService::new(
        ProfileStore::with_base_path(root.join("profiles")),
        SettingsStore::with_base_path(root.join("settings")),
        MetadataStore::disabled(),
    )
}

/// Returns `(path-within-bundle, contents)` for every archive entry.
fn archive_entries(archive: &Path) -> Vec<(String, Vec<u8>)> {
    let decoder = flate2::read::GzDecoder::new(File::open(archive).unwrap());
    let mut archive = tar::Archive::new(decoder);
    archive
        .entries()
        .unwrap()
        .map(|entry| {
            let mut entry = entry.unwrap();
            let path = entry.path().unwrap().to_string_lossy().into_owned();
            // Strip the `crosshook-diagnostics-<timestamp>/` prefix.
            let relative = path.split_once('/').map(|(_, rest)| rest.to_string());
            let mut contents = Vec::new();
            entry.read_to_end(&mut contents).unwrap();
            (relative.unwrap_or(path), contents)
        })
        .collect()
}

#[tokio::test]
async fn export_writes_expected_files_and_attachments() {
    let temp = tempdir().unwrap();
    let output = temp.path().join("out");
    // Treat the tempdir as `$HOME` so the display path must be redacted.
    let result = service(temp.path())
        .with_display_home(temp.path().to_path_buf())
        .export(DiagnosticsRequest {
            redact_paths: true,
            output_dir: Some(output.clone()),
            attachments: vec![
                DiagnosticAttachment {
                    name: "ui.log".to_string(),
                    contents: b"opened /home/someone/game.exe".to_vec(),
                },
                DiagnosticAttachment {
                    name: "crash.bin".to_string(),
                    contents: vec![0xff, 0xfe, 0x00],
                },
            ],
        })
        .await
        .unwrap();

    assert!(result.archive_path.starts_with(&output));
    assert!(result.archive_path.exists());
    let temp_text = temp.path().display().to_string();
    assert!(
        result.display_path.starts_with("$HOME/out/"),
        "{}",
        result.display_path
    );
    assert!(!result.display_path.contains(&temp_text));

    let entries = archive_entries(&result.archive_path);
    let names: Vec<&str> = entries.iter().map(|(name, _)| name.as_str()).collect();
    for expected in [
        "system-info.txt",
        "crosshook-info.txt",
        "health-summary.json",
        "metadata-status.json",
        "steam-diagnostics.txt",
        "proton-installs.json",
        "settings.toml",
        "app/ui.log",
        "app/crash.bin",
    ] {
        assert!(names.contains(&expected), "missing {expected}: {names:?}");
    }
    let content = |name: &str| {
        entries
            .iter()
            .find(|(entry, _)| entry == name)
            .map(|(_, contents)| contents.clone())
            .unwrap()
    };
    assert_eq!(content("app/ui.log"), b"opened $HOME/game.exe");
    assert_eq!(content("app/crash.bin"), vec![0xff, 0xfe, 0x00]);
}

#[tokio::test]
async fn export_keeps_attachment_text_without_redaction() {
    let temp = tempdir().unwrap();
    let result = service(temp.path())
        .export(DiagnosticsRequest {
            redact_paths: false,
            output_dir: Some(temp.path().join("out")),
            attachments: vec![DiagnosticAttachment {
                name: "ui.log".to_string(),
                contents: b"/home/someone/game.exe".to_vec(),
            }],
        })
        .await
        .unwrap();
    let entries = archive_entries(&result.archive_path);
    let (_, contents) = entries
        .iter()
        .find(|(name, _)| name == "app/ui.log")
        .unwrap();
    assert_eq!(contents, b"/home/someone/game.exe");
}

#[tokio::test]
async fn export_rejects_unsafe_attachment_names_without_writing() {
    for name in ["", ".", "..", "../x", "a/b", "a\\b", "nul\0"] {
        let temp = tempdir().unwrap();
        let output = temp.path().join("out");
        let error = service(temp.path())
            .export(DiagnosticsRequest {
                redact_paths: false,
                output_dir: Some(output.clone()),
                attachments: vec![DiagnosticAttachment {
                    name: name.to_string(),
                    contents: Vec::new(),
                }],
            })
            .await
            .unwrap_err();
        assert_eq!(
            error.kind(),
            AppErrorKind::Validation {
                field: "attachments"
            },
            "{name:?}"
        );
        assert!(!output.exists(), "{name:?} must not write anything");
    }
}
