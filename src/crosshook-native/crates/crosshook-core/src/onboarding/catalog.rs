//! Host readiness catalog — embedded TOML, optional user override, merge + global accessor.
//!
//! Mirrors [`crate::launch::catalog`] patterns.

use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;

use serde::Deserialize;

use super::{HostDistroFamily, HostToolEntry, HostToolInstallCommand};

/// Embedded default catalog TOML (compile time).
pub const DEFAULT_READINESS_CATALOG_TOML: &str =
    include_str!("../../../../assets/default_host_readiness_catalog.toml");

const VALID_CATEGORIES: &[&str] = &[
    "runtime",
    "performance",
    "overlay",
    "compatibility",
    "prefix_tools",
];

#[derive(Debug, Deserialize)]
struct RawReadinessCatalogFile {
    #[serde(default)]
    catalog_version: u32,
    #[serde(rename = "tool", default)]
    tools: Vec<RawTool>,
}

#[derive(Debug, Deserialize)]
struct RawTool {
    tool_id: String,
    binary_name: String,
    display_name: String,
    description: String,
    docs_url: String,
    required: bool,
    category: String,
    #[serde(default)]
    install: Vec<HostToolInstallCommand>,
}

/// Validated in-memory readiness catalog.
#[derive(Debug, Clone)]
pub struct ReadinessCatalog {
    pub catalog_version: u32,
    pub entries: Vec<HostToolEntry>,
}

impl ReadinessCatalog {
    pub fn from_entries(catalog_version: u32, entries: Vec<HostToolEntry>) -> Self {
        Self {
            catalog_version: catalog_version.max(1),
            entries,
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&HostToolEntry> {
        self.entries.iter().find(|e| e.tool_id == id)
    }

    /// Resolve install hint for a distro, falling back to `Unknown` if present.
    pub fn install_for_distro(
        entry: &HostToolEntry,
        distro: HostDistroFamily,
    ) -> Option<HostToolInstallCommand> {
        let key = distro.as_str();
        entry
            .install_commands
            .iter()
            .find(|c| c.distro_family == key)
            .cloned()
            .or_else(|| {
                entry
                    .install_commands
                    .iter()
                    .find(|c| c.distro_family == "Unknown")
                    .cloned()
            })
    }
}

/// Parse TOML; returns valid entries, warnings for skipped rows, and `catalog_version` from the file.
pub fn parse_readiness_catalog_toml(
    toml_text: &str,
    source_label: &str,
) -> (Vec<HostToolEntry>, Vec<String>, u32) {
    let mut warnings: Vec<String> = Vec::new();

    let raw: RawReadinessCatalogFile = match toml::from_str(toml_text) {
        Ok(raw) => raw,
        Err(err) => {
            warnings.push(format!(
                "readiness catalog '{source_label}' failed to parse: {err}"
            ));
            return (Vec::new(), warnings, 1);
        }
    };
    let catalog_version = raw.catalog_version.max(1);

    let mut valid = Vec::new();
    let mut seen_ids = HashSet::new();

    for tool in raw.tools {
        if tool.tool_id.is_empty() {
            warnings.push("skipping readiness tool with empty tool_id".to_string());
            continue;
        }
        if !seen_ids.insert(tool.tool_id.clone()) {
            warnings.push(format!(
                "skipping duplicate readiness tool_id: {}",
                tool.tool_id
            ));
            continue;
        }
        if tool.display_name.is_empty() {
            warnings.push(format!(
                "skipping readiness tool '{}': empty display_name",
                tool.tool_id
            ));
            continue;
        }
        if tool.category.is_empty() {
            warnings.push(format!(
                "skipping readiness tool '{}': empty category",
                tool.tool_id
            ));
            continue;
        }
        if !VALID_CATEGORIES.contains(&tool.category.as_str()) {
            warnings.push(format!(
                "skipping readiness tool '{}': unrecognized category '{}'",
                tool.tool_id, tool.category
            ));
            continue;
        }

        valid.push(HostToolEntry {
            tool_id: tool.tool_id,
            binary_name: tool.binary_name,
            display_name: tool.display_name,
            description: tool.description,
            docs_url: tool.docs_url,
            required: tool.required,
            category: tool.category,
            install_commands: tool.install,
        });
    }

    (valid, warnings, catalog_version)
}

/// Merge default entries with user overrides (same-id replaces in-place; novel IDs append).
pub fn merge_readiness_catalogs(
    default_entries: Vec<HostToolEntry>,
    override_entries: Vec<HostToolEntry>,
) -> Vec<HostToolEntry> {
    let mut result = default_entries;
    for over in override_entries {
        if let Some(pos) = result.iter().position(|e| e.tool_id == over.tool_id) {
            result[pos] = over;
        } else {
            result.push(over);
        }
    }
    result
}

/// Load embedded default → optional user `host_readiness_catalog.toml`.
pub fn load_readiness_catalog(user_config_dir: Option<&Path>) -> ReadinessCatalog {
    let (default_entries, default_warnings, catalog_version) =
        parse_readiness_catalog_toml(DEFAULT_READINESS_CATALOG_TOML, "embedded default");
    for w in &default_warnings {
        tracing::warn!(warning = %w, "default host readiness catalog warning");
    }

    let mut merged = default_entries;

    let (user_entries, user_catalog_version_override) = user_config_dir
        .map(|dir| dir.join("host_readiness_catalog.toml"))
        .filter(|p| p.exists())
        .and_then(|path| {
            std::fs::read_to_string(&path)
                .map_err(|err| {
                    tracing::warn!(
                        path = %path.display(),
                        %err,
                        "failed to read user host readiness catalog"
                    );
                })
                .ok()
        })
        .map(|text| {
            let (entries, warnings, user_ver) =
                parse_readiness_catalog_toml(&text, "user override");
            for w in &warnings {
                tracing::warn!(warning = %w, "user host readiness catalog warning");
            }
            // Only treat user_ver as an override when TOML parsed; on parse failure we still
            // return catalog_version=1 from the parser but must not override embedded version.
            let parse_failed = warnings.iter().any(|w| w.contains("failed to parse:"));
            let version_override = if parse_failed { None } else { Some(user_ver) };
            (entries, version_override)
        })
        .unwrap_or((Vec::new(), None));

    merged = merge_readiness_catalogs(merged, user_entries);

    let final_catalog_version = user_catalog_version_override.unwrap_or(catalog_version);
    ReadinessCatalog::from_entries(final_catalog_version, merged)
}

static GLOBAL_READINESS_CATALOG: OnceLock<ReadinessCatalog> = OnceLock::new();

pub fn initialize_readiness_catalog(catalog: ReadinessCatalog) {
    let _ = GLOBAL_READINESS_CATALOG.set(catalog);
}

/// Process-global readiness catalog (default embedded if unset).
pub fn global_readiness_catalog() -> &'static ReadinessCatalog {
    GLOBAL_READINESS_CATALOG.get_or_init(|| load_readiness_catalog(None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_default_embedded_catalog_has_tools() {
        let (entries, warnings, _) =
            parse_readiness_catalog_toml(DEFAULT_READINESS_CATALOG_TOML, "embedded");
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert!(
            entries.iter().any(|e| e.tool_id == "umu_run"),
            "expected umu_run"
        );
        assert!(
            entries.iter().any(|e| e.tool_id == "gamescope"),
            "expected gamescope"
        );
        assert!(
            entries.iter().any(|e| e.tool_id == "game_performance"),
            "expected game_performance"
        );
    }

    #[test]
    fn merge_replaces_by_tool_id() {
        let a = HostToolEntry {
            tool_id: "gamescope".to_string(),
            binary_name: "gamescope".to_string(),
            display_name: "Gamescope".to_string(),
            description: "d".to_string(),
            docs_url: "u".to_string(),
            required: false,
            category: "performance".to_string(),
            install_commands: vec![],
        };
        let b = HostToolEntry {
            tool_id: "gamescope".to_string(),
            binary_name: "gamescope".to_string(),
            display_name: "Patched".to_string(),
            description: "d".to_string(),
            docs_url: "u".to_string(),
            required: false,
            category: "performance".to_string(),
            install_commands: vec![],
        };
        let merged = merge_readiness_catalogs(vec![a], vec![b]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].display_name, "Patched");
    }

    #[test]
    fn load_readiness_catalog_invalid_user_file_keeps_embedded_catalog_version() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("host_readiness_catalog.toml"),
            "not valid toml [[[",
        )
        .expect("write invalid catalog");
        let loaded = load_readiness_catalog(Some(dir.path()));
        let (_, _, embedded_ver) =
            parse_readiness_catalog_toml(DEFAULT_READINESS_CATALOG_TOML, "embedded");
        assert_eq!(
            loaded.catalog_version,
            embedded_ver.max(1),
            "parse failure must not force user catalog_version override"
        );
    }
}
