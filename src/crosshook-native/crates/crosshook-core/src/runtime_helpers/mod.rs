//! Filesystem resolution of the bundled runtime helper shell scripts
//! (`steam-launch-helper.sh`, `steam-launch-trainer.sh`,
//! `steam-host-trainer-runner.sh`).
//!
//! This module only answers "where is the helper script on disk?". It is
//! distinct from [`crate::launch::runtime_helpers`], which builds the
//! environment and command lines used when *running* launches.
//!
//! Lookup order (first acceptable regular file wins):
//! 1. `/app/resources/<name>` when running inside Flatpak;
//! 2. `<exe dir>/../resources/<name>`;
//! 3. `<exe dir>/<name>`;
//! 4. the source-tree `runtime-helpers/` directory (debug builds only).
//!
//! Candidates 2–4 must also pass a trust check (owned by the current user or
//! root; neither the file nor its parent directory group/world-writable)
//! because they live on a writable host filesystem and the scripts are
//! executed.

use std::fmt;
use std::path::{Path, PathBuf};

const FLATPAK_RESOURCES_DIR: &str = "/app/resources";

/// A runtime helper script shipped alongside CrossHook.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeHelper {
    SteamLaunchHelper,
    SteamLaunchTrainer,
    SteamHostTrainerRunner,
}

impl RuntimeHelper {
    pub const ALL: [RuntimeHelper; 3] = [
        RuntimeHelper::SteamLaunchHelper,
        RuntimeHelper::SteamLaunchTrainer,
        RuntimeHelper::SteamHostTrainerRunner,
    ];

    pub fn file_name(&self) -> &'static str {
        match self {
            RuntimeHelper::SteamLaunchHelper => "steam-launch-helper.sh",
            RuntimeHelper::SteamLaunchTrainer => "steam-launch-trainer.sh",
            RuntimeHelper::SteamHostTrainerRunner => "steam-host-trainer-runner.sh",
        }
    }
}

impl fmt::Display for RuntimeHelper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.file_name())
    }
}

/// Directories searched for runtime helpers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HelperRoots {
    /// `/app/resources` inside Flatpak, otherwise `None`.
    pub flatpak_resources: Option<PathBuf>,
    /// Directory containing the running executable.
    pub exe_dir: Option<PathBuf>,
    /// Source-tree `runtime-helpers/` directory; `None` in release builds.
    pub dev_dir: Option<PathBuf>,
}

/// A candidate location and whether it must pass [`is_trusted`].
struct Candidate {
    path: PathBuf,
    requires_trust: bool,
}

impl HelperRoots {
    /// Detects the roots for the current process.
    pub fn detect() -> Self {
        Self {
            flatpak_resources: crate::platform::is_flatpak()
                .then(|| PathBuf::from(FLATPAK_RESOURCES_DIR)),
            exe_dir: std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(Path::to_path_buf)),
            dev_dir: development_dir(),
        }
    }

    fn candidates(&self, helper: RuntimeHelper) -> Vec<Candidate> {
        let name = helper.file_name();
        let mut candidates = Vec::with_capacity(4);
        if let Some(dir) = &self.flatpak_resources {
            // Exempt from the trust check: `/app` is the read-only Flatpak
            // deployment, and files from a system-wide install appear owned
            // by the overflow uid (65534) inside the sandbox, so an ownership
            // check would reject legitimate installs.
            candidates.push(Candidate {
                path: dir.join(name),
                requires_trust: false,
            });
        }
        if let Some(dir) = &self.exe_dir {
            candidates.push(Candidate {
                path: dir.join("..").join("resources").join(name),
                requires_trust: true,
            });
            candidates.push(Candidate {
                path: dir.join(name),
                requires_trust: true,
            });
        }
        if let Some(dir) = &self.dev_dir {
            candidates.push(Candidate {
                path: dir.join(name),
                requires_trust: true,
            });
        }
        candidates
    }
}

#[cfg(debug_assertions)]
fn development_dir() -> Option<PathBuf> {
    Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime-helpers"))
}

#[cfg(not(debug_assertions))]
fn development_dir() -> Option<PathBuf> {
    None
}

/// Error returned when no candidate location holds an acceptable helper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeHelperError {
    NotFound {
        helper: RuntimeHelper,
        /// Every candidate path, in lookup order.
        tried: Vec<PathBuf>,
        /// Regular files that existed but failed the trust check.
        untrusted: Vec<PathBuf>,
    },
}

fn write_paths(f: &mut fmt::Formatter<'_>, paths: &[PathBuf]) -> fmt::Result {
    for (index, path) in paths.iter().enumerate() {
        if index > 0 {
            f.write_str(", ")?;
        }
        write!(f, "{}", path.display())?;
    }
    Ok(())
}

impl fmt::Display for RuntimeHelperError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeHelperError::NotFound {
                helper,
                tried,
                untrusted,
            } => {
                write!(f, "unable to resolve runtime helper '{helper}'; tried: ")?;
                if tried.is_empty() {
                    f.write_str("(no candidate locations)")?;
                } else {
                    write_paths(f, tried)?;
                }
                if !untrusted.is_empty() {
                    f.write_str(
                        "; rejected (not owned by the current user/root or group/world-writable): ",
                    )?;
                    write_paths(f, untrusted)?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for RuntimeHelperError {}

/// Resolves `helper` against explicit roots. The first candidate that is a
/// regular file (and, outside `flatpak_resources`, passes the trust check)
/// wins.
pub fn resolve_with(
    roots: &HelperRoots,
    helper: RuntimeHelper,
) -> Result<PathBuf, RuntimeHelperError> {
    let candidates = roots.candidates(helper);
    let mut untrusted = Vec::new();
    for candidate in &candidates {
        if !candidate.path.is_file() {
            continue;
        }
        if candidate.requires_trust && !is_trusted(&candidate.path) {
            tracing::warn!(path = %candidate.path.display(), helper = helper.file_name(), "skipping untrusted runtime helper");
            untrusted.push(candidate.path.clone());
            continue;
        }
        return Ok(candidate.path.clone());
    }
    Err(RuntimeHelperError::NotFound {
        helper,
        tried: candidates.into_iter().map(|c| c.path).collect(),
        untrusted,
    })
}

/// Resolves `helper` against the roots detected for the current process.
pub fn resolve(helper: RuntimeHelper) -> Result<PathBuf, RuntimeHelperError> {
    let path = resolve_with(&HelperRoots::detect(), helper)?;
    tracing::debug!(path = %path.display(), helper = helper.file_name(), "resolved runtime helper");
    Ok(path)
}

/// A helper is trusted when the file is owned by the current euid or root,
/// and neither the file nor its parent directory is group/world-writable.
#[cfg(unix)]
fn is_trusted(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;

    const GROUP_OTHER_WRITE: u32 = 0o022;

    let Ok(file) = std::fs::metadata(path) else {
        return false;
    };
    let euid = nix::unistd::geteuid().as_raw();
    if file.uid() != euid && file.uid() != 0 {
        return false;
    }
    if file.mode() & GROUP_OTHER_WRITE != 0 {
        return false;
    }
    let Some(parent) = path.parent() else {
        return false;
    };
    match std::fs::metadata(parent) {
        Ok(dir) => dir.mode() & GROUP_OTHER_WRITE == 0,
        Err(_) => false,
    }
}

#[cfg(not(unix))]
fn is_trusted(_path: &Path) -> bool {
    true
}

/// Marks the source-tree helper scripts executable (debug builds only; a
/// no-op in release builds and on non-unix targets). Missing scripts are
/// ignored.
pub fn ensure_development_scripts_executable() -> std::io::Result<()> {
    match development_dir() {
        Some(dir) => ensure_executable_in(&dir),
        None => Ok(()),
    }
}

fn ensure_executable_in(dir: &Path) -> std::io::Result<()> {
    for helper in RuntimeHelper::ALL {
        ensure_executable(&dir.join(helper.file_name()))?;
    }
    Ok(())
}

#[cfg(unix)]
fn ensure_executable(path: &Path) -> std::io::Result<()> {
    use std::fs;
    use std::io::ErrorKind;
    use std::os::unix::fs::PermissionsExt;

    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let mut permissions = metadata.permissions();
    permissions.set_mode(permissions.mode() | 0o111);
    fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn ensure_executable(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    const HELPER: RuntimeHelper = RuntimeHelper::SteamLaunchHelper;

    struct Fixture {
        _tmp: TempDir,
        roots: HelperRoots,
        exe_resources: PathBuf,
    }

    impl Fixture {
        fn flatpak(&self) -> &Path {
            self.roots.flatpak_resources.as_deref().unwrap()
        }
        fn exe_dir(&self) -> &Path {
            self.roots.exe_dir.as_deref().unwrap()
        }
        fn dev(&self) -> &Path {
            self.roots.dev_dir.as_deref().unwrap()
        }
        fn exe_resources_candidate(&self) -> PathBuf {
            self.exe_dir()
                .join("..")
                .join("resources")
                .join(HELPER.file_name())
        }
    }

    fn chmod(path: &Path, mode: u32) {
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }

    /// Layout: `<tmp>/flatpak`, `<tmp>/app/bin` (exe dir), `<tmp>/app/resources`,
    /// `<tmp>/dev` — all created empty with mode 0o755 (independent of umask).
    fn fixture() -> Fixture {
        let tmp = TempDir::new().unwrap();
        let flatpak = tmp.path().join("flatpak");
        let exe_dir = tmp.path().join("app/bin");
        let exe_resources = tmp.path().join("app/resources");
        let dev = tmp.path().join("dev");
        for dir in [&flatpak, &exe_dir, &exe_resources, &dev] {
            fs::create_dir_all(dir).unwrap();
            chmod(dir, 0o755);
        }
        Fixture {
            _tmp: tmp,
            roots: HelperRoots {
                flatpak_resources: Some(flatpak),
                exe_dir: Some(exe_dir),
                dev_dir: Some(dev),
            },
            exe_resources,
        }
    }

    fn place(dir: &Path) -> PathBuf {
        let path = dir.join(HELPER.file_name());
        fs::write(&path, "#!/bin/sh\n").unwrap();
        chmod(&path, 0o755);
        path
    }

    #[test]
    fn flatpak_root_wins_when_only_it_has_file() {
        let f = fixture();
        let expected = place(f.flatpak());
        assert_eq!(resolve_with(&f.roots, HELPER).unwrap(), expected);
    }

    #[test]
    fn exe_relative_resources_wins_when_only_it_has_file() {
        let f = fixture();
        place(&f.exe_resources);
        let resolved = resolve_with(&f.roots, HELPER).unwrap();
        assert_eq!(resolved, f.exe_resources_candidate());
    }

    #[test]
    fn exe_dir_wins_when_only_it_has_file() {
        let f = fixture();
        let expected = place(f.exe_dir());
        assert_eq!(resolve_with(&f.roots, HELPER).unwrap(), expected);
    }

    #[test]
    fn dev_dir_wins_when_only_it_has_file() {
        let f = fixture();
        let expected = place(f.dev());
        assert_eq!(resolve_with(&f.roots, HELPER).unwrap(), expected);
    }

    #[test]
    fn flatpak_beats_exe_relative_resources() {
        let f = fixture();
        let expected = place(f.flatpak());
        place(&f.exe_resources);
        assert_eq!(resolve_with(&f.roots, HELPER).unwrap(), expected);
    }

    #[test]
    fn exe_relative_resources_beats_exe_dir() {
        let f = fixture();
        place(&f.exe_resources);
        place(f.exe_dir());
        assert_eq!(
            resolve_with(&f.roots, HELPER).unwrap(),
            f.exe_resources_candidate()
        );
    }

    #[test]
    fn exe_dir_beats_dev_dir() {
        let f = fixture();
        let expected = place(f.exe_dir());
        place(f.dev());
        assert_eq!(resolve_with(&f.roots, HELPER).unwrap(), expected);
    }

    #[test]
    fn directory_with_helper_name_is_rejected() {
        let f = fixture();
        fs::create_dir(f.flatpak().join(HELPER.file_name())).unwrap();
        let expected = place(f.dev());
        assert_eq!(resolve_with(&f.roots, HELPER).unwrap(), expected);
    }

    #[test]
    fn writable_helper_file_is_rejected_and_later_candidate_wins() {
        let f = fixture();
        let group_writable = place(f.exe_dir());
        chmod(&group_writable, 0o775);
        let expected = place(f.dev());
        assert_eq!(resolve_with(&f.roots, HELPER).unwrap(), expected);

        chmod(&expected, 0o666);
        let RuntimeHelperError::NotFound { untrusted, .. } =
            resolve_with(&f.roots, HELPER).unwrap_err();
        assert_eq!(untrusted, vec![group_writable, expected]);
    }

    #[test]
    fn world_writable_parent_dir_is_rejected() {
        let f = fixture();
        let helper = place(f.dev());
        chmod(f.dev(), 0o777);
        let error = resolve_with(&f.roots, HELPER).unwrap_err();
        let RuntimeHelperError::NotFound { untrusted, .. } = &error;
        assert_eq!(untrusted, &vec![helper.clone()]);
        let message = error.to_string();
        assert!(message.contains("rejected"), "{message}");
        assert!(message.contains(&helper.display().to_string()), "{message}");
    }

    #[test]
    fn flatpak_root_is_exempt_from_trust_check() {
        let f = fixture();
        let expected = place(f.flatpak());
        chmod(&expected, 0o666);
        chmod(f.flatpak(), 0o777);
        assert_eq!(resolve_with(&f.roots, HELPER).unwrap(), expected);
    }

    #[test]
    fn not_found_lists_every_candidate_in_order() {
        let f = fixture();
        let name = HELPER.file_name();
        let expected = vec![
            f.flatpak().join(name),
            f.exe_resources_candidate(),
            f.exe_dir().join(name),
            f.dev().join(name),
        ];

        let error = resolve_with(&f.roots, HELPER).unwrap_err();
        let RuntimeHelperError::NotFound {
            helper,
            tried,
            untrusted,
        } = &error;
        assert_eq!(*helper, HELPER);
        assert_eq!(tried, &expected);
        assert!(untrusted.is_empty());

        let message = error.to_string();
        assert!(!message.contains("rejected"), "{message}");
        for path in &expected {
            assert!(
                message.contains(&path.display().to_string()),
                "{message} missing {}",
                path.display()
            );
        }
    }

    #[test]
    fn ensure_executable_in_sets_exec_bits_and_ignores_missing() {
        let tmp = TempDir::new().unwrap();
        let present = tmp.path().join(HELPER.file_name());
        fs::write(&present, "#!/bin/sh\n").unwrap();
        chmod(&present, 0o644);

        ensure_executable_in(tmp.path()).unwrap();

        let mode = fs::metadata(&present).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111);
        assert!(!tmp
            .path()
            .join(RuntimeHelper::SteamLaunchTrainer.file_name())
            .exists());
    }
}
