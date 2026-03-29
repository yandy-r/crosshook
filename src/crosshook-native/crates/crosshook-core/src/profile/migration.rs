use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::profile::models::resolve_launch_method;
use crate::profile::toml_store::ProfileStore;
use crate::steam::ProtonInstall;
use crate::steam::proton::{discover_compat_tools, normalize_alias};

/// Which profile field contains the stale Proton path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtonPathField {
    /// `steam.proton_path` — used by `steam_applaunch` method.
    SteamProtonPath,
    /// `runtime.proton_path` — used by `proton_run` method.
    RuntimeProtonPath,
}

/// A single migration suggestion for one profile field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationSuggestion {
    pub profile_name: String,
    pub field: ProtonPathField,
    pub old_path: String,
    pub new_path: String,
    pub old_proton_name: String,
    pub new_proton_name: String,
    /// Confidence score: 0.0..=1.0
    pub confidence: f64,
    pub proton_family: String,
    /// True when the suggestion crosses a major version boundary (e.g., 9→10).
    pub crosses_major_version: bool,
}

/// A profile with a stale Proton path that has no matching replacement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnmatchedProfile {
    pub profile_name: String,
    pub field: ProtonPathField,
    pub stale_path: String,
    pub stale_proton_name: String,
}

/// Lightweight Proton install info for frontend display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtonInstallInfo {
    pub name: String,
    /// Executable path (e.g., `.../GE-Proton9-7/proton`), same as `ProtonInstall.path`.
    pub path: String,
    pub is_official: bool,
}

/// Result of scanning all profiles for migration candidates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationScanResult {
    pub suggestions: Vec<MigrationSuggestion>,
    pub unmatched: Vec<UnmatchedProfile>,
    pub profiles_scanned: usize,
    pub affected_count: usize,
    pub installed_proton_versions: Vec<ProtonInstallInfo>,
    pub diagnostics: Vec<String>,
}

/// Outcome of applying a single migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationOutcome {
    Applied,
    AlreadyValid,
    Failed,
}

/// Result of applying a single migration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationApplyResult {
    pub profile_name: String,
    pub field: ProtonPathField,
    pub old_path: String,
    pub new_path: String,
    pub outcome: MigrationOutcome,
    pub error: Option<String>,
}

/// Request to apply a single migration (received from frontend, deserialize only).
#[derive(Debug, Clone, Deserialize)]
pub struct ApplyMigrationRequest {
    pub profile_name: String,
    pub field: ProtonPathField,
    pub new_path: String,
}

/// Request to apply multiple migrations at once (deserialize only).
#[derive(Debug, Clone, Deserialize)]
pub struct BatchMigrationRequest {
    pub migrations: Vec<ApplyMigrationRequest>,
}

/// Result of a batch migration operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchMigrationResult {
    pub results: Vec<MigrationApplyResult>,
    pub applied_count: usize,
    pub failed_count: usize,
    pub skipped_count: usize,
}

// ---------------------------------------------------------------------------
// Core algorithm
// ---------------------------------------------------------------------------

/// Extracts the Proton family key from a name or path component.
///
/// Normalizes via `normalize_alias`, then strips trailing digit sequences.
///
/// TKG-Proton is detected by prefix and returned as `"protontkg"` — its
/// directory names embed git commit hashes that make version ranking unreliable.
///
/// Examples:
/// - `"GE-Proton9-7"`         → `Some("geproton")`
/// - `"Proton 9.0-4"`         → `Some("proton")`
/// - `"Proton Experimental"`  → `Some("protonexperimental")`
/// - `"Proton-9.23-GE-2"`    → `Some("protonge")` (legacy GE)
pub fn extract_proton_family(name: &str) -> Option<String> {
    let normalized = normalize_alias(name)?;

    // TKG embeds git hashes — version ranking is not possible.
    if normalized.starts_with("protontkg") {
        return Some("protontkg".to_string());
    }

    let family = normalized.trim_end_matches(|c: char| c.is_ascii_digit());
    if family.is_empty() {
        Some(normalized)
    } else {
        Some(family.to_string())
    }
}

/// Returns `true` for families that cannot be ranked by version (TKG).
fn is_non_rankable_family(family: &str) -> bool {
    family == "protontkg"
}

/// Extracts integer version segments from a raw Proton directory name.
///
/// Operates on the **raw** (non-normalized) name to preserve multi-digit
/// numbers like `"10"` as a single segment rather than splitting them.
///
/// Examples:
/// - `"GE-Proton10-34"`      → `[10, 34]`
/// - `"Proton 9.0-1"`        → `[9, 0, 1]`
/// - `"Proton Experimental"` → `[]`
pub fn extract_version_segments(name: &str) -> Vec<u32> {
    name.split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<u32>().ok())
        .collect()
}

/// Extracts the Proton install directory name from a full proton executable path.
///
/// Example: `"/home/user/.steam/root/compatibilitytools.d/GE-Proton9-7/proton"` → `"GE-Proton9-7"`
pub fn extract_name_from_proton_path(path: &str) -> String {
    PathBuf::from(path)
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string()
}

/// Finds the best installed replacement for a stale Proton install.
///
/// Returns `(replacement, confidence, crosses_major_version)` or `None` when:
/// - No same-family install is found.
/// - The stale family is non-rankable (TKG).
///
/// Confidence tiers (descending):
/// - 0.9: same family, same major, newer build
/// - 0.8: same family, versionless match (e.g., Proton Experimental)
/// - 0.75: same family, newer major (may need prefix migration)
/// - 0.7: same family, older build (same major)
/// - 0.5: same family, older major
pub fn find_best_replacement(
    stale_name: &str,
    installed: &[ProtonInstall],
) -> Option<(ProtonInstall, f64, bool)> {
    let old_family = extract_proton_family(stale_name)?;

    if is_non_rankable_family(&old_family) {
        return None;
    }

    let old_version = extract_version_segments(stale_name);

    // Versionless families (e.g., Proton Experimental) only match another versionless install.
    if old_version.is_empty() {
        return installed
            .iter()
            .find(|tool| {
                let tool_family = match extract_proton_family(&tool.name) {
                    Some(f) if !is_non_rankable_family(&f) => f,
                    _ => return false,
                };
                tool_family == old_family && extract_version_segments(&tool.name).is_empty()
            })
            .map(|tool: &ProtonInstall| (tool.clone(), 0.8_f64, false));
    }

    let mut candidates: Vec<(&ProtonInstall, f64, bool)> = Vec::new();

    for tool in installed {
        let tool_family = match extract_proton_family(&tool.name) {
            Some(f) if !is_non_rankable_family(&f) => f,
            _ => continue,
        };

        if tool_family != old_family {
            continue;
        }

        let tool_version = extract_version_segments(&tool.name);
        // Skip versionless installs and exact version matches.
        if tool_version.is_empty() || tool_version == old_version {
            continue;
        }

        let is_newer = tool_version > old_version;
        // SAFETY: both vectors are non-empty at this point.
        let crosses_major = tool_version[0] != old_version[0];

        let confidence = match (is_newer, crosses_major) {
            (true, false) => 0.9,
            (true, true) => 0.75,
            (false, false) => 0.7,
            (false, true) => 0.5,
        };

        candidates.push((tool, confidence, crosses_major));
    }

    // Sort by version descending — pick the newest available.
    candidates.sort_by(|a, b| {
        let va = extract_version_segments(&a.0.name);
        let vb = extract_version_segments(&b.0.name);
        vb.cmp(&va)
    });

    candidates
        .into_iter()
        .next()
        .map(|(tool, conf, crosses_major): (&ProtonInstall, f64, bool)| {
            (tool.clone(), conf, crosses_major)
        })
}

// ---------------------------------------------------------------------------
// Scan
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Apply
// ---------------------------------------------------------------------------

/// Applies a single migration using an atomic temp-file + rename write (W-1).
///
/// Does NOT delegate to `ProfileStore::save()` which uses non-atomic `fs::write()`.
pub fn apply_single_migration(
    store: &ProfileStore,
    request: &ApplyMigrationRequest,
) -> MigrationApplyResult {
    let mut profile = match store.load(&request.profile_name) {
        Ok(p) => p,
        Err(err) => {
            return MigrationApplyResult {
                profile_name: request.profile_name.clone(),
                field: request.field,
                old_path: String::new(),
                new_path: request.new_path.clone(),
                outcome: MigrationOutcome::Failed,
                error: Some(err.to_string()),
            };
        }
    };

    let old_path = match request.field {
        ProtonPathField::SteamProtonPath => profile.steam.proton_path.clone(),
        ProtonPathField::RuntimeProtonPath => profile.runtime.proton_path.clone(),
    };

    // Validate replacement path exists and is a regular file.
    if !PathBuf::from(&request.new_path).is_file() {
        return MigrationApplyResult {
            profile_name: request.profile_name.clone(),
            field: request.field,
            old_path,
            new_path: request.new_path.clone(),
            outcome: MigrationOutcome::Failed,
            error: Some(format!(
                "Replacement path does not exist or is not a file: {}",
                request.new_path
            )),
        };
    }

    // If the current path is still valid, no migration is needed.
    if matches!(PathBuf::from(&old_path).try_exists(), Ok(true)) {
        return MigrationApplyResult {
            profile_name: request.profile_name.clone(),
            field: request.field,
            old_path,
            new_path: request.new_path.clone(),
            outcome: MigrationOutcome::AlreadyValid,
            error: None,
        };
    }

    // Update the correct field on the effective profile.
    match request.field {
        ProtonPathField::SteamProtonPath => {
            profile.steam.proton_path = request.new_path.clone();
        }
        ProtonPathField::RuntimeProtonPath => {
            profile.runtime.proton_path = request.new_path.clone();
        }
    }

    // Atomic write: serialize storage form → .toml.tmp → rename to .toml.
    let profile_path = store.base_path.join(format!("{}.toml", request.profile_name));
    let tmp_path = profile_path.with_extension("toml.tmp");

    let toml_str = match toml::to_string_pretty(&profile.storage_profile()) {
        Ok(s) => s,
        Err(err) => {
            return MigrationApplyResult {
                profile_name: request.profile_name.clone(),
                field: request.field,
                old_path,
                new_path: request.new_path.clone(),
                outcome: MigrationOutcome::Failed,
                error: Some(err.to_string()),
            };
        }
    };

    if let Err(err) = fs::write(&tmp_path, &toml_str) {
        return MigrationApplyResult {
            profile_name: request.profile_name.clone(),
            field: request.field,
            old_path,
            new_path: request.new_path.clone(),
            outcome: MigrationOutcome::Failed,
            error: Some(err.to_string()),
        };
    }

    if let Err(err) = fs::rename(&tmp_path, &profile_path) {
        return MigrationApplyResult {
            profile_name: request.profile_name.clone(),
            field: request.field,
            old_path,
            new_path: request.new_path.clone(),
            outcome: MigrationOutcome::Failed,
            error: Some(err.to_string()),
        };
    }

    MigrationApplyResult {
        profile_name: request.profile_name.clone(),
        field: request.field,
        old_path,
        new_path: request.new_path.clone(),
        outcome: MigrationOutcome::Applied,
        error: None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;
    use crate::profile::models::GameProfile;
    use crate::profile::toml_store::ProfileStore;

    // --- extract_proton_family ---

    #[test]
    fn family_ge_proton_modern() {
        assert_eq!(
            extract_proton_family("GE-Proton9-7"),
            Some("geproton".to_string())
        );
    }

    #[test]
    fn family_ge_proton_double_digit_major() {
        assert_eq!(
            extract_proton_family("GE-Proton10-34"),
            Some("geproton".to_string())
        );
    }

    #[test]
    fn family_official_proton() {
        assert_eq!(
            extract_proton_family("Proton 9.0"),
            Some("proton".to_string())
        );
    }

    #[test]
    fn family_proton_experimental() {
        assert_eq!(
            extract_proton_family("Proton Experimental"),
            Some("protonexperimental".to_string())
        );
    }

    #[test]
    fn family_tkg_returns_fixed_key() {
        assert_eq!(
            extract_proton_family("proton_tkg_6.17.r0.g5f19a815.release"),
            Some("protontkg".to_string())
        );
    }

    #[test]
    fn family_legacy_ge() {
        // "Proton-9.23-GE-2" normalizes to "proton923ge2", trailing digit stripped → "proton923ge".
        // In Phase 1 this is a separate family from modern GE ("geproton") — by design.
        assert_eq!(
            extract_proton_family("Proton-9.23-GE-2"),
            Some("proton923ge".to_string())
        );
    }

    // --- extract_version_segments ---

    #[test]
    fn version_ge_proton_double_digit_major() {
        assert_eq!(extract_version_segments("GE-Proton10-34"), vec![10u32, 34]);
    }

    #[test]
    fn version_official_proton() {
        assert_eq!(extract_version_segments("Proton 9.0-1"), vec![9u32, 0, 1]);
    }

    #[test]
    fn version_experimental_is_empty() {
        assert!(extract_version_segments("Proton Experimental").is_empty());
    }

    #[test]
    fn version_ge_proton_single_digit() {
        assert_eq!(extract_version_segments("GE-Proton9-7"), vec![9u32, 7]);
    }

    // --- integer-tuple ordering (critical correctness test) ---

    #[test]
    fn version_tuple_ordering_multi_digit_build() {
        // [9, 10] must sort AFTER [9, 9] — lexicographic comparison gets this wrong.
        let v1: Vec<u32> = vec![9, 10];
        let v2: Vec<u32> = vec![9, 9];
        assert!(v1 > v2, "[9, 10] must be greater than [9, 9]");
    }

    #[test]
    fn version_tuple_ordering_cross_major() {
        let v10_1: Vec<u32> = vec![10, 1];
        let v9_99: Vec<u32> = vec![9, 99];
        assert!(v10_1 > v9_99, "[10, 1] must be greater than [9, 99]");
    }

    // --- find_best_replacement ---

    fn make_install(name: &str, path: &str, is_official: bool) -> ProtonInstall {
        use std::collections::BTreeSet;
        use crate::steam::proton::normalize_alias;

        let mut normalized_aliases = BTreeSet::new();
        if let Some(n) = normalize_alias(name) {
            normalized_aliases.insert(n);
        }
        ProtonInstall {
            name: name.to_string(),
            path: PathBuf::from(path),
            is_official,
            aliases: vec![name.to_string()],
            normalized_aliases,
        }
    }

    #[test]
    fn same_family_newer_gets_09_confidence() {
        let installed = vec![make_install(
            "GE-Proton9-7",
            "/compat/GE-Proton9-7/proton",
            false,
        )];
        let result = find_best_replacement("GE-Proton9-4", &installed);
        let (install, confidence, crosses_major) = result.expect("should find replacement");
        assert_eq!(install.name, "GE-Proton9-7");
        assert!((confidence - 0.9_f64).abs() < f64::EPSILON);
        assert!(!crosses_major);
    }

    #[test]
    fn cross_major_gets_075_confidence_and_crosses_major_true() {
        let installed = vec![make_install(
            "GE-Proton10-1",
            "/compat/GE-Proton10-1/proton",
            false,
        )];
        let result = find_best_replacement("GE-Proton9-7", &installed);
        let (install, confidence, crosses_major) = result.expect("should find replacement");
        assert_eq!(install.name, "GE-Proton10-1");
        assert!((confidence - 0.75_f64).abs() < f64::EPSILON);
        assert!(crosses_major);
    }

    #[test]
    fn same_family_older_gets_07_confidence() {
        let installed = vec![make_install(
            "GE-Proton9-3",
            "/compat/GE-Proton9-3/proton",
            false,
        )];
        let result = find_best_replacement("GE-Proton9-7", &installed);
        let (install, confidence, crosses_major) = result.expect("should find older replacement");
        assert_eq!(install.name, "GE-Proton9-3");
        assert!((confidence - 0.7_f64).abs() < f64::EPSILON);
        assert!(!crosses_major);
    }

    #[test]
    fn picks_newest_when_multiple_same_family_candidates() {
        let installed = vec![
            make_install("GE-Proton9-5", "/compat/GE-Proton9-5/proton", false),
            make_install("GE-Proton9-10", "/compat/GE-Proton9-10/proton", false),
            make_install("GE-Proton9-7", "/compat/GE-Proton9-7/proton", false),
        ];
        let result = find_best_replacement("GE-Proton9-4", &installed);
        let (install, _confidence, _) = result.expect("should find replacement");
        // [9, 10] > [9, 7] > [9, 5]
        assert_eq!(install.name, "GE-Proton9-10");
    }

    #[test]
    fn proton_experimental_only_matches_another_experimental() {
        let installed = vec![
            make_install("GE-Proton9-7", "/compat/GE-Proton9-7/proton", false),
            make_install(
                "Proton Experimental",
                "/steam/Proton Experimental/proton",
                true,
            ),
        ];
        let result = find_best_replacement("Proton Experimental", &installed);
        let (install, confidence, crosses_major) = result.expect("should find experimental");
        assert_eq!(install.name, "Proton Experimental");
        assert!((confidence - 0.8_f64).abs() < f64::EPSILON);
        assert!(!crosses_major);
    }

    #[test]
    fn proton_experimental_no_match_when_none_installed() {
        let installed = vec![make_install(
            "GE-Proton9-7",
            "/compat/GE-Proton9-7/proton",
            false,
        )];
        assert!(
            find_best_replacement("Proton Experimental", &installed).is_none(),
            "Experimental must not match GE-Proton"
        );
    }

    #[test]
    fn no_match_returns_none() {
        let installed = vec![make_install(
            "Proton 9.0",
            "/steam/common/Proton 9.0/proton",
            true,
        )];
        // Stale is GE-Proton, installed is official Proton — different family.
        assert!(find_best_replacement("GE-Proton9-4", &installed).is_none());
    }

    #[test]
    fn tkg_returns_none() {
        let installed = vec![
            make_install(
                "proton_tkg_6.17.r0.g5f19a815.release",
                "/compat/tkg/proton",
                false,
            ),
            make_install("GE-Proton9-7", "/compat/GE-Proton9-7/proton", false),
        ];
        // A stale TKG install should never receive an auto-suggestion.
        assert!(
            find_best_replacement("proton_tkg_6.17.r0.g5f19a815.release", &installed).is_none()
        );
    }

    // --- apply_single_migration round-trip ---

    #[test]
    fn round_trip_migration_updates_steam_proton_path() {
        let dir = tempdir().expect("tempdir");
        let store = ProfileStore::with_base_path(dir.path().to_path_buf());

        // Create a profile with a stale steam.proton_path.
        let mut profile = GameProfile::default();
        profile.launch.method = "steam_applaunch".to_string();
        profile.steam.proton_path = "/stale/GE-Proton9-4/proton".to_string();
        store.save("test-game", &profile).expect("initial save");

        // Create the replacement proton executable.
        let new_proton_dir = dir.path().join("GE-Proton9-7");
        fs::create_dir_all(&new_proton_dir).expect("mkdir new proton dir");
        let new_proton_path = new_proton_dir.join("proton");
        fs::write(&new_proton_path, b"#!/bin/sh\n").expect("write proton file");

        let new_path_str = new_proton_path.to_string_lossy().into_owned();

        // Apply the migration.
        let request = ApplyMigrationRequest {
            profile_name: "test-game".to_string(),
            field: ProtonPathField::SteamProtonPath,
            new_path: new_path_str.clone(),
        };
        let result = apply_single_migration(&store, &request);

        assert_eq!(
            result.outcome,
            MigrationOutcome::Applied,
            "migration should apply successfully; error: {:?}",
            result.error
        );
        assert!(result.error.is_none());
        assert_eq!(result.new_path, new_path_str);

        // Re-load and verify effective path is updated.
        let reloaded = store.load("test-game").expect("reload");
        assert_eq!(
            reloaded.steam.proton_path, new_path_str,
            "effective path must reflect the migration"
        );

        // Verify the on-disk TOML stores the path in local_override.
        let toml_content =
            fs::read_to_string(dir.path().join("test-game.toml")).expect("read toml");
        assert!(
            toml_content.contains(&new_path_str),
            "new path must appear in TOML"
        );
        assert!(
            toml_content.contains("[local_override"),
            "path must be under local_override in TOML"
        );
    }

    #[test]
    fn migration_already_valid_when_old_path_exists() {
        let dir = tempdir().expect("tempdir");
        let store = ProfileStore::with_base_path(dir.path().to_path_buf());

        // Create a "valid" proton executable that actually exists.
        let proton_dir = dir.path().join("GE-Proton9-4");
        fs::create_dir_all(&proton_dir).expect("mkdir");
        let proton_path = proton_dir.join("proton");
        fs::write(&proton_path, b"#!/bin/sh\n").expect("write proton");

        let mut profile = GameProfile::default();
        profile.launch.method = "steam_applaunch".to_string();
        profile.steam.proton_path = proton_path.to_string_lossy().into_owned();
        store.save("valid-game", &profile).expect("save");

        // Create the "new" replacement.
        let new_proton_dir = dir.path().join("GE-Proton9-7");
        fs::create_dir_all(&new_proton_dir).expect("mkdir");
        let new_proton_path = new_proton_dir.join("proton");
        fs::write(&new_proton_path, b"#!/bin/sh\n").expect("write new proton");

        let request = ApplyMigrationRequest {
            profile_name: "valid-game".to_string(),
            field: ProtonPathField::SteamProtonPath,
            new_path: new_proton_path.to_string_lossy().into_owned(),
        };
        let result = apply_single_migration(&store, &request);

        assert_eq!(result.outcome, MigrationOutcome::AlreadyValid);
    }

    #[test]
    fn migration_fails_when_replacement_does_not_exist() {
        let dir = tempdir().expect("tempdir");
        let store = ProfileStore::with_base_path(dir.path().to_path_buf());

        let mut profile = GameProfile::default();
        profile.launch.method = "steam_applaunch".to_string();
        profile.steam.proton_path = "/stale/proton".to_string();
        store.save("broken-game", &profile).expect("save");

        let request = ApplyMigrationRequest {
            profile_name: "broken-game".to_string(),
            field: ProtonPathField::SteamProtonPath,
            new_path: "/nonexistent/proton".to_string(),
        };
        let result = apply_single_migration(&store, &request);

        assert_eq!(result.outcome, MigrationOutcome::Failed);
        assert!(result.error.is_some());
    }
}
