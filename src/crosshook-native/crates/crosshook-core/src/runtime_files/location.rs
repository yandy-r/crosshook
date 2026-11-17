//! Candidate runtime directory resolution.
//!
//! Pure: the host facts arrive as [`RuntimeDirInputs`], so the candidate order
//! is testable without touching the process environment. No candidate is ever
//! under `/tmp`.

use std::path::PathBuf;

use crate::platform;

use super::{FLATPAK_APP_ID, LOG_SUBDIR, NATIVE_SUBDIR};

/// Host facts the resolver depends on, injectable for tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDirInputs {
    /// `$XDG_RUNTIME_DIR`, when set.
    pub runtime_dir: Option<PathBuf>,
    /// `$XDG_STATE_HOME` (or its default), when known.
    pub state_dir: Option<PathBuf>,
    /// Running inside the Flatpak sandbox.
    pub flatpak: bool,
}

impl RuntimeDirInputs {
    /// Reads the facts from the current process environment.
    pub fn detect() -> Self {
        let dirs = platform::base_dirs();
        Self {
            runtime_dir: dirs
                .as_ref()
                .and_then(|dirs| dirs.runtime_dir())
                .map(PathBuf::from),
            state_dir: dirs
                .as_ref()
                .and_then(|dirs| dirs.state_dir())
                .map(PathBuf::from),
            flatpak: platform::is_flatpak(),
        }
    }
}

/// Which base directory a candidate was derived from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateRoot {
    /// Derived from the runtime directory (`$XDG_RUNTIME_DIR`).
    Runtime,
    /// Derived from the state directory (`$XDG_STATE_HOME`).
    State,
}

/// One possible location of the private log directory.
///
/// `root` is an existing, trusted base; `relative` are the components that may
/// have to be created below it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogDirCandidate {
    /// Absolute base directory.
    pub root: PathBuf,
    /// Components below `root` that make up the log directory.
    pub relative: Vec<&'static str>,
    /// Which base directory `root` came from.
    pub kind: CandidateRoot,
}

impl LogDirCandidate {
    /// The full log directory path (`root` joined with `relative`).
    pub fn path(&self) -> PathBuf {
        self.relative
            .iter()
            .fold(self.root.clone(), |path, component| path.join(component))
    }
}

/// Returns the candidate log directories in preference order.
///
/// Non-absolute inputs are dropped. Under Flatpak only host-visible locations
/// are listed (the shared `app/<id>` runtime directory, then the state
/// directory): the sandbox's own `$XDG_RUNTIME_DIR/crosshook` is invisible to
/// the host-side helper scripts and the gamescope PID capture.
pub fn resolve(inputs: &RuntimeDirInputs) -> Vec<LogDirCandidate> {
    let runtime = inputs
        .runtime_dir
        .as_ref()
        .filter(|path| path.is_absolute());
    let state = inputs.state_dir.as_ref().filter(|path| path.is_absolute());

    let mut candidates = Vec::new();
    if let (true, Some(runtime)) = (inputs.flatpak, runtime) {
        candidates.push(LogDirCandidate {
            root: runtime.join("app").join(FLATPAK_APP_ID),
            relative: vec![LOG_SUBDIR],
            kind: CandidateRoot::Runtime,
        });
    }
    if let (false, Some(runtime)) = (inputs.flatpak, runtime) {
        candidates.push(LogDirCandidate {
            root: runtime.clone(),
            relative: vec![NATIVE_SUBDIR, LOG_SUBDIR],
            kind: CandidateRoot::Runtime,
        });
    }
    if let Some(state) = state {
        candidates.push(LogDirCandidate {
            root: state.clone(),
            relative: vec![NATIVE_SUBDIR, LOG_SUBDIR],
            kind: CandidateRoot::State,
        });
    }
    candidates
}
