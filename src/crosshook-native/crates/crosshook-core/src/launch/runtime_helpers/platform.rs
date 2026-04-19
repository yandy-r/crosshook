use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use crate::platform;

use super::DEFAULT_HOST_PATH;

pub(crate) fn flatpak_host_mount_path(path: &Path) -> PathBuf {
    let mut host = PathBuf::from("/run/host");
    for component in path.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            other => host.push(other.as_os_str()),
        }
    }
    host
}

pub(crate) fn flatpak_host_umu_candidates(home: Option<&Path>, user: Option<&str>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    let mut add_home_candidates = |base: &Path| {
        candidates.push(base.join(".local/bin/umu-run"));
        candidates.push(base.join(".local/share/umu/umu-run"));
        candidates.push(base.join(".local/pipx/venvs/umu-launcher/bin/umu-run"));
        candidates.push(base.join(".local/share/pipx/venvs/umu-launcher/bin/umu-run"));
    };

    if let Some(home) = home {
        add_home_candidates(home);
        add_home_candidates(&flatpak_host_mount_path(home));
    }

    if let Some(user) = user {
        let trimmed = user.trim();
        if !trimmed.is_empty() {
            let var_home = PathBuf::from(format!("/var/home/{trimmed}"));
            add_home_candidates(&var_home);
            add_home_candidates(&PathBuf::from(format!("/run/host/var/home/{trimmed}")));
            add_home_candidates(&PathBuf::from(format!("/run/host/home/{trimmed}")));
        }
    }

    candidates
}

pub(crate) fn probe_flatpak_host_umu_candidates(
    home: Option<&Path>,
    user: Option<&str>,
) -> Option<String> {
    flatpak_host_umu_candidates(home, user)
        .into_iter()
        .find_map(|candidate| {
            let exists = is_executable_file(&candidate)
                || crate::platform::normalized_path_is_executable_file_on_host(
                    &candidate.to_string_lossy(),
                );
            tracing::debug!(
                candidate = %candidate.display(),
                exists,
                "flatpak host umu candidate probe"
            );
            exists.then(|| candidate.to_string_lossy().into_owned())
        })
}

/// Returns the absolute path to `umu-run` if found on `PATH`, otherwise `None`.
pub fn resolve_umu_run_path() -> Option<String> {
    #[cfg(test)]
    if let Some(path) = crate::launch::optimizations::resolve_umu_run_path_for_test() {
        return path;
    }

    fn first_executable_umu_on_path(path_value: &std::ffi::OsStr) -> Option<String> {
        for directory in env::split_paths(path_value) {
            let candidate = directory.join("umu-run");
            if is_executable_file(&candidate) {
                return Some(candidate.to_string_lossy().into_owned());
            }
        }
        None
    }

    fn first_executable_umu_on_host_path(path_value: &std::ffi::OsStr) -> Option<String> {
        for directory in env::split_paths(path_value) {
            let candidate = directory.join("umu-run");
            let candidate_str = candidate.to_string_lossy().into_owned();
            if crate::platform::normalized_path_is_executable_file_on_host(&candidate_str) {
                return Some(candidate_str);
            }
        }
        None
    }

    // Under Flatpak, `PATH` reflects the sandbox — do not probe it. Prefer the
    // host environment file Flatpak exposes, then a static host default; never
    // fall back to the sandbox process `PATH`.
    if env::var_os("FLATPAK_ID").is_some() {
        const HOST_ENV_PATH: &str = "/run/host/env/PATH";
        if let Ok(bytes) = fs::read(HOST_ENV_PATH) {
            let host_path = String::from_utf8_lossy(&bytes);
            let trimmed = host_path.trim();
            if !trimmed.is_empty() {
                if let Some(path) = first_executable_umu_on_host_path(std::ffi::OsStr::new(trimmed))
                {
                    return Some(path);
                }
            }
        }
        if let Some(path) =
            first_executable_umu_on_host_path(std::ffi::OsStr::new(DEFAULT_HOST_PATH))
        {
            return Some(path);
        }
        let home = env::var_os("HOME").map(PathBuf::from);
        let user = env::var("USER").ok();
        if let Some(path) = probe_flatpak_host_umu_candidates(home.as_deref(), user.as_deref()) {
            return Some(path);
        }
        return None;
    }

    let path_value =
        env::var_os("PATH").unwrap_or_else(|| std::ffi::OsString::from(DEFAULT_HOST_PATH));
    first_executable_umu_on_path(path_value.as_os_str())
}

pub(crate) fn is_executable_file(path: &Path) -> bool {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}

/// Returns `true` if `unshare --net` is available for the current user.
///
/// Probes by running `unshare --net true` (which immediately exits).
/// Returns `false` if the binary is missing or the kernel blocks the operation.
///
/// The result is cached for the lifetime of the process via `OnceLock` since
/// kernel policy does not change within a single application session.
/// Runtime facts for Flatpak UI badges and launch validation (not persisted).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchPlatformCapabilities {
    pub is_flatpak: bool,
    pub unshare_net_available: bool,
}

/// Returns whether CrossHook is sandboxed as Flatpak and whether host `unshare --net` works.
pub fn launch_platform_capabilities() -> LaunchPlatformCapabilities {
    LaunchPlatformCapabilities {
        is_flatpak: platform::is_flatpak(),
        unshare_net_available: is_unshare_net_available(),
    }
}

pub fn is_unshare_net_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        let mut cmd = if platform::is_flatpak() {
            platform::host_std_command("unshare")
        } else {
            std::process::Command::new("unshare")
        };
        cmd.args(["--net", "true"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        cmd.status().map(|s| s.success()).unwrap_or(false)
    })
}
