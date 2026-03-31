use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// The default catalog TOML, embedded at compile time.
pub const DEFAULT_CATALOG_TOML: &str =
    include_str!("../../../../assets/default_optimization_catalog.toml");

/// A single optimization entry combining functional fields (env, wrappers,
/// conflicts, required binary) with UI metadata (label, description, category).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationEntry {
    pub id: String,
    #[serde(default = "default_applies_to_method")]
    pub applies_to_method: String,
    #[serde(default)]
    pub env: Vec<[String; 2]>,
    #[serde(default)]
    pub wrappers: Vec<String>,
    #[serde(default)]
    pub conflicts_with: Vec<String>,
    #[serde(default)]
    pub required_binary: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub help_text: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub target_gpu_vendor: String,
    #[serde(default)]
    pub advanced: bool,
    #[serde(default)]
    pub community: bool,
    #[serde(default)]
    pub applicable_methods: Vec<String>,
}

fn default_applies_to_method() -> String {
    "proton_run".to_string()
}

#[derive(Debug, Deserialize)]
struct RawCatalogFile {
    #[serde(default)]
    #[allow(dead_code)]
    catalog_version: u32,
    #[serde(rename = "optimization", default)]
    optimizations: Vec<OptimizationEntry>,
}

/// Validated, merged, in-memory catalog.
#[derive(Debug, Clone)]
pub struct OptimizationCatalog {
    pub catalog_version: u32,
    pub entries: Vec<OptimizationEntry>,
    /// Dynamic allowlist of env var keys derived from all entries.
    pub allowed_env_keys: HashSet<String>,
}

impl OptimizationCatalog {
    pub fn from_entries(entries: Vec<OptimizationEntry>) -> Self {
        let allowed_env_keys = entries
            .iter()
            .flat_map(|e| e.env.iter().map(|pair| pair[0].clone()))
            .collect();
        Self {
            catalog_version: 1,
            entries,
            allowed_env_keys,
        }
    }

    pub fn is_known_id(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn find_by_id(&self, id: &str) -> Option<&OptimizationEntry> {
        self.entries.iter().find(|e| e.id == id)
    }
}

const VALID_CATEGORIES: &[&str] = &["input", "performance", "display", "graphics", "compatibility"];

/// Parses a TOML catalog text, validates each entry, and returns valid entries
/// plus a list of warning strings for skipped entries.
pub fn parse_catalog_toml(
    toml_text: &str,
    source_label: &str,
) -> (Vec<OptimizationEntry>, Vec<String>) {
    let mut warnings: Vec<String> = Vec::new();

    let raw: RawCatalogFile = match toml::from_str(toml_text) {
        Ok(raw) => raw,
        Err(err) => {
            warnings.push(format!(
                "catalog '{}' failed to parse: {}",
                source_label, err
            ));
            return (Vec::new(), warnings);
        }
    };

    let mut valid = Vec::new();
    let mut seen_ids = HashSet::new();

    for entry in raw.optimizations {
        if entry.id.is_empty() {
            warnings.push("skipping optimization entry with empty id".to_string());
            continue;
        }
        if !seen_ids.insert(entry.id.clone()) {
            warnings.push(format!(
                "skipping duplicate optimization id: {}",
                entry.id
            ));
            continue;
        }
        if entry.label.is_empty() {
            warnings.push(format!(
                "skipping optimization '{}': empty label",
                entry.id
            ));
            continue;
        }
        if entry.category.is_empty() {
            warnings.push(format!(
                "skipping optimization '{}': empty category",
                entry.id
            ));
            continue;
        }
        if !VALID_CATEGORIES.contains(&entry.category.as_str()) {
            warnings.push(format!(
                "skipping optimization '{}': unrecognized category '{}'",
                entry.id, entry.category
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
    default_entries: Vec<OptimizationEntry>,
    override_entries: Vec<OptimizationEntry>,
) -> Vec<OptimizationEntry> {
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

/// Load the optimization catalog from:
///  1. Embedded default TOML (compile-time fallback, always present)
///  2. Community tap catalog texts (optional, merged in order)
///  3. User override file at `config_dir/optimization_catalog.toml` (optional, highest priority)
///
/// Merge order: default → taps → user override (user always wins).
pub fn load_catalog(
    user_config_dir: Option<&Path>,
    tap_catalog_texts: &[(&str, &str)],
) -> OptimizationCatalog {
    let (default_entries, default_warnings) =
        parse_catalog_toml(DEFAULT_CATALOG_TOML, "embedded default");
    for w in &default_warnings {
        tracing::warn!(warning = %w, "default optimization catalog warning");
    }

    let mut merged = default_entries;

    // Merge community tap catalogs
    for (tap_label, tap_text) in tap_catalog_texts {
        let (mut tap_entries, tap_warnings) = parse_catalog_toml(tap_text, tap_label);
        for w in &tap_warnings {
            tracing::warn!(warning = %w, tap = %tap_label, "tap optimization catalog warning");
        }
        // Force community flag on tap entries
        for entry in &mut tap_entries {
            entry.community = true;
        }
        merged = merge_catalogs(merged, tap_entries);
    }

    // Merge user overrides (highest priority)
    let user_entries = user_config_dir
        .map(|dir| dir.join("optimization_catalog.toml"))
        .filter(|p| p.exists())
        .and_then(|path| {
            std::fs::read_to_string(&path)
                .map_err(|err| {
                    tracing::warn!(
                        path = %path.display(),
                        %err,
                        "failed to read user optimization catalog"
                    );
                })
                .ok()
        })
        .map(|text| {
            let (entries, warnings) = parse_catalog_toml(&text, "user override");
            for w in &warnings {
                tracing::warn!(warning = %w, "user optimization catalog warning");
            }
            entries
        })
        .unwrap_or_default();

    merged = merge_catalogs(merged, user_entries);

    OptimizationCatalog::from_entries(merged)
}

static GLOBAL_CATALOG: OnceLock<OptimizationCatalog> = OnceLock::new();

pub fn initialize_catalog(catalog: OptimizationCatalog) {
    let _ = GLOBAL_CATALOG.set(catalog);
}

/// Returns a reference to the process-global optimization catalog.
///
/// In production, `initialize_catalog` is called during Tauri startup with the
/// merged catalog (default + user overrides). If the catalog has not been
/// explicitly initialized (e.g. in unit tests or edge cases), the embedded
/// default catalog is loaded as a fallback.
pub fn global_catalog() -> &'static OptimizationCatalog {
    GLOBAL_CATALOG.get_or_init(|| {
        let (entries, _) = parse_catalog_toml(DEFAULT_CATALOG_TOML, "fallback default");
        OptimizationCatalog::from_entries(entries)
    })
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn test_entry(id: &str) -> OptimizationEntry {
        OptimizationEntry {
            id: id.to_string(),
            applies_to_method: "proton_run".to_string(),
            env: Vec::new(),
            wrappers: Vec::new(),
            conflicts_with: Vec::new(),
            required_binary: String::new(),
            label: format!("Test {}", id),
            description: "test description".to_string(),
            help_text: "test help".to_string(),
            category: "graphics".to_string(),
            target_gpu_vendor: String::new(),
            advanced: false,
            community: false,
            applicable_methods: vec!["proton_run".to_string()],
        }
    }

    fn make_toml_entry(id: &str, label: &str, category: &str) -> String {
        format!(
            "[[optimization]]\nid = \"{}\"\nlabel = \"{}\"\ncategory = \"{}\"\n",
            id, label, category
        )
    }

    #[test]
    fn parse_valid_catalog_returns_all_entries() {
        let toml = format!(
            "{}{}",
            make_toml_entry("entry_a", "Entry A", "graphics"),
            make_toml_entry("entry_b", "Entry B", "performance"),
        );
        let (entries, warnings) = parse_catalog_toml(&toml, "test");
        assert_eq!(entries.len(), 2, "expected 2 entries");
        assert!(warnings.is_empty(), "expected no warnings");
        assert_eq!(entries[0].id, "entry_a");
        assert_eq!(entries[0].label, "Entry A");
        assert_eq!(entries[0].category, "graphics");
        assert_eq!(entries[1].id, "entry_b");
        assert_eq!(entries[1].label, "Entry B");
        assert_eq!(entries[1].category, "performance");
    }

    #[test]
    fn parse_skips_entry_with_empty_id() {
        let toml = "[[optimization]]\nid = \"\"\nlabel = \"Something\"\ncategory = \"graphics\"\n";
        let (entries, warnings) = parse_catalog_toml(toml, "test");
        assert!(entries.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("empty id"));
    }

    #[test]
    fn parse_skips_entry_with_unrecognized_category() {
        let toml = make_toml_entry("my_opt", "My Opt", "bogus");
        let (entries, warnings) = parse_catalog_toml(&toml, "test");
        assert!(entries.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("unrecognized category"));
        assert!(warnings[0].contains("bogus"));
    }

    #[test]
    fn parse_skips_duplicate_ids_keeping_first() {
        let toml = format!(
            "{}{}",
            make_toml_entry("dup_id", "First", "graphics"),
            make_toml_entry("dup_id", "Second", "graphics"),
        );
        let (entries, warnings) = parse_catalog_toml(&toml, "test");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].label, "First");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("duplicate"));
        assert!(warnings[0].contains("dup_id"));
    }

    #[test]
    fn parse_returns_empty_on_invalid_toml() {
        let (entries, warnings) = parse_catalog_toml("not toml !!!", "test");
        assert!(entries.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("failed to parse"));
    }

    #[test]
    fn merge_user_override_replaces_in_position() {
        let defaults = vec![
            test_entry("a"),
            test_entry("b"),
            test_entry("c"),
        ];
        let mut b_new = test_entry("b");
        b_new.label = "Override B".to_string();
        let overrides = vec![b_new];

        let result = merge_catalogs(defaults, overrides);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].id, "a");
        assert_eq!(result[1].id, "b");
        assert_eq!(result[1].label, "Override B");
        assert_eq!(result[2].id, "c");
    }

    #[test]
    fn merge_novel_user_id_appends_after_defaults() {
        let defaults = vec![test_entry("a"), test_entry("b")];
        let overrides = vec![test_entry("novel")];

        let result = merge_catalogs(defaults, overrides);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].id, "a");
        assert_eq!(result[1].id, "b");
        assert_eq!(result[2].id, "novel");
    }

    #[test]
    fn default_catalog_toml_parses_with_no_warnings() {
        let (entries, warnings) = parse_catalog_toml(DEFAULT_CATALOG_TOML, "embedded default");
        assert!(
            warnings.is_empty(),
            "default catalog had warnings: {:?}",
            warnings
        );
        assert!(!entries.is_empty(), "default catalog should have entries");
    }

    #[test]
    fn default_catalog_has_25_entries() {
        let (entries, _) = parse_catalog_toml(DEFAULT_CATALOG_TOML, "embedded default");
        assert_eq!(
            entries.len(),
            25,
            "expected exactly 25 entries in the default catalog"
        );
    }

    #[test]
    fn load_catalog_uses_embedded_default_when_no_user_file() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let catalog = load_catalog(Some(temp_dir.path()), &[]);
        assert_eq!(
            catalog.entries.len(),
            25,
            "expected 25 entries from the embedded default"
        );
    }

    #[test]
    fn load_catalog_user_file_overrides_default_entry() {
        let temp_dir = tempfile::tempdir().expect("temp dir");

        // Pick the first entry from the default catalog to override
        let (default_entries, _) = parse_catalog_toml(DEFAULT_CATALOG_TOML, "test");
        let first_id = default_entries[0].id.clone();

        let override_toml = format!(
            "[[optimization]]\nid = \"{}\"\nlabel = \"User Override Label\"\ncategory = \"graphics\"\n",
            first_id
        );
        let override_path = temp_dir.path().join("optimization_catalog.toml");
        let mut f = std::fs::File::create(&override_path).expect("create override file");
        f.write_all(override_toml.as_bytes()).expect("write override");

        let catalog = load_catalog(Some(temp_dir.path()), &[]);

        // Should still have 25 entries (override replaces, doesn't add)
        assert_eq!(catalog.entries.len(), 25);
        // The overridden entry should have the new label
        let overridden = catalog.find_by_id(&first_id).expect("entry should exist");
        assert_eq!(overridden.label, "User Override Label");
    }

    #[test]
    fn load_catalog_user_file_with_invalid_toml_falls_back_to_default() {
        let temp_dir = tempfile::tempdir().expect("temp dir");

        let override_path = temp_dir.path().join("optimization_catalog.toml");
        std::fs::write(&override_path, b"this is not valid toml !!!").expect("write bad toml");

        let catalog = load_catalog(Some(temp_dir.path()), &[]);

        assert_eq!(
            catalog.entries.len(),
            25,
            "should fall back to 25 default entries when user file is invalid"
        );
    }
}
