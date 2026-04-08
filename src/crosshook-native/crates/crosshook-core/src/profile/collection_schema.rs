//! TOML wire format for cross-machine collection presets (`*.crosshook-collection.toml`).

use serde::{Deserialize, Serialize};

use crate::profile::CollectionDefaultsSection;

/// Version string embedded in exported presets (`schema_version = "1"`).
pub const COLLECTION_PRESET_SCHEMA_VERSION: &str = "1";

/// One profile identity in a collection preset — used for matching and round-trip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CollectionPresetProfileDescriptor {
    #[serde(default)]
    pub steam_app_id: String,
    #[serde(default)]
    pub game_name: String,
    #[serde(default, rename = "trainer_community_trainer_sha256")]
    pub trainer_community_trainer_sha256: String,
}

/// Top-level manifest serialized to TOML for collection presets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionPresetManifest {
    pub schema_version: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defaults: Option<CollectionDefaultsSection>,
    #[serde(default)]
    pub profiles: Vec<CollectionPresetProfileDescriptor>,
}

impl CollectionPresetManifest {
    /// Validates required fields after TOML deserialization.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version.is_empty() {
            return Err("collection preset must include schema_version".to_string());
        }
        if self.schema_version != COLLECTION_PRESET_SCHEMA_VERSION {
            return Err(format!(
                "unsupported collection preset schema version {:?}; supported version is {:?}",
                self.schema_version,
                COLLECTION_PRESET_SCHEMA_VERSION
            ));
        }
        if self.name.trim().is_empty() {
            return Err("collection preset must include a non-empty name".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_serializes_schema_version_string() {
        let m = CollectionPresetManifest {
            schema_version: COLLECTION_PRESET_SCHEMA_VERSION.to_string(),
            name: "Test".to_string(),
            description: None,
            defaults: None,
            profiles: vec![],
        };
        let s = toml::to_string_pretty(&m).unwrap();
        assert!(s.contains("schema_version = \"1\""));
    }

    #[test]
    fn rejects_empty_name() {
        let m = CollectionPresetManifest {
            schema_version: COLLECTION_PRESET_SCHEMA_VERSION.to_string(),
            name: "   ".to_string(),
            description: None,
            defaults: None,
            profiles: vec![],
        };
        assert!(m.validate().is_err());
    }
}
