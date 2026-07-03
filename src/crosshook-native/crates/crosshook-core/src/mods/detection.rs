//! Bounded, read-only detection scan for well-known mod artifacts inside the
//! game install directory. Existence/name checks only — file contents are
//! never opened, symlinks are never followed, and nothing is executed.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::Path;

use super::model::{ModCategory, ProfileModRecord};

/// Proxy-loader DLL basenames commonly used as in-process load vectors.
/// Shared with `coexistence.rs` for the injection-vector rule.
pub(crate) const PROXY_DLL_NAMES: [&str; 6] = [
    "dxgi.dll",
    "d3d11.dll",
    "d3d9.dll",
    "dinput8.dll",
    "winmm.dll",
    "version.dll",
];

/// Hard cap on directory entries examined per scan (root + allowlisted subdirs).
pub const MAX_SCAN_ENTRIES: u32 = 2048;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectedModCandidate {
    /// Stable id: "reshade", "enb", "proxy_dll:dxgi.dll", "script_extender:skse64".
    pub detector_id: String,
    pub suggested_name: String,
    pub category: ModCategory,
    /// Relative to the scan root, sorted lexicographically.
    pub matched_paths: Vec<String>,
    /// Pure scan always emits false; the IPC layer fills it from the DB.
    #[serde(default)]
    pub already_registered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectionScanReport {
    pub scanned_root: String,
    pub candidates: Vec<DetectedModCandidate>,
    pub entries_scanned: u32,
    pub truncated: bool,
}

#[derive(Debug)]
pub enum ModDetectionError {
    /// Root missing, not a directory, or unreadable.
    RootUnreadable {
        path: String,
        source: std::io::Error,
    },
}

impl fmt::Display for ModDetectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootUnreadable { path, source } => {
                write!(f, "game directory is not readable: {path}: {source}")
            }
        }
    }
}

impl std::error::Error for ModDetectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RootUnreadable { source, .. } => Some(source),
        }
    }
}

/// One row per known script extender; adding a detector is adding a row.
struct ScriptExtenderRule {
    id: &'static str,
    loader: &'static str,
    display: &'static str,
    /// `Data/<dir>/` marker folded into `matched_paths` when the loader matched.
    data_dir: Option<&'static str>,
    /// Companion DLL prefix (e.g. `skse64_*.dll`) folded into `matched_paths`.
    companion_prefix: Option<&'static str>,
}

const SCRIPT_EXTENDERS: [ScriptExtenderRule; 7] = [
    ScriptExtenderRule {
        id: "skse",
        loader: "skse_loader.exe",
        display: "SKSE",
        data_dir: Some("SKSE"),
        companion_prefix: None,
    },
    ScriptExtenderRule {
        id: "skse64",
        loader: "skse64_loader.exe",
        display: "SKSE64",
        data_dir: Some("SKSE"),
        companion_prefix: Some("skse64_"),
    },
    ScriptExtenderRule {
        id: "sksevr",
        loader: "sksevr_loader.exe",
        display: "SKSE VR",
        data_dir: Some("SKSE"),
        companion_prefix: Some("sksevr_"),
    },
    ScriptExtenderRule {
        id: "f4se",
        loader: "f4se_loader.exe",
        display: "F4SE",
        data_dir: Some("F4SE"),
        companion_prefix: Some("f4se_"),
    },
    ScriptExtenderRule {
        id: "obse",
        loader: "obse_loader.exe",
        display: "OBSE",
        data_dir: Some("OBSE"),
        companion_prefix: Some("obse_"),
    },
    ScriptExtenderRule {
        id: "nvse",
        loader: "nvse_loader.exe",
        display: "NVSE",
        data_dir: Some("NVSE"),
        companion_prefix: Some("nvse_"),
    },
    ScriptExtenderRule {
        id: "mwse",
        loader: "mwse.dll",
        display: "MWSE",
        data_dir: None,
        companion_prefix: None,
    },
];

#[derive(Default)]
struct RootListing {
    /// Regular files in the root, original casing.
    files: Vec<String>,
    /// Directories in the root, original casing.
    dirs: Vec<String>,
    /// `Data/<subdir>` markers found, as root-relative paths (original casing).
    data_subdirs: Vec<String>,
    entries_scanned: u32,
    truncated: bool,
}

impl RootListing {
    fn file(&self, name: &str) -> Option<&str> {
        self.files
            .iter()
            .find(|f| f.eq_ignore_ascii_case(name))
            .map(String::as_str)
    }

    fn dir(&self, name: &str) -> Option<&str> {
        self.dirs
            .iter()
            .find(|d| d.eq_ignore_ascii_case(name))
            .map(String::as_str)
    }

    fn data_subdir(&self, name: &str) -> Option<&str> {
        let suffix = format!("/{name}");
        self.data_subdirs
            .iter()
            .find(|p| {
                p.len() >= suffix.len() && p[p.len() - suffix.len()..].eq_ignore_ascii_case(&suffix)
            })
            .map(String::as_str)
    }
}

/// Scans the game install directory for known mod artifacts. Read-only,
/// non-recursive (root plus a fixed one-level allowlist), and bounded by
/// [`MAX_SCAN_ENTRIES`]; symlinked entries are skipped entirely.
pub fn scan_game_directory(root: &Path) -> Result<DetectionScanReport, ModDetectionError> {
    let listing = read_root_listing(root)?;
    let candidates = build_candidates(&listing);

    Ok(DetectionScanReport {
        scanned_root: root.to_string_lossy().into_owned(),
        candidates,
        entries_scanned: listing.entries_scanned,
        truncated: listing.truncated,
    })
}

fn read_root_listing(root: &Path) -> Result<RootListing, ModDetectionError> {
    let entries = fs::read_dir(root).map_err(|source| ModDetectionError::RootUnreadable {
        path: root.to_string_lossy().into_owned(),
        source,
    })?;

    let mut listing = RootListing::default();
    for entry in entries.flatten() {
        if listing.entries_scanned >= MAX_SCAN_ENTRIES {
            listing.truncated = true;
            return Ok(listing);
        }
        listing.entries_scanned += 1;

        let Ok(meta) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if meta.is_dir() {
            listing.dirs.push(name);
        } else if meta.is_file() {
            listing.files.push(name);
        }
    }

    if let Some(data_dir) = listing.dir("Data").map(str::to_string) {
        scan_data_dir(root, &data_dir, &mut listing);
    }

    Ok(listing)
}

/// Reads `Data/` one level deep only to locate the allowlisted script-extender
/// marker directories; nothing inside them is enumerated.
fn scan_data_dir(root: &Path, data_dir: &str, listing: &mut RootListing) {
    let Ok(entries) = fs::read_dir(root.join(data_dir)) else {
        return;
    };
    for entry in entries.flatten() {
        if listing.entries_scanned >= MAX_SCAN_ENTRIES {
            listing.truncated = true;
            return;
        }
        listing.entries_scanned += 1;

        let Ok(meta) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if meta.file_type().is_symlink() || !meta.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if SCRIPT_EXTENDERS
            .iter()
            .filter_map(|rule| rule.data_dir)
            .any(|marker| marker.eq_ignore_ascii_case(&name))
        {
            listing.data_subdirs.push(format!("{data_dir}/{name}"));
        }
    }
}

fn build_candidates(listing: &RootListing) -> Vec<DetectedModCandidate> {
    let mut candidates = Vec::new();

    let proxy_dll_paths: Vec<String> = PROXY_DLL_NAMES
        .iter()
        .filter_map(|dll| listing.file(dll))
        .map(str::to_string)
        .collect();

    let reshade_paths = marker_paths(listing, &["ReShade.ini"], &["reshade-shaders"]);
    let enb_paths = marker_paths(listing, &["enblocal.ini", "enbseries.ini"], &["enbseries"]);

    // Grouping rule: proxy DLLs fold into ReShade (or ENB when ReShade is
    // absent) so a single loader never produces duplicate registration offers.
    let reshade_matched = !reshade_paths.is_empty();
    let proxy_absorbed = reshade_matched || !enb_paths.is_empty();

    if reshade_matched {
        let mut matched = reshade_paths;
        matched.extend(proxy_dll_paths.iter().cloned());
        candidates.push(candidate(
            "reshade",
            "ReShade",
            ModCategory::OverlayInjection,
            matched,
        ));
    }
    if !enb_paths.is_empty() {
        let mut matched = enb_paths;
        if !reshade_matched {
            matched.extend(proxy_dll_paths.iter().cloned());
        }
        candidates.push(candidate(
            "enb",
            "ENB Series",
            ModCategory::OverlayInjection,
            matched,
        ));
    }
    if !proxy_absorbed {
        for path in &proxy_dll_paths {
            candidates.push(candidate(
                &format!("proxy_dll:{}", path.to_ascii_lowercase()),
                &format!("Proxy DLL ({})", path.to_ascii_lowercase()),
                ModCategory::OverlayInjection,
                vec![path.clone()],
            ));
        }
    }

    for rule in &SCRIPT_EXTENDERS {
        let Some(loader) = listing.file(rule.loader) else {
            continue;
        };
        let mut matched = vec![loader.to_string()];
        if let Some(prefix) = rule.companion_prefix {
            matched.extend(
                listing
                    .files
                    .iter()
                    .filter(|f| {
                        let lower = f.to_ascii_lowercase();
                        lower.starts_with(prefix)
                            && lower.ends_with(".dll")
                            && !lower.eq_ignore_ascii_case(rule.loader)
                    })
                    .cloned(),
            );
        }
        if let Some(data_dir) = rule.data_dir.and_then(|d| listing.data_subdir(d)) {
            matched.push(data_dir.to_string());
        }
        candidates.push(candidate(
            &format!("script_extender:{}", rule.id),
            rule.display,
            ModCategory::ScriptExtender,
            matched,
        ));
    }

    candidates.sort_by(|a, b| a.detector_id.cmp(&b.detector_id));
    candidates
}

fn marker_paths(listing: &RootListing, files: &[&str], dirs: &[&str]) -> Vec<String> {
    files
        .iter()
        .filter_map(|f| listing.file(f))
        .chain(dirs.iter().filter_map(|d| listing.dir(d)))
        .map(str::to_string)
        .collect()
}

fn candidate(
    detector_id: &str,
    suggested_name: &str,
    category: ModCategory,
    mut matched_paths: Vec<String>,
) -> DetectedModCandidate {
    matched_paths.sort();
    matched_paths.dedup();
    DetectedModCandidate {
        detector_id: detector_id.to_string(),
        suggested_name: suggested_name.to_string(),
        category,
        matched_paths,
        already_registered: false,
    }
}

/// Flags scan candidates already covered by a registered mod. A candidate is
/// `already_registered` when a registered mod's name equals the suggested name
/// case-insensitively, or when a registered mod of the SAME category shares at
/// least one path case-insensitively. Registered absolute paths under
/// `game_dir` (the scanned root) are compared root-relative; absolute paths
/// outside it never match the root-relative candidate paths.
pub fn mark_already_registered(
    report: &mut DetectionScanReport,
    registered: &[ProfileModRecord],
    game_dir: &Path,
) {
    for candidate in &mut report.candidates {
        candidate.already_registered = registered.iter().any(|record| {
            record.name.eq_ignore_ascii_case(&candidate.suggested_name)
                || (record.category == candidate.category
                    && record.paths.iter().any(|registered_path| {
                        registered_path_matches(registered_path, &candidate.matched_paths, game_dir)
                    }))
        });
    }
}

fn registered_path_matches(
    registered_path: &str,
    candidate_paths: &[String],
    game_dir: &Path,
) -> bool {
    let path = Path::new(registered_path.trim());
    let relative = if path.is_absolute() {
        match path.strip_prefix(game_dir) {
            Ok(stripped) => stripped.to_string_lossy().into_owned(),
            Err(_) => return false,
        }
    } else {
        registered_path.trim().to_string()
    };
    candidate_paths
        .iter()
        .any(|candidate_path| candidate_path.eq_ignore_ascii_case(&relative))
}
