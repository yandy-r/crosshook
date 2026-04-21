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
        // SAFETY: called once from `run()` before any threads spawn; the
        // Tauri Builder is not yet constructed, so there are no concurrent
        // readers of the environment. Unit tests exercise this through a
        // mock `EnvSink` and never touch the real env via this code path.
        unsafe { std::env::set_var(key, value) };
    }

    fn get(&self, key: &str) -> Option<OsString> {
        std::env::var_os(key)
    }
}
