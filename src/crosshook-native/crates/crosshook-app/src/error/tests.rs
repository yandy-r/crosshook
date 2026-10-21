//! Tests for core-error mapping into sanitized [`AppError`].

use std::io::{Error as IoError, ErrorKind};
use std::path::PathBuf;

use crosshook_core::export::DiagnosticBundleError;
use crosshook_core::launch::ValidationError;
use crosshook_core::metadata::MetadataStoreError;
use crosshook_core::profile::ProfileStoreError;
use crosshook_core::settings::{RecentFilesStoreError, SettingsStoreError};

use super::{AppError, AppErrorKind};

#[tokio::test]
async fn join_panic_maps_to_internal_with_fixed_message() {
    let join = tokio::spawn(async { panic!("secret /home/user/token") });
    let error = AppError::from(join.await.unwrap_err());
    assert_eq!(error.kind(), AppErrorKind::Internal);
    assert_eq!(error.message(), "background task failed");
}

#[tokio::test]
async fn join_cancel_maps_to_cancelled() {
    let join = tokio::spawn(std::future::pending::<()>());
    join.abort();
    let error = AppError::from(join.await.unwrap_err());
    assert_eq!(error.kind(), AppErrorKind::Cancelled);
}

#[test]
fn io_error_keeps_only_kind() {
    let error = AppError::from(IoError::new(ErrorKind::PermissionDenied, "/home/user/x"));
    assert_eq!(error.kind(), AppErrorKind::Io);
    assert!(!error.message().contains("/home/"));
}

#[test]
fn launch_validation_maps_to_validation() {
    let error = AppError::from(ValidationError::RuntimePrefixPathMissing);
    assert_eq!(error.kind(), AppErrorKind::Validation { field: "launch" });
}

#[test]
fn profile_store_variants_map() {
    let kind = |e: ProfileStoreError| AppError::from(e).kind();
    assert_eq!(
        kind(ProfileStoreError::NotFound(PathBuf::from("/x"))),
        AppErrorKind::NotFound
    );
    assert_eq!(
        kind(ProfileStoreError::LaunchPresetNotFound("p".into())),
        AppErrorKind::NotFound
    );
    assert_eq!(
        kind(ProfileStoreError::AlreadyExists("n".into())),
        AppErrorKind::Conflict
    );
    assert_eq!(
        kind(ProfileStoreError::InvalidName(String::new())),
        AppErrorKind::Validation { field: "profile" }
    );
    assert_eq!(
        kind(ProfileStoreError::CommandArgumentValidation(
            ValidationError::CommandArgumentCustomTokenEmpty
        )),
        AppErrorKind::Validation { field: "launch" }
    );
}

#[test]
fn profile_store_io_hides_path() {
    let error = AppError::from(ProfileStoreError::Io(IoError::new(
        ErrorKind::NotFound,
        "/home/user/p.toml",
    )));
    assert_eq!(error.kind(), AppErrorKind::Io);
    assert!(!error.message().contains("/home/"));
}

#[test]
fn settings_io_hides_path() {
    let error = AppError::from(SettingsStoreError::Io(IoError::new(
        ErrorKind::NotFound,
        "/home/user/s.toml",
    )));
    assert_eq!(error.kind(), AppErrorKind::Io);
    assert!(!error.message().contains("/home/"));
}

/// TOML parse errors echo the offending source line; a path outside `$HOME`
/// would survive home redaction, so messages must be fixed strings.
fn toml_de_error_with_path() -> toml::de::Error {
    toml::from_str::<toml::Table>("game = \"/mnt/secret/game.exe\" oops").unwrap_err()
}

#[test]
fn toml_errors_map_to_fixed_messages_without_source_text() {
    let de = toml_de_error_with_path();
    assert!(
        de.to_string().contains("/mnt/secret"),
        "fixture must echo source"
    );
    let cases = [
        (
            AppError::from(SettingsStoreError::TomlDe(de.clone())),
            AppErrorKind::Validation { field: "settings" },
            "settings file is not valid TOML",
        ),
        (
            AppError::from(RecentFilesStoreError::TomlDe(de)),
            AppErrorKind::Validation {
                field: "recent_files",
            },
            "recent files data is not valid TOML",
        ),
    ];
    for (error, kind, message) in cases {
        assert_eq!(error.kind(), kind);
        assert_eq!(error.message(), message);
        assert!(!error.message().contains("/mnt/secret"));
    }
}

#[test]
fn recent_files_io_hides_path() {
    let error = AppError::from(RecentFilesStoreError::Io(IoError::new(
        ErrorKind::PermissionDenied,
        "/mnt/secret/recent.toml",
    )));
    assert_eq!(error.kind(), AppErrorKind::Io);
    assert!(!error.message().contains("/mnt/secret"));
}

#[test]
fn diagnostic_bundle_errors_hide_paths() {
    let cases = [
        DiagnosticBundleError::Io {
            action: "create the archive file",
            path: PathBuf::from("/mnt/secret/out.tar.gz"),
            source: IoError::new(ErrorKind::PermissionDenied, "/mnt/secret"),
        },
        DiagnosticBundleError::Archive("/mnt/secret/entry".to_string()),
        DiagnosticBundleError::ProfileStore("/mnt/secret/p.toml".to_string()),
    ];
    for case in cases {
        let error = AppError::from(case);
        assert!(!error.message().contains("/mnt/secret"), "{error:?}");
    }
}

#[test]
fn metadata_variants_map_to_fixed_messages() {
    let schema = AppError::from(MetadataStoreError::NewerSchema {
        found: 99,
        supported: 1,
    });
    assert_eq!(schema.kind(), AppErrorKind::Unavailable);
    assert_eq!(schema.message(), "metadata database is unavailable");

    let symlink = AppError::from(MetadataStoreError::SymlinkDetected(PathBuf::from(
        "/home/user/metadata.db",
    )));
    assert!(!symlink.message().contains("/home/"));

    let io = AppError::from(MetadataStoreError::Io {
        action: "open",
        path: PathBuf::from("/home/user/metadata.db"),
        source: IoError::from(ErrorKind::PermissionDenied),
    });
    assert_eq!(io.kind(), AppErrorKind::Io);
    assert!(!io.message().contains("/home/"));
}
