//! Filesystem utility helpers shared across crate modules.

use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

/// Atomically replaces the file at `path` with `bytes`.
///
/// Steps: when `path` itself is a symlink, canonicalize it (resolving parent and
/// final-component symlinks; writes follow the link and the link itself is kept),
/// write a same-directory temp file named `.<name>.tmp-<pid>-<rand>` (`<name>`
/// truncated to at most 64 bytes on a char boundary), write all bytes, copy the existing file's
/// permissions (new files get `0644` on Unix), sync the temp file, rename it over
/// the target, then fsync the parent directory on Unix.
///
/// # Errors
///
/// - Any failure **before** the rename leaves the original file untouched and the
///   temp file is removed.
/// - A dangling (or looping) symlink at `path` returns an error; it is never
///   replaced by a regular file.
/// - If the rename succeeded but the parent-directory fsync fails, the new content
///   is already in place and an error is still returned (durability across power
///   loss is unconfirmed, the file is not rolled back).
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_atomic_inner(
        path,
        bytes,
        #[cfg(test)]
        || Ok(()),
    )
}

/// Keeps the temp name well under `NAME_MAX` (255 bytes) for long target names.
const TEMP_NAME_MAX_BYTES: usize = 64;

fn truncate_on_char_boundary(value: &str, max_bytes: usize) -> &str {
    let mut end = value.len().min(max_bytes);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

fn resolve_write_target(path: &Path) -> io::Result<PathBuf> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => fs::canonicalize(path).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "refusing to replace unresolvable symlink {}: {error}",
                    path.display()
                ),
            )
        }),
        Ok(_) => Ok(path.to_path_buf()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(path.to_path_buf()),
        Err(error) => Err(error),
    }
}

fn write_atomic_inner(
    path: &Path,
    bytes: &[u8],
    #[cfg(test)] before_persist: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    let target = resolve_write_target(path)?;
    let name = target.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("path has no file name: {}", target.display()),
        )
    })?;
    let dir = match target.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };

    let existing_permissions = match fs::metadata(&target) {
        Ok(meta) => Some(meta.permissions()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };

    let prefix = format!(
        ".{}.tmp-{}-",
        truncate_on_char_boundary(&name.to_string_lossy(), TEMP_NAME_MAX_BYTES),
        std::process::id()
    );
    let mut temp = tempfile::Builder::new()
        .prefix(&prefix)
        .rand_bytes(8)
        .tempfile_in(&dir)?;
    temp.write_all(bytes)?;

    match existing_permissions {
        Some(permissions) => temp.as_file().set_permissions(permissions)?,
        None => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                temp.as_file()
                    .set_permissions(fs::Permissions::from_mode(0o644))?;
            }
        }
    }
    temp.as_file().sync_all()?;

    #[cfg(test)]
    before_persist()?;
    // Dropping the PersistError removes the temp file.
    temp.persist(&target).map_err(|error| error.error)?;

    #[cfg(unix)]
    fs::File::open(dir)?.sync_all()?;

    Ok(())
}

/// Recursively copies a directory tree from `src` to `dst`.
///
/// Creates `dst` if it does not exist. Symlinks are preserved as symlinks
/// (not dereferenced). Files are copied byte-for-byte.
pub(crate) fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let meta = path.symlink_metadata()?;
        let file_type = meta.file_type();
        let dest = dst.join(entry.file_name());
        if file_type.is_symlink() {
            copy_symlink(&path, &dest)?;
        } else if file_type.is_dir() {
            copy_dir_recursive(&path, &dest)?;
        } else {
            fs::copy(&path, &dest)?;
        }
    }
    Ok(())
}

/// Copies a symlink from `src` to `dst`, preserving the link target without
/// dereferencing.
///
/// # Safety filter
///
/// Symlinks whose target is absolute or whose target path contains any
/// [`Component::ParentDir`] (`..`) component are rejected with an
/// [`io::ErrorKind::InvalidInput`] error and a `tracing::warn!`. This
/// prevents a malicious or accidental symlink in the source tree (e.g.
/// `/run/host/etc/passwd` or `../../sensitive`) from being reproduced in the
/// destination and later followed by sandbox I/O.
fn copy_symlink(link: &Path, dest: &Path) -> io::Result<()> {
    let target = fs::read_link(link)?;

    // Reject absolute targets and targets that contain `..` components.
    let is_absolute = target.is_absolute();
    let has_parent_dir = target.components().any(|c| c == Component::ParentDir);
    if is_absolute || has_parent_dir {
        tracing::warn!(
            link = %link.display(),
            target = %target.display(),
            "skipping unsafe symlink (absolute or parent-traversing target)"
        );
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "unsafe symlink at {}: target '{}' is absolute or contains '..'",
                link.display(),
                target.display()
            ),
        ));
    }

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&target, dest)
    }
    #[cfg(windows)]
    {
        let target_is_dir = fs::metadata(link)?.is_dir();
        if target_is_dir {
            std::os::windows::fs::symlink_dir(target, dest)
        } else {
            std::os::windows::fs::symlink_file(target, dest)
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (link, dest);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "symlink copy not supported on this platform",
        ))
    }
}

/// Returns `Ok(true)` if `path` is an empty directory, `Ok(false)` if it has
/// at least one entry, or an `Err` if `path` does not exist or cannot be read.
pub(crate) fn dir_is_empty(path: &Path) -> io::Result<bool> {
    let mut it = fs::read_dir(path)?;
    Ok(it.next().is_none())
}

/// Why [`ensure_within_workspace`] refused a candidate path.
///
/// [`std::fmt::Display`] never embeds a raw filesystem path, so the error can
/// cross an IPC boundary without leaking local layout details.
#[derive(Debug)]
pub enum WorkspacePathError {
    /// The root is missing or is not an existing directory.
    InvalidRoot,
    /// The candidate contained a `..` component, even one that resolves back inside.
    Traversal,
    /// The candidate does not component-prefix the root, or resolves outside it.
    OutsideWorkspace,
    /// The candidate (or a dangling symlink along it) does not exist; the helper
    /// resolves existing paths only.
    NotFound,
    /// Permission, ELOOP, ENOTDIR or another I/O failure while resolving.
    Io {
        /// Short lowercase verb phrase naming the failing operation.
        operation: &'static str,
        /// Underlying I/O error.
        source: std::io::Error,
    },
}

impl fmt::Display for WorkspacePathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRoot => f.write_str("workspace root is not an available directory"),
            Self::Traversal => f.write_str("path may not contain parent-directory references"),
            Self::OutsideWorkspace => f.write_str("path is not inside the workspace"),
            Self::NotFound => f.write_str("path was not found"),
            Self::Io { operation, source } => {
                write!(f, "failed to {operation}: {}", source.kind())
            }
        }
    }
}

impl std::error::Error for WorkspacePathError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Resolves `candidate` against `root` and returns the canonical, contained path.
///
/// Semantics, in order:
///
/// 1. `root` is canonicalized once and must be an existing directory
///    ([`WorkspacePathError::InvalidRoot`] otherwise). A trusted root symlink
///    resolves to its real location.
/// 2. Any `..` component in the **original** candidate is rejected as
///    [`WorkspacePathError::Traversal`], even when the path would resolve back
///    inside; an empty candidate is [`WorkspacePathError::OutsideWorkspace`].
///    No case folding, Unicode normalization or `~` expansion is performed.
/// 3. A relative candidate resolves from the canonical root, never from the
///    process working directory; an absolute candidate must component-prefix
///    the supplied or the canonical root spelling.
/// 4. Each remaining component is walked from the canonical root with
///    `symlink_metadata`; symlinks resolve and must stay inside the root at
///    that prefix (an intermediate link escaping the root fails there, even if
///    a later link would come back). Intermediate components must be
///    directories; missing components and dangling links are
///    [`WorkspacePathError::NotFound`].
/// 5. The final target is canonicalized again and must component-wise prefix
///    the canonical root.
///
/// A candidate equal to the root returns the canonical root itself; callers
/// must still require a regular file before reading. This is a point-in-time
/// check, not a race-free open (YAN-835 D11).
pub fn ensure_within_workspace(
    root: &Path,
    candidate: &Path,
) -> Result<PathBuf, WorkspacePathError> {
    const RESOLVE: &str = "resolve workspace path";

    // (1) canonicalize the root once; it must be an existing directory.
    let canonical_root = fs::canonicalize(root).map_err(|source| match source.kind() {
        io::ErrorKind::NotFound => WorkspacePathError::InvalidRoot,
        _ => WorkspacePathError::Io {
            operation: RESOLVE,
            source,
        },
    })?;
    if !fs::symlink_metadata(&canonical_root)
        .map_err(|source| WorkspacePathError::Io {
            operation: RESOLVE,
            source,
        })?
        .is_dir()
    {
        return Err(WorkspacePathError::InvalidRoot);
    }

    // (2) reject raw parent references in the original candidate, always.
    if candidate
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(WorkspacePathError::Traversal);
    }
    if candidate.as_os_str().is_empty() {
        return Err(WorkspacePathError::OutsideWorkspace);
    }

    // (3) resolve the candidate relative to the canonical root.
    let suffix: Vec<OsString> = if candidate.is_absolute() {
        let stripped = candidate
            .strip_prefix(root)
            .or_else(|_| candidate.strip_prefix(&canonical_root))
            .map_err(|_| WorkspacePathError::OutsideWorkspace)?;
        normal_components(stripped)
    } else {
        normal_components(candidate)
    };

    // (4) walk each component, resolving links as they are encountered.
    let mut current = canonical_root.clone();
    for (index, component) in suffix.iter().enumerate() {
        let next = current.join(component);
        let metadata = fs::symlink_metadata(&next).map_err(|source| match source.kind() {
            io::ErrorKind::NotFound => WorkspacePathError::NotFound,
            _ => WorkspacePathError::Io {
                operation: RESOLVE,
                source,
            },
        })?;
        let file_type = metadata.file_type();
        current = if file_type.is_symlink() {
            fs::canonicalize(&next).map_err(|source| match source.kind() {
                io::ErrorKind::NotFound => WorkspacePathError::NotFound,
                _ => WorkspacePathError::Io {
                    operation: RESOLVE,
                    source,
                },
            })?
        } else {
            next
        };
        if !current.starts_with(&canonical_root) {
            return Err(WorkspacePathError::OutsideWorkspace);
        }
        let resolved_is_dir = if file_type.is_symlink() {
            fs::symlink_metadata(&current)
                .map_err(|source| WorkspacePathError::Io {
                    operation: RESOLVE,
                    source,
                })?
                .is_dir()
        } else {
            file_type.is_dir()
        };
        if index + 1 < suffix.len() && !resolved_is_dir {
            return Err(WorkspacePathError::Io {
                operation: RESOLVE,
                source: io::Error::new(
                    io::ErrorKind::NotADirectory,
                    "workspace path component is not a directory",
                ),
            });
        }
    }

    // (5) mandatory final canonical containment check.
    let canonical = fs::canonicalize(&current).map_err(|source| match source.kind() {
        io::ErrorKind::NotFound => WorkspacePathError::NotFound,
        _ => WorkspacePathError::Io {
            operation: RESOLVE,
            source,
        },
    })?;
    if !canonical.starts_with(&canonical_root) {
        return Err(WorkspacePathError::OutsideWorkspace);
    }
    Ok(canonical)
}

/// Collects only the normal components of `path`, dropping `.`, root and
/// prefix markers. Parent components have already been rejected by the caller.
fn normal_components(path: &Path) -> Vec<OsString> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_os_string()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod workspace_tests;

#[cfg(test)]
mod tests;
