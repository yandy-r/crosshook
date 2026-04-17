//! Plan / execute split for Proton tool uninstall.
//!
//! The two-phase API lets callers show a confirmation dialog with conflict
//! warnings before any filesystem mutation occurs.
//!
//! # Safety model
//!
//! System-owned paths (`/usr`, `/opt`, `/snap`, the Flatpak runtime tree, and
//! the explicit `SYSTEM_COMPAT_TOOL_ROOTS` recognised by Steam) are always
//! refused. Paths that do not resolve under a known user-writable
//! `compatibilitytools.d` root are also refused, providing belt-and-suspenders
//! protection against accidental deletion of unrelated directories.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::protonup::install_root::{resolve_candidates_with, InstallRootKind};
use crate::steam::proton::collect_compat_tool_mappings;

// ── system-path denylist ──────────────────────────────────────────────────────

/// Explicit Steam system compat-tool roots that must never be touched.
/// Mirrors the constant in `steam::proton` without re-exporting it.
const STEAM_SYSTEM_ROOTS: &[&str] = &[
    "/usr/share/steam/compatibilitytools.d",
    "/usr/local/share/steam/compatibilitytools.d",
    "/usr/share/steam/compatibilitytools",
    "/usr/local/share/steam/compatibilitytools",
];

/// Broad prefix denylist — belt-and-suspenders on top of `STEAM_SYSTEM_ROOTS`.
const SYSTEM_PREFIX_DENYLIST: &[&str] = &["/usr", "/opt", "/snap", "/var/lib/flatpak/runtime"];

// ── public types ──────────────────────────────────────────────────────────────

/// A validated, ready-to-execute uninstall plan.
///
/// Constructing this value guarantees that `tool_dir` is safe to remove:
/// it is canonical, under a known user root, and not a system path.
/// `conflicting_app_ids` is advisory — callers may surface it as a warning
/// but it does not prevent execution.
#[derive(Debug, Clone)]
pub struct UninstallPlan {
    /// Canonical target directory to remove.
    pub tool_dir: PathBuf,
    /// Steam App IDs currently mapped to this tool (warning; does not block).
    pub conflicting_app_ids: Vec<String>,
    /// Root kind this tool belongs to (for telemetry / logging).
    pub root_kind: InstallRootKind,
}

/// Errors that can occur while planning or executing an uninstall.
#[derive(Debug)]
pub enum UninstallError {
    /// The path resolves to a system-managed location that CrossHook must not touch.
    SystemPathRefused(PathBuf),
    /// The path is not under any known user `compatibilitytools.d` root.
    PathOutsideKnownRoots(PathBuf),
    /// The path does not exist on disk.
    NotFound(PathBuf),
    /// A filesystem error occurred during execution.
    Io(std::io::Error),
}

impl fmt::Display for UninstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SystemPathRefused(p) => {
                write!(f, "refusing to delete system path {}", p.display())
            }
            Self::PathOutsideKnownRoots(p) => write!(
                f,
                "path {} is not under a known user compatibilitytools.d root",
                p.display()
            ),
            Self::NotFound(p) => write!(f, "path {} does not exist", p.display()),
            Self::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for UninstallError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

// ── public API ────────────────────────────────────────────────────────────────

/// Build an [`UninstallPlan`] for `tool_dir`.
///
/// `steam_client_install_path` is the configured Steam root used to derive
/// both install-root candidates and the Steam config path for compat-tool
/// mapping lookups. Pass `None` to fall back to default home-relative paths.
///
/// Returns an error without touching the filesystem if any safety check fails.
pub fn plan_uninstall(
    tool_dir: &Path,
    steam_client_install_path: Option<&Path>,
) -> Result<UninstallPlan, UninstallError> {
    // Derive steam root candidates for both install-root validation and
    // compat-tool mapping lookups.
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let install_candidates = resolve_candidates_with(
        steam_client_install_path,
        home.as_deref(),
        crate::platform::is_flatpak(),
    );

    // Steam root candidates for `collect_compat_tool_mappings`.
    // Use the same home-relative steam roots that `discover_steam_root_candidates`
    // would use (but without requiring steamapps/ to exist, as this is a
    // read-only config scan, not a full discovery).
    let steam_roots = steam_roots_for_mapping(steam_client_install_path, home.as_deref());

    plan_uninstall_with(tool_dir, &install_candidates, &steam_roots)
}

/// Execute a previously validated [`UninstallPlan`].
///
/// Removes `plan.tool_dir` recursively. On success the caller should
/// refresh the installed-tool inventory via the existing rescan function;
/// this module does not maintain a separate registry.
pub fn execute_uninstall(plan: UninstallPlan) -> Result<(), UninstallError> {
    std::fs::remove_dir_all(&plan.tool_dir).map_err(UninstallError::Io)
}

/// Convenience helper that resolves and executes uninstall in one call.
///
/// Intended for callers that do not need to show preflight conflicts.
pub fn execute_uninstall_for_path(
    tool_dir: &Path,
    steam_client_install_path: Option<&Path>,
) -> Result<(), UninstallError> {
    let plan = plan_uninstall(tool_dir, steam_client_install_path)?;
    execute_uninstall(plan)
}

// ── internal testable core ────────────────────────────────────────────────────

/// Testable variant that accepts injected install-root candidates and steam
/// roots instead of deriving them from the environment.
#[cfg(test)]
pub(crate) fn plan_uninstall_with_mappings(
    tool_dir: &Path,
    install_candidates: &[crate::protonup::install_root::InstallRootCandidate],
    mappings: &crate::steam::proton::CompatToolMappings,
) -> Result<UninstallPlan, UninstallError> {
    plan_uninstall_core(tool_dir, install_candidates, mappings)
}

fn plan_uninstall_with(
    tool_dir: &Path,
    install_candidates: &[crate::protonup::install_root::InstallRootCandidate],
    steam_roots: &[PathBuf],
) -> Result<UninstallPlan, UninstallError> {
    let mut diag = Vec::new();
    let mappings = collect_compat_tool_mappings(steam_roots, &mut diag);
    plan_uninstall_core(tool_dir, install_candidates, &mappings)
}

fn plan_uninstall_core(
    tool_dir: &Path,
    install_candidates: &[crate::protonup::install_root::InstallRootCandidate],
    mappings: &crate::steam::proton::CompatToolMappings,
) -> Result<UninstallPlan, UninstallError> {
    // ── 1. Canonicalize ───────────────────────────────────────────────────────

    let canonical = std::fs::canonicalize(tool_dir)
        .map_err(|_| UninstallError::NotFound(tool_dir.to_path_buf()))?;
    let canonical_tool_dir = normalize_tool_dir_path(&canonical);

    plan_uninstall_canonicalized(canonical_tool_dir, install_candidates, mappings)
}

fn plan_uninstall_canonicalized(
    canonical_tool_dir: PathBuf,
    install_candidates: &[crate::protonup::install_root::InstallRootCandidate],
    mappings: &crate::steam::proton::CompatToolMappings,
) -> Result<UninstallPlan, UninstallError> {
    // ── 2. System-path refusal ────────────────────────────────────────────────

    for &root in STEAM_SYSTEM_ROOTS {
        if canonical_tool_dir.starts_with(root) {
            return Err(UninstallError::SystemPathRefused(canonical_tool_dir));
        }
    }
    for &prefix in SYSTEM_PREFIX_DENYLIST {
        if canonical_tool_dir.starts_with(prefix) {
            return Err(UninstallError::SystemPathRefused(canonical_tool_dir));
        }
    }

    // ── 3. User-root validation ───────────────────────────────────────────────
    //
    // The canonical path's parent must equal one of the candidate roots, OR
    // the canonical path itself must be directly under a candidate root.
    // Both checks are equivalent (parent == candidate iff path starts_with
    // candidate and has exactly one more component), but we spell both out
    // for clarity.

    let matching_candidate = install_candidates.iter().find(|c| {
        // Canonicalize the candidate path for comparison; fall back to raw if it
        // doesn't exist yet (candidates may point to not-yet-created dirs).
        let candidate_canonical = std::fs::canonicalize(&c.path).unwrap_or_else(|_| c.path.clone());
        canonical_tool_dir.starts_with(&candidate_canonical)
            && canonical_tool_dir
                .parent()
                .map(|p| {
                    p == candidate_canonical
                        || std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
                            == candidate_canonical
                })
                .unwrap_or(false)
    });

    let Some(candidate) = matching_candidate else {
        return Err(UninstallError::PathOutsideKnownRoots(canonical_tool_dir));
    };

    // ── 4. Profile-mapping scan ───────────────────────────────────────────────

    let tool_id = canonical_tool_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    // Invert the mappings: mappings is AppID -> BTreeSet<tool_name>.
    // Collect all AppIDs that reference this tool.
    let conflicting_app_ids: Vec<String> = mappings
        .iter()
        .filter(|(_app_id, tool_names)| tool_names.contains(&tool_id))
        .map(|(app_id, _)| app_id.clone())
        .collect();

    Ok(UninstallPlan {
        tool_dir: canonical_tool_dir,
        conflicting_app_ids,
        root_kind: candidate.kind,
    })
}

#[cfg(test)]
fn plan_uninstall_with_canonical_path(
    canonical_tool_dir: &Path,
    install_candidates: &[crate::protonup::install_root::InstallRootCandidate],
    mappings: &crate::steam::proton::CompatToolMappings,
) -> Result<UninstallPlan, UninstallError> {
    plan_uninstall_canonicalized(
        canonical_tool_dir.to_path_buf(),
        install_candidates,
        mappings,
    )
}

// ── private helpers ───────────────────────────────────────────────────────────

fn normalize_tool_dir_path(path: &Path) -> PathBuf {
    if path.is_file()
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("proton"))
    {
        return path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| path.to_path_buf());
    }

    path.to_path_buf()
}

/// Build a minimal list of Steam root directories for compat-tool mapping
/// lookups. These are the Steam roots (not the `compatibilitytools.d` subdirs)
/// because `collect_compat_tool_mappings` reads `config/config.vdf` and
/// `userdata/<uid>/config/localconfig.vdf` from each root.
fn steam_roots_for_mapping(
    steam_client_install_path: Option<&Path>,
    home: Option<&Path>,
) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(p) = steam_client_install_path {
        if p.is_dir() {
            roots.push(p.to_path_buf());
        }
    }

    if let Some(home) = home {
        let candidates = [
            home.join(".steam/root"),
            home.join(".local/share/Steam"),
            home.join(".var/app/com.valvesoftware.Steam/data/Steam"),
        ];
        for c in candidates {
            if c.is_dir() && !roots.contains(&c) {
                roots.push(c);
            }
        }
    }

    roots
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protonup::install_root::{InstallRootCandidate, InstallRootKind};
    use crate::steam::proton::CompatToolMappings;
    use std::collections::{BTreeSet, HashMap};
    use std::fs;
    use tempfile::tempdir;

    fn candidate(path: PathBuf, kind: InstallRootKind) -> InstallRootCandidate {
        InstallRootCandidate {
            kind,
            path,
            writable: true,
            reason: None,
        }
    }

    fn empty_mappings() -> CompatToolMappings {
        HashMap::new()
    }

    fn mappings_with(app_id: &str, tool_name: &str) -> CompatToolMappings {
        let mut m = HashMap::new();
        let mut set = BTreeSet::new();
        set.insert(tool_name.to_string());
        m.insert(app_id.to_string(), set);
        m
    }

    // ── test 1 ────────────────────────────────────────────────────────────────

    #[test]
    fn plan_uninstall_refuses_system_prefix_path() {
        let canonical_tool_dir =
            PathBuf::from("/usr/share/steam/compatibilitytools.d/GE-Proton10-1");

        let result = plan_uninstall_with_canonical_path(
            &canonical_tool_dir,
            &[candidate(
                PathBuf::from("/usr/share/steam/compatibilitytools.d"),
                InstallRootKind::NativeSteam,
            )],
            &empty_mappings(),
        );

        assert!(
            matches!(
                result,
                Err(UninstallError::SystemPathRefused(ref path)) if path == &canonical_tool_dir
            ),
            "expected SystemPathRefused for /usr prefix, got: {result:?}"
        );
    }

    #[test]
    fn plan_uninstall_refuses_explicit_steam_system_root() {
        let canonical_tool_dir = PathBuf::from(STEAM_SYSTEM_ROOTS[0]).join("GE-Proton10-1");

        let result = plan_uninstall_with_canonical_path(
            &canonical_tool_dir,
            &[candidate(
                PathBuf::from(STEAM_SYSTEM_ROOTS[0]),
                InstallRootKind::NativeSteam,
            )],
            &empty_mappings(),
        );

        assert!(
            matches!(
                result,
                Err(UninstallError::SystemPathRefused(ref path)) if path == &canonical_tool_dir
            ),
            "expected SystemPathRefused for explicit Steam system root, got: {result:?}"
        );
    }

    #[test]
    fn plan_uninstall_home_prefix_outside_known_roots() {
        let canonical_tool_dir = PathBuf::from("/home/test-user/Games/GE-Proton10-1");

        let result = plan_uninstall_with_canonical_path(
            &canonical_tool_dir,
            &[candidate(
                PathBuf::from("/home/test-user/.local/share/Steam/compatibilitytools.d"),
                InstallRootKind::NativeSteam,
            )],
            &empty_mappings(),
        );

        assert!(
            matches!(
                result,
                Err(UninstallError::PathOutsideKnownRoots(ref path)) if path == &canonical_tool_dir
            ),
            "expected PathOutsideKnownRoots for broad /home prefix, got: {result:?}"
        );
    }

    #[test]
    fn plan_uninstall_accepts_valid_home_compat_root() {
        let compat_root = PathBuf::from("/home/test-user/.local/share/Steam/compatibilitytools.d");
        let canonical_tool_dir = compat_root.join("GE-Proton10-1");

        let result = plan_uninstall_with_canonical_path(
            &canonical_tool_dir,
            &[candidate(compat_root, InstallRootKind::NativeSteam)],
            &empty_mappings(),
        );

        assert!(
            matches!(
                result,
                Ok(UninstallPlan {
                    ref tool_dir,
                    root_kind: InstallRootKind::NativeSteam,
                    ..
                }) if tool_dir == &canonical_tool_dir
            ),
            "expected valid home compat root to be accepted, got: {result:?}"
        );
    }

    // ── test 2 ────────────────────────────────────────────────────────────────

    /// A tool directory that is not under any known install root must return
    /// `PathOutsideKnownRoots`.
    #[test]
    fn plan_uninstall_refuses_path_outside_known_roots() {
        let unrelated = tempdir().unwrap();
        let tool = unrelated.path().join("SomeTool");
        fs::create_dir_all(&tool).unwrap();

        // Candidate roots that have nothing to do with `unrelated`.
        let other_root = tempdir().unwrap();
        let candidates = vec![candidate(
            other_root.path().to_path_buf(),
            InstallRootKind::NativeSteam,
        )];

        let result = plan_uninstall_with_mappings(&tool, &candidates, &empty_mappings());

        assert!(
            matches!(result, Err(UninstallError::PathOutsideKnownRoots(_))),
            "expected PathOutsideKnownRoots, got: {result:?}"
        );
    }

    // ── test 3 ────────────────────────────────────────────────────────────────

    /// When a Steam mapping exists for the tool basename, `conflicting_app_ids`
    /// must be populated with the matched App ID.
    #[test]
    fn plan_uninstall_attaches_conflicting_app_ids() {
        let root = tempdir().unwrap();
        let tool_name = "GE-Proton10-1";
        let tool_dir = root.path().join(tool_name);
        fs::create_dir_all(&tool_dir).unwrap();

        let candidates = vec![candidate(
            root.path().to_path_buf(),
            InstallRootKind::NativeSteam,
        )];
        let mappings = mappings_with("123", tool_name);

        let plan = plan_uninstall_with_mappings(&tool_dir, &candidates, &mappings)
            .expect("plan should succeed");

        assert_eq!(
            plan.conflicting_app_ids,
            vec!["123".to_string()],
            "expected App ID 123 in conflicting_app_ids"
        );
    }

    // ── test 4 ────────────────────────────────────────────────────────────────

    /// Happy-path: valid user-root child with no conflicts.
    #[test]
    fn plan_uninstall_accepts_user_root_happy_path() {
        let root = tempdir().unwrap();
        let tool_dir = root.path().join("GE-Proton9-20");
        fs::create_dir_all(&tool_dir).unwrap();

        let candidates = vec![candidate(
            root.path().to_path_buf(),
            InstallRootKind::NativeSteam,
        )];

        let plan = plan_uninstall_with_mappings(&tool_dir, &candidates, &empty_mappings())
            .expect("plan should succeed");

        assert!(
            plan.conflicting_app_ids.is_empty(),
            "expected no conflicting app IDs"
        );
        assert_eq!(plan.root_kind, InstallRootKind::NativeSteam);
    }

    // ── test 5 ────────────────────────────────────────────────────────────────

    /// A discovered compat tool may be represented by its `proton`
    /// executable path; the uninstall planner should normalize that to the
    /// containing tool directory.
    #[test]
    fn plan_uninstall_accepts_proton_executable_path() {
        let root = tempdir().unwrap();
        let tool_name = "proton-EM-10.0-36-HDRTEST";
        let tool_dir = root.path().join(tool_name);
        fs::create_dir_all(&tool_dir).unwrap();
        let proton_exe = tool_dir.join("proton");
        fs::write(&proton_exe, "#!/bin/sh\n").unwrap();

        let candidates = vec![candidate(
            root.path().to_path_buf(),
            InstallRootKind::NativeSteam,
        )];
        let mappings = mappings_with("123", tool_name);

        let plan = plan_uninstall_with_mappings(&proton_exe, &candidates, &mappings)
            .expect("plan should succeed for proton executable path");

        assert_eq!(plan.tool_dir, tool_dir);
        assert_eq!(plan.conflicting_app_ids, vec!["123".to_string()]);
    }

    // ── test 6 ────────────────────────────────────────────────────────────────

    /// `execute_uninstall` must remove the target directory tree.
    #[test]
    fn execute_uninstall_removes_directory() {
        let root = tempdir().unwrap();
        let tool_dir = root.path().join("GE-Proton9-21");
        fs::create_dir_all(tool_dir.join("files")).unwrap();
        fs::write(tool_dir.join("files/proton"), b"fake binary").unwrap();

        let candidates = vec![candidate(
            root.path().to_path_buf(),
            InstallRootKind::NativeSteam,
        )];
        let plan = plan_uninstall_with_mappings(&tool_dir, &candidates, &empty_mappings())
            .expect("plan should succeed");

        execute_uninstall(plan).expect("execute should succeed");

        assert!(
            !tool_dir.exists(),
            "tool directory should have been removed"
        );
    }

    // ── test 6 ────────────────────────────────────────────────────────────────

    /// A path that doesn't exist must return `NotFound`.
    #[test]
    fn plan_uninstall_returns_not_found_for_missing_path() {
        let root = tempdir().unwrap();
        let missing = root.path().join("nonexistent-tool");
        // Do NOT create this directory.

        let result = plan_uninstall(&missing, None);

        assert!(
            matches!(result, Err(UninstallError::NotFound(_))),
            "expected NotFound, got: {result:?}"
        );
    }
}
