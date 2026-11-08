//! Frozen IPC fallbacks: results where an `Unavailable` service error is a
//! normal answer on the wire.

use crate::{AppError, AppErrorKind};

/// Maps `Unavailable` to `Ok(fallback)` and passes every other outcome
/// through. Used where the frozen contract treats "nothing available" as an
/// empty value rather than an error (no usable cover art, no bundled presets
/// while the metadata store is down).
///
/// # Errors
/// Any error other than `Unavailable`, unchanged.
pub fn unavailable_or<T>(result: Result<T, AppError>, fallback: T) -> Result<T, AppError> {
    match result {
        Err(error) if error.kind() == AppErrorKind::Unavailable => Ok(fallback),
        other => other,
    }
}

/// Maps a cancel result to the frozen IPC `bool`: accepted cancel -> `true`;
/// `NotFound` (unknown, foreign or already-finished operation) -> `false`.
///
/// # Errors
/// Any error other than `NotFound`, unchanged.
pub fn cancel_accepted(result: Result<(), AppError>) -> Result<bool, AppError> {
    match result {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == AppErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests;
