use std::path::{Path, PathBuf};

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
    let runner_dir = lutris_root.join("runners").join("wine").join(version);
    if runner_dir.is_dir() {
        RunnerResolution::Installed(runner_dir)
    } else {
        RunnerResolution::Missing(version.to_string())
    }
}
