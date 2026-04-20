use std::fs;
use std::path::{Path, PathBuf};

use directories::BaseDirs;

use crate::run_executable::RunExecutableError;

/// Root namespace under `~/.local/share/crosshook/prefixes/` for ad-hoc runs.
///
/// Underscore prefix sorts above alphanumeric profile prefixes in `ls`/`tree`,
/// making throwaway runner prefixes visually distinct from real game prefixes.
const ADHOC_PREFIX_ROOT_SEGMENT: &str = "crosshook/prefixes/_run-adhoc";

/// Default fallback slug used when an executable file stem cannot be slugified.
const ADHOC_FALLBACK_SLUG: &str = "adhoc";

/// Returns `true` when `prefix_path` is a direct child of the throwaway
/// `_run-adhoc/` namespace under the platform data-local directory — i.e. it
/// looks exactly like something [`resolve_default_adhoc_prefix_path`] would
/// have produced.
///
/// Used by the Tauri layer as a defense-in-depth guard before any
/// `remove_dir_all` / `rm -rf` against the prefix path. The check is strict:
/// the parent must be the canonical adhoc namespace root, the path must
/// have a non-empty file name, and there must be no `..` traversal in the
/// resolved chain.
pub fn is_throwaway_prefix_path(prefix_path: &Path) -> bool {
    let Some(base_dirs) = BaseDirs::new() else {
        return false;
    };
    let expected_parent = base_dirs.data_local_dir().join(ADHOC_PREFIX_ROOT_SEGMENT);

    // Reject any `..` components — a malicious or buggy slug could otherwise
    // synthesize a path that *looks* rooted under `_run-adhoc/` but actually
    // escapes via traversal once symlinks resolve.
    if prefix_path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return false;
    }

    let Some(parent) = prefix_path.parent() else {
        return false;
    };
    let Some(file_name) = prefix_path.file_name() else {
        return false;
    };

    // The slug must be non-empty and must not itself contain a separator,
    // both of which `slugify` already guarantees but we re-verify here so
    // the guard is independently sound.
    if file_name.is_empty() {
        return false;
    }

    parent == expected_parent.as_path()
}

/// Resolves the default `_run-adhoc/<slug>` prefix path for an executable.
///
/// Returns [`RunExecutableError::HomeDirectoryUnavailable`] when no platform
/// home directory can be located (e.g. headless CI without `$HOME`).
pub fn resolve_default_adhoc_prefix_path(
    executable_path: &Path,
) -> Result<PathBuf, RunExecutableError> {
    let base_dirs = BaseDirs::new().ok_or(RunExecutableError::HomeDirectoryUnavailable)?;
    Ok(resolve_default_adhoc_prefix_path_from_data_local_dir(
        base_dirs.data_local_dir(),
        executable_path,
    ))
}

pub(crate) fn resolve_default_adhoc_prefix_path_from_data_local_dir(
    data_local_dir: &Path,
    executable_path: &Path,
) -> PathBuf {
    let stem = executable_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    data_local_dir
        .join(ADHOC_PREFIX_ROOT_SEGMENT)
        .join(slugify(stem))
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

fn slugify(name: &str) -> String {
    let slug: String = name
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
    let trimmed = slug.trim_matches('-').to_string();
    if trimmed.is_empty() {
        ADHOC_FALLBACK_SLUG.to_string()
    } else {
        trimmed
    }
}
