//! Path helpers shared by Steam root, library and compat-tool discovery.

use std::fs;
use std::path::{Path, PathBuf};

/// Trim surrounding whitespace from a user- or VDF-supplied path.
///
/// Returns an empty path when nothing is left after trimming.
pub(crate) fn trim_path(path: &Path) -> PathBuf {
    let trimmed = path.as_os_str().to_string_lossy().trim().to_string();
    if trimmed.is_empty() {
        return PathBuf::new();
    }

    PathBuf::from(trimmed)
}

/// Resolve symlinks so one physical directory has one identity.
///
/// A standard native Steam install has `~/.steam/root` as a symlink to
/// `~/.local/share/Steam`, and `libraryfolders.vdf` lists the real path.
/// Discovery dedupes on this canonical form so the same library or Proton
/// install is not reported twice. Falls back to the raw path when the path
/// cannot be resolved (missing, or not readable).
pub(crate) fn canonical_or_raw(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::{canonical_or_raw, trim_path};
    use std::path::{Path, PathBuf};

    #[test]
    fn trim_path_strips_whitespace_and_empties_blank_input() {
        assert_eq!(trim_path(Path::new("  /a/b \n")), PathBuf::from("/a/b"));
        assert_eq!(trim_path(Path::new("   ")), PathBuf::new());
    }

    #[test]
    fn canonical_or_raw_resolves_symlinks() {
        let temp = tempfile::tempdir().expect("temp dir");
        let real = temp.path().join("real");
        let link = temp.path().join("link");
        std::fs::create_dir(&real).expect("real dir");
        std::os::unix::fs::symlink(&real, &link).expect("symlink");

        assert_eq!(
            canonical_or_raw(&link),
            std::fs::canonicalize(&real).expect("canonical real")
        );
    }

    #[test]
    fn canonical_or_raw_keeps_unresolvable_path() {
        let missing = Path::new("/does/not/exist/crosshook");
        assert_eq!(canonical_or_raw(missing), missing.to_path_buf());
    }
}
