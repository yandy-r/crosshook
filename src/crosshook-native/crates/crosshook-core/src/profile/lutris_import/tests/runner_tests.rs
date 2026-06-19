use crate::profile::lutris_import::runner::{
    classify_runner, resolve_runner_path, RunnerKind, RunnerResolution,
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
