use std::collections::HashSet;
use std::env;
use std::path::{Path, PathBuf};

/// Discover Steam roots in priority order.
///
/// The explicit Steam client path wins if it resolves to a valid root. If not,
/// the native Linux fallbacks are checked in the same order used by the
/// existing C# implementation plus the Flatpak Steam root needed for Steam
/// Deck installs.
pub fn discover_steam_root_candidates(
    steam_client_install_path: impl AsRef<Path>,
    diagnostics: &mut Vec<String>,
) -> Vec<PathBuf> {
    discover_steam_root_candidates_with_home(
        steam_client_install_path,
        env::var_os("HOME").map(PathBuf::from),
        diagnostics,
    )
}

fn discover_steam_root_candidates_with_home(
    steam_client_install_path: impl AsRef<Path>,
    home_path: Option<PathBuf>,
    diagnostics: &mut Vec<String>,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut seen_paths = HashSet::new();

    add_directory_candidate(
        &mut candidates,
        &mut seen_paths,
        steam_client_install_path.as_ref(),
        diagnostics,
        "Configured Steam client path",
    );

    if !candidates.is_empty() {
        return candidates;
    }

    let Some(home_path) = home_path else {
        return candidates;
    };

    add_directory_candidate(
        &mut candidates,
        &mut seen_paths,
        home_path.join(".steam/root"),
        diagnostics,
        "Default Steam root",
    );
    add_directory_candidate(
        &mut candidates,
        &mut seen_paths,
        home_path.join(".local/share/Steam"),
        diagnostics,
        "Default local Steam install",
    );
    add_directory_candidate(
        &mut candidates,
        &mut seen_paths,
        home_path.join(".var/app/com.valvesoftware.Steam/data/Steam"),
        diagnostics,
        "Flatpak Steam root",
    );

    candidates
}

fn add_directory_candidate(
    candidates: &mut Vec<PathBuf>,
    seen_paths: &mut HashSet<String>,
    path: impl AsRef<Path>,
    diagnostics: &mut Vec<String>,
    source_description: &str,
) {
    let candidate_path = normalize_path(path.as_ref());
    if candidate_path.as_os_str().is_empty() {
        return;
    }

    if !candidate_path.join("steamapps").is_dir() {
        return;
    }

    let key = candidate_path.to_string_lossy().to_string();
    if seen_paths.insert(key) {
        diagnostics.push(format!(
            "{source_description}: {}",
            candidate_path.display()
        ));
        candidates.push(candidate_path);
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let trimmed = path.as_os_str().to_string_lossy().trim().to_string();
    if trimmed.is_empty() {
        return PathBuf::new();
    }

    PathBuf::from(trimmed)
}

#[cfg(test)]
mod tests {
    use super::discover_steam_root_candidates;
    use std::fs;

    #[test]
    fn prefers_explicit_steam_client_path() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(temp_dir.path().join("steamapps")).expect("steamapps");
        let mut diagnostics = Vec::new();

        let candidates = discover_steam_root_candidates(temp_dir.path(), &mut diagnostics);

        assert_eq!(candidates, vec![temp_dir.path().to_path_buf()]);
        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn falls_back_to_home_roots_and_deduplicates() {
        let temp_home = tempfile::tempdir().expect("temp home");
        let steam_root = temp_home.path().join(".steam/root");
        let local_steam = temp_home.path().join(".local/share/Steam");
        let flatpak_steam = temp_home
            .path()
            .join(".var/app/com.valvesoftware.Steam/data/Steam");

        fs::create_dir_all(steam_root.join("steamapps")).expect("steam root");
        fs::create_dir_all(local_steam.join("steamapps")).expect("local steam");
        fs::create_dir_all(flatpak_steam.join("steamapps")).expect("flatpak steam");

        let mut diagnostics = Vec::new();
        let candidates = super::discover_steam_root_candidates_with_home(
            "",
            Some(temp_home.path().to_path_buf()),
            &mut diagnostics,
        );

        assert_eq!(candidates, vec![steam_root, local_steam, flatpak_steam]);
        assert_eq!(diagnostics.len(), 3);
    }
}
