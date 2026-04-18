use std::collections::HashMap;
use std::env;
use std::ffi::OsString;

use super::super::xdg::EnvSink;

/// Test-only env key so we never mutate the real `FLATPAK_ID` variable.
pub(super) const TEST_ENV_KEY: &str = "CROSSHOOK_TEST_FLATPAK_ID";

/// Mutex that serialises all tests mutating `CROSSHOOK_TEST_FLATPAK_ID`.
static FLATPAK_ID_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Scoped env var override for testing.
///
/// Acquires `FLATPAK_ID_LOCK` for its lifetime so concurrent tests do not
/// race on the environment. Drop order is LIFO, so the lock is released
/// only after the original value has been restored.
pub(super) struct ScopedEnv {
    key: &'static str,
    original: Option<OsString>,
    _guard: std::sync::MutexGuard<'static, ()>,
}

impl ScopedEnv {
    pub(super) fn set(key: &'static str, value: &str) -> Self {
        let guard = FLATPAK_ID_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let original = env::var_os(key);
        // SAFETY: single-threaded access guaranteed by the mutex.
        unsafe { env::set_var(key, value) };
        Self {
            key,
            original,
            _guard: guard,
        }
    }

    pub(super) fn unset(key: &'static str) -> Self {
        let guard = FLATPAK_ID_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let original = env::var_os(key);
        // SAFETY: single-threaded access guaranteed by the mutex.
        unsafe { env::remove_var(key) };
        Self {
            key,
            original,
            _guard: guard,
        }
    }
}

impl Drop for ScopedEnv {
    fn drop(&mut self) {
        match &self.original {
            // SAFETY: mutex is still held; no other thread touches the key.
            Some(value) => unsafe { env::set_var(self.key, value) },
            None => unsafe { env::remove_var(self.key) },
        }
    }
}

/// In-memory `EnvSink` that records writes instead of mutating the real
/// process environment. Lets xdg-override tests run concurrently without
/// racing on `XDG_*_HOME`.
///
/// Pre-populate `reads` to inject env-var values that `get()` will return,
/// simulating `HOST_XDG_*_HOME` vars set by the Flatpak runtime.
#[derive(Default)]
pub(super) struct FakeEnv {
    pub(super) writes: Vec<(String, OsString)>,
    pub(super) reads: HashMap<String, OsString>,
}

impl EnvSink for FakeEnv {
    fn set(&mut self, key: &str, value: &OsString) {
        self.writes.push((key.to_string(), value.clone()));
    }

    fn get(&self, key: &str) -> Option<OsString> {
        self.reads.get(key).cloned()
    }
}
