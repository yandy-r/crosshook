use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// The default MangoHud preset catalog TOML, embedded at compile time.
const DEFAULT_MANGOHUD_PRESETS_TOML: &str =
    include_str!("../../../../assets/default_mangohud_presets.toml");

/// A single MangoHud display preset defining which overlay metrics to show.
///
/// Presets describe appearance/metrics only — they do not control whether
/// MangoHud is active. The user controls that at the launch configuration level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MangoHudPreset {
    pub id: String,
    pub label: String,
    pub description: String,
    #[serde(default)]
    pub fps_limit: Option<u32>,
    #[serde(default)]
    pub gpu_stats: bool,
    #[serde(default)]
    pub cpu_stats: bool,
    #[serde(default)]
    pub ram: bool,
    #[serde(default)]
    pub frametime: bool,
    #[serde(default)]
    pub battery: bool,
    #[serde(default)]
    pub watt: bool,
    /// Optional overlay position string (e.g. `"top-left"`, `"bottom-left"`).
    /// Conversion to a typed enum happens at the UI/application layer.
    #[serde(default)]
    pub position: Option<String>,
}

/// In-memory MangoHud preset catalog deserialized from TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MangoHudPresetCatalog {
    pub catalog_version: u32,
    /// Matches the `[[preset]]` TOML array key.
    #[serde(default)]
    pub preset: Vec<MangoHudPreset>,
}

fn parse_mangohud_presets(toml_str: &str) -> (MangoHudPresetCatalog, Vec<String>) {
    let mut warnings = Vec::new();
    match toml::from_str::<MangoHudPresetCatalog>(toml_str) {
        Ok(catalog) => {
            if catalog.preset.is_empty() {
                warnings.push("MangoHud preset catalog contains no presets".to_string());
            }
            (catalog, warnings)
        }
        Err(e) => {
            warnings.push(format!("Failed to parse MangoHud presets: {e}"));
            (
                MangoHudPresetCatalog {
                    catalog_version: 0,
                    preset: Vec::new(),
                },
                warnings,
            )
        }
    }
}

static GLOBAL_MANGOHUD_PRESETS: OnceLock<MangoHudPresetCatalog> = OnceLock::new();

/// Returns a reference to the process-global MangoHud preset catalog.
///
/// Loads from the embedded default TOML on first call. Any parse warnings
/// are emitted via `tracing::warn!`.
pub fn global_mangohud_presets() -> &'static MangoHudPresetCatalog {
    GLOBAL_MANGOHUD_PRESETS.get_or_init(|| {
        let (catalog, warnings) = parse_mangohud_presets(DEFAULT_MANGOHUD_PRESETS_TOML);
        for w in &warnings {
            tracing::warn!("{w}");
        }
        catalog
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_toml_parses_with_no_warnings() {
        let (catalog, warnings) = parse_mangohud_presets(DEFAULT_MANGOHUD_PRESETS_TOML);
        assert!(
            warnings.is_empty(),
            "default MangoHud preset catalog had warnings: {warnings:?}"
        );
        assert!(
            !catalog.preset.is_empty(),
            "default MangoHud preset catalog should have presets"
        );
    }

    #[test]
    fn default_catalog_has_three_presets() {
        let (catalog, _) = parse_mangohud_presets(DEFAULT_MANGOHUD_PRESETS_TOML);
        assert_eq!(
            catalog.preset.len(),
            3,
            "expected exactly 3 presets in the default catalog"
        );
    }

    #[test]
    fn preset_ids_are_minimal_performance_battery() {
        let (catalog, _) = parse_mangohud_presets(DEFAULT_MANGOHUD_PRESETS_TOML);
        let ids: Vec<&str> = catalog.preset.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["minimal", "performance", "battery"]);
    }

    #[test]
    fn minimal_preset_has_frametime_only() {
        let (catalog, _) = parse_mangohud_presets(DEFAULT_MANGOHUD_PRESETS_TOML);
        let minimal = catalog.preset.iter().find(|p| p.id == "minimal").unwrap();
        assert!(!minimal.gpu_stats);
        assert!(!minimal.cpu_stats);
        assert!(!minimal.ram);
        assert!(minimal.frametime);
        assert!(!minimal.battery);
        assert!(!minimal.watt);
    }

    #[test]
    fn battery_preset_has_position() {
        let (catalog, _) = parse_mangohud_presets(DEFAULT_MANGOHUD_PRESETS_TOML);
        let battery = catalog.preset.iter().find(|p| p.id == "battery").unwrap();
        assert_eq!(battery.position.as_deref(), Some("bottom-left"));
    }

    #[test]
    fn invalid_toml_returns_empty_catalog_with_warning() {
        let (catalog, warnings) = parse_mangohud_presets("not valid toml !!!");
        assert!(catalog.preset.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Failed to parse MangoHud presets"));
    }

    #[test]
    fn empty_preset_list_produces_warning() {
        let toml = "catalog_version = 1\n";
        let (catalog, warnings) = parse_mangohud_presets(toml);
        assert!(catalog.preset.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("no presets"));
    }

    #[test]
    fn all_presets_have_non_empty_id_and_label() {
        let (catalog, _) = parse_mangohud_presets(DEFAULT_MANGOHUD_PRESETS_TOML);
        for preset in &catalog.preset {
            assert!(
                !preset.id.is_empty(),
                "preset has empty id: {preset:?}"
            );
            assert!(
                !preset.label.is_empty(),
                "preset '{}' has empty label",
                preset.id
            );
        }
    }
}
