use std::path::PathBuf;

use crosshook_core::protonup::install::InstallError;
use crosshook_core::protonup::progress::Phase;
use crosshook_core::protonup::uninstall::UninstallError;
use crosshook_core::protonup::ProtonUpInstallErrorKind;

use super::super::identity::IdentityError;
use super::super::messages::*;
use super::helpers::request;

const TMP: &str = "/tmp/secret-dir/GE-Proton9-21";

fn assert_path_free(text: &str) {
    assert!(!text.contains('/'), "contains '/': {text}");
    assert!(!text.contains("http"), "contains 'http': {text}");
}

#[test]
fn fixed_texts_match_the_contract_and_are_path_free() {
    let table = [
        (MSG_OFFLINE, "offline mode is enabled"),
        (MSG_OP_NOT_FOUND, "install operation was not found"),
        (MSG_INSTALL_FAILED, "Proton install failed"),
        (MSG_INSTALL_CANCELLED, "Proton install cancelled"),
        (UNINSTALL_SYSTEM, "refusing to delete a system path"),
        (
            UNINSTALL_OUTSIDE,
            "path is not inside a known Proton install root",
        ),
        (UNINSTALL_NOT_FOUND, "Proton tool was not found"),
        (UNINSTALL_FAILED, "failed to remove the Proton tool"),
    ];
    for (actual, expected) in table {
        assert_eq!(actual, expected);
        assert_path_free(actual);
    }
}

#[test]
fn identity_messages_keep_the_legacy_texts() {
    let req = request("GE-Proton9-21");
    assert_eq!(
        identity_message(IdentityError::Missing, &req),
        "requested version 'GE-Proton9-21' for provider 'ge-proton' was not found in the server catalog"
    );
    assert_eq!(
        identity_message(IdentityError::Mismatch, &req),
        "install request/version mismatch with server catalog identity"
    );
    assert_eq!(
        identity_message(IdentityError::MetadataMismatch, &req),
        "install metadata did not match server-resolved catalog entry"
    );
}

#[test]
fn uninstall_error_text_maps_each_variant_without_the_path() {
    let path = PathBuf::from(TMP);
    let table = [
        (
            UninstallError::SystemPathRefused(path.clone()),
            UNINSTALL_SYSTEM,
        ),
        (
            UninstallError::PathOutsideKnownRoots(path.clone()),
            UNINSTALL_OUTSIDE,
        ),
        (UninstallError::NotFound(path), UNINSTALL_NOT_FOUND),
        (
            UninstallError::Io(std::io::Error::other(TMP)),
            UNINSTALL_FAILED,
        ),
    ];
    for (error, expected) in table {
        let text = uninstall_error_text(&error);
        assert_eq!(text, expected);
        assert_path_free(text);
    }
}

#[test]
fn install_error_result_maps_kinds_and_is_path_free() {
    let table = [
        (
            InstallError::InvalidPath(TMP.into()),
            ProtonUpInstallErrorKind::InvalidPath,
            "install path is invalid",
        ),
        (
            InstallError::PermissionDenied(TMP.into()),
            ProtonUpInstallErrorKind::PermissionDenied,
            "permission denied while installing",
        ),
        (
            InstallError::NetworkError("https://x.example/a".into()),
            ProtonUpInstallErrorKind::NetworkError,
            "network error while installing",
        ),
        (
            InstallError::UntrustedUrl("https://x.example/a".into()),
            ProtonUpInstallErrorKind::NetworkError,
            "network error while installing",
        ),
        (
            InstallError::ChecksumMissing(TMP.into()),
            ProtonUpInstallErrorKind::ChecksumFailed,
            "checksum verification failed",
        ),
        (
            InstallError::ChecksumFailed(TMP.into()),
            ProtonUpInstallErrorKind::ChecksumFailed,
            "checksum verification failed",
        ),
        (
            InstallError::DependencyMissing { reason: TMP.into() },
            ProtonUpInstallErrorKind::DependencyMissing,
            "install dependency is missing",
        ),
        (
            InstallError::NoWritableInstallRoot,
            ProtonUpInstallErrorKind::InvalidPath,
            "no writable Proton install root found; install Steam first",
        ),
        (
            InstallError::Cancelled,
            ProtonUpInstallErrorKind::Cancelled,
            "install cancelled",
        ),
        (
            InstallError::Unknown(TMP.into()),
            ProtonUpInstallErrorKind::Unknown,
            "install failed",
        ),
    ];
    for (error, kind, text) in table {
        let result = install_error_result(&error);
        assert!(!result.success);
        assert_eq!(result.installed_path, None);
        assert_eq!(result.error_kind, Some(kind));
        let message = result.error_message.expect("message");
        assert_eq!(message, text);
        assert_path_free(&message);
    }
}

#[test]
fn already_installed_result_keeps_the_path_only_in_the_structured_field() {
    let result = install_error_result(&InstallError::AlreadyInstalled {
        path: PathBuf::from(TMP),
    });
    assert!(!result.success);
    assert_eq!(result.installed_path.as_deref(), Some(TMP));
    assert_eq!(
        result.error_kind,
        Some(ProtonUpInstallErrorKind::AlreadyInstalled)
    );
    let message = result.error_message.expect("message");
    assert_eq!(message, "Proton version is already installed");
    assert_path_free(&message);
}

#[test]
fn progress_message_replaces_failure_and_cancel_texts() {
    assert_eq!(
        progress_message(&Phase::Failed, Some(TMP)).as_deref(),
        Some(MSG_INSTALL_FAILED)
    );
    assert_eq!(
        progress_message(&Phase::Failed, None).as_deref(),
        Some(MSG_INSTALL_FAILED)
    );
    assert_eq!(
        progress_message(&Phase::Cancelled, Some(TMP)).as_deref(),
        Some(MSG_INSTALL_CANCELLED)
    );
}

#[test]
fn progress_message_keeps_slash_free_text_and_drops_the_rest() {
    let phases = [
        Phase::Resolving,
        Phase::Downloading,
        Phase::Verifying,
        Phase::Extracting,
        Phase::Finalizing,
        Phase::Done,
    ];
    for phase in phases {
        assert_eq!(
            progress_message(&phase, Some("verifying archive")).as_deref(),
            Some("verifying archive")
        );
        assert_eq!(progress_message(&phase, Some(TMP)), None);
        assert_eq!(progress_message(&phase, Some("https://x.example")), None);
        assert_eq!(progress_message(&phase, None), None);
    }
}
