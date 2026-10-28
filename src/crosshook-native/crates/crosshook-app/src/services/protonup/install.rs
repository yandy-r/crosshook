//! Proton installs: the legacy synchronous entry point and the supervised
//! `protonup-install` job with relayed, sanitized progress.

use std::sync::Arc;

use crosshook_core::protonup::install::InstallError;
use crosshook_core::protonup::progress::{Phase, ProgressEmitter, ProtonInstallProgress};
use crosshook_core::protonup::{
    ProtonUpAvailableVersion, ProtonUpInstallErrorKind, ProtonUpInstallRequest,
    ProtonUpInstallResult,
};
use tokio::sync::{broadcast, oneshot};

use super::identity::verify_install_identity;
use super::messages::{
    identity_message, install_error_result, MSG_INSTALL_CANCELLED, MSG_INSTALL_FAILED,
    MSG_INSTALL_IN_PROGRESS,
};
use super::owners::OwnerLedger;
use super::progress::forward_progress;
use super::seams::ProtonInstaller;
use super::{InstallOwner, ProtonInstallStart, ProtonUpService, PROTONUP_INSTALL_JOB_KIND};
use crate::jobs::{JobContext, JobKey, JobSpec, Outcome};
use crate::{AppError, AppErrorKind};

/// How an install attempt ended, as far as progress and the job outcome care.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum End {
    Done,
    Cancelled,
    Failed,
}

impl ProtonUpService {
    /// Installs a catalog version and waits for it to finish (legacy
    /// synchronous entry point; no progress, not cancellable).
    ///
    /// Every failure is reported in the returned result with a fixed,
    /// path-free message; the installed entry is always the catalog's.
    ///
    /// # Errors
    /// Reserved for service-level failures; none occur today.
    pub async fn install_version(
        &self,
        request: ProtonUpInstallRequest,
    ) -> Result<ProtonUpInstallResult, AppError> {
        let flags = self.flags().await;
        if let Some(text) = flags.network_block() {
            return Ok(failure_result(
                ProtonUpInstallErrorKind::NetworkError,
                text.to_string(),
            ));
        }
        let catalog = self
            .catalog
            .versions(&request.provider, false, flags.include_prereleases)
            .await;
        let Ok(entry) = verify_install_identity(&request, None, &catalog.versions) else {
            return Ok(failure_result(
                ProtonUpInstallErrorKind::DependencyMissing,
                format!("version {} not found in catalog", request.version),
            ));
        };
        // The same job key as `start_install`, so the two entry points can
        // never install one version (and share its temp archive) at once.
        let entry = entry.clone();
        let installer = Arc::clone(&self.installer);
        let (result_tx, result_rx) = oneshot::channel();
        let spec = JobSpec::new(PROTONUP_INSTALL_JOB_KIND).with_key(install_key(&entry));
        let job_request = request;
        let handle = match self.jobs.start(spec, move |_ctx| async move {
            let result = installer.install(&job_request, &entry, None, None).await;
            let outcome = match &result {
                Ok(done) if done.success => Outcome::Succeeded,
                Ok(_) | Err(_) => {
                    Outcome::Failed(AppError::new(AppErrorKind::Io, MSG_INSTALL_FAILED))
                }
            };
            let _ = result_tx.send(result);
            outcome
        }) {
            Ok(handle) => handle,
            Err(error) if error.kind() == AppErrorKind::Conflict => {
                return Ok(failure_result(
                    ProtonUpInstallErrorKind::AlreadyInstalled,
                    MSG_INSTALL_IN_PROGRESS.to_string(),
                ));
            }
            Err(error) => return Err(error),
        };
        let id = handle.id().clone();
        let mut events = handle.into_events();
        while events.recv().await.is_some() {}
        if let Err(error) = self.jobs.reap(&id) {
            tracing::debug!(%error, "legacy install job was already reaped");
        }
        match result_rx.await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(error)) => {
                tracing::warn!(%error, "proton install failed");
                Ok(install_error_result(&error))
            }
            Err(_) => Ok(failure_result(
                ProtonUpInstallErrorKind::Unknown,
                MSG_INSTALL_FAILED.to_string(),
            )),
        }
    }

    /// Starts an install as a supervised job and returns its `op_id` and a
    /// progress receiver.
    ///
    /// `version` must be exactly the catalog's entry (download URL, checksum
    /// URL and checksum kind); the job installs the catalog entry, never
    /// `version`. Nothing is started or registered when any check fails.
    ///
    /// # Errors
    /// - `Unavailable` in offline mode, or when the job could not start.
    /// - `Validation` (field `version`) when the request is not in the
    ///   catalog or `version` differs from the catalog entry.
    /// - `Conflict` when the same provider version is already installing.
    pub async fn start_install(
        &self,
        owner: InstallOwner,
        request: ProtonUpInstallRequest,
        version: ProtonUpAvailableVersion,
    ) -> Result<ProtonInstallStart, AppError> {
        let flags = self.flags().await;
        if let Some(text) = flags.network_block() {
            return Err(AppError::new(AppErrorKind::Unavailable, text));
        }
        let catalog = self
            .catalog
            .versions(&request.provider, false, flags.include_prereleases)
            .await;
        let entry = verify_install_identity(&request, Some(&version), &catalog.versions)
            .map_err(|error| AppError::validation("version", identity_message(error, &request)))?
            .clone();

        let spec = JobSpec::new(PROTONUP_INSTALL_JOB_KIND).with_key(install_key(&entry));
        let (started_tx, started_rx) = oneshot::channel();
        let owners = self.owners.clone();
        let installer = Arc::clone(&self.installer);
        let handle = self.jobs.start(spec, move |ctx| {
            run_install(
                ctx,
                Job {
                    owners,
                    installer,
                    owner,
                    request,
                    entry,
                },
                started_tx,
            )
        })?;

        let op_id = handle.id().clone();
        let mut events = handle.into_events();
        let jobs = self.jobs.clone();
        let reap_id = op_id.clone();
        drop(tokio::spawn(async move {
            while events.recv().await.is_some() {}
            if let Err(error) = jobs.reap(&reap_id) {
                tracing::debug!(%error, "install job was already reaped");
            }
        }));

        let progress = started_rx.await.map_err(|_| {
            AppError::new(AppErrorKind::Unavailable, "install could not be started")
        })?;
        Ok(ProtonInstallStart {
            op_id: op_id.to_string(),
            progress,
        })
    }
}

/// Everything the install job body owns.
struct Job {
    owners: OwnerLedger,
    installer: Arc<dyn ProtonInstaller>,
    owner: InstallOwner,
    request: ProtonUpInstallRequest,
    entry: ProtonUpAvailableVersion,
}

type ProgressReceiver = broadcast::Receiver<ProtonInstallProgress>;

/// Job body: registers the owner, relays sanitized progress and runs the
/// installer.
async fn run_install(
    ctx: JobContext,
    job: Job,
    started: oneshot::Sender<ProgressReceiver>,
) -> Outcome {
    let id = ctx.id().clone();
    // Registered before `start_install` can observe the start, so the owner
    // can cancel as soon as it holds the op id; removed when the job ends.
    let _guard = job.owners.register(id.clone(), job.owner);
    let (core_emitter, core_rx) = ProgressEmitter::new(id.as_str());
    let (out_emitter, out_rx) = ProgressEmitter::new(id.as_str());
    if started.send(out_rx).is_err() {
        // The caller stopped waiting; nobody can observe or cancel this install.
        return Outcome::Cancelled;
    }
    if ctx.is_cancelled() {
        out_emitter.emit(
            Phase::Cancelled,
            0,
            None,
            Some(MSG_INSTALL_CANCELLED.to_string()),
        );
        return Outcome::Cancelled;
    }

    let (result, terminal) = tokio::join!(
        job.installer.install(
            &job.request,
            &job.entry,
            Some(core_emitter),
            Some(ctx.cancel_token().clone()),
        ),
        forward_progress(core_rx, out_emitter.clone()),
    );

    let end = match &result {
        Ok(done) if done.success => End::Done,
        Ok(done) if done.error_kind == Some(ProtonUpInstallErrorKind::Cancelled) => End::Cancelled,
        Ok(_) => End::Failed,
        Err(InstallError::Cancelled) => End::Cancelled,
        Err(_) => End::Failed,
    };
    match &result {
        Ok(done) if !done.success => {
            tracing::warn!(kind = ?done.error_kind, "proton install did not succeed");
        }
        Err(error) if end == End::Failed => tracing::warn!(%error, "proton install failed"),
        _ => {}
    }
    if !terminal {
        // The UI must never be left on a non-terminal phase.
        match end {
            End::Done => {}
            End::Cancelled => {
                out_emitter.emit(
                    Phase::Cancelled,
                    0,
                    None,
                    Some(MSG_INSTALL_CANCELLED.to_string()),
                );
            }
            End::Failed => {
                out_emitter.emit(Phase::Failed, 0, None, Some(MSG_INSTALL_FAILED.to_string()));
            }
        }
    }
    match end {
        End::Done => Outcome::Succeeded,
        End::Cancelled => Outcome::Cancelled,
        End::Failed => Outcome::Failed(AppError::new(AppErrorKind::Io, MSG_INSTALL_FAILED)),
    }
}

/// `protonup-install:<provider>:<version>`, shared by both install entry points.
fn install_key(entry: &ProtonUpAvailableVersion) -> JobKey {
    JobKey::new(format!(
        "{PROTONUP_INSTALL_JOB_KIND}:{}:{}",
        entry.provider, entry.version
    ))
}

fn failure_result(kind: ProtonUpInstallErrorKind, message: String) -> ProtonUpInstallResult {
    ProtonUpInstallResult {
        success: false,
        installed_path: None,
        error_kind: Some(kind),
        error_message: Some(message),
    }
}
