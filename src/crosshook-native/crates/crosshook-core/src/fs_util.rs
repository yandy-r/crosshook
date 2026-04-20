//! Filesystem utility helpers shared across crate modules.

use std::fs;
use std::io;
use std::path::Path;

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
fn copy_symlink(link: &Path, dest: &Path) -> io::Result<()> {
    let target = fs::read_link(link)?;
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
}
