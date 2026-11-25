//! Trainer entry point, preserving the effective Proton path for Steam trainers.

use std::borrow::Cow;
use std::process::Stdio;

use crosshook_core::launch::script_runner::{
    build_flatpak_steam_trainer_command, build_proton_trainer_command, build_trainer_command,
    flatpak_steam_trainer_request,
};
use crosshook_core::launch::{
    LaunchRequest, SessionKind, METHOD_NATIVE, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
};
use crosshook_core::runtime_helpers::RuntimeHelper;

use super::{run_blocking, LaunchService, LaunchSession, LaunchStartError};
use crate::{AppError, AppErrorKind};

/// Whether a `steam_applaunch` request executes through the direct Proton
/// builder: a Steam trainer inside Flatpak.
fn steam_runs_as_proton_run(request: &LaunchRequest, execution_method: &str) -> bool {
    request.resolved_method() == METHOD_STEAM_APPLAUNCH && execution_method == METHOD_PROTON_RUN
}

/// The request the session's process executes as, which its hooks share.
///
/// Called for every session kind; only a trainer can pair a `steam_applaunch`
/// request with a `proton_run` execution method, so games always borrow.
///
/// A Steam trainer inside Flatpak runs as [`flatpak_steam_trainer_request`],
/// so its pre-launch and post-exit hooks see the same `proton_run`
/// environment as the trainer. Every other session runs its own request.
pub(super) fn execution_request<'a>(
    request: &'a LaunchRequest,
    execution_method: &str,
) -> Cow<'a, LaunchRequest> {
    if steam_runs_as_proton_run(request, execution_method) {
        Cow::Owned(flatpak_steam_trainer_request(request))
    } else {
        Cow::Borrowed(request)
    }
}

impl LaunchService {
    /// Starts a trainer and returns its session-scoped event stream.
    ///
    /// Steam trainers use the direct Proton builder inside Flatpak and the
    /// Steam trainer helper on the host. Registration and atomic parent linking
    /// are shared with the game path in `run_session`.
    ///
    /// # Errors
    /// Returns validation, helper resolution, command construction or spawn failures.
    pub async fn start_trainer(
        &self,
        request: LaunchRequest,
    ) -> Result<LaunchSession, LaunchStartError> {
        let prepared = self.prepare(request, SessionKind::Trainer).await?;
        let helpers = self.helpers.clone();
        let (prepared, command) = run_blocking(move || {
            let command = match prepared.request.resolved_method() {
                METHOD_STEAM_APPLAUNCH
                    if steam_runs_as_proton_run(&prepared.request, prepared.execution_method) =>
                {
                    let mut command =
                        build_flatpak_steam_trainer_command(&prepared.request, &prepared.log_path)
                            .map_err(|error| {
                                AppError::new(
                                    AppErrorKind::Io,
                                    format!(
                                        "failed to build Flatpak Steam trainer launch: {error}"
                                    ),
                                )
                            })?;
                    command.stdout(Stdio::piped()).stderr(Stdio::piped());
                    command
                }
                METHOD_STEAM_APPLAUNCH => {
                    let helper =
                        Self::resolve_helper_with(&helpers, RuntimeHelper::SteamLaunchTrainer)?;
                    build_trainer_command(&prepared.request, &helper, &prepared.log_path).map_err(
                        |error| {
                            AppError::new(
                                AppErrorKind::Io,
                                format!("failed to build Steam trainer launch: {error}"),
                            )
                        },
                    )?
                }
                METHOD_PROTON_RUN => {
                    let mut command =
                        build_proton_trainer_command(&prepared.request, &prepared.log_path)
                            .map_err(|error| {
                                AppError::new(
                                    AppErrorKind::Io,
                                    format!("failed to build Proton trainer launch: {error}"),
                                )
                            })?;
                    command.stdout(Stdio::piped()).stderr(Stdio::piped());
                    command
                }
                METHOD_NATIVE => {
                    return Err(AppError::new(
                        AppErrorKind::Io,
                        "native launch does not support trainer launch.",
                    ));
                }
                other => {
                    return Err(AppError::new(
                        AppErrorKind::Io,
                        format!("unsupported launch method: {other}"),
                    ));
                }
            };
            Ok((prepared, command))
        })
        .await?;
        self.run_session(prepared, command).await
    }
}
