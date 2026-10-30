//! Run-executable service tests over a fake `proton` shell script, a
//! tempdir `_run-adhoc` root, a tempdir log directory and fifo gates (no
//! timing sleeps).

use std::collections::VecDeque;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crosshook_core::run_executable::RunExecutableRequest;
use tempfile::TempDir;

use super::{RunCompletion, RunExecutableResponse, RunExecutableService};
use crate::jobs::{JobKey, JobRegistry};
use crate::log_tail::LogBatch;
use crate::services::process::{prefix_job_key, JobOwner, ProcessRun, RunEvent};

mod cancel;
mod cleanup;
mod start;

/// The fake's body for a run that blocks until its gate opens: it creates
/// `<prefix>/gate`, announces `ready`, waits on the gate, then says `done`.
const GATED: &str =
    "mkfifo \"$WINEPREFIX/gate\"\necho ready\nread x < \"$WINEPREFIX/gate\"\necho done";

/// A service over a tempdir with an executable fake Proton, a game `.exe`,
/// a throwaway root and a log directory.
struct Fixture {
    _dir: TempDir,
    root: PathBuf,
    jobs: JobRegistry,
    service: RunExecutableService,
    proton: PathBuf,
    exe: PathBuf,
    adhoc_root: PathBuf,
    log_dir: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        // Canonical root so paths compare equal to what the child reports.
        let root = dir.path().canonicalize().unwrap();
        let game_dir = root.join("game");
        std::fs::create_dir(&game_dir).unwrap();
        let exe = game_dir.join("Setup.exe");
        std::fs::write(&exe, b"MZ").unwrap();
        let adhoc_root = root.join("prefixes").join("_run-adhoc");
        let log_dir = root.join("logs");
        let jobs = JobRegistry::default();
        let service =
            RunExecutableService::with_dirs(jobs.clone(), log_dir.clone(), adhoc_root.clone());
        Self {
            _dir: dir,
            proton: root.join("fake-proton"),
            root,
            jobs,
            service,
            exe,
            adhoc_root,
            log_dir,
        }
    }

    /// Installs the fake Proton. It refuses (exit 97, `bad-args` line)
    /// unless invoked as `<proton> run <exe>` from the exe's directory with
    /// `WINEPREFIX` equal to `STEAM_COMPAT_DATA_PATH`; otherwise it puts a
    /// file into the prefix, reports `prefix=<WINEPREFIX>` and runs `body`.
    fn install_proton(&self, body: &str) {
        let script = format!(
            "#!/bin/sh\n\
             if [ \"$#\" -ne 2 ] || [ \"$1\" != run ] || [ \"$2\" != '{exe}' ] \
             || [ \"$(pwd -P)\" != '{cwd}' ] || [ -z \"$WINEPREFIX\" ] \
             || [ \"$WINEPREFIX\" != \"$STEAM_COMPAT_DATA_PATH\" ]; then\n\
             echo bad-args; exit 97\n\
             fi\n\
             mkdir -p \"$WINEPREFIX/drive_c\" && echo data > \"$WINEPREFIX/drive_c/file.txt\"\n\
             echo \"prefix=$WINEPREFIX\"\n\
             {body}\n",
            exe = self.exe.display(),
            cwd = self.exe.parent().unwrap().display(),
        );
        std::fs::write(&self.proton, script).unwrap();
        std::fs::set_permissions(&self.proton, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// A throwaway-prefix request (empty `prefix_path`).
    fn request(&self) -> RunExecutableRequest {
        RunExecutableRequest {
            executable_path: self.exe.display().to_string(),
            proton_path: self.proton.display().to_string(),
            prefix_path: String::new(),
            working_directory: String::new(),
            steam_client_install_path: String::new(),
        }
    }

    /// A request for the explicit user prefix `prefix`.
    fn request_with_prefix(&self, prefix: &Path) -> RunExecutableRequest {
        RunExecutableRequest {
            prefix_path: prefix.display().to_string(),
            ..self.request()
        }
    }

    /// Starts a run for `owner`, panicking on a start error.
    async fn start(
        &self,
        owner: &str,
        request: RunExecutableRequest,
    ) -> (RunExecutableResponse, Run) {
        let (response, run) = self
            .service
            .start(JobOwner::new(owner), request)
            .await
            .expect("run starts");
        (response, Run::new(run))
    }

    /// Entries of the throwaway root (missing root = none).
    fn throwaway_dirs(&self) -> Vec<PathBuf> {
        match std::fs::read_dir(&self.adhoc_root) {
            Ok(entries) => entries.map(|entry| entry.unwrap().path()).collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Files in the log directory (missing directory = none).
    fn log_files(&self) -> Vec<PathBuf> {
        match std::fs::read_dir(&self.log_dir) {
            Ok(entries) => entries.map(|entry| entry.unwrap().path()).collect(),
            Err(_) => Vec::new(),
        }
    }
}

/// The exclusivity key of `prefix` (the resolved prefix of a response).
fn key_of(prefix: &str) -> JobKey {
    prefix_job_key(prefix).unwrap().1
}

/// Opens the gate a gated run blocks on (open-for-write rendezvous).
async fn open_gate(prefix: &str) {
    let gate = Path::new(prefix).join("gate");
    tokio::task::spawn_blocking(move || std::fs::write(gate, b"go\n"))
        .await
        .unwrap()
        .unwrap();
}

/// A [`ProcessRun`] read line by line: lines of one batch are buffered, so
/// none is lost when a batch carries several.
struct Run {
    inner: ProcessRun<RunCompletion>,
    pending: VecDeque<String>,
}

impl Run {
    fn new(inner: ProcessRun<RunCompletion>) -> Self {
        Self {
            inner,
            pending: VecDeque::new(),
        }
    }

    fn run_id(&self) -> &str {
        self.inner.run_id()
    }

    /// Next log line, skipping notices; panics when the run finishes first.
    async fn next_line(&mut self) -> String {
        loop {
            if let Some(line) = self.pending.pop_front() {
                return line;
            }
            match self.inner.next().await.expect("run ended") {
                RunEvent::Log(LogBatch::Lines(batch)) => self.pending.extend(batch),
                RunEvent::Log(_) => {}
                RunEvent::Finished(_) => panic!("run finished before the expected line"),
            }
        }
    }

    /// Waits until a gated run reported its prefix and is blocked on its
    /// gate.
    async fn wait_ready(&mut self, prefix: &str) {
        assert_eq!(self.next_line().await, format!("prefix={prefix}"));
        assert_eq!(self.next_line().await, "ready");
    }

    /// Collects the remaining log lines until `Finished`; returns them with
    /// the completion and checks that nothing follows `Finished`.
    async fn drain(&mut self) -> (Vec<String>, Option<RunCompletion>) {
        let mut lines: Vec<String> = self.pending.drain(..).collect();
        loop {
            match self
                .inner
                .next()
                .await
                .expect("Finished before the stream ends")
            {
                RunEvent::Log(LogBatch::Lines(batch)) => lines.extend(batch),
                RunEvent::Log(_) => {}
                RunEvent::Finished(completion) => {
                    assert_eq!(self.inner.next().await, None, "nothing after Finished");
                    return (lines, completion);
                }
            }
        }
    }
}
