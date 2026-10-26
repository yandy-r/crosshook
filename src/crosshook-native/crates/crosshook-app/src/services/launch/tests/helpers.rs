//! Shared harness: tempdir stores, a service wired to a scripted
//! [`FakeSpawner`], hermetic launch requests, and an event collector.
//!
//! Every helper here is used by at least one slice.

use std::collections::{BTreeMap, VecDeque};
use std::ffi::OsString;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crosshook_core::launch::{
    LaunchRequest, RuntimeLaunchConfig, SteamLaunchConfig, METHOD_PROTON_RUN,
    METHOD_STEAM_APPLAUNCH,
};
use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::{ProfileStore, TrainerLoadingMode};
use crosshook_core::runtime_helpers::HelperRoots;
use crosshook_core::settings::{SettingsStore, UmuPreference};
use tempfile::TempDir;
use tokio::process::{Child, Command};

use super::super::{
    HelperSource, LaunchEnv, LaunchEvent, LaunchService, LaunchSession, ProcessSpawner,
};

/// How long [`Harness::collect`] waits for `Exited`.
const COLLECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Metadata store flavor for a [`Harness`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Metadata {
    /// In-memory SQLite store.
    Enabled,
    /// `MetadataStore::disabled()`.
    Disabled,
}

/// How a scripted child is wired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScriptMode {
    /// Piped stdout and stderr, as the Proton builders configure them.
    Piped,
    /// Output appended to the path after `--log-file` in the captured args,
    /// as the real Steam helper does.
    LogFile,
}

/// One scripted child: a `sh -c` body plus its stdio wiring.
#[derive(Debug, Clone)]
pub(super) struct Script {
    pub(super) body: String,
    pub(super) mode: ScriptMode,
}

impl Script {
    /// A child with piped stdout and stderr.
    pub(super) fn piped(body: impl Into<String>) -> Self {
        Self {
            body: body.into(),
            mode: ScriptMode::Piped,
        }
    }

    /// A child whose output goes to the `--log-file` path.
    pub(super) fn log_file(body: impl Into<String>) -> Self {
        Self {
            body: body.into(),
            mode: ScriptMode::LogFile,
        }
    }
}

/// The command a core builder handed to the spawner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CapturedCommand {
    pub(super) program: OsString,
    pub(super) args: Vec<OsString>,
    pub(super) envs: BTreeMap<OsString, Option<OsString>>,
    pub(super) current_dir: Option<PathBuf>,
}

impl CapturedCommand {
    fn capture(command: &Command) -> Self {
        let std = command.as_std();
        Self {
            program: std.get_program().to_owned(),
            args: std.get_args().map(ToOwned::to_owned).collect(),
            envs: std
                .get_envs()
                .map(|(key, value)| (key.to_owned(), value.map(ToOwned::to_owned)))
                .collect(),
            current_dir: std.get_current_dir().map(ToOwned::to_owned),
        }
    }

    /// The path following `--log-file`, if present.
    pub(super) fn log_file_arg(&self) -> Option<PathBuf> {
        self.args
            .windows(2)
            .find(|pair| pair[0] == "--log-file")
            .map(|pair| PathBuf::from(&pair[1]))
    }
}

#[derive(Default)]
struct FakeState {
    queue: VecDeque<Script>,
    fail_next: Option<io::Error>,
    captured: Vec<CapturedCommand>,
}

/// Spawner that records every command, then runs a scripted `sh -c` child
/// instead of the real one. With an empty queue it runs `exit 0` piped.
#[derive(Default)]
pub(super) struct FakeSpawner {
    state: Mutex<FakeState>,
}

impl FakeSpawner {
    pub(super) fn new() -> Self {
        Self::default()
    }

    /// Queues the script for a later spawn call (FIFO).
    pub(super) fn push(&self, script: Script) {
        self.lock().queue.push_back(script);
    }

    /// Makes the next spawn call fail with `error` (the command is still captured).
    pub(super) fn fail_next(&self, error: io::Error) {
        self.lock().fail_next = Some(error);
    }

    /// Every command captured so far, in spawn order.
    pub(super) fn captured(&self) -> Vec<CapturedCommand> {
        self.lock().captured.clone()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, FakeState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl ProcessSpawner for FakeSpawner {
    fn spawn(&self, command: &mut Command) -> io::Result<Child> {
        let captured = CapturedCommand::capture(command);
        let (script, failure) = {
            let mut state = self.lock();
            state.captured.push(captured.clone());
            (
                state
                    .queue
                    .pop_front()
                    .unwrap_or_else(|| Script::piped("exit 0")),
                state.fail_next.take(),
            )
        };
        if let Some(error) = failure {
            return Err(error);
        }

        // `sh` is not a denylisted host tool; the fake never runs the real command.
        let mut child = Command::new("sh");
        child
            .arg("-c")
            .arg(&script.body)
            .stdin(Stdio::null())
            .kill_on_drop(true);
        match script.mode {
            ScriptMode::Piped => {
                child.stdout(Stdio::piped()).stderr(Stdio::piped());
            }
            ScriptMode::LogFile => {
                let path = captured.log_file_arg().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "no --log-file argument")
                })?;
                let file = fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)?;
                let stderr = file.try_clone()?;
                child.stdout(Stdio::from(file)).stderr(Stdio::from(stderr));
            }
        }
        child.spawn()
    }
}

/// A service over tempdir stores and a [`FakeSpawner`].
pub(super) struct Harness {
    /// Keeps the tempdir alive for the harness lifetime.
    pub(super) root: TempDir,
    /// Where this harness's session logs land (inside [`Self::root`], never
    /// the shared `/tmp/crosshook-logs`).
    pub(super) log_dir: PathBuf,
    pub(super) profiles: ProfileStore,
    pub(super) settings: SettingsStore,
    pub(super) metadata: MetadataStore,
    pub(super) spawner: Arc<FakeSpawner>,
    pub(super) service: LaunchService,
}

impl Harness {
    /// Host env fixed to a plain native session; helpers come from stub scripts.
    pub(super) fn new(metadata: Metadata) -> Self {
        let root = tempfile::tempdir().unwrap();
        let profiles = ProfileStore::with_base_path(root.path().join("profiles"));
        let settings = SettingsStore::with_base_path(root.path().join("config"));
        let metadata = match metadata {
            Metadata::Enabled => MetadataStore::open_in_memory().unwrap(),
            Metadata::Disabled => MetadataStore::disabled(),
        };

        let log_dir = root.path().join("launch-logs");
        fs::create_dir_all(&log_dir).unwrap();

        // The flatpak-resources candidate is exempt from the trust check, so
        // plain 0755 stubs resolve without any uid games.
        let helper_dir = root.path().join("helpers");
        fs::create_dir_all(&helper_dir).unwrap();
        for name in ["steam-launch-helper.sh", "steam-launch-trainer.sh"] {
            write_file(&helper_dir.join(name), "#!/bin/sh\nexit 0\n", 0o755);
        }

        let spawner = Arc::new(FakeSpawner::new());
        let service =
            LaunchService::new(profiles.clone(), settings.clone(), metadata.clone(), None)
                .with_spawner(spawner.clone())
                .with_env(LaunchEnv {
                    is_flatpak: false,
                    inside_gamescope_session: false,
                })
                .with_helper_source(HelperSource::Roots(HelperRoots {
                    flatpak_resources: Some(helper_dir),
                    exe_dir: None,
                    dev_dir: None,
                }))
                .with_log_dir(log_dir.clone());

        Self {
            root,
            log_dir,
            profiles,
            settings,
            metadata,
            spawner,
            service,
        }
    }

    /// A valid `proton_run` game request for `name`; every path lives under
    /// the harness tempdir.
    pub(super) fn proton_request(&self, name: &str) -> LaunchRequest {
        let dir = self.root.path().join("requests").join(name);
        let prefix = dir.join("prefix");
        fs::create_dir_all(prefix.join("pfx")).unwrap();
        let proton = dir.join("proton");
        // Unique per request: the log slug derives from the exe name, so
        // distinct requests never share a log file even inside one harness.
        let game = dir.join(format!("{name}.exe"));
        let trainer = dir.join("trainer.exe");
        write_file(&proton, "#!/bin/sh\nexit 0\n", 0o755);
        write_file(&game, "game", 0o755);
        write_file(&trainer, "trainer", 0o644);
        let trainer = path_string(&trainer);

        LaunchRequest {
            method: METHOD_PROTON_RUN.to_string(),
            game_path: path_string(&game),
            trainer_path: trainer.clone(),
            trainer_host_path: trainer,
            trainer_loading_mode: TrainerLoadingMode::SourceDirectory,
            runtime: RuntimeLaunchConfig {
                prefix_path: path_string(&prefix),
                proton_path: path_string(&proton),
                ..Default::default()
            },
            profile_name: Some(name.to_string()),
            umu_preference: UmuPreference::Proton,
            ..Default::default()
        }
    }

    /// A valid `steam_applaunch` game request for `name`; every path lives
    /// under the harness tempdir.
    pub(super) fn steam_request(&self, name: &str) -> LaunchRequest {
        let dir = self.root.path().join("requests").join(name);
        let steam_root = dir.join("steam");
        let compatdata = steam_root.join("steamapps/compatdata/12345");
        fs::create_dir_all(compatdata.join("pfx")).unwrap();
        fs::create_dir_all(steam_root.join("config")).unwrap();
        let proton = dir.join("proton");
        let game = dir.join(format!("{name}.sh"));
        let trainer = dir.join("trainer.exe");
        write_file(&proton, "#!/bin/sh\nexit 0\n", 0o755);
        write_file(&game, "#!/bin/sh\nexit 0\n", 0o755);
        write_file(&trainer, "trainer", 0o644);
        let trainer = path_string(&trainer);

        LaunchRequest {
            method: METHOD_STEAM_APPLAUNCH.to_string(),
            game_path: path_string(&game),
            trainer_path: trainer.clone(),
            trainer_host_path: trainer,
            trainer_loading_mode: TrainerLoadingMode::SourceDirectory,
            steam: SteamLaunchConfig {
                app_id: "12345".to_string(),
                compatdata_path: path_string(&compatdata),
                proton_path: path_string(&proton),
                steam_client_install_path: path_string(&steam_root),
            },
            profile_name: Some(name.to_string()),
            umu_preference: UmuPreference::Proton,
            ..Default::default()
        }
    }

    /// Drains `session`'s events until `Exited` (inclusive), in order.
    ///
    /// # Panics
    /// When `Exited` does not arrive within 10 s or the stream ends without it.
    pub(super) async fn collect(mut session: LaunchSession) -> Vec<LaunchEvent> {
        let drain = async {
            let mut events = Vec::new();
            while let Some(event) = session.events.recv().await {
                let exited = matches!(event, LaunchEvent::Exited { .. });
                events.push(event);
                if exited {
                    return events;
                }
            }
            panic!("event stream ended without Exited: {events:?}");
        };
        tokio::time::timeout(COLLECT_TIMEOUT, drain)
            .await
            .expect("Exited event within 10 s")
    }
}

fn write_file(path: &Path, body: &str, mode: u32) {
    fs::write(path, body).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
