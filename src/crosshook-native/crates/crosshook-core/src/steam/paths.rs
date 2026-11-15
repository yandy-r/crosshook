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
/// install is not reported twice.
///
/// When the path itself does not exist (for example a game executable that
/// was removed), the longest existing ancestor is resolved and the missing
/// tail is appended, so the result still lines up with canonical library
/// paths. Falls back to the raw path when no ancestor can be resolved.
pub(crate) fn canonical_or_raw(path: &Path) -> PathBuf {
    let mut missing_tail = Vec::new();
    let mut current = path;
    loop {
        if let Ok(resolved) = fs::canonicalize(current) {
            return missing_tail
                .iter()
                .rev()
                .fold(resolved, |resolved, part| resolved.join(part));
        }
        match (current.parent(), current.file_name()) {
            (Some(parent), Some(name)) if !parent.as_os_str().is_empty() => {
                missing_tail.push(name.to_os_string());
                current = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
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
    fn canonical_or_raw_resolves_existing_ancestor_of_missing_path() {
        let temp = tempfile::tempdir().expect("temp dir");
        let real = temp.path().join("real");
        let link = temp.path().join("link");
        std::fs::create_dir(&real).expect("real dir");
        std::os::unix::fs::symlink(&real, &link).expect("symlink");

        assert_eq!(
            canonical_or_raw(&link.join("missing/game.exe")),
            std::fs::canonicalize(&real)
                .expect("canonical real")
                .join("missing/game.exe")
        );
    }

    #[test]
    fn canonical_or_raw_keeps_unresolvable_relative_path() {
        let relative = Path::new("crosshook-missing-dir/crosshook-missing-file");
        assert_eq!(canonical_or_raw(relative), relative.to_path_buf());
    }
}
