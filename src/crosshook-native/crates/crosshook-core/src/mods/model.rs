use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModCategory {
    OverlayInjection,
    ScriptExtender,
    FileReplacement,
    Other,
}

impl ModCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OverlayInjection => "overlay_injection",
            Self::ScriptExtender => "script_extender",
            Self::FileReplacement => "file_replacement",
            Self::Other => "other",
        }
    }
}

impl FromStr for ModCategory {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "overlay_injection" => Ok(Self::OverlayInjection),
            "script_extender" => Ok(Self::ScriptExtender),
            "file_replacement" => Ok(Self::FileReplacement),
            "other" => Ok(Self::Other),
            _ => Err("unsupported mod category"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModProvenance {
    Manual,
    Detected,
}

impl ModProvenance {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Detected => "detected",
        }
    }
}

impl FromStr for ModProvenance {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "manual" => Ok(Self::Manual),
            "detected" => Ok(Self::Detected),
            _ => Err("unsupported mod provenance"),
        }
    }
}

/// Row DTO (SQLite ⇄ IPC). Field names are the wire contract — do not rename.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileModRecord {
    pub mod_id: String,
    pub profile_id: String,
    pub name: String,
    pub category: ModCategory,
    /// Relative to the game install dir, or absolute; advisory hints only.
    pub paths: Vec<String>,
    pub enabled: bool,
    pub provenance: ModProvenance,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    pub created_at: String, // RFC 3339
    pub updated_at: String, // RFC 3339
}

fn default_true() -> bool {
    true
}

/// Create/update payload — boundary-validated in `metadata::mods_store::validate_mod_input`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileModInput {
    pub name: String,
    pub category: ModCategory,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub source_url: Option<String>,
    pub provenance: ModProvenance,
}
