//! Output-path safety guard. Runs BEFORE any `mkdir`/write so a mistyped `--out`/`--root`
//! can never touch the user's real CrossHook, XDG, or Flatpak stores.
//!
//! Refused targets:
//! - empty path, filesystem root, the real `$HOME`, or any ancestor of `$HOME`
//! - the canonical real stores (`config/crosshook`, `data/crosshook`, `cache/crosshook`,
//!   `~/.var/app/dev.crosshook.CrossHook`) and anything inside them
//! - any directory that CONTAINS a real store (writing there could clobber it)
//! - any existing symlink along the path (an ancestor could redirect the write)
//! - the XDG-env-overridden equivalents of the stores above

use std::fmt;
use std::path::{Component, Path, PathBuf};

const FLATPAK_APP_ID: &str = "dev.crosshook.CrossHook";

#[derive(Debug, PartialEq, Eq)]
pub enum GuardError {
    Empty,
    NotAbsoluteAndCwdUnavailable,
    FilesystemRoot,
    HomeOrAncestor(PathBuf),
    InsideProtectedStore { target: PathBuf, store: PathBuf },
    ContainsProtectedStore { target: PathBuf, store: PathBuf },
    SymlinkAncestor(PathBuf),
}

impl fmt::Display for GuardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "refusing empty output path"),
            Self::NotAbsoluteAndCwdUnavailable => {
                write!(
                    f,
                    "relative output path but current directory is unavailable"
                )
            }
            Self::FilesystemRoot => write!(f, "refusing filesystem root as output path"),
            Self::HomeOrAncestor(p) => {
                write!(
                    f,
                    "refusing real HOME (or an ancestor of it): {}",
                    p.display()
                )
            }
            Self::InsideProtectedStore { target, store } => write!(
                f,
                "refusing {}: inside real CrossHook store {}",
                target.display(),
                store.display()
            ),
            Self::ContainsProtectedStore { target, store } => write!(
                f,
                "refusing {}: contains real CrossHook store {}",
                target.display(),
                store.display()
            ),
            Self::SymlinkAncestor(p) => {
                write!(
                    f,
                    "refusing output path with symlink component: {}",
                    p.display()
                )
            }
        }
    }
}

impl std::error::Error for GuardError {}

/// Real-store locations to protect, derived from a given `$HOME` and optional XDG overrides.
/// Pure so tests can pass fake homes without touching process env.
pub struct ProtectedStores {
    pub home: PathBuf,
    pub stores: Vec<PathBuf>,
}

impl ProtectedStores {
    pub fn from_roots(
        home: &Path,
        xdg_config: Option<&Path>,
        xdg_data: Option<&Path>,
        xdg_cache: Option<&Path>,
    ) -> Self {
        let config = xdg_config.map_or_else(|| home.join(".config"), Path::to_path_buf);
        let data = xdg_data.map_or_else(|| home.join(".local/share"), Path::to_path_buf);
        let cache = xdg_cache.map_or_else(|| home.join(".cache"), Path::to_path_buf);
        let mut stores = vec![
            config.join("crosshook"),
            data.join("crosshook"),
            cache.join("crosshook"),
            home.join(".var/app").join(FLATPAK_APP_ID),
        ];
        // Defaults too, even when XDG overrides are set: the real stores may live in either.
        stores.push(home.join(".config/crosshook"));
        stores.push(home.join(".local/share/crosshook"));
        stores.push(home.join(".cache/crosshook"));
        stores.sort();
        stores.dedup();
        Self {
            home: home.to_path_buf(),
            stores,
        }
    }

    /// Reads the real process environment (`HOME`, `XDG_*`).
    pub fn from_env() -> Option<Self> {
        let home = std::env::var_os("HOME").map(PathBuf::from)?;
        let xdg = |k: &str| {
            std::env::var_os(k)
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
        };
        Some(Self::from_roots(
            &home,
            xdg("XDG_CONFIG_HOME").as_deref(),
            xdg("XDG_DATA_HOME").as_deref(),
            xdg("XDG_CACHE_HOME").as_deref(),
        ))
    }
}

/// Lexically normalise (`.`/`..` resolved, no filesystem access). Rejects `..` escaping root.
fn lexical_absolute(path: &Path) -> Result<PathBuf, GuardError> {
    if path.as_os_str().is_empty() {
        return Err(GuardError::Empty);
    }
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|_| GuardError::NotAbsoluteAndCwdUnavailable)?
            .join(path)
    };
    let mut out = PathBuf::new();
    for comp in abs.components() {
        match comp {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    Ok(out)
}

/// Resolves the deepest existing ancestor through symlinks, then re-appends the non-existing
/// tail. Lets us compare against real stores even if the target is reached via a symlink.
fn canonical_best_effort(path: &Path) -> PathBuf {
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    let mut cur = path.to_path_buf();
    loop {
        if let Ok(real) = cur.canonicalize() {
            let mut out = real;
            for part in tail.iter().rev() {
                out.push(part);
            }
            return out;
        }
        match (cur.file_name().map(ToOwned::to_owned), cur.parent()) {
            (Some(name), Some(parent)) => {
                tail.push(name);
                cur = parent.to_path_buf();
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// Every existing path component (prefix) must not be a symlink.
fn reject_symlink_components(path: &Path) -> Result<(), GuardError> {
    let mut cur = PathBuf::new();
    for comp in path.components() {
        cur.push(comp.as_os_str());
        if let Ok(meta) = std::fs::symlink_metadata(&cur) {
            if meta.file_type().is_symlink() {
                return Err(GuardError::SymlinkAncestor(cur));
            }
        }
    }
    Ok(())
}

/// Validates `target` against `protected`. Touches the filesystem read-only (stat/canonicalize).
/// Returns the lexically-normalised absolute path to use for all subsequent writes.
pub fn check_output_dir(target: &Path, protected: &ProtectedStores) -> Result<PathBuf, GuardError> {
    let lexical = lexical_absolute(target)?;
    if lexical.parent().is_none() {
        return Err(GuardError::FilesystemRoot);
    }
    reject_symlink_components(&lexical)?;

    let real_target = canonical_best_effort(&lexical);
    // Compare both spellings of target against both spellings of home/stores.
    let home_variants = path_variants(&protected.home);
    for t in [&lexical, &real_target] {
        if t.parent().is_none() {
            return Err(GuardError::FilesystemRoot);
        }
        for h in &home_variants {
            // target == home, or target is an ancestor of home (home.starts_with(target)).
            if h.starts_with(t) {
                return Err(GuardError::HomeOrAncestor(t.clone()));
            }
        }
        for store in &protected.stores {
            for s in path_variants(store) {
                if t.starts_with(&s) {
                    return Err(GuardError::InsideProtectedStore {
                        target: t.clone(),
                        store: s,
                    });
                }
                if s.starts_with(t) {
                    return Err(GuardError::ContainsProtectedStore {
                        target: t.clone(),
                        store: s,
                    });
                }
            }
        }
    }
    Ok(lexical)
}

fn path_variants(path: &Path) -> Vec<PathBuf> {
    let lexical = lexical_absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let real = canonical_best_effort(&lexical);
    if real == lexical {
        vec![lexical]
    } else {
        vec![lexical, real]
    }
}

/// Production entry: guard against the real environment. Fails closed when `HOME` is unset.
pub fn check_output_dir_real_env(target: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let protected =
        ProtectedStores::from_env().ok_or("HOME is not set; refusing to write fixtures")?;
    Ok(check_output_dir(target, &protected)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake(home: &Path) -> ProtectedStores {
        ProtectedStores::from_roots(home, None, None, None)
    }

    #[test]
    fn refuses_empty_and_root() {
        let p = fake(Path::new("/home/u"));
        assert_eq!(check_output_dir(Path::new(""), &p), Err(GuardError::Empty));
        assert_eq!(
            check_output_dir(Path::new("/"), &p),
            Err(GuardError::FilesystemRoot)
        );
    }

    #[test]
    fn refuses_home_and_ancestors() {
        let p = fake(Path::new("/home/u"));
        for t in ["/home/u", "/home", "/home/u/../u"] {
            assert!(
                matches!(
                    check_output_dir(Path::new(t), &p),
                    Err(GuardError::HomeOrAncestor(_))
                ),
                "{t}"
            );
        }
    }

    #[test]
    fn refuses_real_stores_and_children_and_containers() {
        let p = fake(Path::new("/home/u"));
        for t in [
            "/home/u/.config/crosshook",
            "/home/u/.config/crosshook/profiles",
            "/home/u/.local/share/crosshook/cache/images",
            "/home/u/.cache/crosshook",
            "/home/u/.var/app/dev.crosshook.CrossHook",
            "/home/u/.var/app/dev.crosshook.CrossHook/data",
        ] {
            assert!(
                matches!(
                    check_output_dir(Path::new(t), &p),
                    Err(GuardError::InsideProtectedStore { .. })
                ),
                "{t}"
            );
        }
        for t in [
            "/home/u/.config",
            "/home/u/.local/share",
            "/home/u/.var/app",
        ] {
            assert!(
                matches!(
                    check_output_dir(Path::new(t), &p),
                    Err(GuardError::ContainsProtectedStore { .. })
                ),
                "{t}"
            );
        }
    }

    #[test]
    fn refuses_xdg_overridden_stores() {
        let p = ProtectedStores::from_roots(
            Path::new("/home/u"),
            Some(Path::new("/xdg/cfg")),
            Some(Path::new("/xdg/data")),
            Some(Path::new("/xdg/cache")),
        );
        for t in [
            "/xdg/cfg/crosshook/x",
            "/xdg/data/crosshook",
            "/xdg/cache/crosshook",
        ] {
            assert!(check_output_dir(Path::new(t), &p).is_err(), "{t}");
        }
        assert!(check_output_dir(Path::new("/xdg/other/out"), &p).is_ok());
    }

    #[test]
    fn allows_unrelated_scratch_dir_without_creating_it() {
        let tmp = tempfile::tempdir().unwrap();
        let p = fake(Path::new("/home/u"));
        let target = tmp.path().join("fresh/out");
        let got = check_output_dir(&target, &p).unwrap();
        assert_eq!(got, target);
        assert!(!target.exists(), "guard must not mkdir");
    }

    #[test]
    fn refuses_symlink_ancestor() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let p = fake(Path::new("/home/u"));
        assert!(matches!(
            check_output_dir(&link.join("out"), &p),
            Err(GuardError::SymlinkAncestor(_))
        ));
    }

    #[test]
    fn refuses_symlink_that_resolves_into_real_store() {
        let tmp = tempfile::tempdir().unwrap();
        // Fake home inside tmp; store exists; symlink elsewhere points at it.
        let home = tmp.path().join("home");
        let store = home.join(".local/share/crosshook");
        std::fs::create_dir_all(&store).unwrap();
        let link = tmp.path().join("innocent");
        std::os::unix::fs::symlink(&store, &link).unwrap();
        let p = fake(&home);
        assert!(check_output_dir(&link, &p).is_err());
        assert!(check_output_dir(&link.join("deeper"), &p).is_err());
    }

    #[test]
    fn refuses_real_process_home_from_env() {
        // Whatever HOME is in this process, it must be refused when env-derived.
        if let Some(p) = ProtectedStores::from_env() {
            let home = p.home.clone();
            assert!(check_output_dir(&home, &p).is_err());
            assert!(check_output_dir(&home.join(".config/crosshook"), &p).is_err());
        }
    }
}
