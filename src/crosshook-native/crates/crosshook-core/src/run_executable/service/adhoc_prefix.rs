use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::run_executable::RunExecutableError;

/// Directory name of the throwaway namespace inside the prefix root.
///
/// Underscore prefix sorts above alphanumeric profile prefixes in `ls`/`tree`,
/// making throwaway runner prefixes visually distinct from real game prefixes.
const ADHOC_ROOT_DIR: &str = "_run-adhoc";

/// Prefix root relative to the platform data-local directory.
const PREFIXES_SEGMENT: &str = "crosshook/prefixes";

/// Default fallback slug used when an executable file stem cannot be slugified.
const ADHOC_FALLBACK_SLUG: &str = "adhoc";

/// Returns the throwaway `_run-adhoc` namespace root.
///
/// Under Flatpak this is `<host prefix root>/_run-adhoc`; otherwise it is
/// `<data_local>/crosshook/prefixes/_run-adhoc`. Returns
/// [`RunExecutableError::HomeDirectoryUnavailable`] when no platform home
/// directory can be located (e.g. headless CI without `$HOME`).
pub fn adhoc_prefix_root() -> Result<PathBuf, RunExecutableError> {
    adhoc_prefix_root_with(crate::flatpak_migration::host_prefix_root())
}

pub(crate) fn adhoc_prefix_root_with(
    host_override: Option<PathBuf>,
) -> Result<PathBuf, RunExecutableError> {
    if let Some(root) = host_override {
        return Ok(root.join(ADHOC_ROOT_DIR));
    }
    let base_dirs =
        crate::platform::base_dirs().ok_or(RunExecutableError::HomeDirectoryUnavailable)?;
    Ok(adhoc_prefix_root_from_data_local_dir(
        base_dirs.data_local_dir(),
    ))
}

fn adhoc_prefix_root_from_data_local_dir(data_local_dir: &Path) -> PathBuf {
    data_local_dir.join(PREFIXES_SEGMENT).join(ADHOC_ROOT_DIR)
}

/// Resolves a per-run throwaway prefix `<adhoc_root>/<slug>-<run_suffix>`.
///
/// The slug comes from the executable file stem. The suffix makes every run
/// unique, so two runs of executables with the same stem never share (and
/// later delete) the same prefix. Both parts are reduced to `[a-z0-9-]`, so
/// the result is always a direct child of `adhoc_root`.
pub fn resolve_adhoc_prefix_path(
    adhoc_root: &Path,
    executable_path: &Path,
    run_suffix: &str,
) -> PathBuf {
    let slug = slugify(executable_stem(executable_path));
    let suffix = sanitize_component(run_suffix);
    if suffix.is_empty() {
        adhoc_root.join(slug)
    } else {
        adhoc_root.join(format!("{slug}-{suffix}"))
    }
}

/// Returns `true` when `prefix_path` is a direct child of the throwaway
/// namespace `adhoc_root` — i.e. it looks exactly like something
/// [`resolve_adhoc_prefix_path`] would have produced. The parent must be
/// exactly `adhoc_root`, the file name must be non-empty, and no component
/// may be `..`.
///
/// Used as a defense-in-depth guard before any `remove_dir_all` against the
/// prefix path.
pub fn is_throwaway_prefix_path_in(prefix_path: &Path, adhoc_root: &Path) -> bool {
    // Reject any `..` components — a malicious or buggy slug could otherwise
    // synthesize a path that *looks* rooted under `_run-adhoc/` but actually
    // escapes via traversal once symlinks resolve.
    if prefix_path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return false;
    }

    let (Some(parent), Some(file_name)) = (prefix_path.parent(), prefix_path.file_name()) else {
        return false;
    };
    if file_name.is_empty() {
        return false;
    }

    parent == adhoc_root
}

pub(crate) fn provision_prefix(prefix_path: &Path) -> Result<(), RunExecutableError> {
    if let Ok(metadata) = fs::metadata(prefix_path) {
        if !metadata.is_dir() {
            return Err(RunExecutableError::PrefixCreationFailed {
                path: prefix_path.to_path_buf(),
                message: "Path exists but is not a directory.".to_string(),
            });
        }
        return Ok(());
    }

    fs::create_dir_all(prefix_path).map_err(|error| RunExecutableError::PrefixCreationFailed {
        path: prefix_path.to_path_buf(),
        message: error.to_string(),
    })
}

fn executable_stem(executable_path: &Path) -> &str {
    executable_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("")
}

/// Lowercases ASCII alphanumerics, maps everything else to `-` and trims
/// leading/trailing dashes.
fn sanitize_component(name: &str) -> String {
    let mapped: String = name
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    mapped.trim_matches('-').to_string()
}

fn slugify(name: &str) -> String {
    let slug = sanitize_component(name);
    if slug.is_empty() {
        ADHOC_FALLBACK_SLUG.to_string()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOST_ROOT: &str = "/h/.local/share/crosshook/prefixes";

    #[test]
    fn adhoc_prefix_root_with_host_override_uses_override_root() {
        let root = adhoc_prefix_root_with(Some(PathBuf::from(HOST_ROOT))).expect("root");
        assert_eq!(root, PathBuf::from(HOST_ROOT).join("_run-adhoc"));
    }

    #[test]
    fn adhoc_prefix_root_without_override_falls_through_to_base_dirs() {
        let root = adhoc_prefix_root_with(None).expect("root without override");
        assert!(
            root.ends_with("crosshook/prefixes/_run-adhoc"),
            "expected path ending in 'crosshook/prefixes/_run-adhoc', got '{}'",
            root.display()
        );
    }

    #[test]
    fn resolve_adhoc_prefix_path_gives_distinct_dirs_per_suffix_under_one_root() {
        let root = PathBuf::from(HOST_ROOT).join("_run-adhoc");
        let executable = Path::new("/media/Setup Wizard.exe");

        let first = resolve_adhoc_prefix_path(&root, executable, "0a1b2c3d4e5f");
        let second = resolve_adhoc_prefix_path(&root, executable, "f5e4d3c2b1a0");

        assert_eq!(first, root.join("setup-wizard-0a1b2c3d4e5f"));
        assert_eq!(second, root.join("setup-wizard-f5e4d3c2b1a0"));
        assert_ne!(first, second);
        assert_eq!(first.parent(), Some(root.as_path()));
        assert_eq!(second.parent(), Some(root.as_path()));
    }

    #[test]
    fn resolve_adhoc_prefix_path_sanitizes_suffix_to_a_single_component() {
        let root = PathBuf::from("/data/_run-adhoc");
        let resolved = resolve_adhoc_prefix_path(&root, Path::new("/x/---.exe"), "../Evil/x");
        assert_eq!(resolved, root.join("adhoc-evil-x"));
        assert!(is_throwaway_prefix_path_in(&resolved, &root));
    }

    #[test]
    fn is_throwaway_prefix_path_in_accepts_slug_with_suffix() {
        let root = PathBuf::from("/data/crosshook/prefixes/_run-adhoc");
        assert!(is_throwaway_prefix_path_in(
            &root.join("setup-0a1b2c3d4e5f"),
            &root
        ));
    }

    #[test]
    fn is_throwaway_prefix_path_in_rejects_root_traversal_sibling_and_grandchild() {
        let root = PathBuf::from("/data/crosshook/prefixes/_run-adhoc");
        let rejected = [
            root.clone(),
            root.join("..").join("real-game"),
            root.join("setup-abc").join("..").join("other"),
            PathBuf::from("/data/crosshook/prefixes/real-game"),
            PathBuf::from("/data/crosshook/prefixes/_run-adhoc-other/setup-abc"),
            root.join("setup-abc").join("drive_c"),
            PathBuf::from("/"),
        ];
        for candidate in rejected {
            assert!(
                !is_throwaway_prefix_path_in(&candidate, &root),
                "must NOT be classified as throwaway: {}",
                candidate.display()
            );
        }
    }
}
