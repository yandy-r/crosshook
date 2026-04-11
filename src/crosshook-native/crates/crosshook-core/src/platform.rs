//! Runtime platform detection for Flatpak sandboxing.
//!
//! CrossHook runs both as a native Linux binary (AppImage, dev build) and
//! inside a Flatpak sandbox. Several subsystems need to know which of the two
//! environments they are running in so they can adjust process spawning and
//! resource path resolution. This module is the single source of truth for
//! that decision.

use std::collections::BTreeMap;
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
///
/// # Warning — env vars are silently dropped inside Flatpak
///
/// **Do NOT call `.env()` or `.envs()` on the `Command` returned by this
/// function when running inside Flatpak.** `flatpak-spawn --host` does not
/// forward env vars set on the `Command` object; they are silently ignored.
/// Any env vars set this way will be absent from the spawned process, causing
/// subtly wrong behaviour for Proton/Wine callers that depend on
/// `STEAM_COMPAT_*`, `WINEPREFIX`, `MANGOHUD_CONFIG`, etc.
///
/// **Use [`host_command_with_env`] instead**, which threads env vars through
/// `--env=KEY=VALUE` arguments automatically when inside Flatpak and falls
/// back to `.envs()` outside Flatpak.
pub fn host_command(program: &str) -> Command {
    host_command_with(program, is_flatpak())
}

/// Creates a [`tokio::process::Command`] that executes on the host when
/// running inside a Flatpak sandbox, and as a normal child process otherwise,
/// forwarding the given environment variables correctly in both cases.
///
/// Inside Flatpak, `flatpak-spawn --host` does not propagate env vars set via
/// `.env()` / `.envs()` on the `Command`. This helper threads every entry from
/// `envs` through explicit `--env=KEY=VALUE` arguments before the program name,
/// which is the only reliable way to pass env vars through `flatpak-spawn`.
/// Outside Flatpak the vars are forwarded normally via `.envs()`.
///
/// Phase 3 Proton/Wine callers should use this helper unconditionally so the
/// code behaves correctly in both AppImage and Flatpak deployments.
pub fn host_command_with_env(program: &str, envs: &BTreeMap<String, String>) -> Command {
    host_command_with_env_inner(program, envs, is_flatpak())
}

/// Redirects `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, and `XDG_CACHE_HOME` to the
/// host's real XDG locations when running inside a Flatpak sandbox, so the
/// Flatpak build and the AppImage share the same data on disk.
///
/// Flatpak normally remaps these three variables to per-app directories under
/// `~/.var/app/<app-id>/`, which means `directories::BaseDirs::new()` — and
/// therefore every CrossHook store that derives its base path from it —
/// resolves to an empty sandbox location instead of the user's existing
/// `~/.config/crosshook/`, `~/.local/share/crosshook/`, and
/// `~/.cache/crosshook/`. The data is visible to the sandbox via
/// `--filesystem=home`; only the env var remap is hiding it.
///
/// The override honours Flatpak's `HOST_XDG_CONFIG_HOME`, `HOST_XDG_DATA_HOME`,
/// `HOST_XDG_CACHE_HOME`, and `HOST_XDG_STATE_HOME` env vars (set by the
/// Flatpak runtime to carry the host's real XDG values) when they are present.
/// This ensures users with a customised XDG layout (e.g. `XDG_CONFIG_HOME=/data/configs`)
/// get the correct paths rather than the `$HOME`-derived defaults.
///
/// For Phase 1 this function restores the default XDG paths so the Flatpak
/// build and the AppImage share the same data on disk. Called from the very
/// top of `crosshook_native::run()` before any store initializes.
///
/// Phase 4 (Flathub submission) will replace this with a proper per-app
/// isolation model and a first-run migration — see the tracking issue linked
/// from `docs/prps/prds/flatpak-distribution.prd.md` §10.2.
///
/// # Safety
///
/// Must only be called during single-threaded process startup, before any
/// threads are spawned and before any code reads XDG env vars. This function
/// mutates process environment variables through `SystemEnv::set`; see that
/// method's SAFETY note for the concrete preconditions.
pub unsafe fn override_xdg_for_flatpak_host_access() {
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

fn host_command_with_env_inner(
    program: &str,
    envs: &BTreeMap<String, String>,
    flatpak: bool,
) -> Command {
    if flatpak {
        tracing::debug!(program, "wrapping command with flatpak-spawn --host (with env)");
        let mut cmd = Command::new("flatpak-spawn");
        cmd.arg("--host");
        for (key, value) in envs {
            cmd.arg(format!("--env={key}={value}"));
        }
        cmd.arg(program);
        cmd
    } else {
        let mut cmd = Command::new(program);
        cmd.envs(envs);
        cmd
    }
}

/// Indirection for env-var access so unit tests can observe writes and
/// inject reads without mutating the real process environment.
trait EnvSink {
    /// Write an environment variable.
    fn set(&mut self, key: &str, value: &OsString);
    /// Read an environment variable. Returns `None` when the variable is unset.
    fn get(&self, key: &str) -> Option<OsString>;
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

    fn get(&self, key: &str) -> Option<OsString> {
        std::env::var_os(key)
    }
}

/// Resolve one XDG path: prefer the Flatpak `HOST_XDG_*_HOME` var when the
/// runtime has set it (carries the host's real XDG value), otherwise fall
/// back to `<home>/<default_rel…>`.
fn host_xdg_or_default(
    host_var: &str,
    home: &Path,
    default_rel: &[&str],
    env: &dyn EnvSink,
) -> OsString {
    if let Some(v) = env.get(host_var) {
        return v;
    }
    let mut p = home.to_path_buf();
    for s in default_rel {
        p.push(s);
    }
    p.into_os_string()
}

/// Applies XDG path overrides so the Flatpak sandbox sees the host's real XDG
/// directories rather than the per-app sandbox locations.
///
/// `HOST_XDG_CONFIG_HOME`, `HOST_XDG_DATA_HOME`, and `HOST_XDG_CACHE_HOME`
/// are preferred when set (Flatpak exposes them for exactly this purpose);
/// the `$HOME`-derived defaults are used as fallbacks.
fn apply_xdg_host_override(home: Option<PathBuf>, sink: &mut dyn EnvSink) -> bool {
    let Some(home) = home else {
        tracing::warn!("xdg host override skipped: HOME is unset");
        return false;
    };

    let config = host_xdg_or_default("HOST_XDG_CONFIG_HOME", &home, &[".config"], sink);
    let data = host_xdg_or_default("HOST_XDG_DATA_HOME", &home, &[".local", "share"], sink);
    let cache = host_xdg_or_default("HOST_XDG_CACHE_HOME", &home, &[".cache"], sink);

    sink.set("XDG_CONFIG_HOME", &config);
    sink.set("XDG_DATA_HOME", &data);
    sink.set("XDG_CACHE_HOME", &cache);
    tracing::info!(
        home = %home.display(),
        "xdg host override applied (flatpak → host XDG paths)"
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

    #[test]
    fn host_command_with_env_threads_envs_as_env_args_in_flatpak() {
        // In Flatpak, each env entry must become a --env=KEY=VALUE arg placed
        // before the program name.  .env()/.envs() on the Command object are
        // silently dropped by flatpak-spawn --host.
        let envs = BTreeMap::from([
            ("WINEPREFIX".to_string(), "/home/alice/.wine".to_string()),
            ("DXVK_ASYNC".to_string(), "1".to_string()),
        ]);
        let cmd = host_command_with_env_inner("wine", &envs, true);
        let std_cmd = cmd.as_std();
        assert_eq!(std_cmd.get_program(), "flatpak-spawn");
        let args: Vec<&std::ffi::OsStr> = std_cmd.get_args().collect();
        // First arg must be --host; the two --env args come next (BTreeMap is
        // sorted so DXVK_ASYNC < WINEPREFIX); last arg is the program.
        assert_eq!(args[0], std::ffi::OsStr::new("--host"));
        assert!(
            args.iter().any(|a| *a == std::ffi::OsStr::new("--env=DXVK_ASYNC=1")),
            "expected --env=DXVK_ASYNC=1 in args, got: {args:?}"
        );
        assert!(
            args.iter()
                .any(|a| *a == std::ffi::OsStr::new("--env=WINEPREFIX=/home/alice/.wine")),
            "expected --env=WINEPREFIX=/home/alice/.wine in args, got: {args:?}"
        );
        assert_eq!(*args.last().unwrap(), std::ffi::OsStr::new("wine"));
    }

    #[test]
    fn host_command_with_env_uses_envs_method_when_not_flatpak() {
        // Outside Flatpak, env vars should be forwarded via .envs(), not as
        // --env=K=V arguments (there is no flatpak-spawn wrapper).
        let envs = BTreeMap::from([("DXVK_ASYNC".to_string(), "1".to_string())]);
        let cmd = host_command_with_env_inner("wine", &envs, false);
        let std_cmd = cmd.as_std();
        assert_eq!(std_cmd.get_program(), "wine");
        // No --env= style args; the env var is set on the Command directly.
        let args: Vec<&std::ffi::OsStr> = std_cmd.get_args().collect();
        assert!(
            args.is_empty(),
            "expected no extra args for non-flatpak, got: {args:?}"
        );
        let envs_on_cmd: Vec<(&std::ffi::OsStr, Option<&std::ffi::OsStr>)> =
            std_cmd.get_envs().collect();
        assert!(
            envs_on_cmd
                .iter()
                .any(|(k, v)| *k == std::ffi::OsStr::new("DXVK_ASYNC")
                    && *v == Some(std::ffi::OsStr::new("1"))),
            "expected DXVK_ASYNC=1 in command envs, got: {envs_on_cmd:?}"
        );
    }

    /// In-memory `EnvSink` that records writes instead of mutating the real
    /// process environment. Lets xdg-override tests run concurrently without
    /// racing on `XDG_*_HOME`.
    ///
    /// Pre-populate `reads` to inject env-var values that `get()` will return,
    /// simulating `HOST_XDG_*_HOME` vars set by the Flatpak runtime.
    #[derive(Default)]
    struct FakeEnv {
        writes: Vec<(String, OsString)>,
        reads: std::collections::HashMap<String, OsString>,
    }

    impl EnvSink for FakeEnv {
        fn set(&mut self, key: &str, value: &OsString) {
            self.writes.push((key.to_string(), value.clone()));
        }

        fn get(&self, key: &str) -> Option<OsString> {
            self.reads.get(key).cloned()
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

    #[test]
    fn xdg_override_prefers_host_xdg_config_home_when_set() {
        // Simulate a user with a customised XDG layout where Flatpak exposes
        // HOST_XDG_CONFIG_HOME=/data/configs.  The override must write that
        // value rather than the $HOME/.config fallback.
        let _guard = ScopedEnv::set(TEST_ENV_KEY, "dev.crosshook.CrossHook");
        let mut env = FakeEnv::default();
        env.reads.insert(
            "HOST_XDG_CONFIG_HOME".to_string(),
            OsString::from("/data/configs"),
        );
        let applied = apply_xdg_host_override(Some(PathBuf::from("/home/alice")), &mut env);
        assert!(applied);
        let config_write = env
            .writes
            .iter()
            .find(|(k, _)| k == "XDG_CONFIG_HOME")
            .expect("XDG_CONFIG_HOME must be written");
        assert_eq!(
            config_write.1,
            OsString::from("/data/configs"),
            "should use HOST_XDG_CONFIG_HOME, not $HOME/.config"
        );
        // Data and cache fall back to $HOME-derived defaults when their HOST_XDG_* vars are absent.
        let data_write = env
            .writes
            .iter()
            .find(|(k, _)| k == "XDG_DATA_HOME")
            .expect("XDG_DATA_HOME must be written");
        assert_eq!(data_write.1, OsString::from("/home/alice/.local/share"));
    }

    #[test]
    fn xdg_override_prefers_all_host_xdg_vars_when_set() {
        // When all three HOST_XDG_* vars are present, none of the $HOME-derived
        // defaults should appear in the writes.
        let mut env = FakeEnv::default();
        env.reads.insert(
            "HOST_XDG_CONFIG_HOME".to_string(),
            OsString::from("/data/configs"),
        );
        env.reads.insert(
            "HOST_XDG_DATA_HOME".to_string(),
            OsString::from("/data/share"),
        );
        env.reads.insert(
            "HOST_XDG_CACHE_HOME".to_string(),
            OsString::from("/data/cache"),
        );
        let applied = apply_xdg_host_override(Some(PathBuf::from("/home/alice")), &mut env);
        assert!(applied);
        assert_eq!(
            env.writes,
            vec![
                ("XDG_CONFIG_HOME".to_string(), OsString::from("/data/configs")),
                ("XDG_DATA_HOME".to_string(), OsString::from("/data/share")),
                ("XDG_CACHE_HOME".to_string(), OsString::from("/data/cache")),
            ]
        );
    }
}
