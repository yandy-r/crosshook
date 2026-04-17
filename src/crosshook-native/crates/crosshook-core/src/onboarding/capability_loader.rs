//! TOML parsing, map merging, and process-global singleton for the host capability map.
//!
//! Extracted from [`super::capability`] so that `capability.rs` stays focused
//! on struct definitions and the `derive_capabilities` logic.

use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;

use serde::Deserialize;

use super::capability::{CapabilityDefinition, CapabilityMap};

/// Embedded default capability map TOML (compile time).
pub const DEFAULT_CAPABILITY_MAP_TOML: &str =
    include_str!("../../../../assets/default_capability_map.toml");

#[derive(Debug, Deserialize)]
struct RawCapabilityMapFile {
    #[serde(default)]
    catalog_version: u32,
    #[serde(rename = "capability", default)]
    capabilities: Vec<RawCapabilityDefinition>,
}

#[derive(Debug, Deserialize)]
struct RawCapabilityDefinition {
    id: String,
    label: String,
    category: String,
    #[serde(default)]
    required_tools: Vec<String>,
    #[serde(default)]
    optional_tools: Vec<String>,
}

fn sanitize_tool_list(tool_ids: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut sanitized = Vec::new();

    for tool_id in tool_ids {
        let trimmed = tool_id.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            sanitized.push(trimmed.to_string());
        }
    }

    sanitized
}

/// Parse TOML; returns valid entries, warnings for skipped rows, and `catalog_version`.
pub fn parse_capability_map_toml(
    toml_text: &str,
    source_label: &str,
) -> (Vec<CapabilityDefinition>, Vec<String>, u32) {
    let mut warnings: Vec<String> = Vec::new();

    let raw: RawCapabilityMapFile = match toml::from_str(toml_text) {
        Ok(raw) => raw,
        Err(err) => {
            warnings.push(format!(
                "capability map '{source_label}' failed to parse: {err}"
            ));
            return (Vec::new(), warnings, 1);
        }
    };
    let catalog_version = raw.catalog_version.max(1);

    let mut valid = Vec::new();
    let mut seen_ids = HashSet::new();

    for entry in raw.capabilities {
        if entry.id.trim().is_empty() {
            warnings.push("skipping capability entry with empty id".to_string());
            continue;
        }
        if !seen_ids.insert(entry.id.clone()) {
            warnings.push(format!("skipping duplicate capability id: {}", entry.id));
            continue;
        }
        if entry.label.trim().is_empty() {
            warnings.push(format!("skipping capability '{}': empty label", entry.id));
            continue;
        }
        if entry.category.trim().is_empty() {
            warnings.push(format!(
                "skipping capability '{}': empty category",
                entry.id
            ));
            continue;
        }

        let required_tools = sanitize_tool_list(&entry.required_tools);
        let mut optional_tools = sanitize_tool_list(&entry.optional_tools);
        optional_tools.retain(|tool_id| !required_tools.contains(tool_id));

        if required_tools.is_empty() && optional_tools.is_empty() {
            warnings.push(format!(
                "skipping capability '{}': at least one required_tools or optional_tools entry is required",
                entry.id
            ));
            continue;
        }

        valid.push(CapabilityDefinition {
            id: entry.id,
            label: entry.label,
            category: entry.category,
            required_tools,
            optional_tools,
        });
    }

    (valid, warnings, catalog_version)
}

pub fn merge_capability_maps(
    default_entries: Vec<CapabilityDefinition>,
    override_entries: Vec<CapabilityDefinition>,
) -> Vec<CapabilityDefinition> {
    let mut result = default_entries;
    for over in override_entries {
        if let Some(pos) = result.iter().position(|entry| entry.id == over.id) {
            result[pos] = over;
        } else {
            result.push(over);
        }
    }
    result
}

/// Load embedded default → optional user `host_capability_map.toml`.
pub fn load_capability_map(user_config_dir: Option<&Path>) -> CapabilityMap {
    let (default_entries, default_warnings, catalog_version) =
        parse_capability_map_toml(DEFAULT_CAPABILITY_MAP_TOML, "embedded default");
    for warning in &default_warnings {
        tracing::warn!(warning = %warning, "default capability map warning");
    }

    let mut merged = default_entries;

    let (user_entries, user_catalog_version_override) = user_config_dir
        .map(|dir| dir.join("host_capability_map.toml"))
        .filter(|path| path.exists())
        .and_then(|path| {
            std::fs::read_to_string(&path)
                .map_err(|err| {
                    tracing::warn!(
                        path = %path.display(),
                        %err,
                        "failed to read user capability map"
                    );
                })
                .ok()
        })
        .map(|text| {
            let (entries, warnings, user_ver) = parse_capability_map_toml(&text, "user override");
            for warning in &warnings {
                tracing::warn!(warning = %warning, "user capability map warning");
            }
            let parse_failed = warnings
                .iter()
                .any(|warning| warning.contains("failed to parse:"));
            let version_override = if parse_failed { None } else { Some(user_ver) };
            (entries, version_override)
        })
        .unwrap_or((Vec::new(), None));

    merged = merge_capability_maps(merged, user_entries);

    CapabilityMap::from_entries(
        user_catalog_version_override.unwrap_or(catalog_version),
        merged,
    )
}

static GLOBAL_CAPABILITY_MAP: OnceLock<CapabilityMap> = OnceLock::new();

pub fn initialize_capability_map(capability_map: CapabilityMap) -> bool {
    let ok = GLOBAL_CAPABILITY_MAP.set(capability_map).is_ok();
    if !ok {
        tracing::warn!("capability map was already initialized; ignoring duplicate set");
    }
    ok
}

pub fn global_capability_map() -> &'static CapabilityMap {
    GLOBAL_CAPABILITY_MAP.get_or_init(|| load_capability_map(None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_default_embedded_capability_map_has_expected_entries() {
        let (entries, warnings, _) =
            parse_capability_map_toml(DEFAULT_CAPABILITY_MAP_TOML, "embedded");
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert!(entries.iter().any(|entry| entry.id == "gamescope"));
        assert!(entries.iter().any(|entry| entry.id == "prefix_tools"));
        assert!(entries.iter().any(|entry| entry.id == "non_steam_launch"));
    }

    #[test]
    fn merge_replaces_by_capability_id() {
        let merged = merge_capability_maps(
            vec![CapabilityDefinition {
                id: "gamescope".to_string(),
                label: "Gamescope".to_string(),
                category: "performance".to_string(),
                required_tools: vec!["gamescope".to_string()],
                optional_tools: vec![],
            }],
            vec![CapabilityDefinition {
                id: "gamescope".to_string(),
                label: "Patched".to_string(),
                category: "performance".to_string(),
                required_tools: vec!["gamescope".to_string()],
                optional_tools: vec![],
            }],
        );
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].label, "Patched");
    }

    #[test]
    fn load_capability_map_user_override_replaces_default_entry() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("host_capability_map.toml"),
            r#"
catalog_version = 7

[[capability]]
id = "gamescope"
label = "Gamescope override"
category = "performance"
required_tools = ["gamescope"]
optional_tools = []
"#,
        )
        .expect("write override");

        let loaded = load_capability_map(Some(dir.path()));
        let gamescope = loaded.find_by_id("gamescope").expect("gamescope entry");
        assert_eq!(gamescope.label, "Gamescope override");
        assert_eq!(loaded.catalog_version, 7);
    }

    #[test]
    fn load_capability_map_invalid_user_file_keeps_embedded_catalog_version() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("host_capability_map.toml"),
            "not valid toml [[[",
        )
        .expect("write invalid capability map");

        let loaded = load_capability_map(Some(dir.path()));
        let (_, _, embedded_ver) =
            parse_capability_map_toml(DEFAULT_CAPABILITY_MAP_TOML, "embedded");
        assert_eq!(loaded.catalog_version, embedded_ver.max(1));
    }
}
