use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// The default command-argument catalog TOML, embedded at compile time.
pub const DEFAULT_CATALOG_TOML: &str =
    include_str!("../../../../../assets/default_command_argument_catalog.toml");

/// A single command-argument catalog entry (argv tokens plus UI metadata).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandArgumentEntry {
    pub id: String,
    #[serde(default)]
    pub tokens: Vec<String>,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub help_text: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub advanced: bool,
    #[serde(default)]
    pub community: bool,
    #[serde(default)]
    pub applicable_methods: Vec<String>,
    #[serde(default)]
    pub conflicts_with: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawCatalogFile {
    #[serde(default)]
    #[allow(dead_code)]
    catalog_version: u32,
    #[serde(rename = "command_argument", default)]
    arguments: Vec<CommandArgumentEntry>,
}

/// Validated, merged, in-memory command-argument catalog.
#[derive(Debug, Clone)]
pub struct CommandArgumentCatalog {
    pub catalog_version: u32,
    pub entries: Vec<CommandArgumentEntry>,
}

impl CommandArgumentCatalog {
    pub fn from_entries(entries: Vec<CommandArgumentEntry>) -> Self {
        Self {
            catalog_version: 1,
            entries,
        }
    }

    pub fn is_known_id(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn find_by_id(&self, id: &str) -> Option<&CommandArgumentEntry> {
        self.entries.iter().find(|e| e.id == id)
    }
}

const VALID_CATEGORIES: &[&str] = &[
    "input",
    "performance",
    "display",
    "graphics",
    "compatibility",
];

/// Parses a TOML catalog text, validates each entry, and returns valid entries
/// plus warning strings for skipped entries.
pub fn parse_catalog_toml(
    toml_text: &str,
    source_label: &str,
) -> (Vec<CommandArgumentEntry>, Vec<String>) {
    let mut warnings: Vec<String> = Vec::new();

    let raw: RawCatalogFile = match toml::from_str(toml_text) {
        Ok(raw) => raw,
        Err(err) => {
            warnings.push(format!("catalog '{source_label}' failed to parse: {err}"));
            return (Vec::new(), warnings);
        }
    };

    let mut valid = Vec::new();
    let mut seen_ids = HashSet::new();

    for entry in raw.arguments {
        if entry.id.is_empty() {
            warnings.push("skipping command_argument entry with empty id".to_string());
            continue;
        }
        if !seen_ids.insert(entry.id.clone()) {
            warnings.push(format!(
                "skipping duplicate command_argument id: {}",
                entry.id
            ));
            continue;
        }
        if entry.label.is_empty() {
            warnings.push(format!(
                "skipping command_argument '{}': empty label",
                entry.id
            ));
            continue;
        }
        if entry.category.is_empty() {
            warnings.push(format!(
                "skipping command_argument '{}': empty category",
                entry.id
            ));
            continue;
        }
        if !VALID_CATEGORIES.contains(&entry.category.as_str()) {
            warnings.push(format!(
                "skipping command_argument '{}': unrecognized category '{}'",
                entry.id, entry.category
            ));
            continue;
        }
        if entry.tokens.is_empty() {
            warnings.push(format!(
                "skipping command_argument '{}': empty tokens",
                entry.id
            ));
            continue;
        }
        if entry.tokens.iter().any(std::string::String::is_empty) {
            warnings.push(format!(
                "skipping command_argument '{}': token entry is empty",
                entry.id
            ));
            continue;
        }
        if entry.applicable_methods.is_empty() {
            warnings.push(format!(
                "skipping command_argument '{}': empty applicable_methods",
                entry.id
            ));
            continue;
        }
        valid.push(entry);
    }

    (valid, warnings)
}

/// Merge default entries with user overrides.
///
/// User entries whose `id` matches a default entry replace it in-place,
/// preserving its original position. User entries with novel IDs are appended.
pub fn merge_catalogs(
    default_entries: Vec<CommandArgumentEntry>,
    override_entries: Vec<CommandArgumentEntry>,
) -> Vec<CommandArgumentEntry> {
    let mut result = default_entries;
    for over in override_entries {
        if let Some(pos) = result.iter().position(|e| e.id == over.id) {
            result[pos] = over;
        } else {
            result.push(over);
        }
    }
    result
}

/// Load the command-argument catalog from:
///  1. Embedded default TOML (compile-time fallback, always present)
///  2. Community tap catalog texts (optional, merged in order)
///  3. User override file at `config_dir/command_argument_catalog.toml` (optional, highest priority)
pub fn load_catalog(
    user_config_dir: Option<&Path>,
    tap_catalog_texts: &[(&str, &str)],
) -> CommandArgumentCatalog {
    let (default_entries, default_warnings) =
        parse_catalog_toml(DEFAULT_CATALOG_TOML, "embedded default");
    for w in &default_warnings {
        tracing::warn!(warning = %w, "default command argument catalog warning");
    }

    let mut merged = default_entries;

    for (tap_label, tap_text) in tap_catalog_texts {
        let (mut tap_entries, tap_warnings) = parse_catalog_toml(tap_text, tap_label);
        for w in &tap_warnings {
            tracing::warn!(warning = %w, tap = %tap_label, "tap command argument catalog warning");
        }
        for entry in &mut tap_entries {
            entry.community = true;
        }
        merged = merge_catalogs(merged, tap_entries);
    }

    let user_entries = user_config_dir
        .map(|dir| dir.join("command_argument_catalog.toml"))
        .filter(|p| p.exists())
        .and_then(|path| {
            std::fs::read_to_string(&path)
                .map_err(|err| {
                    tracing::warn!(
                        path = %path.display(),
                        %err,
                        "failed to read user command argument catalog"
                    );
                })
                .ok()
        })
        .map(|text| {
            let (entries, warnings) = parse_catalog_toml(&text, "user override");
            for w in &warnings {
                tracing::warn!(warning = %w, "user command argument catalog warning");
            }
            entries
        })
        .unwrap_or_default();

    merged = merge_catalogs(merged, user_entries);

    CommandArgumentCatalog::from_entries(merged)
}

static GLOBAL_CATALOG: OnceLock<CommandArgumentCatalog> = OnceLock::new();

pub fn initialize_catalog(catalog: CommandArgumentCatalog) {
    let _ = GLOBAL_CATALOG.set(catalog);
}

/// Returns a reference to the process-global command-argument catalog.
pub fn global_catalog() -> &'static CommandArgumentCatalog {
    GLOBAL_CATALOG.get_or_init(|| {
        let (entries, _) = parse_catalog_toml(DEFAULT_CATALOG_TOML, "fallback default");
        CommandArgumentCatalog::from_entries(entries)
    })
}
