//! Game entry point: prepare once, dispatch to a core builder, then run the session.

use std::process::Stdio;

use crosshook_core::launch::script_runner::{
    build_helper_command, build_native_game_command, build_proton_game_command,
};
use crosshook_core::launch::{
    LaunchRequest, SessionKind, METHOD_NATIVE, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
};
use crosshook_core::runtime_helpers::RuntimeHelper;

use super::{LaunchService, LaunchSession, LaunchStartError};
use crate::{AppError, AppErrorKind};

impl LaunchService {
    /// Starts a game and returns its session-scoped event stream.
    ///
    /// # Errors
    /// Returns validation, helper resolution, command construction or spawn failures.
    pub async fn start_game(
        &self,
        request: LaunchRequest,
    ) -> Result<LaunchSession, LaunchStartError> {
        let prepared = self.prepare(request, SessionKind::Game).await?;
        let command = match prepared.execution_method {
            METHOD_STEAM_APPLAUNCH => {
                let helper = self.resolve_helper(RuntimeHelper::SteamLaunchHelper)?;
                build_helper_command(&prepared.request, &helper, &prepared.log_path).map_err(
                    |error| {
                        AppError::new(
                            AppErrorKind::Io,
                            format!("failed to build Steam helper launch: {error}"),
                        )
                    },
                )?
            }
            METHOD_PROTON_RUN => {
                let mut command = build_proton_game_command(&prepared.request, &prepared.log_path)
                    .map_err(|error| {
                        AppError::new(
                            AppErrorKind::Io,
                            format!("failed to build Proton game launch: {error}"),
                        )
                    })?;
                command.stdout(Stdio::piped()).stderr(Stdio::piped());
                command
            }
            METHOD_NATIVE => build_native_game_command(&prepared.request, &prepared.log_path)
                .map_err(|error| {
                    AppError::new(
                        AppErrorKind::Io,
                        format!("failed to build native game launch: {error}"),
                    )
                })?,
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
