//! Canonical fixture tree layout. Paths under `<out>` mirror an isolated XDG root so the
//! app works with:
//!   XDG_CONFIG_HOME=<root>/config  XDG_DATA_HOME=<root>/data
//!   XDG_CACHE_HOME=<root>/cache    XDG_STATE_HOME=<root>/state  HOME=<root>/home
//!
//! All persisted paths inside generated content use `@CROSSHOOK_BENCH_ROOT@/...` so a
//! canonical tree is byte-identical regardless of where it was generated; `materialize`
//! rewrites the token to a real root.

use std::path::{Path, PathBuf};

pub const TOKEN: &str = "@CROSSHOOK_BENCH_ROOT@";

#[derive(Clone)]
pub struct Layout {
    pub out: PathBuf,
    pub config_dir: PathBuf,         // <out>/config/crosshook
    pub profiles_dir: PathBuf,       // <out>/config/crosshook/profiles
    pub data_dir: PathBuf,           // <out>/data/crosshook
    pub metadata_db: PathBuf,        // <out>/data/crosshook/metadata.db
    pub image_cache_dir: PathBuf,    // <out>/data/crosshook/cache/images
    pub community_taps_dir: PathBuf, // <out>/data/crosshook/community/taps
    pub logs_dir: PathBuf,           // <out>/logs
    pub manifest: PathBuf,           // <out>/manifest.json
}

impl Layout {
    pub fn new(out: &Path) -> Self {
        let out = out.to_path_buf();
        let config_dir = out.join("config/crosshook");
        let data_dir = out.join("data/crosshook");
        Self {
            profiles_dir: config_dir.join("profiles"),
            metadata_db: data_dir.join("metadata.db"),
            image_cache_dir: data_dir.join("cache/images"),
            community_taps_dir: data_dir.join("community/taps"),
            logs_dir: out.join("logs"),
            manifest: out.join("manifest.json"),
            config_dir,
            data_dir,
            out,
        }
    }

    /// `<token>/...` string for a path inside the canonical tree (forward slashes, portable).
    pub fn tokenize(&self, path: &Path) -> String {
        let rel = path
            .strip_prefix(&self.out)
            .unwrap_or_else(|_| panic!("{} not under fixture root", path.display()));
        format!("{TOKEN}/{}", rel.to_string_lossy().replace('\\', "/"))
    }

    pub fn create_base_dirs(&self) -> std::io::Result<()> {
        for d in [
            &self.config_dir,
            &self.profiles_dir,
            &self.data_dir,
            &self.image_cache_dir,
            &self.logs_dir,
            &self.out.join("cache"),
            &self.out.join("state"),
            &self.out.join("home"),
        ] {
            std::fs::create_dir_all(d)?;
        }
        Ok(())
    }
}

/// Sorted list of `(relative path, bytes)` for every file under `root`. Test-only; the
/// CLI does not use it.
#[cfg(test)]
pub fn snapshot_tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    collect(root, root, &mut out);
    out.sort();
    out
}

#[cfg(test)]
fn collect(root: &Path, dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
        .map(|e| e.expect("direntry").path())
        .collect();
    entries.sort();
    for path in entries {
        let meta = std::fs::symlink_metadata(&path).expect("stat");
        if meta.is_dir() {
            collect(root, &path, out);
        } else if meta.is_file() {
            let rel = path.strip_prefix(root).expect("under root").to_path_buf();
            let bytes =
                std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            out.push((rel, bytes));
        } else {
            panic!("unexpected symlink in fixture tree: {}", path.display());
        }
    }
}
