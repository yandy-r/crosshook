use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::constants::DRIVE_C_RELATIVE;
use super::types::PrefixReference;
use super::utils::normalized_path_string;

pub(super) fn collect_referenced_profiles(
    references: &[PrefixReference],
) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for reference in references {
        let key = normalized_path_string(Path::new(&reference.resolved_prefix_path));
        if key.is_empty() {
            continue;
        }
        map.entry(key)
            .or_default()
            .push(reference.profile_name.clone());
    }

    for profile_names in map.values_mut() {
        profile_names.sort();
        profile_names.dedup();
    }

    map
}

pub(super) fn discover_candidate_prefixes(referenced_prefixes: &[String]) -> BTreeSet<String> {
    let mut candidates = BTreeSet::new();
    for raw in referenced_prefixes {
        let prefix = PathBuf::from(raw);
        if !prefix.is_dir() {
            continue;
        }

        if let Some(parent) = prefix.parent() {
            candidates.extend(discover_prefixes_in_directory(parent));
        }

        if prefix.file_name().and_then(|value| value.to_str()) == Some("pfx") {
            if let Some(compatdata_root) = prefix.parent().and_then(Path::parent) {
                candidates.extend(discover_prefixes_in_compatdata_root(compatdata_root));
            }
        }
    }
    candidates
}

fn discover_prefixes_in_directory(directory: &Path) -> Vec<String> {
    let mut result = Vec::new();
    let entries = match fs::read_dir(directory) {
        Ok(value) => value,
        Err(_) => return result,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            continue;
        }
        if path.join(DRIVE_C_RELATIVE).is_dir() {
            result.push(normalized_path_string(&path));
        }
    }

    result
}

fn discover_prefixes_in_compatdata_root(compatdata_root: &Path) -> Vec<String> {
    let mut result = Vec::new();
    let entries = match fs::read_dir(compatdata_root) {
        Ok(value) => value,
        Err(_) => return result,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            continue;
        }
        let pfx_path = path.join("pfx");
        if pfx_path.join(DRIVE_C_RELATIVE).is_dir() {
            result.push(normalized_path_string(&pfx_path));
        }
    }

    result
}
