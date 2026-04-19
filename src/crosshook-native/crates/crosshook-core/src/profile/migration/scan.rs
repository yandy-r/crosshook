use std::collections::HashSet;
use std::path::PathBuf;

use crate::profile::models::resolve_launch_method;
use crate::profile::toml_store::ProfileStore;
use crate::steam::proton::discover_compat_tools;

use super::proton::{extract_name_from_proton_path, extract_proton_family, find_best_replacement};
use super::types::{
    MigrationScanResult, MigrationSuggestion, ProtonInstallInfo, ProtonPathField, UnmatchedProfile,
};

/// Scans all profiles for stale Proton paths and returns migration suggestions.
///
/// Only flags a path as stale when `try_exists()` returns `Ok(false)`.
/// Permission errors (`Err(_)`) are not treated as stale — the path may still exist.
pub fn scan_proton_migrations(
    store: &ProfileStore,
    steam_root_candidates: &[PathBuf],
    diagnostics: &mut Vec<String>,
) -> MigrationScanResult {
    let installed_tools = discover_compat_tools(steam_root_candidates, diagnostics);

    let profile_names = match store.list() {
        Ok(names) => names,
        Err(err) => {
            diagnostics.push(format!("Could not list profiles: {err}"));
            return MigrationScanResult {
                suggestions: Vec::new(),
                unmatched: Vec::new(),
                profiles_scanned: 0,
                affected_count: 0,
                installed_proton_versions: Vec::new(),
                diagnostics: diagnostics.clone(),
            };
        }
    };

    let mut suggestions = Vec::new();
    let mut unmatched = Vec::new();
    let mut affected_profiles: HashSet<String> = HashSet::new();

    for name in &profile_names {
        let profile = match store.load(name) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let launch_method = resolve_launch_method(&profile);

        // Check steam.proton_path for steam_applaunch profiles.
        if launch_method == "steam_applaunch" && !profile.steam.proton_path.trim().is_empty() {
            let path = &profile.steam.proton_path;
            if matches!(PathBuf::from(path).try_exists(), Ok(false)) {
                affected_profiles.insert(name.clone());
                let old_name = extract_name_from_proton_path(path);
                match find_best_replacement(&old_name, &installed_tools) {
                    Some((replacement, confidence, crosses_major)) => {
                        suggestions.push(MigrationSuggestion {
                            profile_name: name.clone(),
                            field: ProtonPathField::SteamProtonPath,
                            old_path: path.clone(),
                            new_path: replacement.path.to_string_lossy().into_owned(),
                            old_proton_name: old_name,
                            new_proton_name: replacement.name.clone(),
                            confidence,
                            proton_family: extract_proton_family(&replacement.name)
                                .unwrap_or_default(),
                            crosses_major_version: crosses_major,
                        });
                    }
                    None => {
                        unmatched.push(UnmatchedProfile {
                            profile_name: name.clone(),
                            field: ProtonPathField::SteamProtonPath,
                            stale_path: path.clone(),
                            stale_proton_name: old_name,
                        });
                    }
                }
            }
        }

        // Check runtime.proton_path for proton_run profiles.
        if launch_method == "proton_run" && !profile.runtime.proton_path.trim().is_empty() {
            let path = &profile.runtime.proton_path;
            if matches!(PathBuf::from(path).try_exists(), Ok(false)) {
                affected_profiles.insert(name.clone());
                let old_name = extract_name_from_proton_path(path);
                match find_best_replacement(&old_name, &installed_tools) {
                    Some((replacement, confidence, crosses_major)) => {
                        suggestions.push(MigrationSuggestion {
                            profile_name: name.clone(),
                            field: ProtonPathField::RuntimeProtonPath,
                            old_path: path.clone(),
                            new_path: replacement.path.to_string_lossy().into_owned(),
                            old_proton_name: old_name,
                            new_proton_name: replacement.name.clone(),
                            confidence,
                            proton_family: extract_proton_family(&replacement.name)
                                .unwrap_or_default(),
                            crosses_major_version: crosses_major,
                        });
                    }
                    None => {
                        unmatched.push(UnmatchedProfile {
                            profile_name: name.clone(),
                            field: ProtonPathField::RuntimeProtonPath,
                            stale_path: path.clone(),
                            stale_proton_name: old_name,
                        });
                    }
                }
            }
        }
    }

    let installed_proton_versions = installed_tools
        .iter()
        .map(|tool| ProtonInstallInfo {
            name: tool.name.clone(),
            path: tool.path.to_string_lossy().into_owned(),
            is_official: tool.is_official,
        })
        .collect();

    MigrationScanResult {
        profiles_scanned: profile_names.len(),
        affected_count: affected_profiles.len(),
        suggestions,
        unmatched,
        installed_proton_versions,
        diagnostics: diagnostics.clone(),
    }
}
