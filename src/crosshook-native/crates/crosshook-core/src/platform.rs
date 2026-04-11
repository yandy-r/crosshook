//! Runtime platform detection for Flatpak sandboxing.
//!
//! CrossHook runs both as a native Linux binary (AppImage, dev build) and
//! inside a Flatpak sandbox. Several subsystems need to know which of the two
//! environments they are running in so they can adjust process spawning and
//! resource path resolution. This module is the single source of truth for
//! that decision.

use std::path::Path;

use tokio::process::Command;

const FLATPAK_ID_ENV: &str = "FLATPAK_ID";
const FLATPAK_INFO_PATH: &str = "/.flatpak-info";

/// Returns `true` when running inside a Flatpak sandbox.
///
/// Detection uses the two signals documented by the Flatpak runtime:
/// the `FLATPAK_ID` environment variable (set automatically by `flatpak run`)
/// and the presence of `/.flatpak-info` (always mounted inside the sandbox).
pub fn is_flatpak() -> bool {
    is_flatpak_with(FLATPAK_ID_ENV, Path::new(FLATPAK_INFO_PATH))
}

/// Creates a [`tokio::process::Command`] that executes on the host when
/// running inside a Flatpak sandbox, and as a normal child process otherwise.
///
/// Inside Flatpak the returned command is equivalent to
/// `flatpak-spawn --host <program>`; outside Flatpak it is `Command::new(program)`.
pub fn host_command(program: &str) -> Command {
    host_command_with(program, is_flatpak())
}

fn is_flatpak_with(env_key: &str, info_path: &Path) -> bool {
    std::env::var_os(env_key).is_some() || info_path.exists()
}

fn host_command_with(program: &str, flatpak: bool) -> Command {
    if flatpak {
        tracing::debug!(program, "wrapping command with flatpak-spawn --host");
        let mut cmd = Command::new("flatpak-spawn");
        cmd.arg("--host").arg(program);
        cmd
    } else {
        Command::new(program)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::ffi::OsString;
    use std::fs;

    use tempfile::tempdir;

    /// Test-only env key so we never mutate the real `FLATPAK_ID` variable.
    const TEST_ENV_KEY: &str = "CROSSHOOK_TEST_FLATPAK_ID";

    /// Mutex that serialises all tests mutating `CROSSHOOK_TEST_FLATPAK_ID`.
    static FLATPAK_ID_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Scoped env var override for testing.
    ///
    /// Acquires `FLATPAK_ID_LOCK` for its lifetime so concurrent tests do not
    /// race on the environment. Drop order is LIFO, so the lock is released
    /// only after the original value has been restored.
    struct ScopedEnv {
        key: &'static str,
        original: Option<OsString>,
        _guard: std::sync::MutexGuard<'static, ()>,
    }

    impl ScopedEnv {
        fn set(key: &'static str, value: &str) -> Self {
            let guard = FLATPAK_ID_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let original = env::var_os(key);
            // SAFETY: single-threaded access guaranteed by the mutex.
            unsafe { env::set_var(key, value) };
            Self {
                key,
                original,
                _guard: guard,
            }
        }

        fn unset(key: &'static str) -> Self {
            let guard = FLATPAK_ID_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
                Some(val) => unsafe { env::set_var(self.key, val) },
                None => unsafe { env::remove_var(self.key) },
            }
        }
    }

    #[test]
    fn returns_true_when_env_var_set_and_file_absent() {
        let _guard = ScopedEnv::set(TEST_ENV_KEY, "io.github.yandy-r.crosshook");
        let tmp = tempdir().unwrap();
        let missing = tmp.path().join("does-not-exist");
        assert!(is_flatpak_with(TEST_ENV_KEY, &missing));
    }

    #[test]
    fn returns_true_when_file_present_and_env_var_unset() {
        let _guard = ScopedEnv::unset(TEST_ENV_KEY);
        let tmp = tempdir().unwrap();
        let present = tmp.path().join(".flatpak-info");
        fs::write(&present, b"[Application]\nname=test\n").unwrap();
        assert!(is_flatpak_with(TEST_ENV_KEY, &present));
    }

    #[test]
    fn returns_true_when_both_present() {
        let _guard = ScopedEnv::set(TEST_ENV_KEY, "io.github.yandy-r.crosshook");
        let tmp = tempdir().unwrap();
        let present = tmp.path().join(".flatpak-info");
        fs::write(&present, b"[Application]\nname=test\n").unwrap();
        assert!(is_flatpak_with(TEST_ENV_KEY, &present));
    }

    #[test]
    fn returns_false_when_neither_present() {
        let _guard = ScopedEnv::unset(TEST_ENV_KEY);
        let tmp = tempdir().unwrap();
        let missing = tmp.path().join("does-not-exist");
        assert!(!is_flatpak_with(TEST_ENV_KEY, &missing));
    }

    #[test]
    fn host_command_wraps_program_when_flatpak() {
        let cmd = host_command_with("ls", true);
        let std_cmd = cmd.as_std();
        assert_eq!(std_cmd.get_program(), "flatpak-spawn");
        let args: Vec<&std::ffi::OsStr> = std_cmd.get_args().collect();
        assert_eq!(args, vec![std::ffi::OsStr::new("--host"), std::ffi::OsStr::new("ls")]);
    }

    #[test]
    fn host_command_passes_through_when_not_flatpak() {
        let cmd = host_command_with("ls", false);
        let std_cmd = cmd.as_std();
        assert_eq!(std_cmd.get_program(), "ls");
        assert_eq!(std_cmd.get_args().count(), 0);
    }
}
