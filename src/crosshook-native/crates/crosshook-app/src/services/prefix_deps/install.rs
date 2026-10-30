//! The dependency install job: pre-install recovery, record-before-mutate,
//! the supervised install child, the restore (skipped on app shutdown) and
//! the journal resolution, all under the prefix key (I6).

use std::path::Path;

use crosshook_core::metadata::MetadataStore;
use crosshook_core::prefix_deps::runner::{prepare_install_packages, sanitize_output_for_ui};
use crosshook_core::prefix_deps::validation::validate_protontricks_verbs;
use crosshook_core::prefix_deps::{
    PrefixDependencyInstallOutcome, PrefixVersionRestoreOutcome, PrefixVersionRestorePlan,
    PrefixVersionRestoreState,
};
use tokio::process::Command;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use super::journal::JournalTicket;
use super::messages::{deps_error, msg_already_installing, MSG_OFFLINE};
use super::tools::ResolvedTool;
use super::types::{completion_payload, DepInstallCompletion};
use super::{PrefixDepsService, PREFIX_DEPS_INSTALL_JOB_KIND};
use crate::jobs::{create_log_file_in, slugify_target, ChildExit, JobContext, JobSpec, Outcome};
use crate::log_tail::LogBatch;
use crate::services::process::child::{run_child, ChildError, ChildOutput, LogFile};
use crate::services::process::run::start_run;
use crate::services::process::{
    conflict_as, prefix_job_key, prefix_job_key_for_path, remove_unused_log, ProcessRun,
};
use crate::services::run_blocking;
use crate::{AppError, AppErrorKind};

/// Everything the job needs, validated before the job starts.
struct InstallJob {
    service: PrefixDepsService,
    profile_name: String,
    prefix_path: String,
    canonical: String,
    packages: Vec<String>,
    tool: ResolvedTool,
    log: LogFile,
}

impl PrefixDepsService {
    /// Starts installing `packages` into `prefix_path` as a
    /// `prefix-deps-install` job holding the prefix key.
    ///
    /// Offline mode, tool selection, verb validation, prefix normalization and
    /// log creation fail before any job. Inside the job, in order: pre-install
    /// recovery of a `pending` row, the profile id lookup, prepare (captures
    /// the restore plan), the durable `pending` row (I1), the install child
    /// (piped, sanitized lines; `started` once it spawned), the restore on its
    /// own timeout (also after a failed install, I8), the dependency states,
    /// the journal resolution (I2/I3) and the completion. A failure before
    /// the child spawned (spawn failure included) is returned here and leaves
    /// a recorded row `pending`. A cancelled install (only app shutdown
    /// cancels it) skips the restore, leaves the row `pending` for startup
    /// recovery and ends `Cancelled` with its completion still sent
    /// (`restore_state` pending).
    ///
    /// # Errors
    /// `Unavailable` [`MSG_OFFLINE`] in offline mode or without a tool,
    /// `Validation` for the App ID, verbs or prefix, `Conflict`
    /// ([`msg_already_installing`]) while another job holds the prefix,
    /// `Conflict` [`MSG_REPAIR_REQUIRED`](super::MSG_REPAIR_REQUIRED) when
    /// the prefix still needs repair, `NotFound` for an unindexed profile,
    /// store errors, and `Io` when the install tool cannot be spawned.
    pub async fn start_install(
        &self,
        profile_name: String,
        prefix_path: String,
        packages: Vec<String>,
    ) -> Result<ProcessRun<DepInstallCompletion>, AppError> {
        let service = self.clone();
        let job =
            run_blocking(move || service.prepare_job(profile_name, prefix_path, packages)).await?;
        let canonical = job.canonical.clone();
        let log_path = job.log.path.clone();
        let key = prefix_job_key_for_path(Path::new(&canonical));
        let spec = JobSpec::new(PREFIX_DEPS_INSTALL_JOB_KIND).with_key(key);
        let started = match start_run(&self.jobs, spec, move |ctx, completion, started| {
            run_install(ctx, completion, started, job)
        }) {
            Ok(starting) => starting.started().await,
            Err(error) => Err(conflict_as(error, &msg_already_installing(&canonical))),
        };
        if started.is_err() {
            // Nothing ran: drop the empty log this attempt created.
            remove_unused_log(&log_path);
        }
        started
    }

    /// The synchronous checks of [`start_install`](Self::start_install), in
    /// order: offline gate first (D12). Blocking.
    fn prepare_job(
        self,
        profile_name: String,
        prefix_path: String,
        packages: Vec<String>,
    ) -> Result<InstallJob, AppError> {
        let settings = self.settings.load()?;
        if settings.offline_mode {
            return Err(AppError::new(AppErrorKind::Unavailable, MSG_OFFLINE));
        }
        let tool = self.resolve_profile_tool(&settings, &profile_name, false)?;
        validate_protontricks_verbs(&packages).map_err(|error| deps_error("packages", error))?;
        let (canonical, _) = prefix_job_key(&prefix_path)?;
        let (path, file) = create_log_file_in(
            self.log_dir.clone(),
            "prefix-deps",
            &slugify_target(&profile_name, "prefix-deps"),
        )
        .map_err(|message| AppError::new(AppErrorKind::Io, message))?;
        Ok(InstallJob {
            service: self,
            profile_name,
            prefix_path,
            canonical,
            packages,
            tool,
            log: LogFile { path, file },
        })
    }
}

/// The job body.
async fn run_install(
    ctx: JobContext,
    completion: oneshot::Sender<DepInstallCompletion>,
    started: oneshot::Sender<()>,
    job: InstallJob,
) -> Outcome {
    let InstallJob {
        service,
        profile_name,
        prefix_path,
        canonical,
        packages,
        tool,
        log,
    } = job;
    let journal = service.journal.clone();
    if let Err(error) = journal
        .recover_before_install(&canonical, &service.recoveries, ctx.cancel_token())
        .await
    {
        // Cancelled (app shutdown) before the recovery restore started.
        if error.kind() == AppErrorKind::Cancelled {
            return Outcome::Cancelled;
        }
        return Outcome::Failed(error);
    }
    if ctx.is_cancelled() {
        return Outcome::Cancelled;
    }
    let prepare_journal = journal.clone();
    let metadata = service.metadata.clone();
    let prepare_args = (profile_name.clone(), prefix_path.clone(), packages.clone());
    let prepared = run_blocking(move || {
        let (profile_name, prefix_path, packages) = prepare_args;
        let profile_id = metadata.lookup_profile_id(&profile_name)?.ok_or_else(|| {
            AppError::new(
                AppErrorKind::NotFound,
                format!("profile is not indexed in metadata: {profile_name}"),
            )
        })?;
        let (command, plan) = prepare_install_packages(
            &tool.binary_path,
            &prefix_path,
            &packages,
            tool.tool,
            tool.steam_app_id.as_deref(),
        )
        .map_err(|error| deps_error("prefix_path", error))?
        .into_parts();
        // I1: the pending row is durable before the child spawns.
        let ticket = plan
            .as_ref()
            .map(|plan| prepare_journal.record(plan, &profile_id))
            .transpose()?;
        Ok((profile_id, command, plan.zip(ticket)))
    })
    .await;
    let (profile_id, command, restore) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => return Outcome::Failed(error),
    };

    let exit = match run_install_child(&ctx, command, log, started).await {
        // Nothing ran: the error becomes the caller's, and a recorded
        // `pending` row stays for pre-install or startup recovery (I1).
        Err(ChildError::Cancelled) => return Outcome::Cancelled,
        Err(ChildError::Spawn(error)) => {
            return Outcome::Failed(AppError::new(
                AppErrorKind::Io,
                sanitize_output_for_ui(&format!("failed to spawn install process: {error}")),
            ));
        }
        Err(ChildError::Supervise(error)) => Err(error),
        Ok(exit) => Ok(exit),
    };
    // `Err`: supervision broke, so the child's fate is unknown. The `pending`
    // row stays for recovery (fail closed); nothing is restored meanwhile.
    let ran = exit.is_ok();
    // Only app shutdown cancels the job token: a restore started now would
    // outlive the process (`App::shutdown` is bounded), so the `pending` row
    // stays for startup recovery instead.
    let cancelled = ctx.is_cancelled();
    let mut outcome = install_outcome(exit);
    let restored = match restore {
        Some(restore) if ran && !cancelled => finish_restore(&service, restore).await,
        Some((_, ticket)) => {
            drop(ticket);
            PrefixVersionRestoreOutcome {
                state: PrefixVersionRestoreState::Pending,
                error: None,
            }
        }
        None => PrefixVersionRestoreOutcome {
            state: PrefixVersionRestoreState::NotRequired,
            error: None,
        },
    };
    outcome.restore_state = restored.state;
    outcome.restore_error = restored.error;

    persist_dep_states(
        &service.metadata,
        &profile_id,
        &prefix_path,
        &packages,
        &outcome,
    )
    .await;
    let job_outcome = job_outcome(&profile_name, &prefix_path, &outcome, cancelled);
    let _ = completion.send(DepInstallCompletion { outcome });
    job_outcome
}

/// Runs the install child; output lines are sanitized before they reach the
/// job's subscribers. `started` is sent once the child spawned. The command
/// has no stop: only the job token (TERM, grace, KILL) ends it early.
async fn run_install_child(
    ctx: &JobContext,
    command: Command,
    log: LogFile,
    started: oneshot::Sender<()>,
) -> Result<ChildExit, ChildError> {
    let sink_ctx = ctx.clone();
    run_child(
        ctx,
        command,
        log,
        ChildOutput::Piped,
        &CancellationToken::new(),
        started,
        move |batch| sink_ctx.log_batch(sanitize_batch(batch)),
    )
    .await
}

/// Sanitizes every line of `batch` (ANSI codes, absolute user paths).
pub(super) fn sanitize_batch(batch: LogBatch) -> LogBatch {
    match batch {
        LogBatch::Lines(lines) => LogBatch::Lines(
            lines
                .iter()
                .map(|line| sanitize_output_for_ui(line))
                .collect(),
        ),
        other => other,
    }
}

/// The install half of the outcome; the restore half starts `NotRequired`.
fn install_outcome(exit: std::io::Result<ChildExit>) -> PrefixDependencyInstallOutcome {
    let (install_succeeded, install_exit_code, install_error) = match exit {
        Ok(exit) if exit.was_cancelled() => (
            false,
            exit.status().code(),
            Some("dependency install was cancelled".to_string()),
        ),
        Ok(exit) => (exit.status().success(), exit.status().code(), None),
        Err(error) => (
            false,
            None,
            Some(sanitize_output_for_ui(&format!(
                "install process supervision failed: {error}"
            ))),
        ),
    };
    PrefixDependencyInstallOutcome {
        install_succeeded,
        install_exit_code,
        install_error,
        restore_state: PrefixVersionRestoreState::NotRequired,
        restore_error: None,
    }
}

/// Restores the captured version after the child exited, on the restorer's
/// own timeout rather than the job token (I8), then resolves the row.
async fn finish_restore(
    service: &PrefixDepsService,
    (plan, ticket): (PrefixVersionRestorePlan, JournalTicket),
) -> PrefixVersionRestoreOutcome {
    let restored = service.journal.restore(plan).await;
    let journal = service.journal.clone();
    let fallback = restored.clone();
    run_blocking(move || Ok(journal.resolve(ticket, restored)))
        .await
        .unwrap_or_else(|error| {
            tracing::warn!(%error, "prefix repair journal resolution did not run");
            PrefixVersionRestoreOutcome {
                state: PrefixVersionRestoreState::Failed,
                error: Some(
                    fallback
                        .error
                        .unwrap_or_else(|| error.message().to_string()),
                ),
            }
        })
}

/// Persists each package's install state (fail-soft), keyed on the raw
/// request prefix as before.
async fn persist_dep_states(
    metadata: &MetadataStore,
    profile_id: &str,
    prefix_path: &str,
    packages: &[String],
    outcome: &PrefixDependencyInstallOutcome,
) {
    let metadata = metadata.clone();
    let profile_id = profile_id.to_string();
    let prefix_path = prefix_path.to_string();
    let packages = packages.to_vec();
    let succeeded = outcome.install_succeeded;
    let error = outcome
        .install_error
        .clone()
        .unwrap_or_else(|| "install process failed".to_string());
    let persisted = run_blocking(move || {
        let state = if succeeded {
            "installed"
        } else {
            "install_failed"
        };
        let error = (!succeeded).then_some(error.as_str());
        for package in &packages {
            if let Err(error) =
                metadata.upsert_prefix_dep_state(&profile_id, package, &prefix_path, state, error)
            {
                tracing::warn!(%error, package, "failed to persist prefix dep state after install");
            }
        }
        Ok(())
    })
    .await;
    if let Err(error) = persisted {
        tracing::warn!(%error, "prefix dep states were not persisted");
    }
}

/// The job outcome: `Cancelled` after a cancel, else success only when the
/// install succeeded and the prefix version is safe.
fn job_outcome(
    profile_name: &str,
    prefix_path: &str,
    outcome: &PrefixDependencyInstallOutcome,
    cancelled: bool,
) -> Outcome {
    if cancelled {
        return Outcome::Cancelled;
    }
    let payload = completion_payload(profile_name, prefix_path, outcome.clone());
    if payload.succeeded {
        return Outcome::Succeeded;
    }
    let message = payload
        .restore_error
        .or_else(|| outcome.install_error.clone())
        .unwrap_or_else(|| "dependency install failed".to_string());
    Outcome::Failed(AppError::new(AppErrorKind::Io, message))
}
