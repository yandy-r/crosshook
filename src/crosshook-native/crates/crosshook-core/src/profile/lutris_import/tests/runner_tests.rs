use std::path::{Path, PathBuf};

use crate::profile::lutris_import::map::{map_lutris_to_profile, MappedLutrisInput};
use crate::profile::lutris_import::parse::{LutrisGameConfig, LutrisWineSection};
use crate::profile::lutris_import::runner::{
    classify_runner, resolve_runner_path, runner_host_path, RunnerKind, RunnerResolution,
};
use tempfile::tempdir;

#[test]
fn classify_lutris_ge_proton_runner() {
    assert_eq!(
        classify_runner("lutris-GE-Proton8-14-x86_64"),
        RunnerKind::Proton
    );
}

#[test]
fn resolve_runner_path_missing_directory() {
    let root = tempdir().unwrap();
    match resolve_runner_path(root.path(), "lutris-GE-Proton8-14-x86_64") {
        RunnerResolution::Missing(version) => {
            assert_eq!(version, "lutris-GE-Proton8-14-x86_64");
        }
        RunnerResolution::Installed(_) => panic!("expected Missing"),
    }
}

/// A runner below a Lutris root picked through the `/run/host` mount is
/// stored as the host path that `proton_path` is run with.
#[test]
fn runner_host_path_strips_flatpak_host_mount() {
    let runner_dir = Path::new("/run/host/home/user/.local/share/lutris")
        .join("runners")
        .join("wine")
        .join("GE-Proton9-20");

    assert_eq!(
        runner_host_path(&runner_dir),
        "/home/user/.local/share/lutris/runners/wine/GE-Proton9-20"
    );
}

#[test]
fn runner_host_path_keeps_plain_host_paths() {
    assert_eq!(
        runner_host_path(Path::new(
            "/home/user/.local/share/lutris/runners/wine/wine-ge"
        )),
        "/home/user/.local/share/lutris/runners/wine/wine-ge"
    );
}

#[test]
fn map_installed_runner_sets_host_proton_path() {
    let root = tempdir().unwrap();
    let runner_dir = root.path().join("runners/wine/GE-Proton9-20");
    std::fs::create_dir_all(&runner_dir).unwrap();
    let config = LutrisGameConfig {
        wine: Some(LutrisWineSection {
            version: Some("GE-Proton9-20".to_string()),
            ..LutrisWineSection::default()
        }),
        ..LutrisGameConfig::default()
    };

    let result = map_lutris_to_profile(MappedLutrisInput {
        config,
        pga: None,
        lutris_root: root.path().to_path_buf(),
        source_path: PathBuf::from("/tmp/lutris/games/test.yml"),
    });

    assert_eq!(
        result.profile.runtime.proton_path,
        runner_dir.display().to_string()
    );
}
