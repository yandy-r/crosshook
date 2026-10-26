//! Shared pre-spawn launch steps (Task 3.1).
//!
//! Forces the session kind's flags, validates before any side effect, then
//! enriches the UMU GAMEID, collects warnings, and allocates the log file and
//! gamescope plan both entry points share.

use std::path::PathBuf;

use crosshook_core::launch::{
    validate, CoexistenceAdvisoryRecord, LaunchRequest, LaunchValidationIssue, SessionKind,
};

use super::gamescope::{execution_method, gamescope_plan, GamescopePlan};
use super::queries::enrich_umu_gameid;
use super::warnings::collect_launch_warnings;
use super::LaunchService;
use crate::jobs::create_log_path;
use crate::{AppError, AppErrorKind};

/// Validated launch plus the facts both entry points need before building a command.
pub(super) struct Prepared {
    /// Request with the kind's flags forced and the UMU GAMEID filled in.
    pub request: LaunchRequest,
    /// Game or trainer. Kind-specific registration lives in `run_session`.
    pub kind: SessionKind,
    /// Method the child actually executes with.
    pub execution_method: &'static str,
    /// Advisories collected before spawn. `run_session` appends pre-launch
    /// hook warnings after the command is built.
    pub warnings: Vec<LaunchValidationIssue>,
    /// Mod-coexistence subset carried into the diagnostic report.
    pub coexistence: Vec<CoexistenceAdvisoryRecord>,
    /// Log file created for this launch.
    pub log_path: PathBuf,
    /// Gamescope predicate evaluated once for [`Self::log_path`].
    pub gamescope: GamescopePlan,
}

impl LaunchService {
    /// Runs the shared steps that happen before the command is built.
    ///
    /// The kind's flags are forced, then validation runs before UMU enrichment,
    /// warning collection, and log-file creation, so an invalid request has no
    /// side effects.
    ///
    /// # Errors
    /// A validation failure, or an I/O failure creating the log file.
    pub(super) async fn prepare(
        &self,
        mut request: LaunchRequest,
        kind: SessionKind,
    ) -> Result<Prepared, AppError> {
        force_kind_flags(&mut request, kind);
        // Before enrichment, warning DB writes, and log creation (H-4).
        validate(&request)?;

        enrich_umu_gameid(&self.settings, &self.metadata, &mut request).await;
        let collected = collect_launch_warnings(&request, &self.profiles, &self.metadata).await;
        let env = self.launch_env();
        let method = execution_method(&request, kind, env);
        let log_path = create_log_path(log_prefix(kind), &request.log_target_slug())
            .map_err(|error| AppError::new(AppErrorKind::Io, error))?;
        let gamescope = gamescope_plan(&request, kind, env, &log_path);

        Ok(Prepared {
            request,
            kind,
            execution_method: method,
            warnings: collected.issues,
            coexistence: collected.coexistence,
            log_path,
            gamescope,
        })
    }
}

/// `launch_game_only` / `launch_trainer_only` change validation, hooks, and
/// helper arguments, so they are forced before `validate`.
fn force_kind_flags(request: &mut LaunchRequest, kind: SessionKind) {
    match kind {
        SessionKind::Game => {
            request.launch_game_only = true;
            request.launch_trainer_only = false;
        }
        SessionKind::Trainer => {
            request.launch_trainer_only = true;
            request.launch_game_only = false;
        }
    }
}

fn log_prefix(kind: SessionKind) -> &'static str {
    match kind {
        SessionKind::Game => "game",
        SessionKind::Trainer => "trainer",
    }
}
