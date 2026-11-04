use std::path::{Path, PathBuf};

use crate::platform::normalize_flatpak_host_path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerKind {
    Proton,
    Wine,
}

pub fn classify_runner(wine_version: &str) -> RunnerKind {
    if wine_version.to_lowercase().contains("proton") {
        RunnerKind::Proton
    } else {
        RunnerKind::Wine
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerResolution {
    Installed(PathBuf),
    Missing(String),
}

pub fn resolve_runner_path(lutris_root: &Path, version: &str) -> RunnerResolution {
    if version.contains('/') || version.contains('\\') || version.contains("..") {
        return RunnerResolution::Missing(version.to_string());
    }
    let runner_dir = lutris_root.join("runners").join("wine").join(version);
    if runner_dir.is_dir() {
        RunnerResolution::Installed(runner_dir)
    } else {
        RunnerResolution::Missing(version.to_string())
    }
}

/// Host form of a runner directory found by [`resolve_runner_path`].
///
/// The Lutris root is a sandbox-use pick, so under Flatpak it (and every
/// runner below it) can be a `/run/host/…` or document-portal path. The runner
/// is executed on the host as `proton_path`, so it is stored in host
/// form when that can be determined (`/run/host` stripping, or a successful
/// document-portal xattr lookup). If the portal lookup fails, the sandbox path
/// is stored unchanged and the launch-time `normalize_flatpak_host_path` repair
/// gets another chance. The directory check above keeps using the sandbox path.
pub fn runner_host_path(runner_dir: &Path) -> String {
    normalize_flatpak_host_path(&runner_dir.to_string_lossy())
}
