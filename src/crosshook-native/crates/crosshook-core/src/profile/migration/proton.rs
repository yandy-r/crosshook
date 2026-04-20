use std::path::PathBuf;

use crate::steam::proton::normalize_alias;
use crate::steam::ProtonInstall;

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
