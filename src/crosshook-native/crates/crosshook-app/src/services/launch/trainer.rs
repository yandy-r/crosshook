//! Trainer entry point, preserving the effective Proton path for Steam trainers.

use std::process::Stdio;

use crosshook_core::launch::script_runner::{
    build_flatpak_steam_trainer_command, build_proton_trainer_command, build_trainer_command,
};
use crosshook_core::launch::{
    LaunchRequest, SessionKind, METHOD_NATIVE, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
};
use crosshook_core::runtime_helpers::RuntimeHelper;

use super::{LaunchService, LaunchSession, LaunchStartError};
use crate::{AppError, AppErrorKind};

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
        let command = match prepared.request.resolved_method() {
            METHOD_STEAM_APPLAUNCH if prepared.execution_method == METHOD_PROTON_RUN => {
                let mut command =
                    build_flatpak_steam_trainer_command(&prepared.request, &prepared.log_path)
                        .map_err(|error| {
                            AppError::new(
                                AppErrorKind::Io,
                                format!("failed to build Flatpak Steam trainer launch: {error}"),
                            )
                        })?;
                command.stdout(Stdio::piped()).stderr(Stdio::piped());
                command
            }
            METHOD_STEAM_APPLAUNCH => {
                let helper = self.resolve_helper(RuntimeHelper::SteamLaunchTrainer)?;
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
                    build_proton_trainer_command(&prepared.request, &prepared.log_path).map_err(
                        |error| {
                            AppError::new(
                                AppErrorKind::Io,
                                format!("failed to build Proton trainer launch: {error}"),
                            )
                        },
                    )?;
                command.stdout(Stdio::piped()).stderr(Stdio::piped());
                command
            }
            METHOD_NATIVE => {
                return Err(AppError::new(
                    AppErrorKind::Io,
                    "native launch does not support trainer launch.",
                )
                .into());
            }
            other => {
                return Err(AppError::new(
                    AppErrorKind::Io,
                    format!("unsupported launch method: {other}"),
                )
                .into());
            }
        };
        self.run_session(prepared, command).await
    }
}
