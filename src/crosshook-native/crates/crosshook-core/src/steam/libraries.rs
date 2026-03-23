use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::models::SteamLibrary;
use super::vdf::{parse_vdf, VdfNode};

pub fn discover_steam_libraries(
    steam_roots: &[PathBuf],
    diagnostics: &mut Vec<String>,
) -> Vec<SteamLibrary> {
    let mut libraries = Vec::new();
    let mut seen_paths = HashSet::new();

    for steam_root in steam_roots {
        add_library_candidate(
            &mut libraries,
            &mut seen_paths,
            steam_root,
            diagnostics,
            "Steam root",
        );

        let library_folders_path = steam_root.join("steamapps/libraryfolders.vdf");
        let Ok(content) = fs::read_to_string(&library_folders_path) else {
            diagnostics.push(format!("Unable to read {}", library_folders_path.display()));
            continue;
        };

        let Ok(root_node) = parse_vdf(&content) else {
            diagnostics.push(format!(
                "Unable to parse {}",
                library_folders_path.display()
            ));
            continue;
        };

        let Some(libraryfolders_node) = find_libraryfolders_node(&root_node) else {
            diagnostics.push(format!(
                "Missing libraryfolders node in {}",
                library_folders_path.display()
            ));
            continue;
        };

        for entry_node in libraryfolders_node.children.values() {
            if let Some(library_path) = extract_library_path(entry_node) {
                add_library_candidate(
                    &mut libraries,
                    &mut seen_paths,
                    &library_path,
                    diagnostics,
                    "Steam library",
                );
            }
        }
    }

    libraries
}

fn find_libraryfolders_node(root: &VdfNode) -> Option<&VdfNode> {
    root.get_child("libraryfolders")
        .or_else(|| root.find_descendant("libraryfolders"))
}

fn extract_library_path(entry_node: &VdfNode) -> Option<PathBuf> {
    entry_node
        .value
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            entry_node
                .get_child("path")
                .and_then(|node| node.value.as_deref())
                .filter(|value| !value.trim().is_empty())
                .map(PathBuf::from)
        })
}

fn add_library_candidate(
    libraries: &mut Vec<SteamLibrary>,
    seen_paths: &mut HashSet<String>,
    path: impl AsRef<Path>,
    diagnostics: &mut Vec<String>,
    source: &str,
) {
    let library_path = normalize_path(path.as_ref());
    if library_path.as_os_str().is_empty() {
        return;
    }

    let steamapps_path = library_path.join("steamapps");
    if !steamapps_path.is_dir() {
        return;
    }

    let key = library_path.to_string_lossy().to_string();
    if seen_paths.insert(key) {
        diagnostics.push(format!("{source}: {}", library_path.display()));
        libraries.push(SteamLibrary {
            path: library_path.clone(),
            steamapps_path,
        });
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
    use super::discover_steam_libraries;
    use std::fs;

    #[test]
    fn includes_root_and_libraryfolders_entries() {
        let temp_root = tempfile::tempdir().expect("temp root");
        let library_one = temp_root.path().join("steamapps");
        let library_two = temp_root.path().join("external");
        let library_two_steamapps = library_two.join("steamapps");

        fs::create_dir_all(&library_one).expect("root steamapps");
        fs::create_dir_all(&library_two_steamapps).expect("library steamapps");

        fs::write(
            temp_root.path().join("steamapps/libraryfolders.vdf"),
            r#"
            "libraryfolders"
            {
              "1"
              {
                "path" "/does/not/exist"
              }
              "2" "/tmp/ignored"
            }
            "#,
        )
        .expect("libraryfolders.vdf");

        let mut diagnostics = Vec::new();
        let libraries =
            discover_steam_libraries(&[temp_root.path().to_path_buf()], &mut diagnostics);

        assert_eq!(libraries.len(), 1);
        assert_eq!(libraries[0].path, temp_root.path());
        assert_eq!(libraries[0].steamapps_path, library_one);
        assert!(diagnostics.iter().any(|entry| entry.contains("Steam root")));
    }

    #[test]
    fn loads_valid_entries_from_both_vdf_shapes() {
        let temp_root = tempfile::tempdir().expect("temp root");
        let root_library = temp_root.path().join("steamapps");
        let vdf_library = temp_root.path().join("library-a");
        let vdf_library_steamapps = vdf_library.join("steamapps");

        fs::create_dir_all(&root_library).expect("root steamapps");
        fs::create_dir_all(&vdf_library_steamapps).expect("library steamapps");

        fs::write(
            temp_root.path().join("steamapps/libraryfolders.vdf"),
            format!(
                r#"
                "libraryfolders"
                {{
                  "2"
                  {{
                    "path" "{}"
                  }}
                  "3" "{}"
                }}
                "#,
                vdf_library.display(),
                vdf_library.display()
            ),
        )
        .expect("libraryfolders.vdf");

        let mut diagnostics = Vec::new();
        let libraries =
            discover_steam_libraries(&[temp_root.path().to_path_buf()], &mut diagnostics);

        assert_eq!(libraries.len(), 2);
        assert_eq!(libraries[0].path, temp_root.path());
        assert_eq!(libraries[1].path, vdf_library);
        assert!(diagnostics
            .iter()
            .any(|entry| entry.contains("Steam library")));
    }
}
