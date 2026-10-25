//! Environment-variable indirection for platform code.
//!
//! Defines [`EnvSink`] — a read/write abstraction over the process environment
//! — and [`SystemEnv`], the production implementation backed by
//! `std::env`. Unit tests can supply an in-memory fake instead of touching
//! the real process environment.

use std::ffi::OsString;

/// Indirection for env-var access so unit tests can observe writes and inject
/// reads without mutating the real process environment.
pub(crate) trait EnvSink {
    /// Write an environment variable.
    fn set(&mut self, key: &str, value: &OsString);
    /// Read an environment variable. Returns `None` when the variable is unset.
    fn get(&self, key: &str) -> Option<OsString>;
}

pub(crate) struct SystemEnv;

impl EnvSink for SystemEnv {
    fn set(&mut self, key: &str, value: &OsString) {
        super::trace::env_mutation(key);
        // SAFETY: called by `crosshook_app::bootstrap::pre_runtime` after its
        // single-thread check, before any runtime or UI builder. Unit tests
        // use a mock EnvSink and never mutate the real process environment.
        unsafe { std::env::set_var(key, value) };
    }

    fn get(&self, key: &str) -> Option<OsString> {
        std::env::var_os(key)
    }
}
