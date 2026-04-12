use std::cmp::Ordering;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use crate::launch::runtime_helpers::resolve_wine_prefix_path;

const MAX_SCAN_DEPTH: usize = 8;
const MAX_VISITED_DIRECTORIES: usize = 1_024;
const MAX_SCANNED_FILES: usize = 4_096;
const MAX_RETURNED_CANDIDATES: usize = 24;

const SKIP_DIRECTORY_TERMS: &[&str] = &[
    "cache",
    "crash",
    "crashhandler",
    "crashreport",
    "directx",
    "dotnet",
    "easyanticheat",
    "logs",
    "prereq",
    "prerequisites",
    "redist",
    "redistributable",
    "support",
    "temp",
    "tmp",
    "vcredist",
];

const SUSPICIOUS_FILE_TERMS: &[&str] = &[
    "anti-cheat",
    "anticheat",
    "bootstrap",
    "cleanup",
    "crash",
    "debug",
    "installer",
    "install",
    "launcher",
    "log",
    "patch",
    "patcher",
    "prereq",
    "redist",
    "redistributable",
    "setup",
    "unins",
    "uninstall",
    "update",
    "updater",
];

const SUSPICIOUS_PATH_TERMS: &[&str] = &[
    "anti-cheat",
    "anticheat",
    "battleye",
    "commonredist",
    "crash",
    "directx",
    "easyanticheat",
    "launcher",
    "nvidia",
    "physx",
    "prereq",
    "redist",
    "redistributable",
    "support",
    "vcredist",
];

#[derive(Debug, Clone)]
struct Candidate {
    path: PathBuf,
    score: i32,
    depth: usize,
}

pub fn discover_game_executable_candidates(
    prefix_path: &Path,
    profile_name: &str,
    display_name: &str,
    installer_path: &str,
) -> Vec<PathBuf> {
    let drive_c = resolve_wine_prefix_path(prefix_path).join("drive_c");
    if !drive_c.is_dir() {
        return Vec::new();
    }

    let target_tokens = target_tokens(profile_name, display_name);
    let installer_hint = path_stem_lower(installer_path);
    let mut state = ScanState::default();
    let mut candidates = Vec::new();
    scan_directory(
        &drive_c,
        0,
        &target_tokens,
        &installer_hint,
        &mut state,
        &mut candidates,
    );

    candidates.sort_by(compare_candidates);

    let mut seen = HashSet::new();
    let mut ranked_paths = Vec::new();
    for candidate in candidates {
        let candidate_key = candidate.path.to_string_lossy().into_owned();
        if seen.insert(candidate_key) {
            ranked_paths.push(candidate.path);
        }

        if ranked_paths.len() >= MAX_RETURNED_CANDIDATES {
            break;
        }
    }

    ranked_paths
}

#[derive(Default)]
struct ScanState {
    visited_directories: usize,
    scanned_files: usize,
}

fn scan_directory(
    directory: &Path,
    depth: usize,
    target_tokens: &[String],
    installer_hint: &str,
    state: &mut ScanState,
    candidates: &mut Vec<Candidate>,
) {
    if depth > MAX_SCAN_DEPTH
        || state.visited_directories >= MAX_VISITED_DIRECTORIES
        || state.scanned_files >= MAX_SCANNED_FILES
    {
        return;
    }

    state.visited_directories += 1;

    let mut entries = match read_directory_entries(directory) {
        Some(entries) => entries,
        None => return,
    };

    entries.sort_by(|left, right| {
        let left_name = left.file_name().to_string_lossy().to_lowercase();
        let right_name = right.file_name().to_string_lossy().to_lowercase();
        left_name.cmp(&right_name)
    });

    for entry in entries {
        if state.visited_directories >= MAX_VISITED_DIRECTORIES
            || state.scanned_files >= MAX_SCANNED_FILES
        {
            return;
        }

        let path = entry.path();
        let file_name = entry.file_name();

        if path.is_dir() {
            if should_skip_directory(&file_name) {
                continue;
            }

            scan_directory(
                &path,
                depth + 1,
                target_tokens,
                installer_hint,
                state,
                candidates,
            );
            continue;
        }

        if !is_windows_executable(&path) {
            continue;
        }

        state.scanned_files += 1;
        if state.scanned_files > MAX_SCANNED_FILES {
            return;
        }

        let score = score_candidate(&path, depth, target_tokens, installer_hint);
        candidates.push(Candidate { path, score, depth });
    }
}

fn compare_candidates(left: &Candidate, right: &Candidate) -> Ordering {
    right
        .score
        .cmp(&left.score)
        .then_with(|| left.depth.cmp(&right.depth))
        .then_with(|| {
            left.path
                .to_string_lossy()
                .cmp(&right.path.to_string_lossy())
        })
}

fn read_directory_entries(directory: &Path) -> Option<Vec<fs::DirEntry>> {
    let entries = fs::read_dir(directory).ok()?;
    Some(entries.filter_map(Result::ok).collect())
}

fn score_candidate(
    path: &Path,
    depth: usize,
    target_tokens: &[String],
    installer_hint: &str,
) -> i32 {
    let stem = path_stem_lower(path);
    let path_string = path.to_string_lossy().to_lowercase();
    let path_segments = path_segments_lower(path);

    let mut score = 20;

    if !installer_hint.is_empty() && (stem == installer_hint || stem.contains(installer_hint)) {
        score -= 150;
    }

    if contains_any(&stem, SUSPICIOUS_FILE_TERMS) {
        score -= 120;
    }

    if contains_any(&path_string, SUSPICIOUS_PATH_TERMS) {
        score -= 90;
    }

    let stem_token_hits = token_hits(&stem, target_tokens);
    if stem_token_hits > 0 {
        score += 40 + (stem_token_hits as i32 * 12);
    }

    let path_token_hits = path_segments
        .iter()
        .map(|segment| token_hits(segment, target_tokens))
        .sum::<usize>();
    if path_token_hits > 0 {
        score += (path_token_hits as i32) * 4;
    }

    if !contains_any(&stem, SUSPICIOUS_FILE_TERMS) {
        score += 8;
    }

    if depth <= 2 {
        score += 12 - (depth as i32 * 2);
    } else if depth <= 4 {
        score += 4;
    }

    if path_string.contains("program files") {
        score += 4;
    }

    if stem.len() > 3 {
        score += 1;
    }

    score
}

fn token_hits(value: &str, target_tokens: &[String]) -> usize {
    target_tokens
        .iter()
        .filter(|token| value.contains(token.as_str()))
        .count()
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn target_tokens(profile_name: &str, display_name: &str) -> Vec<String> {
    let mut tokens = tokenize(profile_name);
    tokens.extend(tokenize(display_name));
    tokens.retain(|token| !is_generic_install_token(token));
    tokens.sort();
    tokens.dedup();
    tokens
}

fn tokenize(value: &str) -> Vec<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter_map(|token| {
            let token = token.trim().to_lowercase();
            if token.len() >= 2 {
                Some(token)
            } else {
                None
            }
        })
        .collect()
}

fn is_generic_install_token(token: &str) -> bool {
    matches!(
        token,
        "and"
            | "deluxe"
            | "edition"
            | "game"
            | "installer"
            | "install"
            | "launcher"
            | "of"
            | "setup"
            | "the"
            | "ultimate"
            | "upgrade"
    )
}

fn path_segments_lower(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| component.as_os_str().to_str())
        .map(str::to_lowercase)
        .collect()
}

fn path_stem_lower<T: AsRef<OsStr>>(path: T) -> String {
    let path = Path::new(path.as_ref());
    path.file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase()
}

fn is_windows_executable(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
}

fn should_skip_directory(name: &OsStr) -> bool {
    let lowered = name.to_string_lossy().to_lowercase();
    SKIP_DIRECTORY_TERMS
        .iter()
        .any(|term| lowered.contains(term))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_prefers_game_like_executables_over_setup_and_redist_binaries() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let drive_c = temp_dir.path().join("drive_c");
        let game_dir = drive_c.join("Games").join("Example Game");
        let redist_dir = drive_c.join("Program Files").join("Common Files");
        let setup_dir = drive_c.join("Downloads");

        fs::create_dir_all(&game_dir).expect("game dir");
        fs::create_dir_all(&redist_dir).expect("redist dir");
        fs::create_dir_all(&setup_dir).expect("setup dir");

        fs::write(game_dir.join("ExampleGame.exe"), b"game").expect("game exe");
        fs::write(game_dir.join("ExampleGameLauncher.exe"), b"launcher").expect("launcher exe");
        fs::write(redist_dir.join("vcredist_x64.exe"), b"redist").expect("redist exe");
        fs::write(setup_dir.join("setup.exe"), b"setup").expect("setup exe");

        let candidates = discover_game_executable_candidates(
            temp_dir.path(),
            "example-game",
            "Example Game",
            setup_dir.join("setup.exe").to_string_lossy().as_ref(),
        );

        assert_eq!(
            candidates
                .first()
                .map(|path| path.file_name().unwrap().to_string_lossy()),
            Some("ExampleGame.exe".into())
        );
        assert!(
            candidates
                .iter()
                .position(|path| path == &setup_dir.join("setup.exe"))
                .unwrap()
                > 0,
            "installer media should be de-ranked behind the main executable"
        );
    }

    #[test]
    fn discovery_uses_bounded_recursive_scan_under_drive_c() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let drive_c = temp_dir.path().join("drive_c");
        let deep_dir = drive_c
            .join("level1")
            .join("level2")
            .join("level3")
            .join("level4")
            .join("level5")
            .join("level6");

        fs::create_dir_all(&deep_dir).expect("deep dir");
        fs::write(deep_dir.join("DeepGame.exe"), b"deep game").expect("deep game exe");

        let candidates =
            discover_game_executable_candidates(temp_dir.path(), "deep-game", "Deep Game", "");

        assert!(
            candidates
                .iter()
                .any(|path| path.file_name().and_then(|value| value.to_str())
                    == Some("DeepGame.exe")),
            "deep executables within the configured scan depth should still be discovered"
        );
    }

    #[test]
    fn discovery_uses_pfx_child_when_prefix_path_is_compatdata_root() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let compatdata_root = temp_dir.path().join("compatdata-root");
        let drive_c = compatdata_root.join("pfx").join("drive_c");
        let game_dir = drive_c.join("Games").join("Example Game");

        fs::create_dir_all(&game_dir).expect("game dir");
        fs::write(game_dir.join("ExampleGame.exe"), b"game").expect("game exe");

        let candidates = discover_game_executable_candidates(
            &compatdata_root,
            "example-game",
            "Example Game",
            "",
        );

        assert_eq!(
            candidates
                .first()
                .map(|path| path.file_name().and_then(|value| value.to_str())),
            Some(Some("ExampleGame.exe"))
        );
    }
}
