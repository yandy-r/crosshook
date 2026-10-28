//! Harness: tempdir settings, in-memory metadata, fake catalog/installer/roots
//! and gating helpers. Gating uses `Notify` and the job's cancel token, never
//! timing sleeps; no helper touches process environment.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use crosshook_core::metadata::MetadataStore;
use crosshook_core::protonup::install::InstallError;
use crosshook_core::protonup::install_root::{InstallRootCandidate, InstallRootKind};
use crosshook_core::protonup::progress::{Phase, ProgressEmitter};
use crosshook_core::protonup::{
    ProtonUpAvailableVersion, ProtonUpCacheMeta, ProtonUpCatalogResponse, ProtonUpInstallRequest,
    ProtonUpInstallResult,
};
use crosshook_core::settings::SettingsStore;
use tempfile::TempDir;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use super::super::seams::{ProtonCatalogSource, ProtonInstaller, ProtonRootSource};
use super::super::{InstallOwner, ProtonUpService};
use crate::jobs::JobRegistry;
use crate::services::{BoxFuture, SteamService};

/// Catalog fake returning a fixed response and recording its calls.
pub(super) struct FakeCatalog {
    response: Mutex<ProtonUpCatalogResponse>,
    pub calls: AtomicUsize,
    /// `(provider_id, force_refresh, include_prereleases)` of the last call.
    pub last_args: Mutex<Option<(String, bool, bool)>>,
}

impl FakeCatalog {
    pub(super) fn new(response: ProtonUpCatalogResponse) -> Self {
        Self {
            response: Mutex::new(response),
            calls: AtomicUsize::new(0),
            last_args: Mutex::new(None),
        }
    }

    /// Replaces the response served from now on.
    pub(super) fn set_response(&self, response: ProtonUpCatalogResponse) {
        *self.response.lock().unwrap_or_else(PoisonError::into_inner) = response;
    }
}

impl ProtonCatalogSource for FakeCatalog {
    fn versions<'a>(
        &'a self,
        provider_id: &'a str,
        force_refresh: bool,
        include_prereleases: bool,
    ) -> BoxFuture<'a, ProtonUpCatalogResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        *self
            .last_args
            .lock()
            .unwrap_or_else(PoisonError::into_inner) =
            Some((provider_id.to_string(), force_refresh, include_prereleases));
        let response = self
            .response
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        Box::pin(async move { response })
    }
}

/// How [`FakeInstaller`] behaves.
pub(super) enum Mode {
    /// Emits `Resolving`, then `Done`, and succeeds.
    Succeed,
    /// Returns `Err(Unknown("<tmp path>"))` without emitting a `Failed` phase.
    FailNoTerminal,
    /// Emits `Failed` with a tmp-path message, then returns `Err`.
    FailWithPathMessage,
    /// Emits `Resolving`, then waits. Notify → emits `Done` and succeeds;
    /// cancel token → emits `Cancelled` and returns `Err(Cancelled)`.
    Gated(Arc<Notify>),
}

/// Installer fake recording every catalog entry it was asked to install.
pub(super) struct FakeInstaller {
    pub calls: AtomicUsize,
    pub seen: Mutex<Vec<ProtonUpAvailableVersion>>,
    mode: Mode,
    tmp: PathBuf,
}

impl FakeInstaller {
    pub(super) fn new(mode: Mode, tmp: PathBuf) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            seen: Mutex::new(Vec::new()),
            mode,
            tmp,
        }
    }
}

fn emit(emitter: &Option<ProgressEmitter>, phase: Phase, message: Option<String>) {
    if let Some(emitter) = emitter {
        emitter.emit(phase, 0, None, message);
    }
}

fn success_result(tmp: &Path) -> ProtonUpInstallResult {
    ProtonUpInstallResult {
        success: true,
        installed_path: Some(tmp.join("installed").to_string_lossy().into_owned()),
        error_kind: None,
        error_message: None,
    }
}

impl ProtonInstaller for FakeInstaller {
    fn install<'a>(
        &'a self,
        _request: &'a ProtonUpInstallRequest,
        version: &'a ProtonUpAvailableVersion,
        emitter: Option<ProgressEmitter>,
        cancel: Option<CancellationToken>,
    ) -> BoxFuture<'a, Result<ProtonUpInstallResult, InstallError>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(version.clone());
        Box::pin(async move {
            match &self.mode {
                Mode::Succeed => {
                    emit(&emitter, Phase::Resolving, None);
                    emit(&emitter, Phase::Done, None);
                    Ok(success_result(&self.tmp))
                }
                Mode::FailNoTerminal => Err(InstallError::Unknown(
                    self.tmp.to_string_lossy().into_owned(),
                )),
                Mode::FailWithPathMessage => {
                    let message = format!("failed at {}", self.tmp.display());
                    emit(&emitter, Phase::Failed, Some(message.clone()));
                    Err(InstallError::Unknown(message))
                }
                Mode::Gated(notify) => {
                    emit(&emitter, Phase::Resolving, None);
                    let cancelled = async {
                        match &cancel {
                            Some(token) => token.cancelled().await,
                            None => std::future::pending().await,
                        }
                    };
                    tokio::select! {
                        () = notify.notified() => {
                            emit(&emitter, Phase::Done, None);
                            Ok(success_result(&self.tmp))
                        }
                        () = cancelled => {
                            emit(&emitter, Phase::Cancelled, None);
                            Err(InstallError::Cancelled)
                        }
                    }
                }
            }
        })
    }
}

/// Root source fake with fixed trusted Steam roots and install candidates.
pub(super) struct FakeRoots {
    steam: Vec<PathBuf>,
    installs: Vec<InstallRootCandidate>,
}

impl FakeRoots {
    pub(super) fn new(steam: Vec<PathBuf>, installs: Vec<InstallRootCandidate>) -> Self {
        Self { steam, installs }
    }
}

impl ProtonRootSource for FakeRoots {
    fn steam_roots(&self) -> BoxFuture<'_, Vec<PathBuf>> {
        Box::pin(async move { self.steam.clone() })
    }

    fn install_roots(&self, _steam_root: Option<&Path>) -> Vec<InstallRootCandidate> {
        self.installs.clone()
    }
}

/// Writable native-Steam install root candidate at `path`.
pub(super) fn native_root(path: PathBuf) -> InstallRootCandidate {
    InstallRootCandidate {
        kind: InstallRootKind::NativeSteam,
        path,
        writable: true,
        reason: None,
    }
}

/// A service over tempdir stores and fakes, with handles to every fake.
pub(super) struct Harness {
    /// Keeps the tempdir alive for the harness lifetime.
    pub root: TempDir,
    pub service: ProtonUpService,
    pub jobs: JobRegistry,
    pub settings: SettingsStore,
    pub catalog: Arc<FakeCatalog>,
    pub installer: Arc<FakeInstaller>,
}

impl Harness {
    /// Harness with [`ge_catalog`], an installer in `mode` and a root source
    /// trusting `<root>/Steam` with its `compatibilitytools.d` as install root.
    pub(super) fn new(mode: Mode) -> Self {
        Self::build(mode, None)
    }

    /// Like [`Harness::new`] but with `roots` as the root source (built from
    /// the harness tempdir by `roots`).
    pub(super) fn with_roots(mode: Mode, roots: impl FnOnce(&Path) -> FakeRoots + 'static) -> Self {
        Self::build(mode, Some(Box::new(roots)))
    }

    fn build(mode: Mode, roots: Option<Box<dyn FnOnce(&Path) -> FakeRoots>>) -> Self {
        let root = tempfile::tempdir().unwrap();
        let settings = SettingsStore::with_base_path(root.path().join("config"));
        let metadata = MetadataStore::open_in_memory().unwrap();
        let jobs = JobRegistry::default();
        let steam = SteamService::new(metadata, settings.clone());
        let catalog = Arc::new(FakeCatalog::new(ge_catalog()));
        let installer = Arc::new(FakeInstaller::new(mode, root.path().to_path_buf()));
        let steam_root = root.path().join("Steam");
        fs::create_dir_all(steam_root.join("steamapps")).unwrap();
        let roots = match roots {
            Some(build) => build(root.path()),
            None => FakeRoots::new(
                vec![steam_root.clone()],
                vec![native_root(steam_root.join("compatibilitytools.d"))],
            ),
        };
        let service = ProtonUpService::with_seams(
            settings.clone(),
            steam,
            jobs.clone(),
            catalog.clone(),
            installer.clone(),
            Arc::new(roots),
        );
        Self {
            root,
            service,
            jobs,
            settings,
            catalog,
            installer,
        }
    }

    /// Turns offline mode on in the harness settings.
    pub(super) fn with_offline(self) -> Self {
        let mut settings = self.settings.load().unwrap();
        settings.offline_mode = true;
        self.settings.save(&settings).unwrap();
        self
    }

    /// `<root>/Steam`, which contains `steamapps/`.
    pub(super) fn steam_root(&self) -> PathBuf {
        let path = self.root.path().join("Steam");
        fs::create_dir_all(path.join("steamapps")).unwrap();
        path
    }

    /// Creates `<root>/Steam/compatibilitytools.d/<name>/proton` and returns
    /// the path of that `proton` file (its parent is the tool directory).
    pub(super) fn make_tool(&self, name: &str) -> PathBuf {
        let dir = self.steam_root().join("compatibilitytools.d").join(name);
        fs::create_dir_all(&dir).unwrap();
        let proton = dir.join("proton");
        fs::write(&proton, b"#!/bin/sh\n").unwrap();
        proton
    }

    /// Asserts a request was rejected before anything was allocated: no
    /// installer call, no job and no owner-ledger entry.
    pub(super) fn assert_rejected(&self) {
        assert_eq!(self.installer.calls.load(Ordering::SeqCst), 0);
        assert_eq!(self.jobs.job_counts(), (0, 0));
        assert_eq!(self.service.owner_count(), 0);
    }
}

/// A catalog with one `GE-Proton9-21` entry (github.com URLs, sha512 sidecar).
pub(super) fn ge_catalog() -> ProtonUpCatalogResponse {
    ProtonUpCatalogResponse {
        versions: vec![ProtonUpAvailableVersion {
            provider: "ge-proton".to_string(),
            version: "GE-Proton9-21".to_string(),
            release_url: Some(
                "https://github.com/GloriousEggroll/proton-ge-custom/releases/tag/GE-Proton9-21"
                    .to_string(),
            ),
            download_url: Some(
                "https://github.com/GloriousEggroll/proton-ge-custom/releases/download/GE-Proton9-21/GE-Proton9-21.tar.gz"
                    .to_string(),
            ),
            checksum_url: Some(
                "https://github.com/GloriousEggroll/proton-ge-custom/releases/download/GE-Proton9-21/GE-Proton9-21.sha512sum"
                    .to_string(),
            ),
            checksum_kind: Some("sha512".to_string()),
            asset_size: Some(123_456),
            published_at: Some("2024-11-01T00:00:00Z".to_string()),
        }],
        cache: ProtonUpCacheMeta::default(),
    }
}

/// An install request for `version` from the `ge-proton` provider.
pub(super) fn request(version: &str) -> ProtonUpInstallRequest {
    ProtonUpInstallRequest {
        provider: "ge-proton".to_string(),
        version: version.to_string(),
        target_root: String::new(),
        force: false,
    }
}

/// An [`InstallOwner`] for `label`.
pub(super) fn owner(label: &str) -> InstallOwner {
    InstallOwner::new(label)
}

/// Waits until every job has been finished and reaped (`job_counts() == (0, 0)`).
///
/// Yields to the scheduler instead of sleeping; panics after 10 seconds.
pub(super) async fn finished(jobs: &JobRegistry) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while jobs.job_counts() != (0, 0) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("jobs did not finish and reap within 10s");
}
