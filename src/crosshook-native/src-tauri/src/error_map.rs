//! The two IPC error wire formats. Commands return either a plain
//! `Result<T, String>` carrying the sanitized [`AppError`] message, or (for
//! run-executable) the structured [`RunCommandError`] envelope.

use crosshook_app::services::profiles::ProfileCommit;
use crosshook_app::AppError;

pub(crate) use crosshook_app::ipc::process::run_executable::RunCommandError;

/// The plain IPC error string: the sanitized message, never a path or SQL.
pub(crate) fn message(error: AppError) -> String {
    error.message().to_string()
}

/// Maps a service result onto the plain `Result<T, String>` wire format.
pub(crate) fn ipc<T>(result: Result<T, AppError>) -> Result<T, String> {
    result.map_err(message)
}

/// Maps a profile mutation onto the plain wire format: the commit metadata
/// (revision id, advisory warnings) is dropped, keeping the unchanged IPC value.
pub(crate) fn commit<T>(result: Result<ProfileCommit<T>, AppError>) -> Result<T, String> {
    result.map(ProfileCommit::into_output).map_err(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_error_maps_to_its_plain_message() {
        let error = AppError::validation("updater_path", "updater path is required");
        assert_eq!(message(error.clone()), "updater path is required");
        assert_eq!(
            ipc::<()>(Err(error)),
            Err("updater path is required".to_string())
        );
        assert_eq!(ipc(Ok(7)), Ok(7));
    }
}
