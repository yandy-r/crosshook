use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// Embedded default trainer type catalog TOML.
pub const DEFAULT_TRAINER_TYPE_CATALOG_TOML: &str =
    include_str!("../../../../assets/default_trainer_type_catalog.toml");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OfflineCapability {
    Full,
    FullWithRuntime,
    ConditionalKey,
    ConditionalSession,
    OnlineOnly,
    #[default]
    Unknown,
}

impl OfflineCapability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::FullWithRuntime => "full_with_runtime",
            Self::ConditionalKey => "conditional_key",
            Self::ConditionalSession => "conditional_session",
            Self::OnlineOnly => "online_only",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainerTypeEntry {
    pub id: String,
    pub display_name: String,
    pub offline_capability: OfflineCapability,
    pub requires_network: bool,
    #[serde(default)]
    pub detection_hints: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score_cap: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info_modal: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawTrainerTypeCatalogFile {
    #[serde(default)]
    #[allow(dead_code)]
    catalog_version: u32,
    #[serde(rename = "trainer_type", default)]
    entries: Vec<TrainerTypeEntry>,
}

/// Validated in-memory trainer type catalog.
#[derive(Debug, Clone)]
pub struct TrainerTypeCatalog {
    pub entries: Vec<TrainerTypeEntry>,
}

impl TrainerTypeCatalog {
    pub fn from_entries(entries: Vec<TrainerTypeEntry>) -> Self {
        Self { entries }
    }

    pub fn lookup(&self, id: &str) -> Option<&TrainerTypeEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn entries(&self) -> &[TrainerTypeEntry] {
        &self.entries
    }
}

pub fn parse_trainer_type_catalog_toml(
    toml_text: &str,
    source_label: &str,
) -> (Vec<TrainerTypeEntry>, Vec<String>) {
    let mut warnings: Vec<String> = Vec::new();

    let raw: RawTrainerTypeCatalogFile = match toml::from_str(toml_text) {
        Ok(raw) => raw,
        Err(err) => {
            warnings.push(format!(
                "trainer type catalog '{source_label}' failed to parse: {err}"
            ));
            return (Vec::new(), warnings);
        }
    };

    let mut valid = Vec::new();
    let mut seen_ids = HashSet::new();

    for entry in raw.entries {
        if entry.id.is_empty() {
            warnings.push("skipping trainer_type entry with empty id".to_string());
            continue;
        }
        if !seen_ids.insert(entry.id.clone()) {
            warnings.push(format!("skipping duplicate trainer_type id: {}", entry.id));
            continue;
        }
        valid.push(entry);
    }

    (valid, warnings)
}

pub fn merge_trainer_type_catalogs(
    default_entries: Vec<TrainerTypeEntry>,
    override_entries: Vec<TrainerTypeEntry>,
) -> Vec<TrainerTypeEntry> {
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

/// Load merged trainer type catalog: embedded default → tap texts → user override file.
pub fn load_trainer_type_catalog(
    user_config_dir: Option<&Path>,
    tap_catalog_texts: &[(&str, &str)],
) -> TrainerTypeCatalog {
    let (default_entries, default_warnings) =
        parse_trainer_type_catalog_toml(DEFAULT_TRAINER_TYPE_CATALOG_TOML, "embedded default");
    for w in &default_warnings {
        tracing::warn!(warning = %w, "default trainer type catalog warning");
    }

    let mut merged = default_entries;

    for (tap_label, tap_text) in tap_catalog_texts {
        let (tap_entries, tap_warnings) = parse_trainer_type_catalog_toml(tap_text, tap_label);
        for w in &tap_warnings {
            tracing::warn!(warning = %w, tap = %tap_label, "tap trainer type catalog warning");
        }
        merged = merge_trainer_type_catalogs(merged, tap_entries);
    }

    let user_entries = user_config_dir
        .map(|dir| dir.join("trainer_type_catalog.toml"))
        .filter(|p| p.exists())
        .and_then(|path| {
            std::fs::read_to_string(&path)
                .map_err(|err| {
                    tracing::warn!(
                        path = %path.display(),
                        %err,
                        "failed to read user trainer type catalog"
                    );
                })
                .ok()
        })
        .map(|text| {
            let (entries, warnings) = parse_trainer_type_catalog_toml(&text, "user override");
            for w in &warnings {
                tracing::warn!(warning = %w, "user trainer type catalog warning");
            }
            entries
        })
        .unwrap_or_default();

    merged = merge_trainer_type_catalogs(merged, user_entries);

    TrainerTypeCatalog::from_entries(merged)
}

static GLOBAL_TRAINER_TYPE_CATALOG: OnceLock<TrainerTypeCatalog> = OnceLock::new();

pub fn initialize_trainer_type_catalog(catalog: TrainerTypeCatalog) -> bool {
    let ok = GLOBAL_TRAINER_TYPE_CATALOG.set(catalog).is_ok();
    if !ok {
        tracing::warn!("trainer type catalog was already initialized; ignoring duplicate set");
    }
    ok
}

pub fn global_trainer_type_catalog() -> &'static TrainerTypeCatalog {
    GLOBAL_TRAINER_TYPE_CATALOG.get_or_init(|| {
        let (entries, warnings) =
            parse_trainer_type_catalog_toml(DEFAULT_TRAINER_TYPE_CATALOG_TOML, "fallback default");
        for w in &warnings {
            tracing::warn!(warning = %w, "fallback trainer type catalog parse warning");
        }
        TrainerTypeCatalog::from_entries(entries)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entry(id: &str) -> TrainerTypeEntry {
        TrainerTypeEntry {
            id: id.to_string(),
            display_name: format!("Display {id}"),
            offline_capability: OfflineCapability::Full,
            requires_network: false,
            detection_hints: vec![],
            score_cap: Some(100),
            info_modal: None,
        }
    }

    #[test]
    fn offline_capability_serde_roundtrip() {
        #[derive(Serialize, Deserialize)]
        struct Wrap {
            c: OfflineCapability,
        }
        for cap in [
            OfflineCapability::Full,
            OfflineCapability::FullWithRuntime,
            OfflineCapability::ConditionalKey,
            OfflineCapability::ConditionalSession,
            OfflineCapability::OnlineOnly,
            OfflineCapability::Unknown,
        ] {
            let w = Wrap { c: cap };
            let s = toml::to_string(&w).unwrap();
            let back: Wrap = toml::from_str(&s).unwrap();
            assert_eq!(back.c, cap);
        }
    }

    #[test]
    fn parse_valid_catalog_returns_entries() {
        let toml = r#"
[[trainer_type]]
id = "a"
display_name = "A"
offline_capability = "full"
requires_network = false

[[trainer_type]]
id = "b"
display_name = "B"
offline_capability = "unknown"
requires_network = true
"#;
        let (entries, warnings) = parse_trainer_type_catalog_toml(toml, "test");
        assert_eq!(entries.len(), 2);
        assert!(warnings.is_empty());
        assert_eq!(entries[0].id, "a");
        assert_eq!(entries[1].offline_capability, OfflineCapability::Unknown);
    }

    #[test]
    fn parse_skips_empty_id() {
        let toml = r#"
[[trainer_type]]
id = ""
display_name = "X"
offline_capability = "full"
requires_network = false
"#;
        let (entries, warnings) = parse_trainer_type_catalog_toml(toml, "test");
        assert!(entries.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("empty id"));
    }

    #[test]
    fn parse_skips_duplicate_ids() {
        let toml = r#"
[[trainer_type]]
id = "dup"
display_name = "First"
offline_capability = "full"
requires_network = false

[[trainer_type]]
id = "dup"
display_name = "Second"
offline_capability = "full"
requires_network = false
"#;
        let (entries, warnings) = parse_trainer_type_catalog_toml(toml, "test");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].display_name, "First");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("duplicate"));
    }

    #[test]
    fn parse_invalid_toml_returns_empty() {
        let (entries, warnings) = parse_trainer_type_catalog_toml("not toml !!!", "test");
        assert!(entries.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("failed to parse"));
    }

    #[test]
    fn default_catalog_toml_parses() {
        let (entries, warnings) =
            parse_trainer_type_catalog_toml(DEFAULT_TRAINER_TYPE_CATALOG_TOML, "embedded");
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert!(entries.len() >= 6);
    }

    #[test]
    fn global_trainer_type_catalog_returns_embedded_default() {
        let g = global_trainer_type_catalog();
        assert!(!g.entries().is_empty());
        assert!(g.lookup("unknown").is_some());
    }

    #[test]
    fn merge_replaces_in_position() {
        let defaults = vec![sample_entry("a"), sample_entry("b")];
        let mut b_new = sample_entry("b");
        b_new.display_name = "B override".to_string();
        let merged = merge_trainer_type_catalogs(defaults, vec![b_new]);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[1].display_name, "B override");
    }
}
