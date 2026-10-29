//! InstallService tests: request validation, default prefix path and error
//! mapping here; the installer job in `start`.

mod start;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crosshook_core::install::{InstallGameError, InstallGameRequest};
use tempfile::TempDir;

use super::start::install_job_key;
use super::{install_error, InstallService};
use crate::jobs::JobRegistry;
use crate::services::process::{prefix_job_key, prefix_job_key_for_path};
use crate::AppErrorKind;

/// A temp workspace with an installer, a fake runner script standing in for
/// Proton, a prefix path, a log directory and a FIFO gate.
pub(super) struct Fixture {
    pub(super) dir: TempDir,
    pub(super) installer: PathBuf,
    pub(super) runner: PathBuf,
    pub(super) prefix: PathBuf,
    pub(super) log_dir: PathBuf,
    pub(super) gate: PathBuf,
}

impl Fixture {
    /// Lays out the workspace. The runner script is `runner_body` after a
    /// guard that exits 9–11 unless it was called as exactly
    /// `run <installer>`; in the body `@PREFIX@` is the prefix path and
    /// `@GATE@` a FIFO that blocks a reader until [`Fixture::open_gate`].
    pub(super) fn new(runner_body: &str) -> Self {
        let dir = TempDir::new().unwrap();
        let installer = dir.path().join("setup.exe");
        fs::write(&installer, b"installer").unwrap();
        let prefix = dir.path().join("prefix");
        let log_dir = dir.path().join("logs");
        fs::create_dir(&log_dir).unwrap();
        let gate = make_fifo(dir.path());
        let runner = dir.path().join("fake-runner");
        let body = runner_body
            .replace("@PREFIX@", &prefix.to_string_lossy())
            .replace("@GATE@", &gate.to_string_lossy());
        let script = format!(
            "#!/bin/sh\n[ \"$1\" = run ] || exit 9\n[ \"$2\" = '{}' ] || exit 10\n[ $# -eq 2 ] || exit 11\n{body}\n",
            installer.display()
        );
        fs::write(&runner, script).unwrap();
        fs::set_permissions(&runner, fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            dir,
            installer,
            runner,
            prefix,
            log_dir,
            gate,
        }
    }

    /// Opens the gate the runner blocks on (open-for-write rendezvous).
    pub(super) async fn open_gate(&self) {
        let gate = self.gate.clone();
        tokio::task::spawn_blocking(move || fs::write(gate, b"go\n"))
            .await
            .unwrap()
            .unwrap();
    }

    pub(super) fn request(&self) -> InstallGameRequest {
        InstallGameRequest {
            profile_name: "x-game".to_string(),
            display_name: "X Game".to_string(),
            installer_path: self.installer.to_string_lossy().into_owned(),
            proton_path: self.runner.to_string_lossy().into_owned(),
            prefix_path: self.prefix.to_string_lossy().into_owned(),
            ..InstallGameRequest::default()
        }
    }

    pub(super) fn service(&self, jobs: JobRegistry) -> InstallService {
        InstallService::with_log_dir(jobs, self.log_dir.clone())
    }

    /// The log files created so far.
    pub(super) fn log_files(&self) -> Vec<PathBuf> {
        fs::read_dir(&self.log_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect()
    }
}

/// Creates a FIFO at `<dir>/gate`.
fn make_fifo(dir: &Path) -> PathBuf {
    let fifo = dir.join("gate");
    let status = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap();
    assert!(status.success());
    fifo
}

#[tokio::test]
async fn validate_names_the_offending_field() {
    let fixture = Fixture::new("exit 0");
    let service = fixture.service(JobRegistry::default());
    service.validate(fixture.request()).await.unwrap();

    let mut request = fixture.request();
    request.installer_path = fixture.dir.path().join("missing.exe").display().to_string();
    let error = service.validate(request).await.unwrap_err();
    assert_eq!(
        error.kind(),
        AppErrorKind::Validation {
            field: "installer_path"
        }
    );
    assert_eq!(error.message(), "The installer path does not exist.");

    let mut request = fixture.request();
    request.proton_path = String::new();
    let error = service.validate(request).await.unwrap_err();
    assert_eq!(
        error.kind(),
        AppErrorKind::Validation {
            field: "proton_path"
        }
    );
    // Validation provisions nothing.
    assert!(!fixture.prefix.exists());
}

#[tokio::test]
async fn default_prefix_path_slugifies_and_rejects_an_empty_name() {
    let service = InstallService::new(JobRegistry::default());
    let path = service
        .default_prefix_path("My Game".to_string())
        .await
        .unwrap();
    assert!(path.ends_with("/my-game"), "{path}");

    let error = service
        .default_prefix_path("  ".to_string())
        .await
        .unwrap_err();
    assert_eq!(
        error.kind(),
        AppErrorKind::Validation {
            field: "profile_name"
        }
    );
}

#[test]
fn install_errors_keep_core_text_with_explicit_kinds() {
    let prefix = PathBuf::from("/tmp/prefix-file");
    let as_file = install_error(InstallGameError::PrefixPathExistsAsFile {
        path: prefix.clone(),
    });
    assert_eq!(
        as_file.kind(),
        AppErrorKind::Validation {
            field: "prefix_path"
        }
    );
    assert!(as_file.message().contains("already exists as a file"));

    let failed = install_error(InstallGameError::PrefixCreationFailed {
        path: prefix,
        message: "denied".to_string(),
    });
    assert_eq!(failed.kind(), AppErrorKind::Io);
    assert!(failed.message().ends_with(": denied"));

    let home = install_error(InstallGameError::HomeDirectoryUnavailable);
    assert_eq!(home.kind(), AppErrorKind::Unavailable);
}

#[test]
fn install_key_is_the_effective_pfx_every_other_prefix_job_uses() {
    let dir = TempDir::new().unwrap();
    let prefix = dir.path().join("prefix");
    fs::create_dir(&prefix).unwrap();
    let canonical = fs::canonicalize(&prefix).unwrap();

    // A fresh prefix is keyed on the `pfx` Proton is about to create ...
    let fresh = install_job_key(&prefix).unwrap();
    assert_eq!(fresh, prefix_job_key_for_path(&canonical.join("pfx")));

    // ... which is the key prefix-deps, update and run-executable take once
    // it exists, from either the compat-data root or `pfx` itself.
    fs::create_dir(prefix.join("pfx")).unwrap();
    let (_, shared) = prefix_job_key(&prefix.to_string_lossy()).unwrap();
    assert_eq!(install_job_key(&prefix).unwrap(), shared);
    assert_eq!(install_job_key(&prefix.join("pfx")).unwrap(), shared);
    assert_eq!(fresh, shared);

    let error = install_job_key(&dir.path().join("missing")).unwrap_err();
    assert_eq!(
        error.kind(),
        AppErrorKind::Validation {
            field: "prefix_path"
        }
    );
}
