//! Runtime platform detection for Flatpak sandboxing.
//!
//! CrossHook runs both as a native Linux binary (AppImage, dev build) and
//! inside a Flatpak sandbox. Several subsystems need to know which of the two
//! environments they are running in so they can adjust process spawning and
//! resource path resolution. This module is the single source of truth for
//! that decision.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

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

/// Redirects `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, and `XDG_CACHE_HOME` to the
/// host's default locations (`$HOME/.config`, `$HOME/.local/share`,
/// `$HOME/.cache`) when running inside a Flatpak sandbox.
///
/// Flatpak normally remaps these three variables to per-app directories under
/// `~/.var/app/<app-id>/`, which means `directories::BaseDirs::new()` — and
/// therefore every CrossHook store that derives its base path from it —
/// resolves to an empty sandbox location instead of the user's existing
/// `~/.config/crosshook/`, `~/.local/share/crosshook/`, and
/// `~/.cache/crosshook/`. The data is visible to the sandbox via
/// `--filesystem=home`; only the env var remap is hiding it.
///
/// For Phase 1 this function restores the default XDG paths so the Flatpak
/// build and the AppImage share the same data on disk. Called from the very
/// top of `crosshook_native::run()` before any store initializes.
///
/// Phase 4 (Flathub submission) will replace this with a proper per-app
/// isolation model and a first-run migration — see the tracking issue linked
/// from `docs/prps/prds/flatpak-distribution.prd.md` §10.2.
pub fn override_xdg_for_flatpak_host_access() {
    if !is_flatpak() {
        return;
    }
    let mut sink = SystemEnv;
    apply_xdg_host_override(std::env::var_os("HOME").map(PathBuf::from), &mut sink);
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

/// Indirection for `std::env::set_var` so unit tests can observe the writes
/// without mutating the real process environment.
trait EnvSink {
    fn set(&mut self, key: &str, value: &OsString);
}

struct SystemEnv;

impl EnvSink for SystemEnv {
    fn set(&mut self, key: &str, value: &OsString) {
        // SAFETY: called once from `run()` before any threads spawn; the
        // Tauri Builder is not yet constructed, so there are no concurrent
        // readers of the environment. Unit tests exercise this through a
        // mock `EnvSink` and never touch the real env via this code path.
        unsafe { std::env::set_var(key, value) };
    }
}

fn apply_xdg_host_override(home: Option<PathBuf>, sink: &mut dyn EnvSink) -> bool {
    let Some(home) = home else {
        tracing::warn!("xdg host override skipped: HOME is unset");
        return false;
    };

    let config = home.join(".config");
    let data = home.join(".local").join("share");
    let cache = home.join(".cache");

    sink.set("XDG_CONFIG_HOME", &config.into_os_string());
    sink.set("XDG_DATA_HOME", &data.into_os_string());
    sink.set("XDG_CACHE_HOME", &cache.into_os_string());
    tracing::info!(
        home = %home.display(),
        "xdg host override applied (flatpak → host $HOME/.config, .local/share, .cache)"
    );
    true
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
        let _guard = ScopedEnv::set(TEST_ENV_KEY, "dev.crosshook.CrossHook");
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
        let _guard = ScopedEnv::set(TEST_ENV_KEY, "dev.crosshook.CrossHook");
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

    /// In-memory `EnvSink` that records writes instead of mutating the real
    /// process environment. Lets xdg-override tests run concurrently without
    /// racing on `XDG_*_HOME`.
    #[derive(Default)]
    struct FakeEnv {
        writes: Vec<(String, OsString)>,
    }

    impl EnvSink for FakeEnv {
        fn set(&mut self, key: &str, value: &OsString) {
            self.writes.push((key.to_string(), value.clone()));
        }
    }

    #[test]
    fn xdg_override_sets_all_three_paths_from_home() {
        let mut env = FakeEnv::default();
        let applied = apply_xdg_host_override(Some(PathBuf::from("/home/alice")), &mut env);
        assert!(applied);
        assert_eq!(
            env.writes,
            vec![
                ("XDG_CONFIG_HOME".to_string(), OsString::from("/home/alice/.config")),
                ("XDG_DATA_HOME".to_string(), OsString::from("/home/alice/.local/share")),
                ("XDG_CACHE_HOME".to_string(), OsString::from("/home/alice/.cache")),
            ]
        );
    }

    #[test]
    fn xdg_override_noop_when_home_unset() {
        let mut env = FakeEnv::default();
        let applied = apply_xdg_host_override(None, &mut env);
        assert!(!applied);
        assert!(env.writes.is_empty());
    }

    #[test]
    fn xdg_override_preserves_trailing_slash_behavior() {
        let mut env = FakeEnv::default();
        apply_xdg_host_override(Some(PathBuf::from("/home/bob/")), &mut env);
        let (_, config) = &env.writes[0];
        // `Path::join` normalizes; the trailing slash is absorbed.
        assert_eq!(config, &OsString::from("/home/bob/.config"));
    }

    #[test]
    fn xdg_override_uses_exact_home_without_expansion() {
        // HOME may legitimately be something other than /home/<user>
        // (containers, per-user mount points, etc.) — honour it as-is.
        let mut env = FakeEnv::default();
        apply_xdg_host_override(Some(PathBuf::from("/var/home/charlie")), &mut env);
        assert_eq!(
            env.writes[0].1,
            OsString::from("/var/home/charlie/.config")
        );
    }
}
