//! Update service tests over a fake `proton` shell script, a tempdir prefix
//! and a tempdir log directory.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crosshook_core::update::UpdateGameRequest;
use tempfile::TempDir;

use super::{UpdateCompletion, UpdateService};
use crate::jobs::JobRegistry;
use crate::log_tail::LogBatch;
use crate::services::process::{ProcessRun, RunEvent};

mod start;

/// A service over a tempdir with an executable fake Proton, an updater
/// `.exe` and a prefix directory.
struct Fixture {
    dir: TempDir,
    jobs: JobRegistry,
    service: UpdateService,
    proton: PathBuf,
    updater: PathBuf,
    prefix: PathBuf,
    log_dir: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        // Canonical root so paths compare equal to what the child reports.
        let root = dir.path().canonicalize().unwrap();
        let prefix = root.join("prefix");
        std::fs::create_dir(&prefix).unwrap();
        let updater_dir = root.join("game");
        std::fs::create_dir(&updater_dir).unwrap();
        let updater = updater_dir.join("patch.exe");
        std::fs::write(&updater, b"MZ").unwrap();
        let log_dir = root.join("logs");
        let jobs = JobRegistry::default();
        let service = UpdateService::with_log_dir(jobs.clone(), log_dir.clone());
        Self {
            dir,
            jobs,
            service,
            proton: root.join("fake-proton"),
            updater,
            prefix,
            log_dir,
        }
    }

    /// Installs the fake Proton: it refuses (exit 97, `bad-args` line) unless
    /// it was invoked as `<proton> run <updater>` with `WINEPREFIX` set to
    /// the prefix and the updater's directory as working directory, then
    /// runs `body`.
    fn install_proton(&self, body: &str) {
        let script = format!(
            "#!/bin/sh\n\
             if [ \"$#\" -ne 2 ] || [ \"$1\" != run ] || [ \"$2\" != '{updater}' ] \
             || [ \"$WINEPREFIX\" != '{prefix}' ] || [ \"$(pwd -P)\" != '{cwd}' ]; then\n\
             echo bad-args; exit 97\n\
             fi\n\
             {body}\n",
            updater = self.updater.display(),
            prefix = self.prefix.display(),
            cwd = self.updater.parent().unwrap().display(),
        );
        write_executable(&self.proton, &script);
    }

    fn request(&self) -> UpdateGameRequest {
        UpdateGameRequest {
            profile_name: "Test Game".to_string(),
            updater_path: self.updater.display().to_string(),
            proton_path: self.proton.display().to_string(),
            prefix_path: self.prefix.display().to_string(),
            steam_client_install_path: String::new(),
        }
    }

    /// A fifo the fake blocks on until [`open_gate`] (or forever).
    fn fifo(&self) -> PathBuf {
        let fifo = self.dir.path().join("gate");
        let status = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap();
        assert!(status.success());
        fifo
    }

    /// Files in the log directory (missing directory = none).
    fn log_files(&self) -> Vec<PathBuf> {
        match std::fs::read_dir(&self.log_dir) {
            Ok(entries) => entries.map(|entry| entry.unwrap().path()).collect(),
            Err(_) => Vec::new(),
        }
    }
}

fn write_executable(path: &Path, contents: &str) {
    std::fs::write(path, contents).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// Opens the gate the fake blocks on (open-for-write rendezvous).
async fn open_gate(fifo: PathBuf) {
    tokio::task::spawn_blocking(move || std::fs::write(fifo, b"go\n"))
        .await
        .unwrap()
        .unwrap();
}

/// Next log line, skipping notices; panics when the run finishes first.
async fn next_line(run: &mut ProcessRun<UpdateCompletion>) -> String {
    loop {
        match run.next().await.expect("run ended") {
            RunEvent::Log(LogBatch::Lines(mut batch)) if !batch.is_empty() => {
                return batch.remove(0)
            }
            RunEvent::Log(_) => {}
            RunEvent::Finished(_) => panic!("run finished before the expected line"),
        }
    }
}

/// Collects log lines until `Finished`; returns them with the completion.
async fn drain(run: &mut ProcessRun<UpdateCompletion>) -> (Vec<String>, Option<UpdateCompletion>) {
    let mut lines = Vec::new();
    loop {
        match run.next().await.expect("Finished before the stream ends") {
            RunEvent::Log(LogBatch::Lines(batch)) => lines.extend(batch),
            RunEvent::Log(_) => {}
            RunEvent::Finished(completion) => return (lines, completion),
        }
    }
}
