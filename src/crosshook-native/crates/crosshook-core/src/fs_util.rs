//! Filesystem utility helpers shared across crate modules.

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn copies_empty_dir() {
        let t = tempdir().unwrap();
        let src = t.path().join("src");
        let dst = t.path().join("dst");
        fs::create_dir_all(&src).unwrap();

        copy_dir_recursive(&src, &dst).unwrap();

        assert!(dst.exists(), "dst directory should exist");
        let mut entries = fs::read_dir(&dst).unwrap();
        assert!(entries.next().is_none(), "dst should be empty");
    }

    #[test]
    fn copies_nested_files() {
        let t = tempdir().unwrap();
        let src = t.path().join("src");
        let dst = t.path().join("dst");

        fs::create_dir_all(src.join("a/b")).unwrap();
        fs::write(src.join("a/b/c.txt"), "héllo wörld — unicode content").unwrap();

        copy_dir_recursive(&src, &dst).unwrap();

        let content = fs::read_to_string(dst.join("a/b/c.txt")).unwrap();
        assert_eq!(content, "héllo wörld — unicode content");
    }

    #[cfg(unix)]
    #[test]
    fn preserves_symlinks() {
        use std::os::unix::fs::symlink;

        let t = tempdir().unwrap();
        let src = t.path().join("src");
        let dst = t.path().join("dst");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("target.txt"), b"data").unwrap();
        symlink("target.txt", src.join("link.txt")).unwrap();

        copy_dir_recursive(&src, &dst).unwrap();

        assert!(
            dst.join("link.txt").is_symlink(),
            "dst entry should remain a symlink"
        );
        let link_target = fs::read_link(dst.join("link.txt")).unwrap();
        assert_eq!(
            link_target,
            std::path::PathBuf::from("target.txt"),
            "symlink target must not be dereferenced"
        );
    }

    #[test]
    fn handles_unicode_names() {
        let t = tempdir().unwrap();
        let src = t.path().join("src");
        let dst = t.path().join("dst");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("résumé.txt"), b"cv data").unwrap();

        copy_dir_recursive(&src, &dst).unwrap();

        assert!(
            dst.join("résumé.txt").exists(),
            "unicode filename must be copied verbatim"
        );
        assert_eq!(fs::read(dst.join("résumé.txt")).unwrap(), b"cv data");
    }

    #[test]
    fn dir_is_empty_true_for_empty_dir() {
        let t = tempdir().unwrap();
        let dir = t.path().join("empty");
        fs::create_dir_all(&dir).unwrap();

        assert!(dir_is_empty(&dir).unwrap());
    }

    #[test]
    fn dir_is_empty_false_when_populated() {
        let t = tempdir().unwrap();
        let dir = t.path().join("nonempty");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("file.txt"), b"x").unwrap();

        assert!(!dir_is_empty(&dir).unwrap());
    }

    #[test]
    fn dir_is_empty_propagates_notfound() {
        let t = tempdir().unwrap();
        let nonexistent = t.path().join("does_not_exist");

        let result = dir_is_empty(&nonexistent);
        assert!(result.is_err(), "expected an error for nonexistent path");
        assert_eq!(
            result.unwrap_err().kind(),
            io::ErrorKind::NotFound,
            "expected NotFound error kind"
        );
    }

    fn temp_files_in(dir: &Path) -> Vec<String> {
        fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".tmp-"))
            .collect()
    }

    #[test]
    fn write_atomic_creates_new_file_0644() {
        let t = tempdir().unwrap();
        let path = t.path().join("settings.toml");

        write_atomic(&path, b"hello").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"hello");
        assert!(temp_files_in(t.path()).is_empty(), "temp file leaked");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o644);
        }
    }

    #[test]
    fn write_atomic_preserves_existing_permissions() {
        let t = tempdir().unwrap();
        let path = t.path().join("data.toml");
        fs::write(&path, b"old").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        }

        write_atomic(&path, b"new").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"new");
        assert!(temp_files_in(t.path()).is_empty(), "temp file leaked");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[cfg(unix)]
    #[test]
    fn write_atomic_follows_symlink_to_target() {
        use std::os::unix::fs::symlink;

        let t = tempdir().unwrap();
        let real = t.path().join("real.toml");
        fs::write(&real, b"old").unwrap();
        let link = t.path().join("link.toml");
        symlink("real.toml", &link).unwrap();

        write_atomic(&link, b"via-link").unwrap();

        assert!(link.is_symlink(), "symlink must be preserved, not replaced");
        assert_eq!(fs::read_link(&link).unwrap(), PathBuf::from("real.toml"));
        assert_eq!(fs::read(&real).unwrap(), b"via-link");
        assert!(temp_files_in(t.path()).is_empty(), "temp file leaked");
    }

    #[cfg(unix)]
    #[test]
    fn write_atomic_rejects_dangling_symlink_without_replacement() {
        use std::os::unix::fs::symlink;

        let t = tempdir().unwrap();
        let link = t.path().join("dangling.toml");
        symlink("nowhere.toml", &link).unwrap();

        let err = write_atomic(&link, b"x").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
        assert!(link.is_symlink(), "dangling symlink must stay a symlink");
        assert_eq!(fs::read_link(&link).unwrap(), PathBuf::from("nowhere.toml"));
        assert!(!t.path().join("nowhere.toml").exists());
        assert!(
            temp_files_in(t.path()).is_empty(),
            "no temp file should remain after failure"
        );
        assert!(
            err.to_string().contains("symlink"),
            "error should mention the symlink: {err}"
        );
    }

    #[test]
    fn write_atomic_failure_before_rename_keeps_original_and_cleans_temp() {
        let t = tempdir().unwrap();
        let path = t.path().join("profile.toml");
        fs::write(&path, b"original").unwrap();

        let err = write_atomic_inner(&path, b"clobbered", || {
            assert_eq!(temp_files_in(t.path()).len(), 1, "temp should be staged");
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "injected"))
        })
        .unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(
            fs::read(&path).unwrap(),
            b"original",
            "original content must survive a pre-rename failure"
        );
        assert!(
            temp_files_in(t.path()).is_empty(),
            "temp file must be removed after a pre-rename failure"
        );
    }

    #[test]
    fn write_atomic_handles_max_length_multibyte_name() {
        let t = tempdir().unwrap();
        // 85 * 3 bytes = 255 bytes: valid name, but too long with temp affixes.
        let path = t.path().join("€".repeat(85));

        write_atomic(&path, b"long").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"long");
        assert!(temp_files_in(t.path()).is_empty());
        assert_eq!(truncate_on_char_boundary("aé", 2), "a");
    }

    #[test]
    fn write_atomic_replaces_content_and_double_write_is_byte_identical() {
        let t = tempdir().unwrap();
        let path = t.path().join("p.toml");

        write_atomic(&path, b"v1").unwrap();
        let first = fs::read(&path).unwrap();
        write_atomic(&path, b"v1").unwrap();
        let second = fs::read(&path).unwrap();
        assert_eq!(first, second);
        write_atomic(&path, b"v2").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"v2");
        assert!(temp_files_in(t.path()).is_empty());
    }
}
