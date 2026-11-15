//! Tests for the `location` module.

use std::path::{Path, PathBuf};

use super::super::{
    resolve, CandidateRoot, LogDirCandidate, RuntimeDirInputs, FLATPAK_APP_ID, LOG_SUBDIR,
    NATIVE_SUBDIR,
};

fn inputs(runtime: Option<&str>, state: Option<&str>, flatpak: bool) -> RuntimeDirInputs {
    RuntimeDirInputs {
        runtime_dir: runtime.map(PathBuf::from),
        state_dir: state.map(PathBuf::from),
        flatpak,
    }
}

fn paths(inputs: &RuntimeDirInputs) -> Vec<PathBuf> {
    resolve(inputs).iter().map(LogDirCandidate::path).collect()
}

#[test]
fn flatpak_lists_only_host_visible_candidates() {
    let candidates = resolve(&inputs(Some("/run/user/1000"), Some("/state"), true));
    assert_eq!(
        candidates,
        vec![
            LogDirCandidate {
                root: Path::new("/run/user/1000/app").join(FLATPAK_APP_ID),
                relative: vec![LOG_SUBDIR],
                kind: CandidateRoot::Runtime,
            },
            LogDirCandidate {
                root: PathBuf::from("/state"),
                relative: vec![NATIVE_SUBDIR, LOG_SUBDIR],
                kind: CandidateRoot::State,
            },
        ]
    );
}

#[test]
fn native_with_runtime_skips_the_flatpak_candidate() {
    let candidates = resolve(&inputs(Some("/run/user/1000"), Some("/state"), false));
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].kind, CandidateRoot::Runtime);
    assert_eq!(
        candidates[0].path(),
        PathBuf::from("/run/user/1000/crosshook/logs")
    );
    assert_eq!(candidates[1].kind, CandidateRoot::State);
    assert_eq!(candidates[1].path(), PathBuf::from("/state/crosshook/logs"));
}

#[test]
fn no_inputs_yield_no_candidates() {
    assert!(resolve(&inputs(None, None, false)).is_empty());
    assert!(resolve(&inputs(None, None, true)).is_empty());
}

#[test]
fn relative_runtime_dir_is_dropped() {
    for flatpak in [false, true] {
        for runtime in ["", "relative/run", "./run"] {
            assert!(
                resolve(&inputs(Some(runtime), None, flatpak)).is_empty(),
                "{runtime:?} flatpak={flatpak}"
            );
        }
    }
}

#[test]
fn relative_state_dir_is_dropped() {
    let candidates = resolve(&inputs(
        Some("/run/user/1000"),
        Some("relative/state"),
        false,
    ));
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].kind, CandidateRoot::Runtime);
}

#[test]
fn unset_runtime_with_state_falls_back_to_state_only() {
    for flatpak in [false, true] {
        let candidates = resolve(&inputs(None, Some("/home/u/.local/state"), flatpak));
        assert_eq!(candidates.len(), 1, "flatpak={flatpak}");
        assert_eq!(candidates[0].kind, CandidateRoot::State);
        assert_eq!(
            candidates[0].path(),
            PathBuf::from("/home/u/.local/state/crosshook/logs")
        );
    }
}

#[test]
fn no_candidate_is_ever_under_tmp() {
    let tables = [
        inputs(Some("/run/user/1000"), Some("/state"), true),
        inputs(Some("/run/user/1000"), Some("/state"), false),
        inputs(None, Some("/state"), false),
        inputs(Some("relative"), None, true),
        inputs(None, None, false),
    ];
    for table in tables {
        for path in paths(&table) {
            assert!(!path.starts_with("/tmp"), "{path:?}");
        }
    }
}

#[test]
fn path_joins_root_and_relative_components() {
    let candidate = LogDirCandidate {
        root: PathBuf::from("/run/user/1000"),
        relative: vec![NATIVE_SUBDIR, LOG_SUBDIR],
        kind: CandidateRoot::Runtime,
    };
    assert_eq!(
        candidate.path(),
        PathBuf::from("/run/user/1000/crosshook/logs")
    );
}
