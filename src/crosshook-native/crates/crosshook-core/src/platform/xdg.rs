use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::detect::is_flatpak;

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

/// Indirection for env-var access so unit tests can observe writes and inject
/// reads without mutating the real process environment.
pub(crate) trait EnvSink {
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
    if let Some(value) = env.get(host_var) {
        return value;
    }
    let mut path = home.to_path_buf();
    for segment in default_rel {
        path.push(segment);
    }
    path.into_os_string()
}

/// Applies XDG path overrides so the Flatpak sandbox sees the host's real XDG
/// directories rather than the per-app sandbox locations.
///
/// `HOST_XDG_CONFIG_HOME`, `HOST_XDG_DATA_HOME`, and `HOST_XDG_CACHE_HOME`
/// are preferred when set (Flatpak exposes them for exactly this purpose);
/// the `$HOME`-derived defaults are used as fallbacks.
pub(crate) fn apply_xdg_host_override(home: Option<PathBuf>, sink: &mut dyn EnvSink) -> bool {
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
